// SPDX-License-Identifier: Apache-2.0
/* engine/guard/parent_filter_tests.rs - kiểm điều kiện lan theo khóa ngoại (RowFilter::Parent, #215) ở bước 3 của
mod.rs. Hai loại test: so chuỗi bản shown cho vài ca để thấy dạng câu guard dựng (một cột, nhiều cột, nhiều bước), và
chạy trên DuckDB thật như row_filter_tests.rs: mỗi câu chạy qua guard trên dữ liệu đầy đủ và nguyên văn trên bảng chỉ
còn dòng được phép, viết tay bằng WHERE với id cụ thể (không qua code của guard). Hai kết quả phải giống hệt.

Lược đồ: order_items -> orders -> customers (order_id, customer_id), order_items.gift_customer_id -> customers (đường
thứ hai tới customers), order_items (maker, sku) -> products (khóa hai cột). Dòng mang khóa ngoại rỗng ở từng quan hệ
kiểm luật NULL của build_parent_conditions: một đường thì khóa rỗng bị ẩn (đơn 13); nhiều đường thì đường rỗng được bỏ
qua (dòng hàng 2, 6, 9), mọi đường rỗng thì ẩn (dòng hàng 8, và 10 khi products không bị lọc), khóa hai cột rỗng một
phần thì ẩn (dòng hàng 7 khi products bị lọc). */

use std::path::{Path, PathBuf};

use duckdb::types::Value;
use duckdb::Connection;

use super::join_tests::link;
use super::{build_guarded_sql, AllowedTable, GuardError, JoinLink, RowFilter};
use crate::engine::analytics::parquet_glob;

const MAX_ROWS: u64 = 100;

/// Đủ lớn để LIMIT guard thêm vào không cắt kết quả nào.
const NO_ROW_LIMIT: u64 = 1_000_000;

const CUSTOMERS_ROWS: &str =
    "SELECT * FROM (VALUES (1, 'South'), (2, 'North'), (3, NULL)) v(id, region)";
const ORDERS_ROWS: &str =
    "SELECT * FROM (VALUES (10, 1, 'done'), (11, 2, 'done'), (12, 3, 'open'), \
                           (13, NULL, 'done'), (14, 1, 'open')) v(id, customer_id, status)";
const PRODUCTS_ROWS: &str =
    "SELECT * FROM (VALUES ('acme', 1, 'Retail'), ('acme', 2, 'Gov'), ('zeta', 1, 'Retail')) v(maker, sku, segment)";
/// Cột đầu là id của dòng hàng, để bảng đáp án chọn dòng theo id viết tay.
const ORDER_ITEMS_ROWS: &str = "SELECT * FROM (VALUES (1, 10, 1, 'acme', 1, 4), (2, 10, NULL, 'acme', 2, 1), \
                                (3, 11, 1, 'zeta', 1, 2), (4, 14, 1, 'acme', 2, 3), (5, 14, 2, 'zeta', 1, 5), \
                                (6, NULL, 1, 'acme', 1, 7), (7, 10, 1, NULL, 1, 6), (8, NULL, NULL, NULL, NULL, 9), \
                                (9, 10, NULL, NULL, NULL, 2), (10, NULL, NULL, 'zeta', 1, 3)) \
                                v(id, order_id, gift_customer_id, maker, sku, qty)";

/// Chỉ chính sách customers.region = South. Đơn của khách 1 là 10 và 14 (đơn 13 có customer_id rỗng: một đường, ẩn).
/// Dòng hàng có hai đường (order_id, gift_customer_id): 1, 4 qua cả hai; 2, 9 có khách nhận quà rỗng; 6 có đơn rỗng;
/// 7 như 1 (products không bị lọc). Ẩn: 3 (đơn 11), 5 (khách nhận quà 2 ở vùng khác), 8 và 10 (mọi đường rỗng).
const ALLOWED_SOUTH: &str =
    "CREATE TABLE customers AS SELECT * FROM customers_rows WHERE id IN (1); \
     CREATE TABLE orders AS SELECT * FROM orders_rows WHERE id IN (10, 14); \
     CREATE TABLE order_items AS SELECT * FROM order_items_rows WHERE id IN (1, 2, 4, 6, 7, 9); \
     CREATE TABLE products AS SELECT * FROM products_rows;";

