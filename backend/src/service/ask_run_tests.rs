// SPDX-License-Identifier: Apache-2.0
/* service/ask_run_tests.rs - test vòng guard, run, repair (ask_run.rs, ask_repair.rs) và dạng ô của ResultTable
(ask_result.rs, ask_other_text.rs). Chạy trên file Parquet tạm bằng DuckDB thật, với mô hình giả trả câu viết sẵn;
không cần Postgres. */

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use duckdb::types::Value;
use tokio::sync::Semaphore;

use super::ask_contract::{AskStatus, StepName, ValueKind};
use super::ask_other_text::build_other_text;
use super::ask_repair::{build_safe_error, build_user_error, read_sql_reply};
use super::ask_result::{build_result_table, SENT_ROWS};
use super::ask_run::{run_with_repair, RunInput, RunOutcome};
use crate::engine::analytics::parquet_glob;
use crate::engine::guard::AllowedTable;
use crate::engine::llm::{self, LlmProvider, LlmRequest, LlmTask};
use crate::engine::query::run_query;
use crate::error::AppError;

const MAX_ROWS: u64 = 100_000;
const PROMPT: &str = "Schema: table sales (region, amount, ship_date)";

/// Thư mục tạm chứa Parquet của bảng sales, tự xóa khi ra khỏi phạm vi.
struct Dataset {
    root: PathBuf,
    tables: Vec<AllowedTable>,
}

impl Dataset {
    /// rows: câu SELECT sinh các dòng của bảng, ghi thành một file Parquet.
    fn new(name: &str, columns: &[&str], rows: &str) -> Self {
        let root = std::env::temp_dir().join(format!("xdi-ask-run-{}", uuid::Uuid::new_v4()));
        let dir = root.join("ws").join("dataset").join("v1");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("part-0001.parquet");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
        let tables = vec![AllowedTable {
            name: name.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            parquet_dir: dir,
        }];
        Dataset { root, tables }
    }

    fn sales() -> Self {
        Dataset::new(
            "sales",
            &["region", "amount", "ship_date"],
            "SELECT * FROM (VALUES ('South', 10, '1/31/2018 22:56'), ('North', 5, '2/1/2018 08:00')) \
             v(region, amount, ship_date)",
        )
    }

    fn dir_text(&self) -> String {
        self.tables[0].parquet_dir.display().to_string()
    }
}

