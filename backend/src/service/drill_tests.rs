// SPDX-License-Identifier: Apache-2.0
/* service/drill_tests.rs - test truy dòng nguồn của một ô (drill.rs, drill_sql.rs) trên DuckDB thật với file Parquet
tạm; không cần Postgres. Mỗi test trả lời một câu như luồng hỏi (guard, chạy, ResultTable), lấy khóa từ đúng dòng kết
quả đó rồi truy dòng nguồn, nên tên cột khóa là tên giao diện nhận được. Phần đọc Postgres, 404 và quyền của người bấm
qua router ở api/drill_tests.rs.

Dữ liệu: 240 đơn, mỗi ngày một đơn từ 2017-01-01; Market xoay vòng Europe, LATAM, (trống), USCA; Customer Segment
xoay vòng ba giá trị; số tiền DECIMAL để tổng là phép cộng đúng, so được bằng chuỗi. */

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use super::ask_contract::{CellValue, ResultTable};
use super::ask_result::build_result_table;
use super::ask_run::run_query_in_turn;
use super::drill::{run_drill, DrillInput};
use super::drill_contract::{DrillKey, DrillResponse};
use crate::engine::guard::{build_guarded_sql, AllowedTable, RowFilter};
use crate::error::AppError;

const TABLE: &str = "orders";
const ORDERS: i64 = 240;

/// Thư mục tạm chứa Parquet của bảng orders, tự xóa khi ra khỏi phạm vi.
struct Orders {
    dir: PathBuf,
    jobs: Arc<Semaphore>,
}

impl Orders {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-drill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rows = format!(
            "SELECT CAST(i AS INTEGER) AS \"Order Id\",
                    ['Europe', 'LATAM', NULL, 'USCA'][i % 4 + 1] AS \"Market\",
                    ['Consumer', 'Corporate', 'Home Office'][i % 3 + 1] AS \"Customer Segment\",
                    TIMESTAMP '2017-01-01 10:00:00' + INTERVAL (i) DAY AS \"Order Date\",
                    CAST((i % 7 + 1) * 1.25 AS DECIMAL(12, 2)) AS \"Order Item Total\",
                    'orders.csv' AS _source_file
             FROM range({ORDERS}) t(i)"
        );
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            dir.join("part-0001.parquet").display()
        ))
        .unwrap();
        Orders {
            dir,
            jobs: Arc::new(Semaphore::new(2)),
        }
    }

    fn tables(&self, row_filters: Vec<RowFilter>) -> Vec<AllowedTable> {
        let columns = [
            "Order Id",
            "Market",
            "Customer Segment",
            "Order Date",
            "Order Item Total",
            "_source_file",
        ];
        vec![AllowedTable {
            name: TABLE.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            parquet_dir: self.dir.clone(),
            row_filters,
        }]
    }

    /// Trả lời một câu như luồng hỏi: qua guard (không lọc dòng), chạy, đổi sang ResultTable. Trả sql_guarded và bảng.
    async fn answer(&self, sql: &str) -> (String, ResultTable) {
        let guarded = build_guarded_sql(sql, &self.tables(Vec::new()), 100_000).unwrap();
        let output = run_query_in_turn(&self.jobs, guarded.runnable, Duration::from_secs(30))
            .await
            .unwrap();
        (guarded.shown, build_result_table(output, 100_000))
    }

    async fn drill(
        &self,
        sql_guarded: &str,
        keys: &[DrillKey],
        page: (u64, u32),
        row_filters: Vec<RowFilter>,
    ) -> Result<DrillResponse, AppError> {
        let tables = self.tables(row_filters);
        run_drill(DrillInput {
            sql_guarded,
            keys,
            offset: page.0,
            limit: page.1,
            tables: &tables,
            timeout: Duration::from_secs(30),
            jobs: &self.jobs,
        })
        .await
    }
}

impl Drop for Orders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Dòng đầu tiên của bảng có ô ở cột `column` bằng `value` (None là NULL).
fn find_row<'a>(
    table: &'a ResultTable,
    column: &str,
    value: Option<&str>,
) -> &'a [Option<CellValue>] {
    let i = column_index(table, column);
    table
        .rows
        .iter()
        .find(|row| text(&row[i]).as_deref() == value)
        .unwrap_or_else(|| panic!("no row with {column} = {value:?}"))
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