/// Thêm chính sách riêng orders.status = done và products.segment = Retail: orders còn đơn 10. Dòng hàng thêm đường thứ
/// ba (maker, sku): 1, 6 qua cả ba; 9 có khóa sản phẩm rỗng cả hai cột; 10 chỉ có đường sản phẩm và qua. Ẩn: 2 (sản
/// phẩm Gov), 4 (đơn 14 chưa done), 7 (khóa sản phẩm rỗng một phần), 3, 5, 8.
const ALLOWED_SOUTH_DONE_RETAIL: &str =
    "CREATE TABLE customers AS SELECT * FROM customers_rows WHERE id IN (1); \
     CREATE TABLE orders AS SELECT * FROM orders_rows WHERE id IN (10); \
     CREATE TABLE order_items AS SELECT * FROM order_items_rows WHERE id IN (1, 6, 9, 10); \
     CREATE TABLE products AS SELECT * FROM products_rows WHERE segment = 'Retail';";

/// Quan hệ của lược đồ ở đầu file, để các câu JOIN qua luật nối bảng (guard/joins.rs) cả khi bảng không bị lọc.
fn links() -> Vec<JoinLink> {
    vec![
        link("orders", &["customer_id"], "customers", &["id"]),
        link("order_items", &["order_id"], "orders", &["id"]),
        link("order_items", &["gift_customer_id"], "customers", &["id"]),
        link(
            "order_items",
            &["maker", "sku"],
            "products",
            &["maker", "sku"],
        ),
    ]
}

/// Mỗi dạng là một cách đọc: một bảng đọc riêng, JOIN theo quan hệ, khóa hai cột, câu con, bảng tạm, phép tập hợp.
const QUERY_SHAPES: &[&str] = &[
    "SELECT COUNT(*) AS n FROM orders",
    "SELECT COUNT(*) AS n FROM \"ORDERS\"",
    "SELECT COUNT(*) AS n, SUM(qty) AS q FROM order_items",
    "SELECT o.status, COUNT(*) AS n FROM orders o JOIN customers c ON o.customer_id = c.id GROUP BY o.status",
    "SELECT o.id, c.region FROM orders o LEFT JOIN customers c ON o.customer_id = c.id",
    "SELECT c.region, SUM(oi.qty) AS q FROM order_items oi JOIN orders o ON oi.order_id = o.id \
     JOIN customers c ON o.customer_id = c.id GROUP BY c.region",
    "SELECT p.segment, SUM(oi.qty) AS q FROM order_items oi LEFT JOIN products p \
     ON oi.maker = p.maker AND oi.sku = p.sku GROUP BY p.segment",
    "SELECT id FROM orders WHERE id NOT IN (SELECT order_id FROM order_items WHERE order_id IS NOT NULL)",
    "WITH t AS (SELECT * FROM orders) SELECT status, COUNT(*) AS n FROM t GROUP BY status",
    "SELECT COUNT(*) AS n FROM orders UNION ALL SELECT COUNT(*) FROM order_items",
];

fn table(name: &str, columns: &[&str], dir: &Path, row_filters: Vec<RowFilter>) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir: dir.join(name),
        row_filters,
    }
}

fn values(column: &str, allowed: &[&str]) -> RowFilter {
    RowFilter::Column {
        column: column.to_string(),
        allowed_values: allowed.iter().map(|v| v.to_string()).collect(),
    }
}

fn parent(columns: &[&str], parent_columns: &[&str], parent: AllowedTable) -> RowFilter {
    RowFilter::Parent {
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parent_columns: parent_columns.iter().map(|c| c.to_string()).collect(),
        parent,
    }
}