impl Drop for Dataset {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Mô hình giả: trả lần lượt các câu viết sẵn và ghi lại các request nhận được.
struct ScriptedLlm {
    replies: Mutex<VecDeque<Result<String, AppError>>>,
    requests: Mutex<Vec<(LlmTask, String)>>,
}

impl ScriptedLlm {
    fn new(replies: Vec<Result<String, AppError>>) -> Self {
        ScriptedLlm {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn prompts(&self) -> Vec<String> {
        let requests = self.requests.lock().unwrap();
        requests.iter().map(|(_, prompt)| prompt.clone()).collect()
    }
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        let entry = (request.task, request.prompt.to_string());
        self.requests.lock().unwrap().push(entry);
        let reply = self.replies.lock().unwrap().pop_front();
        reply.expect("model called more times than the test expects")
    }
}

async fn run(llm: &dyn LlmProvider, data: &Dataset, sql: &str, timeout: Duration) -> RunOutcome {
    try_run(llm, data, sql, timeout).await.unwrap()
}

async fn try_run(
    llm: &dyn LlmProvider,
    data: &Dataset,
    sql: &str,
    timeout: Duration,
) -> Result<RunOutcome, AppError> {
    let jobs = Arc::new(Semaphore::new(2));
    let input = RunInput {
        question: "Doanh thu theo khu vực",
        prompt: PROMPT,
        sql: sql.to_string(),
        tables: &data.tables,
        max_rows: MAX_ROWS,
        timeout,
        jobs: &jobs,
    };
    run_with_repair(llm, input).await
}

async fn run_scripted(llm: &ScriptedLlm, sql: &str) -> RunOutcome {
    run(llm, &Dataset::sales(), sql, Duration::from_secs(30)).await
}

/// Tên và ok của từng bước, để so với thứ tự mong đợi.
fn steps(outcome: &RunOutcome) -> Vec<(StepName, bool)> {
    outcome.steps.iter().map(|s| (s.name, s.ok)).collect()
}

const GOOD_SQL: &str =
    "SELECT region, SUM(amount) AS total FROM sales GROUP BY region ORDER BY region";
// Guard không biết kiểu cột nên câu này qua guard; DuckDB báo lỗi khi đổi 'South' sang số.
const BAD_SQL: &str = "SELECT CAST(region AS INTEGER) AS n FROM sales";

#[tokio::test]
async fn good_sql_runs_without_the_model() {
    let llm = ScriptedLlm::new(vec![]);
    let outcome = run_scripted(&llm, GOOD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Answered);
    assert_eq!(
        steps(&outcome),
        [(StepName::Guard, true), (StepName::Run, true)]
    );
    let result = outcome.result.unwrap();
    assert_eq!(result.total_rows, 2);
    assert!(!result.truncated && !result.limit_reached);
    assert!(outcome.sql_guarded.unwrap().ends_with("LIMIT 100000"));
    assert!(outcome.message.is_none());
    // Chi tiết bước guard như mẫu ở ask_examples.rs.
    assert_eq!(
        outcome.steps[0].detail.as_deref(),
        Some("Thêm LIMIT 100000")
    );
}

#[tokio::test]
async fn run_error_is_fixed_once() {
    let llm = ScriptedLlm::new(vec![Ok(format!("```sql\n{GOOD_SQL}\n```"))]);
    let outcome = run_scripted(&llm, BAD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, true),
            (StepName::Run, false),
            (StepName::Repair, true),
            (StepName::Guard, true),
            (StepName::Run, true),
        ]
    );
    // sql_generated là câu đã sửa, lấy ra khỏi khối ```sql.
    assert_eq!(outcome.sql_generated, GOOD_SQL);
    let prompts = llm.prompts();
    assert_eq!(prompts.len(), 1);
    let prompt = &prompts[0];
    assert!(prompt.starts_with(PROMPT), "{prompt}");
    assert!(
        prompt.contains(BAD_SQL) && prompt.contains("Could not convert string 'South'"),
        "{prompt}"
    );
}

/// Đúng thứ tự bước của mẫu failed trong ask_examples.rs; không có vòng sửa thứ hai.
#[tokio::test]
async fn second_failure_stops_the_question() {
    let still_bad = "SELECT CAST(region AS DOUBLE) AS n FROM sales";
    let llm = ScriptedLlm::new(vec![Ok(still_bad.to_string())]);
    let outcome = run_scripted(&llm, BAD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Failed);
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, true),
            (StepName::Run, false),
            (StepName::Repair, true),
            (StepName::Guard, true),
            (StepName::Run, false),
        ]
    );
    assert_eq!(outcome.sql_generated, still_bad);
    assert!(outcome.sql_guarded.unwrap().starts_with(still_bad));
    // Lời cho người dùng bỏ loại lỗi tiếng Anh của DuckDB ("Conversion Error: "), như mẫu failed.
    let message = outcome.message.unwrap();
    assert!(
        message.starts_with(
            "Không chạy được câu SQL, kể cả sau khi mô hình sửa một lần: Could not convert string"
        ) && message.contains("DOUBLE"),
        "{message}"
    );
    let run_detail = outcome.steps[1].detail.as_deref().unwrap();
    assert!(
        run_detail.starts_with("Could not convert string 'South'"),
        "{run_detail}"
    );
    // Mô hình vẫn nhận cả loại lỗi.
    assert!(llm.prompts()[0].contains("Conversion Error: Could not convert"));
    assert!(outcome.result.is_none());
}

/// File Parquet hỏng là lỗi máy chủ: không gửi mô hình (ScriptedLlm không có câu trả lời, gọi là panic).
#[tokio::test]
async fn corrupt_parquet_file_is_a_server_error() {
    let data = Dataset::sales();
    let file = data.tables[0].parquet_dir.join("part-0001.parquet");
    std::fs::write(&file, b"not a parquet file").unwrap();
    let llm = ScriptedLlm::new(vec![]);
    let result = try_run(&llm, &data, GOOD_SQL, Duration::from_secs(30)).await;
    assert!(matches!(result, Err(AppError::Internal(_))), "{result:?}");
}

