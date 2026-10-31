// SPDX-License-Identifier: Apache-2.0
/* service/drill/join_tests.rs - test truy dòng nguồn của câu nối bảng (drill/sources.rs, drill/columns.rs,
drill/measures.rs) trên DuckDB thật với file Parquet tạm; không cần Postgres. Như drill/tests.rs: mỗi test trả lời một
câu qua guard với quan hệ khóa ngoại, lấy khóa từ dòng kết quả rồi truy dòng nguồn.

Dữ liệu, mỗi bảng một thư mục Parquet: 6 khách (segment xoay vòng ba giá trị, region North, South), 40 đơn (khách
xoay vòng), 3 danh mục, 9 sản phẩm (danh mục xoay vòng, giá tăng dần), 200 dòng đơn (đơn và sản phẩm xoay vòng, số
tiền DECIMAL để tổng là phép cộng đúng, so được bằng chuỗi). */

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::engine::guard::{build_guarded_sql, AllowedTable, JoinLink, RowFilter};
use crate::error::AppError;
use crate::service::ask::contract::{CellValue, ResultTable};
use crate::service::ask::result::build_result_table;
use crate::service::ask::run::run_query_in_turn;
use crate::service::drill::contract::{DrillKey, DrillResponse, MeasureRule};
use crate::service::drill::{run_drill, DrillInput};

const TABLES: &[(&str, &str)] = &[
    (
        "customers",
        "SELECT CAST(i + 1 AS INTEGER) AS customer_id,
                ['Consumer', 'Corporate', 'Home Office'][i % 3 + 1] AS segment,
                ['North', 'South'][i % 2 + 1] AS region
         FROM range(6) t(i)",
    ),
    (
        "orders",
        "SELECT CAST(i + 1 AS INTEGER) AS order_id, CAST(i % 6 + 1 AS INTEGER) AS customer_id,
                ['done', 'open'][i % 2 + 1] AS status
         FROM range(40) t(i)",
    ),
    (
        "categories",
        "SELECT CAST(i + 1 AS INTEGER) AS category_id, ['Tech', 'Office', 'Home'][i + 1] AS category_name
         FROM range(3) t(i)",
    ),
    (
        "products",
        "SELECT CAST(i + 1 AS INTEGER) AS product_id, CAST(i % 3 + 1 AS INTEGER) AS category_id,
                CAST(i * 4 + 2 AS DECIMAL(12, 2)) AS price
         FROM range(9) t(i)",
    ),
    (
        "order_items",
        "SELECT CAST(i + 1 AS INTEGER) AS order_item_id, CAST(i % 40 + 1 AS INTEGER) AS order_id,
                CAST(i % 9 + 1 AS INTEGER) AS product_id, CAST((i % 7 + 1) * 2.5 AS DECIMAL(12, 2)) AS item_total
         FROM range(200) t(i)",
    ),
];

/// Thư mục tạm chứa Parquet của năm bảng, tự xóa khi ra khỏi phạm vi.
struct Shop {
    dir: PathBuf,
    jobs: Arc<Semaphore>,
    columns: Vec<(String, Vec<String>)>,
}