/// Bốn bảng như service dựng cho một người bị lọc: customers theo vùng, orders thêm chính sách riêng nếu có,
/// products theo phân khúc nếu có; điều kiện lan dựng như service/ask/row_filters.rs.
fn sales_tables(dir: &Path, order_status: &[&str], product_segment: &[&str]) -> Vec<AllowedTable> {
    let customers = table(
        "customers",
        &["id", "region"],
        dir,
        vec![values("region", &["South"])],
    );
    let mut order_filters = Vec::new();
    if !order_status.is_empty() {
        order_filters.push(values("status", order_status));
    }
    order_filters.push(parent(&["customer_id"], &["id"], customers.clone()));
    let orders = table(
        "orders",
        &["id", "customer_id", "status"],
        dir,
        order_filters,
    );
    let product_filters = if product_segment.is_empty() {
        Vec::new()
    } else {
        vec![values("segment", product_segment)]
    };
    let products = table(
        "products",
        &["maker", "sku", "segment"],
        dir,
        product_filters,
    );
    let mut item_filters = vec![
        parent(&["order_id"], &["id"], orders.clone()),
        parent(&["gift_customer_id"], &["id"], customers.clone()),
    ];
    if !products.row_filters.is_empty() {
        item_filters.push(parent(
            &["maker", "sku"],
            &["maker", "sku"],
            products.clone(),
        ));
    }
    let order_items = table(
        "order_items",
        &["id", "order_id", "gift_customer_id", "maker", "sku", "qty"],
        dir,
        item_filters,
    );
    vec![order_items, orders, customers, products]
}

fn find<'a>(tables: &'a [AllowedTable], name: &str) -> &'a AllowedTable {
    tables.iter().find(|t| t.name == name).unwrap()
}

#[test]
fn single_key_filter_reads_the_filtered_parent() {
    let dir = PathBuf::from("/data");
    let tables = sales_tables(&dir, &[], &[]);
    let orders_only = vec![find(&tables, "orders").clone()];
    let guarded = build_guarded_sql(
        "SELECT COUNT(*) AS n FROM orders",
        &orders_only,
        &links(),
        MAX_ROWS,
    )
    .unwrap();
    let condition =
        "\"customer_id\" IN (SELECT \"id\" FROM \"customers\" WHERE \"region\" IN ('South'))";
    assert_eq!(
        guarded.shown,
        format!("SELECT COUNT(*) AS n FROM (SELECT * FROM orders WHERE {condition}) AS orders LIMIT 100")
    );
    assert_eq!(guarded.row_filters, [condition]);
    assert_eq!(guarded.tables_read, ["orders"]);
    // Bảng cha đọc từ file của nó dù mô hình không được đọc customers.
    let customers_source = parquet_glob(&dir.join("customers")).unwrap();
    assert!(
        guarded.runnable.contains(&customers_source),
        "{}",
        guarded.runnable
    );
    assert!(
        !guarded.runnable.contains("FROM \"customers\""),
        "{}",
        guarded.runnable
    );
}

/// order_items: điều kiện của orders nằm trong câu con của order_items (lan hai bước); orders chỉ có một đường nên dạng
/// gọn `IN`. order_items có ba đường (orders, gift_customer_id, khóa hai cột tới products): mỗi đường cho qua khóa rỗng,
/// cộng điều kiện ít nhất một khóa không rỗng.
#[test]
fn transitive_two_path_and_composite_filters_are_all_added() {
    let tables = sales_tables(Path::new("/data"), &["done"], &["Retail"]);
    let guarded = build_guarded_sql(
        "SELECT SUM(qty) AS q FROM order_items",
        &tables,
        &links(),
        MAX_ROWS,
    )
    .unwrap();
    let customers = "SELECT \"id\" FROM \"customers\" WHERE \"region\" IN ('South')";
    let orders = format!(
        "SELECT \"id\" FROM \"orders\" WHERE \"status\" IN ('done') AND \"customer_id\" IN ({customers})"
    );
    let products = "SELECT \"maker\", \"sku\" FROM \"products\" WHERE \"segment\" IN ('Retail')";
    assert_eq!(
        guarded.row_filters,
        [
            format!("(\"order_id\" IS NULL OR \"order_id\" IN ({orders}))"),
            format!("(\"gift_customer_id\" IS NULL OR \"gift_customer_id\" IN ({customers}))"),
            format!(
                "((\"maker\" IS NULL AND \"sku\" IS NULL) OR (\"maker\", \"sku\") IN ({products}))"
            ),
            "(\"order_id\" IS NOT NULL OR \"gift_customer_id\" IS NOT NULL OR \"maker\" IS NOT NULL \
             OR \"sku\" IS NOT NULL)"
                .to_string(),
        ]
    );
    assert!(
        guarded.shown.contains(&guarded.row_filters.join(" AND ")),
        "{}",
        guarded.shown
    );
}