#[test]
fn sql_is_read_from_code_blocks_in_any_case() {
    for reply in [
        "SELECT 1",
        "```sql\nSELECT 1\n```",
        "```SQL\nSELECT 1\n```",
        "```Sql SELECT 1```",
        "```\nSELECT 1\n```",
        "  ```sql\n  SELECT 1  \n```  ",
    ] {
        assert_eq!(read_sql_reply(reply), "SELECT 1", "{reply:?}");
    }
    // "select" ngay sau ``` là SQL, không phải tên ngôn ngữ.
    assert_eq!(read_sql_reply("```select 1```"), "select 1");
}

#[test]
fn user_errors_drop_the_duckdb_error_kind() {
    assert_eq!(
        build_user_error(
            "Binder Error: Referenced column \"x\" not found\nCandidate bindings: \"a\""
        ),
        "Referenced column \"x\" not found"
    );
    assert_eq!(
        build_user_error("Không có cột x trong các bảng đang dùng"),
        "Không có cột x trong các bảng đang dùng"
    );
}

/// Guard từ chối vì cột không có trong hồ sơ: lý do của guard gửi cho mô hình sửa.
#[tokio::test]
async fn unknown_column_is_fixed_once() {
    let llm = ScriptedLlm::new(vec![Ok(GOOD_SQL.to_string())]);
    let outcome = run_scripted(&llm, "SELECT revenue FROM sales").await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, false),
            (StepName::Repair, true),
            (StepName::Guard, true),
            (StepName::Run, true),
        ]
    );
    assert!(llm.prompts()[0].contains("Không có cột revenue"));
}

/// Câu ghi bị từ chối ngay, không gọi mô hình (ScriptedLlm không có câu trả lời nào, gọi là panic).
#[tokio::test]
async fn other_guard_refusals_are_not_fixed() {
    let llm = ScriptedLlm::new(vec![]);
    let outcome = run_scripted(&llm, "DELETE FROM sales").await;
    assert_eq!(outcome.status, AskStatus::Rejected);
    assert_eq!(steps(&outcome), [(StepName::Guard, false)]);
    assert!(outcome.sql_guarded.is_none());
    let message = outcome.message.unwrap();
    assert!(message.starts_with("Câu SQL bị từ chối: "), "{message}");
}

#[tokio::test]
async fn fixed_sql_refused_by_the_guard_is_rejected() {
    let llm = ScriptedLlm::new(vec![Ok("SELECT getenv('HOME')".to_string())]);
    let outcome = run_scripted(&llm, BAD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Rejected);
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, true),
            (StepName::Run, false),
            (StepName::Repair, true),
            (StepName::Guard, false),
        ]
    );
    assert_eq!(outcome.sql_generated, "SELECT getenv('HOME')");
    assert!(outcome.sql_guarded.is_none());
}

/// Mô hình lỗi khi sửa: failed kèm các bước, không phải lỗi 502 của cả request.
#[tokio::test]
async fn model_error_during_repair_is_failed() {
    let llm = ScriptedLlm::new(vec![Err(AppError::Upstream(
        "Gemini đang quá tải, thử lại sau ít phút".to_string(),
    ))]);
    let outcome = run_scripted(&llm, BAD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Failed);
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, true),
            (StepName::Run, false),
            (StepName::Repair, false),
        ]
    );
    assert_eq!(outcome.sql_generated, BAD_SQL);
    assert!(outcome.sql_guarded.unwrap().starts_with(BAD_SQL));
    let message = outcome.message.unwrap();
    assert!(message.contains("Gemini đang quá tải"), "{message}");
}

/// Quá giờ thì dừng và báo, không nhờ mô hình sửa. Tích chéo ba lần bảng 3000 dòng (27 tỷ dòng) chạy lâu hơn nhiều so
/// với 300 ms.
#[tokio::test]
async fn query_over_the_time_limit_is_stopped() {
    let data = Dataset::new(
        "sales",
        &["region", "amount"],
        "SELECT 'South' AS region, range AS amount FROM range(3000)",
    );
    let llm = ScriptedLlm::new(vec![]);
    let sql = "SELECT COUNT(*) AS n FROM sales a, sales b, sales c";
    let outcome = run(&llm, &data, sql, Duration::from_millis(300)).await;
    assert_eq!(outcome.status, AskStatus::Failed);
    assert_eq!(
        steps(&outcome),
        [(StepName::Guard, true), (StepName::Run, false)]
    );
    let message = outcome.message.unwrap();
    assert!(message.contains("đã bị dừng"), "{message}");
}

