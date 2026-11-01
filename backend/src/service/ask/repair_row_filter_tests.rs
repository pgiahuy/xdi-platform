// SPDX-License-Identifier: Apache-2.0
/* service/ask/repair_row_filter_tests.rs - lời báo lỗi của DuckDB không chép giá trị của dòng người hỏi không được
thấy. DuckDB có thể chép một ô vào lời báo ("Could not convert string 'Nguyen Van A' to INT32"); lời đó tới mô hình ở
vòng sửa (ask/repair.rs), tới chi tiết bước, lời báo cho người hỏi và nhật ký lỗi. Nếu DuckDB tính biểu thức của câu
mô hình viết trước điều kiện lọc dòng của guard (bước 3, docs/GUARD.md), ô trong lời báo có thể là của dòng bị ẩn.

Đo trên DuckDB đi kèm crate duckdb (Cargo.lock): EXPLAIN cho thấy bộ lọc có CAST của câu ngoài nằm trên bộ lọc theo
vai, và không dạng câu nào dưới đây chép ô của dòng bị ẩn. Đó là cách bộ tối ưu của DuckDB xếp bộ lọc, không phải điều
code của dự án bảo đảm, nên các test này giữ để bắt khi nâng DuckDB. Mỗi dạng câu chạy thêm một lần không lọc để chắc
dữ liệu thật sự làm câu lỗi ở dòng bị ẩn (không thì test không kiểm gì).

Dữ liệu: bảng orders đủ nhiều dòng để có nhiều row group và nhiều lô (vector) của DuckDB; cứ 997 dòng có một dòng
Europe mang chữ không đổi được sang số hay ngày. Bảng customers: dòng Europe mang mã không đổi được sang số. Người hỏi
chỉ thấy LATAM (bốn dạng điều kiện: một giá trị, danh sách IN, nhiều vai AnyOf, lan theo khóa ngoại). Không cần
Postgres. Đường qua route thật với vai dữ liệu ở api/ask_privacy_tests.rs. */

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::engine::guard::{build_guarded_sql, AllowedTable, JoinLink, RowFilter};
use crate::engine::query::{run_query, QueryError};
use crate::service::ask::contract::AskStatus;
use crate::service::ask::run::{run_with_repair, RunInput};
use crate::service::ask::run_tests::{reply, ScriptedLlm};

/// Các chữ chỉ nằm ở dòng bị ẩn. Lời báo nào có một trong số này là lộ dòng bị ẩn.
const HIDDEN: [&str; 3] = ["Nguyen Van A", "secret-date", "Tran Thi B"];

/// Số dòng của orders: hai row group của file Parquet DuckDB ghi (122 880 dòng mỗi nhóm), nhiều lô 2 048 dòng.
const ORDERS_ROWS: u64 = 150_000;

/// Câu mô hình có thể viết, mỗi câu lỗi ở dòng bị ẩn khi không lọc: CAST sang số, ngày, giờ, thập phân, strptime, ở
/// WHERE, ON, câu con, EXISTS, câu con vô hướng, ORDER BY ... LIMIT, HAVING, UNION, cửa sổ, FILTER.
const SHAPES: [&str; 17] = [
    "SELECT CAST(qty AS INTEGER) AS n FROM orders",
    "SELECT id FROM orders WHERE CAST(qty AS INTEGER) > 0",
    "SELECT id FROM orders WHERE id > 0 AND qty::INTEGER = 5",
    "SELECT \"Market\", SUM(CAST(qty AS INTEGER)) AS total FROM orders GROUP BY 1",
    "SELECT COUNT(*) AS n FROM orders WHERE CAST(d AS DATE) = DATE '2024-01-15'",
    "SELECT id FROM orders WHERE CAST(d AS TIMESTAMP) > TIMESTAMP '2020-01-01'",
    "SELECT id FROM orders WHERE strptime(d, '%Y-%m-%d') > TIMESTAMP '2020-01-01'",
    "SELECT id FROM orders WHERE CAST(qty AS DECIMAL(10,2)) > 0",
    "SELECT o.id FROM orders o JOIN customers c ON o.c_id = c.id WHERE CAST(c.code AS INTEGER) > 0",
    "SELECT o.id FROM orders o JOIN customers c ON o.c_id = c.id AND CAST(c.code AS INTEGER) > 0",
    "SELECT id FROM orders WHERE c_id IN (SELECT id FROM customers WHERE CAST(code AS INTEGER) > 0)",
    "SELECT id FROM orders o WHERE EXISTS (SELECT 1 FROM customers c WHERE c.id = o.c_id AND CAST(c.code AS INTEGER) > 0)",
    "SELECT id, (SELECT CAST(code AS INTEGER) FROM customers c WHERE c.id = o.c_id) AS k FROM orders o",
    "SELECT id FROM orders ORDER BY CAST(qty AS INTEGER) LIMIT 5",
    "SELECT CAST(qty AS INTEGER) AS q, COUNT(*) AS n FROM orders GROUP BY 1 HAVING CAST(MIN(qty) AS INTEGER) >= 0",
    "SELECT id FROM orders WHERE CAST(qty AS INTEGER) > 0 UNION ALL SELECT id FROM customers WHERE CAST(code AS INTEGER) > 0",
    "SELECT id, ROW_NUMBER() OVER (ORDER BY CAST(qty AS INTEGER)) AS r FROM orders",
];