impl Shop {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-drill-join-{}", uuid::Uuid::new_v4()));
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let mut columns = Vec::new();
        for (name, rows) in TABLES {
            let table_dir = dir.join(name);
            std::fs::create_dir_all(&table_dir).unwrap();
            conn.execute_batch(&format!(
                "COPY ({rows}) TO '{}' (FORMAT parquet)",
                table_dir.join("part-0001.parquet").display()
            ))
            .unwrap();
            let names: Vec<String> = conn
                .prepare(&format!("SELECT * FROM ({rows}) LIMIT 0"))
                .and_then(|mut statement| {
                    drop(statement.query([])?);
                    Ok(statement.column_names())
                })
                .unwrap();
            columns.push((name.to_string(), names));
        }
        Shop {
            dir,
            jobs: Arc::new(Semaphore::new(2)),
            columns,
        }
    }

    fn table(&self, name: &str, row_filters: Vec<RowFilter>) -> AllowedTable {
        let (_, columns) = self.columns.iter().find(|(n, _)| n == name).unwrap();
        AllowedTable {
            name: name.to_string(),
            columns: columns.clone(),
            parquet_dir: self.dir.join(name),
            row_filters,
        }
    }

    /// Mọi bảng, không lọc.
    fn open(&self) -> Vec<AllowedTable> {
        TABLES
            .iter()
            .map(|(n, _)| self.table(n, Vec::new()))
            .collect()
    }

    /// Người chỉ thấy khách vùng North: điều kiện trên customers lan theo khóa ngoại xuống orders rồi order_items
    /// (#215), như service dựng từ dataset_relations.
    fn north_only(&self) -> Vec<AllowedTable> {
        let customers = self.table(
            "customers",
            vec![RowFilter::Column {
                column: "region".to_string(),
                allowed_values: vec!["North".to_string()],
            }],
        );
        let orders = self.table("orders", vec![parent("customer_id", customers.clone())]);
        let items = self.table("order_items", vec![parent("order_id", orders.clone())]);
        vec![
            items,
            orders,
            customers,
            self.table("products", Vec::new()),
            self.table("categories", Vec::new()),
        ]
    }

    /// Trả lời một câu như luồng hỏi với quyền của tables: qua guard, chạy, đổi sang ResultTable.
    async fn answer(&self, sql: &str, tables: &[AllowedTable]) -> (String, ResultTable) {
        let guarded = build_guarded_sql(sql, tables, &links(), 100_000).unwrap();
        (guarded.shown, self.run(guarded.runnable).await)
    }

    async fn run(&self, runnable: String) -> ResultTable {
        let output = run_query_in_turn(&self.jobs, runnable, Duration::from_secs(30))
            .await
            .unwrap();
        build_result_table(output, 100_000)
    }

    async fn drill(&self, sql: &str, keys: &[DrillKey], tables: &[AllowedTable]) -> DrillResult {
        self.drill_with(sql, keys, tables, (None, None), (0, 1000))
            .await
    }

    async fn drill_with(
        &self,
        sql_guarded: &str,
        keys: &[DrillKey],
        tables: &[AllowedTable],
        slice: (Option<&str>, Option<&str>),
        page: (u64, u32),
    ) -> DrillResult {
        run_drill(DrillInput {
            sql_guarded,
            keys,
            slice_filter: slice.0,
            only_measure: slice.1,
            offset: page.0,
            limit: page.1,
            tables,
            links: &links(),
            timeout: Duration::from_secs(30),
            jobs: &self.jobs,
        })
        .await
    }

    /// Một con số tính thẳng trên Parquet, không qua drill, để so.
    async fn scalar(&self, sql: &str) -> String {
        let guarded = build_guarded_sql(sql, &self.open(), &links(), 10).unwrap();
        let table = self.run(guarded.runnable).await;
        text(&table.rows[0][0]).unwrap()
    }
}

type DrillResult = Result<DrillResponse, AppError>;

impl Drop for Shop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn parent(column: &str, parent: AllowedTable) -> RowFilter {
    RowFilter::Parent {
        columns: vec![column.to_string()],
        parent_columns: vec![column.to_string()],
        parent,
    }
}

fn link(many: &str, one: &str, column: &str) -> JoinLink {
    JoinLink {
        many: many.to_string(),
        many_columns: vec![column.to_string()],
        one: one.to_string(),
        one_columns: vec![column.to_string()],
        fans_out: true,
    }
}

fn links() -> Vec<JoinLink> {
    vec![
        link("order_items", "orders", "order_id"),
        link("orders", "customers", "customer_id"),
        link("order_items", "products", "product_id"),
        link("products", "categories", "category_id"),
    ]
}

fn column_index(table: &ResultTable, column: &str) -> usize {
    table
        .columns
        .iter()
        .position(|c| c.name == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.columns))
}