/// Không lời báo nào, cho người dùng hay cho mô hình, chứa đường dẫn trên máy chủ: lỗi đọc ngày của DuckDB chạy trên
/// Parquet thật, rồi lời báo có cả đường dẫn file và phần LINE chép lại bản runnable.
#[tokio::test]
async fn error_texts_have_no_server_paths() {
    let data = Dataset::sales();
    let llm = ScriptedLlm::new(vec![Ok(
        "SELECT strptime(ship_date, '%d/%m/%Y %H:%M') AS shipped FROM sales".to_string(),
    )]);
    let sql = "SELECT strptime(ship_date, '%d/%m/%Y') AS shipped FROM sales";
    let outcome = run(&llm, &data, sql, Duration::from_secs(30)).await;
    assert_eq!(outcome.status, AskStatus::Failed);
    let root = data.root.display().to_string();
    let mut texts = llm.prompts();
    texts.push(outcome.message.clone().unwrap());
    texts.extend(outcome.steps.iter().filter_map(|s| s.detail.clone()));
    for text in &texts {
        assert!(!text.contains(&root), "{text}");
        assert!(!text.contains("read_parquet"), "{text}");
    }
    assert!(outcome.message.unwrap().contains("Could not parse string"));

    let dir = data.dir_text();
    let glob = parquet_glob(&data.tables[0].parquet_dir).unwrap();
    let raw = format!(
        "IO Error: Could not read file \"{dir}/part-0001.parquet\": broken\n\nLINE 1: SELECT * FROM {glob} AS sales\n    ^"
    );
    let safe = build_safe_error(&raw, &data.tables);
    assert_eq!(
        safe,
        "IO Error: Could not read file \"sales/part-0001.parquet\": broken"
    );
}

/// Câu mẫu của mock: SQL đọc ngày theo ngày/tháng/năm lỗi trên chuỗi "1/31/2018 22:56", câu sửa của mock chạy được.
#[tokio::test]
async fn mock_fixes_its_sample_question() {
    let data = Dataset::new(
        "dataco",
        &["shipping date (DateOrders)", "Order Item Total"],
        "SELECT * FROM (VALUES ('1/31/2018 22:56', 10.5), ('1/31/2018 08:00', 2.0), ('2/1/2018 09:30', 4.0)) \
         v(\"shipping date (DateOrders)\", \"Order Item Total\")",
    );
    let mut config = crate::config::Config::from_env();
    config.llm_provider = "mock".to_string();
    let mock = llm::from_config(&config);
    let question = "Doanh thu theo ngày giao hàng";
    let request = LlmRequest {
        task: LlmTask::Sql,
        question,
        prompt: "",
    };
    let sql = mock.complete(&request).await.unwrap();
    let jobs = Arc::new(Semaphore::new(2));
    let input = RunInput {
        question,
        prompt: "",
        sql,
        tables: &data.tables,
        max_rows: MAX_ROWS,
        timeout: Duration::from_secs(30),
        jobs: &jobs,
    };
    let outcome = run_with_repair(mock.as_ref(), input).await.unwrap();
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let rows = serde_json::to_value(&outcome.result.unwrap().rows).unwrap();
    assert_eq!(
        rows,
        serde_json::json!([["2018-01-31", "12.5"], ["2018-02-01", "4.0"]])
    );
}

// Dạng ô

/// Mỗi kiểu cột ra đúng ValueKind và đúng dạng JSON theo bảng "Giá trị ô" (ARCHITECTURE mục 8).
#[test]
fn cells_follow_the_kind_of_their_column() {
    let sql = "SELECT 7::INTEGER AS int, 9007199254740993::BIGINT AS big, \
               170141183460469231731687303715884105727::HUGEINT AS huge, -1234.50::DECIMAL(10, 2) AS dec, \
               'nan'::DOUBLE AS nan, '-inf'::DOUBLE AS inf, 1.25::DOUBLE AS dbl, DATE '2024-01-02' AS day, \
               TIMESTAMP '2024-01-02 03:04:05' AS ts, TIMESTAMP '2024-01-02 03:04:05.25' AS ts_frac, \
               TIMESTAMPTZ '2024-01-02 03:04:05+07' AS tz, true AS flag, [1, 2] AS list, \
               NULL::INTEGER AS empty, 'Hà Nội' AS text";
    let output = run_query(sql, SENT_ROWS, Duration::from_secs(30)).unwrap();
    let table = build_result_table(output, MAX_ROWS);
    let kinds: Vec<ValueKind> = table.columns.iter().map(|c| c.kind).collect();
    use ValueKind::*;
    assert_eq!(
        kinds,
        [
            Number, Decimal, Decimal, Decimal, Number, Number, Number, Date, Timestamp, Timestamp,
            Timestamp, Boolean, Other, Number, Text
        ]
    );
    assert_eq!(
        serde_json::to_value(&table.rows[0]).unwrap(),
        serde_json::json!([
            7.0,
            "9007199254740993",
            "170141183460469231731687303715884105727",
            "-1234.50",
            null,
            null,
            1.25,
            "2024-01-02",
            "2024-01-02T03:04:05",
            "2024-01-02T03:04:05.250000",
            "2024-01-01T20:04:05Z",
            true,
            "[1, 2]",
            null,
            "Hà Nội"
        ])
    );
}