/// Thư mục tạm chứa Parquet của orders và customers, tự xóa khi ra khỏi phạm vi.
struct Shop {
    root: PathBuf,
    orders: PathBuf,
    customers: PathBuf,
}

impl Shop {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("xdi-row-filter-{}", uuid::Uuid::new_v4()));
        // Dòng Europe (i % 997 = 13) mang chữ không đổi được; dòng LATAM đổi được hết. c_id trỏ tới customers; khách
        // số chẵn là LATAM.
        let orders = write_parquet(
            &root,
            "orders",
            &format!(
                "SELECT CASE WHEN i % 997 = 13 THEN 'Europe' ELSE 'LATAM' END AS \"Market\", \
                 CASE WHEN i % 997 = 13 THEN 'Nguyen Van A' ELSE CAST(i % 100 AS VARCHAR) END AS qty, \
                 CASE WHEN i % 997 = 13 THEN 'secret-date' ELSE '2024-01-15' END AS d, \
                 CASE WHEN i % 997 = 13 THEN 1 ELSE (i % 1000) * 2 END AS c_id, i AS id \
                 FROM range({ORDERS_ROWS}) t(i)"
            ),
        );
        let customers = write_parquet(
            &root,
            "customers",
            "SELECT i AS id, CASE WHEN i % 2 = 0 THEN 'LATAM' ELSE 'Europe' END AS region, \
             CASE WHEN i % 2 = 0 THEN CAST(i AS VARCHAR) ELSE 'Tran Thi B' END AS code FROM range(2000) t(i)",
        );
        Shop {
            root,
            orders,
            customers,
        }
    }

    /// customers chỉ dòng LATAM với người hỏi.
    fn customers(&self) -> AllowedTable {
        AllowedTable {
            name: "customers".to_string(),
            columns: vec!["id".into(), "region".into(), "code".into()],
            parquet_dir: self.customers.clone(),
            row_filters: vec![only("region", &["LATAM"])],
        }
    }

    /// orders với điều kiện lọc cho sẵn, và customers đã lọc.
    fn tables(&self, orders_filters: Vec<RowFilter>) -> Vec<AllowedTable> {
        let orders = AllowedTable {
            name: "orders".to_string(),
            columns: ["Market", "qty", "d", "c_id", "id"]
                .iter()
                .map(|c| c.to_string())
                .collect(),
            parquet_dir: self.orders.clone(),
            row_filters: orders_filters,
        };
        vec![orders, self.customers()]
    }

    /// Bốn cách một người chỉ thấy LATAM của orders: một luật, luật nhiều giá trị (giá trị thứ hai không có trong
    /// bảng), hai vai (AnyOf), và điều kiện lan từ customers theo khóa ngoại.
    fn filter_kinds(&self) -> Vec<(&'static str, Vec<RowFilter>)> {
        vec![
            ("one value", vec![only("Market", &["LATAM"])]),
            ("IN list", vec![only("Market", &["LATAM", "USCA"])]),
            (
                "two roles",
                vec![RowFilter::AnyOf(vec![
                    vec![only("Market", &["LATAM"])],
                    vec![only("Market", &["Pacific Asia"])],
                ])],
            ),
            (
                "parent",
                vec![RowFilter::Parent {
                    columns: vec!["c_id".into()],
                    parent_columns: vec!["id".into()],
                    parent: self.customers(),
                }],
            ),
        ]
    }
}

impl Drop for Shop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_parquet(root: &Path, table: &str, rows: &str) -> PathBuf {
    let dir = root.join(table).join("v1");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("part-0001.parquet");
    let conn = duckdb::Connection::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "COPY ({rows}) TO '{}' (FORMAT parquet)",
        file.display()
    ))
    .unwrap();
    dir
}

fn only(column: &str, values: &[&str]) -> RowFilter {
    RowFilter::Column {
        column: column.to_string(),
        allowed_values: values.iter().map(|v| v.to_string()).collect(),
    }
}

fn links() -> Vec<JoinLink> {
    vec![JoinLink {
        many: "orders".to_string(),
        many_columns: vec!["c_id".to_string()],
        one: "customers".to_string(),
        one_columns: vec!["id".to_string()],
        fans_out: true,
    }]
}