fn text(cell: &Option<CellValue>) -> Option<String> {
    match cell {
        None => None,
        Some(CellValue::String(s)) => Some(s.clone()),
        Some(CellValue::Number(n)) => Some(n.to_string()),
        Some(CellValue::Boolean(b)) => Some(b.to_string()),
    }
}

/// Dòng có ô `column` bằng value, và khóa của dòng đó theo các cột đã nêu.
fn pick(
    table: &ResultTable,
    column: &str,
    value: &str,
    keys: &[&str],
) -> (Vec<Option<CellValue>>, Vec<DrillKey>) {
    let i = column_index(table, column);
    let row = table
        .rows
        .iter()
        .find(|row| text(&row[i]).as_deref() == Some(value))
        .unwrap_or_else(|| panic!("no row with {column} = {value}"))
        .clone();
    let keys = keys
        .iter()
        .map(|k| DrillKey {
            column: k.to_string(),
            value: row[column_index(table, k)].clone(),
        })
        .collect();
    (row, keys)
}

/// Số thập phân dạng chuỗi ("12.50") thành số xu, để cộng đúng không qua số thực.
fn cents(cell: &Option<CellValue>) -> i64 {
    let text = text(cell).expect("amount is not null");
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, "0"));
    let fraction = format!("{fraction:0<2}");
    whole.parse::<i64>().unwrap() * 100 + fraction[..2].parse::<i64>().unwrap()
}

fn sum_column(table: &ResultTable, column: &str) -> i64 {
    let i = column_index(table, column);
    table.rows.iter().map(|row| cents(&row[i])).sum()
}

fn all_equal(table: &ResultTable, column: &str, value: &str) -> bool {
    let i = column_index(table, column);
    table
        .rows
        .iter()
        .all(|row| text(&row[i]).as_deref() == Some(value))
}

fn bad_request(result: DrillResult) -> String {
    match result {
        Err(AppError::BadRequest(message)) => message,
        other => panic!("expected 400, got {other:?}"),
    }
}

const BY_SEGMENT: &str = "SELECT c.segment, SUM(oi.item_total) AS revenue FROM order_items oi
                          JOIN orders o ON oi.order_id = o.order_id
                          JOIN customers c ON o.customer_id = c.customer_id
                          GROUP BY c.segment ORDER BY revenue DESC";

/// Ba bảng order_items, orders, customers: dòng của order_items, cột segment của khách cạnh mỗi dòng, cột revenue
/// từng dòng cộng ra đúng ô, và độ đo tính lại bằng đúng ô.
#[tokio::test]
async fn three_table_join_returns_fact_rows_that_add_up() {
    let shop = Shop::new();
    let (sql, table) = shop.answer(BY_SEGMENT, &shop.open()).await;
    let (row, keys) = pick(&table, "segment", "Consumer", &["segment"]);
    let cell = &row[column_index(&table, "revenue")];

    let drilled = shop.drill(&sql, &keys, &shop.open()).await.unwrap();

    // Khách 1 và 4 là Consumer; đơn i có khách i % 6 + 1, dòng đơn j có đơn j % 40 + 1.
    let expected = shop
        .scalar(
            "SELECT count(*) FROM order_items oi JOIN orders o ON oi.order_id = o.order_id
             WHERE o.customer_id IN (1, 4)",
        )
        .await;
    assert_eq!(drilled.total_rows.to_string(), expected);
    assert_eq!(drilled.result.rows.len() as u64, drilled.total_rows);
    assert_eq!(drilled.source_table.as_deref(), Some("order_items"));
    assert_eq!(drilled.joined_columns, ["segment"]);
    assert!(all_equal(&drilled.result, "segment", "Consumer"));
    // Mọi cột của order_items có mặt, cột của orders thì không.
    for column in ["order_item_id", "order_id", "product_id", "item_total"] {
        column_index(&drilled.result, column);
    }
    assert!(!drilled.result.columns.iter().any(|c| c.name == "status"));
    let [measure] = drilled.measures.as_slice() else {
        panic!("{:?}", drilled.measures);
    };
    assert_eq!(
        (measure.column.as_str(), measure.rule),
        ("revenue", MeasureRule::Sum)
    );
    assert_eq!(text(&measure.value), text(cell));
    let row_column = measure.row_column.as_deref().unwrap();
    assert_eq!(sum_column(&drilled.result, row_column), cents(cell));
    assert!(drilled.sql.contains("JOIN customers"), "{}", drilled.sql);
    assert!(!drilled.sql.contains("GROUP BY"), "{}", drilled.sql);
}