/// Bảng cha không bị lọc gì thì câu con không có WHERE: chỉ giữ dòng có khóa trỏ tới dòng có thật.
#[test]
fn parent_without_conditions_has_no_where() {
    let dir = Path::new("/data");
    let customers = table("customers", &["id", "region"], dir, vec![]);
    let orders = table(
        "orders",
        &["id", "customer_id"],
        dir,
        vec![parent(&["customer_id"], &["id"], customers)],
    );
    let guarded =
        build_guarded_sql("SELECT id FROM orders", &[orders], &links(), MAX_ROWS).unwrap();
    assert_eq!(
        guarded.row_filters,
        ["\"customer_id\" IN (SELECT \"id\" FROM \"customers\")"]
    );
}

/// Bảng tạm trùng tên bảng cha bị từ chối: nhận thì bước 5 đổi `FROM customers` của câu sang file của customers, câu
/// đọc thẳng bảng cha mà không qua hộp lọc nào.
#[test]
fn cte_named_like_a_parent_table_is_rejected() {
    let tables = sales_tables(Path::new("/data"), &[], &[]);
    let orders_only = vec![find(&tables, "orders").clone()];
    let sql = "WITH customers AS (SELECT * FROM orders) SELECT COUNT(*) FROM customers";
    match build_guarded_sql(sql, &orders_only, &links(), MAX_ROWS) {
        Err(GuardError::Rejected(reason)) => assert!(reason.contains("trùng tên"), "{reason}"),
        other => panic!("expected rejection, got {other:?}"),
    }
    // Mô hình cũng không đọc được bảng cha khi nó không có trong danh sách.
    match build_guarded_sql(
        "SELECT COUNT(*) FROM customers",
        &orders_only,
        &links(),
        MAX_ROWS,
    ) {
        Err(GuardError::UnknownName(_)) => {}
        other => panic!("expected unknown table, got {other:?}"),
    }
}

/// Cột khóa không có trong hồ sơ bảng cha hay bảng con là lỗi máy chủ: chạy thì DuckDB gắn tên cột với bảng ở câu
/// ngoài và điều kiện không còn lọc đúng bảng.
#[test]
fn key_columns_must_exist_on_both_tables() {
    let dir = Path::new("/data");
    let customers = table(
        "customers",
        &["id", "region"],
        dir,
        vec![values("region", &["South"])],
    );
    let cases = [
        parent(&["customer_id"], &["customer_key"], customers.clone()),
        parent(&["buyer_id"], &["id"], customers.clone()),
        parent(&["customer_id", "id"], &["id"], customers),
    ];
    for filter in cases {
        let orders = table("orders", &["id", "customer_id"], dir, vec![filter]);
        match build_guarded_sql("SELECT id FROM orders", &[orders], &links(), MAX_ROWS) {
            Err(GuardError::Internal(_)) => {}
            other => panic!("expected internal error, got {other:?}"),
        }
    }
}

fn write_full_data(conn: &Connection, dir: &Path) {
    for (name, rows) in [
        ("customers", CUSTOMERS_ROWS),
        ("orders", ORDERS_ROWS),
        ("products", PRODUCTS_ROWS),
        ("order_items", ORDER_ITEMS_ROWS),
    ] {
        let table_dir = dir.join(name);
        std::fs::create_dir_all(&table_dir).unwrap();
        let file = table_dir.join("part-0.parquet");
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
    }
}

fn open_allowed_only(allowed: &str) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE customers_rows AS {CUSTOMERS_ROWS}; CREATE TABLE orders_rows AS {ORDERS_ROWS}; \
         CREATE TABLE products_rows AS {PRODUCTS_ROWS}; CREATE TABLE order_items_rows AS {ORDER_ITEMS_ROWS}; \
         {allowed}"
    ))
    .unwrap();
    conn
}

/// Thứ tự dòng không cố định khi câu không có ORDER BY, nên sắp lại trước khi so.
fn sorted_rows(conn: &Connection, sql: &str) -> Result<Vec<Vec<Value>>, String> {
    let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let count = row.as_ref().column_count();
        let cells = (0..count)
            .map(|i| row.get::<_, Value>(i).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        result.push(cells);
    }
    result.sort_by_key(|cells| format!("{cells:?}"));
    Ok(result)
}