/// Khóa của một dòng kết quả: giá trị của các cột đã nêu, đúng như ô giao diện nhận được.
fn keys_of(table: &ResultTable, row: &[Option<CellValue>], columns: &[&str]) -> Vec<DrillKey> {
    columns
        .iter()
        .map(|column| DrillKey {
            column: column.to_string(),
            value: row[column_index(table, column)].clone(),
        })
        .collect()
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

fn bad_request(result: Result<DrillResponse, AppError>) -> String {
    match result {
        Err(AppError::BadRequest(message)) => message,
        other => panic!("expected 400, got {other:?}"),
    }
}

const BY_MARKET: &str = "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM orders
                         GROUP BY \"Market\" ORDER BY revenue DESC";

#[tokio::test]
async fn rows_of_a_cell_add_up_to_the_cell() {
    let orders = Orders::new();
    let (sql, table) = orders.answer(BY_MARKET).await;
    let europe = find_row(&table, "Market", Some("Europe"));
    let keys = keys_of(&table, europe, &["Market"]);

    let drilled = orders
        .drill(&sql, &keys, (0, 1000), Vec::new())
        .await
        .unwrap();

    assert_eq!(drilled.total_rows, 60);
    assert_eq!(drilled.result.rows.len(), 60);
    assert_eq!(
        sum_column(&drilled.result, "Order Item Total"),
        cents(&europe[column_index(&table, "revenue")])
    );
    let market = column_index(&drilled.result, "Market");
    assert!(drilled
        .result
        .rows
        .iter()
        .all(|row| text(&row[market]).as_deref() == Some("Europe")));
    // Mọi cột của dataset, kể cả tên file gốc của từng dòng.
    let source = column_index(&drilled.result, "_source_file");
    assert!(drilled
        .result
        .rows
        .iter()
        .all(|row| text(&row[source]).as_deref() == Some("orders.csv")));
    assert!(!drilled.result.limit_reached);
    // SQL trả về là câu sau guard: bảng logic, không có đường dẫn file trên máy chủ, không ORDER BY của câu gốc.
    assert!(drilled.sql.contains("FROM orders"), "{}", drilled.sql);
    assert!(!drilled.sql.contains("parquet"), "{}", drilled.sql);
    assert!(!drilled.sql.contains("ORDER BY"), "{}", drilled.sql);
}

#[tokio::test]
async fn null_key_reads_rows_with_an_empty_cell() {
    let orders = Orders::new();
    let (sql, table) = orders.answer(BY_MARKET).await;
    let empty = find_row(&table, "Market", None);
    let keys = keys_of(&table, empty, &["Market"]);

    let drilled = orders
        .drill(&sql, &keys, (0, 1000), Vec::new())
        .await
        .unwrap();

    assert!(
        drilled.sql.contains("(\"Market\") IS NULL"),
        "{}",
        drilled.sql
    );
    assert_eq!(drilled.total_rows, 60);
    assert_eq!(
        sum_column(&drilled.result, "Order Item Total"),
        cents(&empty[column_index(&table, "revenue")])
    );
}

#[tokio::test]
async fn month_key_keeps_the_where_of_the_question() {
    let orders = Orders::new();
    let question = "SELECT date_trunc('month', \"Order Date\") AS month, \"Market\",
                           SUM(\"Order Item Total\") AS revenue
                    FROM orders WHERE \"Customer Segment\" = 'Consumer' OR \"Customer Segment\" = 'Corporate'
                    GROUP BY 1, 2";
    let (sql, table) = orders.answer(question).await;
    let month = column_index(&table, "month");
    let market = column_index(&table, "Market");
    let row = table
        .rows
        .iter()
        .find(|r| {
            text(&r[month]).as_deref() == Some("2017-03-01T00:00:00")
                && text(&r[market]).as_deref() == Some("LATAM")
        })
        .expect("row for March, LATAM");
    let keys = keys_of(&table, row, &["month", "Market"]);

    let drilled = orders
        .drill(&sql, &keys, (0, 1000), Vec::new())
        .await
        .unwrap();

    // WHERE dạng OR được bọc ngoặc trước khi nối khóa bằng AND.
    assert!(
        drilled.sql.contains(
            "WHERE (\"Customer Segment\" = 'Consumer' OR \"Customer Segment\" = 'Corporate') AND"
        ),
        "{}",
        drilled.sql
    );
    assert!(drilled.total_rows > 0);
    assert_eq!(
        sum_column(&drilled.result, "Order Item Total"),
        cents(&row[column_index(&table, "revenue")])
    );
    let segment = column_index(&drilled.result, "Customer Segment");
    let date = column_index(&drilled.result, "Order Date");
    for r in &drilled.result.rows {
        assert_ne!(text(&r[segment]).as_deref(), Some("Home Office"));
        assert!(text(&r[date]).unwrap().starts_with("2017-03-"));
    }
}

/// Tên cột kết quả của biểu thức không bí danh và của cột viết khác hoa thường do DuckDB đặt; khóa dùng đúng tên
/// giao diện nhận được thì phải khớp. GROUP BY ALL cũng là khóa.
#[tokio::test]
async fn keys_use_result_column_names_without_aliases() {
    let orders = Orders::new();
    let question =
        "SELECT market, date_trunc('month', \"Order Date\"), count(*) FROM orders GROUP BY ALL";
    let (sql, table) = orders.answer(question).await;
    let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    let row = &table.rows[0];
    let keys = keys_of(&table, row, &names[..2]);

    let drilled = orders
        .drill(&sql, &keys, (0, 1000), Vec::new())
        .await
        .unwrap();

    let count = text(&row[2]).unwrap();
    assert_eq!(drilled.total_rows.to_string(), count, "columns {names:?}");
}

#[tokio::test]
async fn pages_do_not_overlap() {
    let orders = Orders::new();
    let (sql, table) = orders.answer(BY_MARKET).await;
    let europe = find_row(&table, "Market", Some("Europe"));
    let keys = keys_of(&table, europe, &["Market"]);

    let mut seen = Vec::new();
    let mut sum = 0;
    for offset in [0, 25, 50] {
        let page = orders
            .drill(&sql, &keys, (offset, 25), Vec::new())
            .await
            .unwrap();
        assert_eq!(page.total_rows, 60);
        let id = column_index(&page.result, "Order Id");
        seen.extend(page.result.rows.iter().map(|r| text(&r[id]).unwrap()));
        sum += sum_column(&page.result, "Order Item Total");
    }
    let count = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!((count, seen.len()), (60, 60));
    assert_eq!(sum, cents(&europe[column_index(&table, "revenue")]));
}

#[tokio::test]
async fn clicker_sees_only_rows_of_their_data_role() {
    let orders = Orders::new();
    // Câu do người không bị lọc hỏi; người bấm chỉ được thấy LATAM.
    let (sql, table) = orders.answer(BY_MARKET).await;
    let latam_only = || {
        vec![RowFilter {
            column: "Market".to_string(),
            allowed_values: vec!["LATAM".to_string()],
        }]
    };
    let europe = keys_of(
        &table,
        find_row(&table, "Market", Some("Europe")),
        &["Market"],
    );
    let latam = keys_of(
        &table,
        find_row(&table, "Market", Some("LATAM")),
        &["Market"],
    );

    let hidden = orders
        .drill(&sql, &europe, (0, 100), latam_only())
        .await
        .unwrap();
    let shown = orders
        .drill(&sql, &latam, (0, 100), latam_only())
        .await
        .unwrap();

    assert_eq!((hidden.total_rows, hidden.result.rows.len()), (0, 0));
    assert!(hidden.sql.contains("IN ('LATAM')"), "{}", hidden.sql);
    assert_eq!(shown.total_rows, 60);
}

#[tokio::test]
async fn unsupported_questions_are_refused() {
    let orders = Orders::new();
    let cases = [
        (
            "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM orders GROUP BY \"Market\"
             UNION ALL SELECT 'all', SUM(\"Order Item Total\") FROM orders",
            "UNION",
        ),
        (
            "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM orders GROUP BY \"Market\"
             HAVING SUM(\"Order Item Total\") > 10",
            "HAVING",
        ),
        (
            "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue,
                    rank() OVER (ORDER BY SUM(\"Order Item Total\")) AS place
             FROM orders GROUP BY \"Market\"",
            "hàm cửa sổ",
        ),
        (
            "SELECT \"Market\", \"Customer Segment\", SUM(\"Order Item Total\") AS revenue FROM orders
             GROUP BY ROLLUP (\"Market\", \"Customer Segment\")",
            "ROLLUP",
        ),
        (
            "WITH t AS (SELECT * FROM orders) SELECT \"Market\", count(*) FROM t GROUP BY \"Market\"",
            "bảng tạm",
        ),
        ("SELECT \"Market\", \"Order Id\" FROM orders", "không có độ đo"),
    ];
    for (question, reason) in cases {
        let (sql, _) = orders.answer(question).await;
        let message = bad_request(orders.drill(&sql, &[], (0, 100), Vec::new()).await);
        assert!(
            message.starts_with("Câu hỏi này chưa truy được dòng nguồn: ")
                && message.contains(reason),
            "{question}: {message}"
        );
    }
}

#[tokio::test]
async fn keys_must_match_the_grouping_columns() {
    let orders = Orders::new();
    let (sql, _) = orders.answer(BY_MARKET).await;
    let key = |column: &str| DrillKey {
        column: column.to_string(),
        value: Some(CellValue::String("Europe".to_string())),
    };
    let cases = [
        (vec![], "Thiếu giá trị của cột Market"),
        (vec![key("Region")], "Kết quả không có cột Region"),
        (vec![key("revenue")], "không phải cột nhóm"),
        (vec![key("Market"), key("Market")], "gửi hai lần"),
    ];
    for (keys, expected) in cases {
        let message = bad_request(orders.drill(&sql, &keys, (0, 100), Vec::new()).await);
        assert!(message.contains(expected), "{message}");
    }
}

/// Câu không có khóa nhóm: một ô duy nhất là tổng của mọi dòng qua WHERE.
#[tokio::test]
async fn total_without_grouping_has_no_keys() {
    let orders = Orders::new();
    let question =
        "SELECT SUM(\"Order Item Total\") AS revenue FROM orders WHERE \"Market\" = 'USCA'";
    let (sql, table) = orders.answer(question).await;

    let drilled = orders
        .drill(&sql, &[], (0, 1000), Vec::new())
        .await
        .unwrap();

    assert_eq!(drilled.total_rows, 60);
    assert_eq!(
        sum_column(&drilled.result, "Order Item Total"),
        cents(&table.rows[0][0])
    );
}