/// order_items, products, categories với điều kiện lọc trên products: cột lọc products.price hiện cạnh dòng, COUNT(*)
/// bằng số dòng nguồn.
#[tokio::test]
async fn join_through_products_keeps_filters_and_shows_filter_columns() {
    let shop = Shop::new();
    let question = "SELECT cat.category_name, COUNT(*) AS items, SUM(oi.item_total) AS revenue
                    FROM order_items oi JOIN products p ON oi.product_id = p.product_id
                    JOIN categories cat ON p.category_id = cat.category_id
                    WHERE p.price > 10 GROUP BY cat.category_name";
    let (sql, table) = shop.answer(question, &shop.open()).await;
    let (row, keys) = pick(&table, "category_name", "Tech", &["category_name"]);

    let drilled = shop.drill(&sql, &keys, &shop.open()).await.unwrap();

    assert_eq!(drilled.source_table.as_deref(), Some("order_items"));
    assert_eq!(drilled.joined_columns, ["category_name", "products.price"]);
    assert_eq!(
        drilled.total_rows.to_string(),
        text(&row[column_index(&table, "items")]).unwrap()
    );
    let price = column_index(&drilled.result, "products.price");
    assert!(drilled.result.rows.iter().all(|r| cents(&r[price]) > 1000));
    let rules: Vec<MeasureRule> = drilled.measures.iter().map(|m| m.rule).collect();
    assert_eq!(rules, [MeasureRule::Count, MeasureRule::Sum]);
    assert_eq!(
        text(&drilled.measures[0].value),
        text(&row[column_index(&table, "items")])
    );
    assert_eq!(drilled.measures[0].row_column, None);
    assert_eq!(
        sum_column(&drilled.result, "revenue"),
        cents(&row[column_index(&table, "revenue")])
    );
}

/// COUNT(*) không đọc cột nào: bảng của dòng là gốc của chuỗi nhiều-một (orders dưới customers), không phải bảng viết
/// đầu tiên.
#[tokio::test]
async fn count_star_lists_rows_of_the_many_side() {
    let shop = Shop::new();
    let question = "SELECT c.segment, COUNT(*) AS orders FROM customers c
                    JOIN orders o ON o.customer_id = c.customer_id GROUP BY c.segment";
    let (sql, table) = shop.answer(question, &shop.open()).await;
    let (row, keys) = pick(&table, "segment", "Corporate", &["segment"]);

    let drilled = shop.drill(&sql, &keys, &shop.open()).await.unwrap();

    assert_eq!(drilled.source_table.as_deref(), Some("orders"));
    column_index(&drilled.result, "status");
    assert_eq!(
        drilled.total_rows.to_string(),
        text(&row[column_index(&table, "orders")]).unwrap()
    );
}

