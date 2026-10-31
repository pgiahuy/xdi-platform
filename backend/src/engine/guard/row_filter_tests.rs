// SPDX-License-Identifier: Apache-2.0
/* engine/guard/row_filter_tests.rs - kiểm lọc dòng (bước 3 trong mod.rs) bằng cách so với đáp án thay vì so chuỗi.
Mỗi câu chạy hai lần trên DuckDB thật: qua guard trên dữ liệu đầy đủ, và nguyên văn trên bảng chỉ còn dòng được phép.
Hai kết quả phải giống hệt. Câu dựng từ tổ hợp dạng câu với cách viết tên bảng, nên phủ cả những ca không ai viết tay:
ví dụ riêng lẻ chỉ chứng minh đúng ví dụ đó. Thêm dạng câu mới vào QUERY_SHAPES là đủ, không cần viết đáp án. */

use std::path::Path;

use duckdb::types::Value;
use duckdb::Connection;

use super::join_tests::link;
use super::{build_guarded_sql, AllowedTable, JoinLink, RowFilter};

/// Đủ lớn để LIMIT guard thêm vào không cắt kết quả nào, nên so được với câu gốc không có LIMIT.
const NO_ROW_LIMIT: u64 = 1_000_000;

const SALES_ROWS: &str = "SELECT * FROM (VALUES ('South', 10, 1), ('South', 5, 2), ('North', 7, 1), ('East', 3, 3), \
                          ('North', 4, 2), ('South', 8, 3), ('West', 6, 4)) v(region, amount, customer_id)";
const CUSTOMERS_ROWS: &str =
    "SELECT * FROM (VALUES (1, 'Retail'), (2, 'Business'), (3, 'Retail'), (4, 'Gov')) v(customer_id, segment)";

/// Đáp án lọc bằng WHERE viết tay trên bảng, không qua code của guard, để lỗi của guard không lọt sang đáp án. Phải khớp
/// chính sách trong tables_with_policies.
const ALLOWED_ONLY_TABLES: &str = "CREATE TABLE sales AS SELECT * FROM sales_rows WHERE region IN ('South', 'East'); \
                                   CREATE TABLE customers AS SELECT * FROM customers_rows WHERE segment IN ('Retail');";

/// Giá trị chỉ có ở dòng bị lọc; kết quả qua guard không được chứa giá trị nào trong số này.
const HIDDEN_VALUES: &[&str] = &["North", "West", "Business", "Gov"];

/// Cách viết tên hai bảng: tên trơn, nháy kép viết hoa, hoa thường lẫn lộn. DuckDB so tên không phân biệt hoa thường.
const TABLE_SPELLINGS: &[(&str, &str)] = &[
    ("sales", "customers"),
    ("\"SALES\"", "\"Customers\""),
    ("Sales", "CUSTOMERS"),
];

/// {S} là sales, {C} là customers. Mỗi dạng là một cách đọc dữ liệu: JOIN mọi kiểu, câu con ở mọi chỗ, bảng tạm, phép
/// tập hợp, bí danh trùng tên bảng khác, hàm cửa sổ.
const QUERY_SHAPES: &[&str] = &[
    "SELECT region, amount FROM {S}",
    "SELECT DISTINCT region FROM {S}",
    "SELECT region, SUM(amount) AS total FROM {S} GROUP BY region",
    "SELECT region FROM {S} GROUP BY region HAVING COUNT(*) > 0",
    "SELECT region FROM {S} WHERE region = 'North' OR amount > 0",
    "SELECT region FROM {S} WHERE NOT (region = 'South')",
    "SELECT s.region, c.segment FROM {S} s JOIN {C} c ON s.customer_id = c.customer_id",
    "SELECT s.region, c.segment FROM {C} c LEFT JOIN {S} s ON s.customer_id = c.customer_id",
    "SELECT s.region, c.segment FROM {S} s LEFT JOIN {C} c ON s.customer_id = c.customer_id",
    "SELECT s.region, c.segment FROM {S} s RIGHT JOIN {C} c ON s.customer_id = c.customer_id",
    "SELECT s.region, c.segment FROM {S} s FULL JOIN {C} c ON s.customer_id = c.customer_id",
    "SELECT COUNT(*) AS n FROM {S} s, {C} c WHERE s.customer_id = c.customer_id",
    "SELECT a.region, b.region FROM {S} a, {S} b WHERE a.customer_id = b.customer_id",
    "SELECT c.segment, SUM(s.amount) AS total FROM {C} c LEFT JOIN {S} s ON s.customer_id = c.customer_id \
     GROUP BY c.segment",
    "SELECT customers.region FROM {S} AS customers",
    "SELECT sales.segment FROM {C} AS sales",
    "SELECT region FROM (SELECT * FROM {S}) AS customers",
    "WITH t AS (SELECT * FROM {S}) SELECT region FROM t",
    "WITH a AS (SELECT * FROM {S}), b AS (SELECT * FROM a WHERE amount > 1) SELECT region FROM b",
    "SELECT region FROM {S} UNION ALL SELECT segment FROM {C}",
    "SELECT region FROM {S} EXCEPT SELECT region FROM {S} WHERE region = 'South'",
    "SELECT region FROM {S} INTERSECT SELECT region FROM {S} WHERE amount > 4",
    "SELECT segment FROM {C} c WHERE EXISTS (SELECT 1 FROM {S} s WHERE s.customer_id = c.customer_id)",
    "SELECT segment FROM {C} WHERE customer_id IN (SELECT customer_id FROM {S})",
    "SELECT segment FROM {C} WHERE customer_id NOT IN (SELECT customer_id FROM {S})",
    "SELECT (SELECT COUNT(*) FROM {S}) AS n",
    "SELECT COALESCE((SELECT MAX(region) FROM {S} WHERE region = 'North'), 'none') AS r",
    "SELECT region, amount FROM {S} WHERE amount > (SELECT AVG(amount) FROM {S})",
    "SELECT s.region, t.total FROM {S} s, \
     LATERAL (SELECT SUM(amount) AS total FROM {S} x WHERE x.customer_id = s.customer_id) t",
    "SELECT region, ROW_NUMBER() OVER (PARTITION BY region ORDER BY amount) AS n FROM {S}",
];