/// Lời báo của DuckDB khi câu qua guard rồi chạy lỗi; None khi câu chạy được.
fn run_error(sql: &str, tables: &[AllowedTable]) -> Option<String> {
    let guarded = build_guarded_sql(sql, tables, &links(), 100)
        .unwrap_or_else(|e| panic!("guard refused {sql}: {e:?}"));
    match run_query(&guarded.runnable, 10, Duration::from_secs(60)) {
        Ok(_) => None,
        Err(QueryError::Sql(message)) => Some(message),
        Err(QueryError::Timeout | QueryError::OutOfMemory) => panic!("{sql} was stopped"),
        Err(QueryError::Internal(e)) => panic!("{sql}: {e:?}"),
    }
}

fn hidden_value_in(text: &str) -> Option<&'static str> {
    HIDDEN.into_iter().find(|value| text.contains(value))
}

/// Mỗi dạng câu: không lọc thì DuckDB chép ô của dòng Europe vào lời báo (dữ liệu đúng là làm câu lỗi); lọc theo vai
/// thì lời báo, nếu có, không chép ô nào của dòng bị ẩn.
#[test]
fn run_errors_never_quote_rows_hidden_by_the_row_filter() {
    let shop = Shop::new();
    let mut open_tables = shop.tables(vec![]);
    open_tables[1].row_filters.clear();
    for sql in SHAPES {
        let error = run_error(sql, &open_tables).unwrap_or_default();
        assert!(
            hidden_value_in(&error).is_some(),
            "without row filters {sql} should fail on a Europe row: {error}"
        );
        for (kind, filters) in shop.filter_kinds() {
            let error = run_error(sql, &shop.tables(filters)).unwrap_or_default();
            assert!(
                hidden_value_in(&error).is_none(),
                "{kind}: {sql} quoted a hidden row: {error}"
            );
        }
    }
}

/// Qua vòng hỏi: câu lỗi cả ở dòng được thấy (chuỗi ngày đọc sai định dạng). Lời báo gửi mô hình, chi tiết bước, nhật
/// ký lỗi chép ô của dòng LATAM ('2024-01-15', mô hình cần để sửa định dạng), không chép ô của dòng bị ẩn.
#[tokio::test]
async fn repair_prompt_and_steps_quote_only_visible_rows() {
    let shop = Shop::new();
    let fixed =
        "SELECT COUNT(*) AS n FROM orders WHERE strptime(d, '%Y-%m-%d') > TIMESTAMP '2020-01-01'";
    let wrong =
        "SELECT COUNT(*) AS n FROM orders WHERE strptime(d, '%d/%m/%Y') > TIMESTAMP '2020-01-01'";
    for (kind, filters) in shop.filter_kinds() {
        let tables = shop.tables(filters);
        let llm = ScriptedLlm::new(vec![Ok(reply(fixed))]);
        let outcome = ask(&llm, &tables, wrong).await;
        assert_eq!(outcome.status, AskStatus::Answered, "{kind}: {outcome:?}");
        let prompts = llm.prompts();
        assert_eq!(prompts.len(), 1, "{kind}");
        assert!(prompts[0].contains("2024-01-15"), "{kind}: {}", prompts[0]);
        let mut seen = prompts.join("\n");
        for step in &outcome.steps {
            seen.push_str(step.detail.as_deref().unwrap_or_default());
        }
        for failure in &outcome.failures {
            seen.push_str(&format!("{failure:?}"));
        }
        assert!(hidden_value_in(&seen).is_none(), "{kind}: {seen}");
    }
}

/// Câu chỉ lỗi ở dòng bị ẩn: với người bị lọc câu chạy được ngay, không vào vòng sửa (mô hình giả không có câu trả lời
/// nào, gọi tới là test dừng).
#[tokio::test]
async fn sql_failing_only_on_hidden_rows_answers_without_repair() {
    let shop = Shop::new();
    let sql = "SELECT SUM(CAST(qty AS INTEGER)) AS total FROM orders";
    for (kind, filters) in shop.filter_kinds() {
        let tables = shop.tables(filters);
        let llm = ScriptedLlm::new(vec![]);
        let outcome = ask(&llm, &tables, sql).await;
        assert_eq!(outcome.status, AskStatus::Answered, "{kind}: {outcome:?}");
        assert!(llm.prompts().is_empty(), "{kind}");
    }
}

async fn ask(
    llm: &ScriptedLlm,
    tables: &[AllowedTable],
    sql: &str,
) -> crate::service::ask::run::RunOutcome {
    let jobs = Arc::new(Semaphore::new(2));
    let first = reply(sql);
    let input = RunInput {
        question: "Số đơn",
        prompt: "Schema: table orders (Market, qty, d, c_id, id)",
        reply: &first,
        tables,
        links: &links(),
        max_rows: 100_000,
        timeout: Duration::from_secs(60),
        jobs: &jobs,
        sensitive: &[],
    };
    run_with_repair(llm, input).await.unwrap()
}