/// Câu do người không bị lọc hỏi; người bấm chỉ thấy khách North (điều kiện lan xuống orders, order_items): chỉ dòng
/// của đơn có khách North, độ đo tính lại bằng tổng các dòng đó, nhỏ hơn ô.
#[tokio::test]
async fn clicker_with_row_filters_sees_only_allowed_rows() {
    let shop = Shop::new();
    let (sql, table) = shop.answer(BY_SEGMENT, &shop.open()).await;
    let (row, keys) = pick(&table, "segment", "Consumer", &["segment"]);
    let cell = cents(&row[column_index(&table, "revenue")]);

    let drilled = shop.drill(&sql, &keys, &shop.north_only()).await.unwrap();

    // Consumer là khách 1 (North) và 4 (South): chỉ còn dòng của đơn có khách 1.
    let expected = shop
        .scalar(
            "SELECT count(*) FROM order_items oi JOIN orders o ON oi.order_id = o.order_id
             WHERE o.customer_id = 1",
        )
        .await;
    assert_eq!(drilled.total_rows.to_string(), expected);
    assert!(drilled.sql.contains("IN ('North')"), "{}", drilled.sql);
    let order = column_index(&drilled.result, "order_id");
    // Đơn của khách 1: order_id % 6 == 1.
    assert!(drilled
        .result
        .rows
        .iter()
        .all(|r| text(&r[order]).unwrap().parse::<i64>().unwrap() % 6 == 1));
    let value = cents(&drilled.measures[0].value);
    assert!(value < cell, "{value} {cell}");
    assert_eq!(sum_column(&drilled.result, "revenue"), value);
}

/// Người hỏi bị lọc nên câu đã lưu có hộp lọc quanh mọi bảng: `oi.*` trên hộp vẫn qua guard, tổng khớp ô; ô của
/// segment không có khách North thì không có dòng.
#[tokio::test]
async fn asker_with_row_filters_drills_through_filter_boxes() {
    let shop = Shop::new();
    let north = shop.north_only();
    let (sql, table) = shop.answer(BY_SEGMENT, &north).await;
    assert!(sql.contains("IN ('North')"), "{sql}");
    let (row, keys) = pick(&table, "segment", "Home Office", &["segment"]);

    let drilled = shop.drill(&sql, &keys, &north).await.unwrap();

    let cell = &row[column_index(&table, "revenue")];
    assert!(drilled.total_rows > 0);
    assert_eq!(text(&drilled.measures[0].value), text(cell));
    assert_eq!(sum_column(&drilled.result, "revenue"), cents(cell));
}