/// FLOAT gửi đúng số ngắn nhất của f32; ngày giờ ngoài năm 0000 tới 9999 và vô cực theo chú thích của ValueKind.
#[test]
fn float_and_edge_dates_follow_the_contract() {
    let sql = "SELECT 0.1::FLOAT AS f, DATE '10000-01-01' AS future, DATE '0001-01-01' - 1 AS bc, \
               'infinity'::DATE AS forever, TIMESTAMP '10000-01-01 00:00:00' AS ts, 'infinity'::TIMESTAMP AS ts_inf";
    let output = run_query(sql, SENT_ROWS, Duration::from_secs(30)).unwrap();
    let table = build_result_table(output, MAX_ROWS);
    assert_eq!(
        serde_json::to_value(&table.rows[0]).unwrap(),
        serde_json::json!([
            0.1,
            "+10000-01-01",
            "0000-12-31",
            null,
            "+10000-01-01T00:00:00",
            null
        ])
    );
}

/// Dạng other giống hệt DuckDB tự in giá trị sang VARCHAR.
#[test]
fn other_values_are_printed_like_duckdb() {
    let values = [
        "TIME '12:34:56.5'",
        "TIME '07:00:00'",
        "INTERVAL '1 year 2 months 3 days 4 hours 5 minutes 6.5 seconds'",
        "INTERVAL '-1 day 2 hours'",
        "INTERVAL '30 hours'",
        "INTERVAL '0 seconds'",
        "[1, 2, 3]",
        "['a', 'b c', NULL]",
        "[1.5, 2]::DOUBLE[]",
        "{'a': 1, 'b': 'x'}",
        "MAP {'k': 1}",
        "'5b2f0c1e-7a4d-4e8b-9c61-0d3e2f1a4b00'::UUID",
        "'\\xAA\\x41'::BLOB",
    ];
    let conn = duckdb::Connection::open_in_memory().unwrap();
    for value in values {
        let sql = format!("SELECT {value}, CAST({value} AS VARCHAR)");
        let (value, printed): (Value, String) = conn
            .query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert_eq!(build_other_text(&value), printed, "{sql}");
    }
}

#[test]
fn at_most_1000_rows_are_sent() {
    let output = run_query(
        "SELECT range AS n FROM range(1500)",
        SENT_ROWS,
        Duration::from_secs(30),
    )
    .unwrap();
    let table = build_result_table(output, MAX_ROWS);
    assert_eq!(table.rows.len(), 1000);
    assert_eq!(table.total_rows, 1500);
    assert!(table.truncated);
    assert!(!table.limit_reached);
}

/// Kết quả có đúng số dòng của trần guard thì có thể còn dòng bị cắt.
#[test]
fn reaching_the_guard_cap_is_reported() {
    let output = run_query(
        "SELECT range AS n FROM range(10)",
        SENT_ROWS,
        Duration::from_secs(30),
    )
    .unwrap();
    let table = build_result_table(output, 10);
    assert!(table.limit_reached);
    assert!(!table.truncated);
}

#[test]
fn duplicate_column_names_are_made_unique() {
    let output = run_query(
        "SELECT 1 AS a, 2 AS a, 3 AS a_2",
        SENT_ROWS,
        Duration::from_secs(30),
    )
    .unwrap();
    let table = build_result_table(output, MAX_ROWS);
    let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "a_3", "a_2"]);
}