fn check_one_query(
    full: &Connection,
    allowed: &Connection,
    tables: &[AllowedTable],
    sql: &str,
) -> Option<String> {
    let guarded = match build_guarded_sql(sql, tables, &links(), NO_ROW_LIMIT) {
        Ok(guarded) => guarded,
        Err(e) => return Some(format!("{sql}\n  guard rejected: {e:?}")),
    };
    let actual = match sorted_rows(full, &guarded.runnable) {
        Ok(rows) => rows,
        Err(e) => return Some(format!("{sql}\n  guarded query failed: {e}")),
    };
    let expected = match sorted_rows(allowed, sql) {
        Ok(rows) => rows,
        Err(e) => return Some(format!("{sql}\n  query on allowed rows failed: {e}")),
    };
    (actual != expected).then(|| format!("{sql}\n  guarded:  {actual:?}\n  expected: {expected:?}"))
}

fn check_scenario(dir: &Path, tables: &[AllowedTable], allowed_sql: &str) -> Vec<String> {
    let full = Connection::open_in_memory().unwrap();
    write_full_data(&full, dir);
    let allowed = open_allowed_only(allowed_sql);
    QUERY_SHAPES
        .iter()
        .filter_map(|sql| check_one_query(&full, &allowed, tables, sql))
        .collect()
}

#[test]
fn guarded_results_match_tables_holding_only_allowed_rows() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-parent-{}", uuid::Uuid::new_v4()));
    let mut failures = check_scenario(&dir, &sales_tables(&dir, &[], &[]), ALLOWED_SOUTH);
    failures.extend(check_scenario(
        &dir,
        &sales_tables(&dir, &["done"], &["Retail"]),
        ALLOWED_SOUTH_DONE_RETAIL,
    ));
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Người hỏi chọn thẳng orders, không bảng liên quan nào: lỗ chính #215 đóng lại. Đếm đơn chỉ ra hai đơn của khách
/// vùng South; đơn có customer_id rỗng (13) và đơn của khách không có vùng (12) bị ẩn.
#[test]
fn orders_read_alone_count_only_orders_of_visible_customers() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-parent-alone-{}", uuid::Uuid::new_v4()));
    let full = Connection::open_in_memory().unwrap();
    write_full_data(&full, &dir);
    let tables = sales_tables(&dir, &[], &[]);
    let orders_only = vec![find(&tables, "orders").clone()];
    let guarded = build_guarded_sql(
        "SELECT id FROM orders ORDER BY id",
        &orders_only,
        &links(),
        MAX_ROWS,
    )
    .unwrap();
    let rows = sorted_rows(&full, &guarded.runnable).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(rows, vec![vec![Value::Int(10)], vec![Value::Int(14)]]);
}

/// Luật NULL của nhiều đường, đọc từng dòng hàng theo id: khóa ngoại tùy chọn rỗng vẫn thấy (2, 9), khóa tùy chọn trỏ
/// ra ngoài phần được thấy thì ẩn (5), mọi đường rỗng thì ẩn (8; 10 khi products không bị lọc), khóa hai cột rỗng một
/// phần thì ẩn (7 khi products bị lọc).
#[test]
fn optional_keys_pass_when_empty_and_rows_without_any_link_stay_hidden() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-parent-null-{}", uuid::Uuid::new_v4()));
    let full = Connection::open_in_memory().unwrap();
    write_full_data(&full, &dir);
    let ids = |tables: &[AllowedTable]| {
        let guarded =
            build_guarded_sql("SELECT id FROM order_items", tables, &links(), MAX_ROWS).unwrap();
        sorted_rows(&full, &guarded.runnable).unwrap()
    };
    let south = ids(&sales_tables(&dir, &[], &[]));
    let retail = ids(&sales_tables(&dir, &["done"], &["Retail"]));
    std::fs::remove_dir_all(&dir).unwrap();
    let expected = |list: &[i32]| -> Vec<Vec<Value>> {
        let mut rows: Vec<Vec<Value>> = list.iter().map(|&n| vec![Value::Int(n)]).collect();
        rows.sort_by_key(|cells| format!("{cells:?}"));
        rows
    };
    assert_eq!(south, expected(&[1, 2, 4, 6, 7, 9]));
    assert_eq!(retail, expected(&[1, 6, 9, 10]));
}