/// Dòng phân rã theo cột của bảng liên quan (#227): câu chiều đọc câu con `LEFT JOIN` ở FROM, như explain/source.rs
/// dựng. Dòng trả về là dòng order_items kèm cột "customers.segment", độ đo của lát cắt được bấm khớp nhóm.
#[tokio::test]
async fn breakdown_row_by_a_related_column_is_traced() {
    let shop = Shop::new();
    let north = shop.north_only();
    let joined = "(SELECT \"b\".*, \"j2\".\"segment\" AS \"customers.segment\" FROM \"order_items\" AS \"b\"
                   LEFT JOIN \"orders\" AS \"j1\" ON \"b\".\"order_id\" = \"j1\".\"order_id\"
                   LEFT JOIN \"customers\" AS \"j2\" ON \"j1\".\"customer_id\" = \"j2\".\"customer_id\") AS \"joined\"";
    let base = "\"order_id\" <= 20";
    let compare = "\"order_id\" > 20";
    let dimension = format!(
        "SELECT \"customers.segment\", SUM(CASE WHEN {base} THEN \"item_total\" END) AS base_value,
                SUM(CASE WHEN {compare} THEN \"item_total\" END) AS compare_value
         FROM {joined} GROUP BY \"customers.segment\""
    );
    let (sql, table) = shop.answer(&dimension, &north).await;
    let (row, keys) = pick(
        &table,
        "customers.segment",
        "Consumer",
        &["customers.segment"],
    );

    let drilled = shop
        .drill_with(
            &sql,
            &keys,
            &north,
            (Some(base), Some("base_value")),
            (0, 1000),
        )
        .await
        .unwrap();

    assert_eq!(drilled.source_table.as_deref(), Some("order_items"));
    assert_eq!(drilled.joined_columns, ["customers.segment"]);
    assert!(all_equal(&drilled.result, "customers.segment", "Consumer"));
    let [measure] = drilled.measures.as_slice() else {
        panic!("{:?}", drilled.measures);
    };
    let cell = &row[column_index(&table, "base_value")];
    assert_eq!(measure.column, "base_value");
    assert_eq!(text(&measure.value), text(cell));
    assert_eq!(
        sum_column(&drilled.result, measure.row_column.as_deref().unwrap()),
        cents(cell)
    );
}

/// Bảng tạm chỉ lọc dòng thì truy được; câu đọc một bảng tạm như vậy trả mọi cột của nó.
#[tokio::test]
async fn row_level_with_table_is_traced() {
    let shop = Shop::new();
    let question = "WITH big AS (SELECT * FROM order_items WHERE item_total > 10)
                    SELECT c.segment, SUM(oi.item_total) AS revenue FROM big oi
                    JOIN orders o ON oi.order_id = o.order_id JOIN customers c ON o.customer_id = c.customer_id
                    GROUP BY c.segment";
    let (sql, _) = shop.answer(question, &shop.open()).await;
    let message = bad_request(
        shop.drill(&sql, &[key("segment", "Consumer")], &shop.open())
            .await,
    );
    assert!(message.contains("độ đo cộng trên bảng tạm"), "{message}");

    let single = "WITH big AS (SELECT * FROM order_items WHERE item_total > 10)
                  SELECT product_id, SUM(item_total) AS revenue FROM big GROUP BY product_id";
    let (sql, table) = shop.answer(single, &shop.open()).await;
    let (row, keys) = pick(&table, "product_id", "3", &["product_id"]);
    let drilled = shop.drill(&sql, &keys, &shop.open()).await.unwrap();
    assert_eq!(drilled.source_table.as_deref(), Some("order_items"));
    assert_eq!(
        sum_column(&drilled.result, "revenue"),
        cents(&row[column_index(&table, "revenue")])
    );
}

fn key(column: &str, value: &str) -> DrillKey {
    DrillKey {
        column: column.to_string(),
        value: Some(CellValue::String(value.to_string())),
    }
}

/// Dạng vẫn chưa truy được trên câu nối bảng: lời báo chung kèm lý do.
#[tokio::test]
async fn unsupported_join_questions_are_refused() {
    let shop = Shop::new();
    let from = "FROM order_items oi JOIN orders o ON oi.order_id = o.order_id
                JOIN customers c ON o.customer_id = c.customer_id";
    let cases = [
        (
            "WITH per_order AS (SELECT order_id, SUM(item_total) AS total FROM order_items GROUP BY order_id)
             SELECT c.segment, SUM(p.total) AS revenue FROM per_order p JOIN orders o ON p.order_id = o.order_id
             JOIN customers c ON o.customer_id = c.customer_id GROUP BY c.segment"
                .to_string(),
            "bảng tạm (WITH) per_order gộp nhóm",
        ),
        (
            "SELECT c.segment, SUM(p.total) AS revenue
             FROM (SELECT order_id, SUM(item_total) AS total FROM order_items GROUP BY order_id) p
             JOIN orders o ON p.order_id = o.order_id JOIN customers c ON o.customer_id = c.customer_id
             GROUP BY c.segment"
                .to_string(),
            "câu con ở FROM gộp nhóm",
        ),
        (
            format!("SELECT c.segment, SUM(oi.item_total) AS revenue {from} GROUP BY c.segment HAVING SUM(oi.item_total) > 10"),
            "HAVING",
        ),
        (
            format!("SELECT c.segment, SUM(oi.item_total) AS revenue, rank() OVER (ORDER BY SUM(oi.item_total)) AS place {from} GROUP BY c.segment"),
            "hàm cửa sổ",
        ),
        (
            format!("SELECT c.segment, SUM(oi.item_total) AS revenue {from} GROUP BY c.segment UNION ALL SELECT 'all', SUM(item_total) FROM order_items"),
            "UNION",
        ),
    ];
    for (question, reason) in cases {
        let (sql, _) = shop.answer(&question, &shop.open()).await;
        let message = bad_request(
            shop.drill(&sql, &[key("segment", "Consumer")], &shop.open())
                .await,
        );
        assert!(
            message.starts_with("Câu hỏi này chưa truy được dòng nguồn: ")
                && message.contains(reason),
            "{question}: {message}"
        );
    }
}