/// sales.customer_id trỏ customers.customer_id, để các dạng câu JOIN qua luật nối bảng (guard/joins.rs).
/// Thêm một khóa tự trỏ sales.customer_id -> sales.customer_id chỉ để giữ các dạng câu tự JOIN của test lọc dòng:
/// bảng tự JOIN phải đi qua khóa tự trỏ.
fn links() -> Vec<JoinLink> {
    vec![
        link("sales", &["customer_id"], "customers", &["customer_id"]),
        link("sales", &["customer_id"], "sales", &["customer_id"]),
    ]
}

/// sales chỉ thấy South và East; customers chỉ thấy Retail.
fn tables_with_policies(dir: &Path) -> Vec<AllowedTable> {
    vec![
        AllowedTable {
            name: "sales".to_string(),
            columns: vec![
                "region".to_string(),
                "amount".to_string(),
                "customer_id".to_string(),
            ],
            parquet_dir: dir.join("sales"),
            row_filters: vec![RowFilter::Column {
                column: "region".to_string(),
                allowed_values: vec!["South".to_string(), "East".to_string()],
            }],
        },
        AllowedTable {
            name: "customers".to_string(),
            columns: vec!["customer_id".to_string(), "segment".to_string()],
            parquet_dir: dir.join("customers"),
            row_filters: vec![RowFilter::Column {
                column: "segment".to_string(),
                allowed_values: vec!["Retail".to_string()],
            }],
        },
    ]
}

fn write_full_data(conn: &Connection, dir: &Path) {
    for (name, rows) in [("sales", SALES_ROWS), ("customers", CUSTOMERS_ROWS)] {
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

fn open_allowed_only() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE sales_rows AS {SALES_ROWS}; CREATE TABLE customers_rows AS {CUSTOMERS_ROWS}; \
         {ALLOWED_ONLY_TABLES}"
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
        let column_count = row.as_ref().column_count();
        let cells = (0..column_count)
            .map(|i| row.get::<_, Value>(i).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        result.push(cells);
    }
    result.sort_by_key(|cells| format!("{cells:?}"));
    Ok(result)
}

fn find_hidden_value(rows: &[Vec<Value>]) -> Option<&'static str> {
    HIDDEN_VALUES.iter().copied().find(|value| {
        let hidden = Value::Text(value.to_string());
        rows.iter().flatten().any(|cell| *cell == hidden)
    })
}

/// Báo mọi ca sai cùng lúc thay vì dừng ở ca đầu, để thấy lỗi theo dạng câu hay theo cách viết tên.
fn check_one_query(
    full: &Connection,
    allowed_only: &Connection,
    tables: &[AllowedTable],
    sql: &str,
) -> Option<String> {
    let guarded = match build_guarded_sql(sql, tables, &links(), NO_ROW_LIMIT) {
        Ok(guarded) => guarded,
        Err(e) => return Some(format!("{sql}\n  guard rejected: {e:?}")),
    };
    if guarded.row_filters.is_empty() {
        return Some(format!("{sql}\n  guard added no row filter"));
    }
    let actual = match sorted_rows(full, &guarded.runnable) {
        Ok(rows) => rows,
        Err(e) => return Some(format!("{sql}\n  guarded query failed: {e}")),
    };
    if let Some(value) = find_hidden_value(&actual) {
        return Some(format!(
            "{sql}\n  guarded result shows hidden value {value}: {actual:?}"
        ));
    }
    let expected = match sorted_rows(allowed_only, sql) {
        Ok(rows) => rows,
        Err(e) => return Some(format!("{sql}\n  query on allowed rows failed: {e}")),
    };
    if actual != expected {
        return Some(format!(
            "{sql}\n  guarded:  {actual:?}\n  expected: {expected:?}"
        ));
    }
    None
}

#[test]
fn guarded_results_match_tables_holding_only_allowed_rows() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-rows-{}", uuid::Uuid::new_v4()));
    let full = Connection::open_in_memory().unwrap();
    write_full_data(&full, &dir);
    let allowed_only = open_allowed_only();
    let tables = tables_with_policies(&dir);

    let mut failures = Vec::new();
    for (sales, customers) in TABLE_SPELLINGS {
        for shape in QUERY_SHAPES {
            let sql = shape.replace("{S}", sales).replace("{C}", customers);
            if let Some(failure) = check_one_query(&full, &allowed_only, &tables, &sql) {
                failures.push(failure);
            }
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
