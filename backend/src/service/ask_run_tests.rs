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
use super::ask_repair::{build_safe_error, build_user_error};
use super::ask_reply::{ChoiceKind, Term, TermKind};
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
            row_filters: vec![],
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
    /// Loại việc, prompt, và request có gửi JSON Schema không.
    requests: Mutex<Vec<(LlmTask, String, bool)>>,
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
        requests
            .iter()
            .map(|(_, prompt, _)| prompt.clone())
            .collect()
    }

    /// Mỗi request có gửi JSON Schema không, theo thứ tự.
    fn schemas_sent(&self) -> Vec<bool> {
        let requests = self.requests.lock().unwrap();
        requests.iter().map(|(_, _, schema)| *schema).collect()
    }
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        let entry = (
            request.task,
            request.prompt.to_string(),
            request.json_schema.is_some(),
        );
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
    try_run_reply(llm, data, &reply(sql), timeout).await
}

/// Như try_run, nhưng câu trả lời đầu của mô hình cho nguyên văn, để thử câu trả lời không đọc được hay hỏi lại.
async fn try_run_reply(
    llm: &dyn LlmProvider,
    data: &Dataset,
    first_reply: &str,
    timeout: Duration,
) -> Result<RunOutcome, AppError> {
    let jobs = Arc::new(Semaphore::new(2));
    let input = RunInput {
        question: "Doanh thu theo khu vực",
        prompt: PROMPT,
        reply: first_reply,
        tables: &data.tables,
        max_rows: MAX_ROWS,
        timeout,
        jobs: &jobs,
    };
    run_with_repair(llm, input).await
}

/// Câu trả lời của mô hình dạng JSON (ask_reply.rs) với câu SQL cho sẵn, chưa có cách hiểu nào.
fn reply(sql: &str) -> String {
    serde_json::json!({"terms": [], "assumptions": [], "sql": sql}).to_string()
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
    let llm = ScriptedLlm::new(vec![Ok(format!("```json\n{}\n```", reply(GOOD_SQL)))]);
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
    // sql_generated là câu đã sửa, lấy ra khỏi object JSON trong khối ```json.
    assert_eq!(outcome.sql_generated.as_deref(), Some(GOOD_SQL));
    let prompts = llm.prompts();
    assert_eq!(prompts.len(), 1);
    let prompt = &prompts[0];
    assert!(prompt.starts_with(PROMPT), "{prompt}");
    assert!(
        prompt.contains(BAD_SQL) && prompt.contains("Could not convert string 'South'"),
        "{prompt}"
    );
    // Câu sửa phải cùng dạng JSON như lần đầu: prompt dặn, và request gửi kèm schema.
    assert!(prompt.contains("one JSON object"), "{prompt}");
    assert_eq!(llm.schemas_sent(), [true]);
}

/// Đúng thứ tự bước của mẫu failed trong ask_examples.rs; không có vòng sửa thứ hai.
#[tokio::test]
async fn second_failure_stops_the_question() {
    let still_bad = "SELECT CAST(region AS DOUBLE) AS n FROM sales";
    let llm = ScriptedLlm::new(vec![Ok(reply(still_bad))]);
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
    assert_eq!(outcome.sql_generated.as_deref(), Some(still_bad));
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

/// Guard từ chối vì câu có thể dựng giá trị quá lớn (số tầng, tham số kích thước): cũng vào vòng sửa.
#[tokio::test]
async fn oversized_sql_is_fixed_once() {
    let five_levels = "SELECT v FROM (SELECT v FROM (SELECT v FROM (SELECT v FROM (SELECT region AS v FROM sales) a) b) c) d";
    let many_pieces = format!("SELECT {} AS v FROM sales", vec!["region"; 21].join(" || "));
    for sql in [
        five_levels,
        many_pieces.as_str(),
        "SELECT regexp_replace(region, 'x*', ' ', 'g') AS r FROM sales",
    ] {
        let llm = ScriptedLlm::new(vec![Ok(reply(GOOD_SQL))]);
        let outcome = run_scripted(&llm, sql).await;
        assert_eq!(outcome.status, AskStatus::Answered, "{sql}: {outcome:?}");
        assert_eq!(
            steps(&outcome),
            [
                (StepName::Guard, false),
                (StepName::Repair, true),
                (StepName::Guard, true),
                (StepName::Run, true),
            ]
        );
    }
}

/// Guard từ chối vì cột không có trong hồ sơ: lý do của guard gửi cho mô hình sửa.
#[tokio::test]
async fn unknown_column_is_fixed_once() {
    let llm = ScriptedLlm::new(vec![Ok(reply(GOOD_SQL))]);
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
    let llm = ScriptedLlm::new(vec![Ok(reply("SELECT getenv('HOME')"))]);
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
    assert_eq!(
        outcome.sql_generated.as_deref(),
        Some("SELECT getenv('HOME')")
    );
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
    assert_eq!(outcome.sql_generated.as_deref(), Some(BAD_SQL));
    assert!(outcome.sql_guarded.unwrap().starts_with(BAD_SQL));
    let message = outcome.message.unwrap();
    assert!(message.contains("Gemini đang quá tải"), "{message}");
}

// Câu trả lời JSON của mô hình

/// Câu trả lời không phải JSON vào vòng sửa như lỗi guard và DuckDB; prompt sửa nói vì sao không đọc được.
#[tokio::test]
async fn unreadable_reply_is_fixed_once() {
    let llm = ScriptedLlm::new(vec![Ok(reply(GOOD_SQL))]);
    let data = Dataset::sales();
    let outcome = try_run_reply(&llm, &data, GOOD_SQL, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Repair, true),
            (StepName::Guard, true),
            (StepName::Run, true),
        ]
    );
    assert_eq!(outcome.sql_generated.as_deref(), Some(GOOD_SQL));
    let prompts = llm.prompts();
    assert!(prompts[0].contains("could not be read"), "{}", prompts[0]);
    assert!(prompts[0].contains("no JSON object"), "{}", prompts[0]);
    assert_eq!(llm.schemas_sent(), [true]);
}

/// Câu sửa cũng không đọc được: hết vòng sửa, failed, không có câu SQL nào.
#[tokio::test]
async fn unreadable_twice_is_failed() {
    let llm = ScriptedLlm::new(vec![Ok("vẫn không phải JSON".to_string())]);
    let data = Dataset::sales();
    let outcome = try_run_reply(&llm, &data, "{\"sql\": ", Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(outcome.status, AskStatus::Failed);
    assert_eq!(steps(&outcome), [(StepName::Repair, false)]);
    assert_eq!(outcome.sql_generated, None);
    assert!(
        outcome
            .message
            .unwrap()
            .contains("kể cả sau khi sửa một lần"),
        "one repair round only"
    );
}

/// Chưa có câu SQL nào mà mô hình lỗi khi sửa: như mô hình lỗi ở bước llm, request nhận lỗi của mô hình (502).
#[tokio::test]
async fn model_error_after_an_unreadable_reply_is_an_error() {
    let llm = ScriptedLlm::new(vec![Err(AppError::Upstream("Gemini quá tải".to_string()))]);
    let data = Dataset::sales();
    let result = try_run_reply(&llm, &data, "không phải JSON", Duration::from_secs(30)).await;
    assert!(matches!(result, Err(AppError::Upstream(_))), "{result:?}");
}

/// Mô hình hỏi lại ở lần đầu: needs_choice, không chạy guard, không gọi mô hình lần nữa.
#[tokio::test]
async fn clarify_needs_a_choice_without_running() {
    let llm = ScriptedLlm::new(vec![]);
    let data = Dataset::sales();
    let first = serde_json::json!({
        "terms": [], "assumptions": [], "sql": "",
        "clarify": {"kind": "metric", "question": "Bạn muốn xem giảm của chỉ số nào?",
                    "options": ["doanh thu", "lợi nhuận"]}
    })
    .to_string();
    let outcome = try_run_reply(&llm, &data, &first, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(outcome.status, AskStatus::NeedsChoice);
    assert!(outcome.steps.is_empty());
    assert_eq!(
        outcome.message.as_deref(),
        Some("Bạn muốn xem giảm của chỉ số nào?")
    );
    let clarify = outcome.clarify.unwrap();
    assert_eq!(clarify.kind, ChoiceKind::Metric);
    assert_eq!(outcome.sql_generated, None);
}

/// Hỏi lại thay vì sửa: không còn lượt nào để dùng câu trả lời của người dùng, nên failed, giữ câu SQL đầu.
#[tokio::test]
async fn clarify_instead_of_a_fix_is_failed() {
    let question = serde_json::json!({
        "sql": "", "clarify": {"kind": "column", "question": "Cột nào?", "options": ["region"]}
    })
    .to_string();
    let llm = ScriptedLlm::new(vec![Ok(question)]);
    let outcome = run_scripted(&llm, BAD_SQL).await;
    assert_eq!(outcome.status, AskStatus::Failed);
    assert_eq!(outcome.sql_generated.as_deref(), Some(BAD_SQL));
    assert!(outcome.clarify.is_none());
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Guard, true),
            (StepName::Run, false),
            (StepName::Repair, false),
        ]
    );
}

/// Cách hiểu đi theo câu trả lời có câu SQL đã chạy: câu sửa có thể hiểu lại câu hỏi.
#[tokio::test]
async fn terms_and_assumptions_come_from_the_answer_that_ran() {
    let fixed = serde_json::json!({
        "terms": [{"text": "miền Nam", "kind": "value", "column": "region", "value": "South"}],
        "assumptions": [{"kind": "time", "chosen": "all", "alternatives": [], "label": "Toàn bộ dữ liệu"}],
        "sql": GOOD_SQL
    })
    .to_string();
    let first = serde_json::json!({
        "terms": [{"text": "miền Nam", "kind": "column", "column": "region"}],
        "assumptions": [], "sql": BAD_SQL
    })
    .to_string();
    let llm = ScriptedLlm::new(vec![Ok(fixed)]);
    let data = Dataset::sales();
    let outcome = try_run_reply(&llm, &data, &first, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert_eq!(
        outcome.terms,
        [Term {
            text: "miền Nam".to_string(),
            kind: TermKind::Value,
            column: Some("region".to_string()),
            value: Some("South".to_string()),
        }]
    );
    assert_eq!(outcome.assumptions.len(), 1);

    // Không sửa thì cách hiểu là của câu trả lời đầu.
    let llm = ScriptedLlm::new(vec![]);
    let first = serde_json::json!({
        "terms": [{"text": "khu vực", "kind": "column", "column": "region"}],
        "assumptions": [], "sql": GOOD_SQL
    })
    .to_string();
    let outcome = try_run_reply(&llm, &data, &first, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(outcome.terms[0].text, "khu vực");
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
    // Dưới một giây thì lời báo nói mili giây, không nói "0 giây".
    let message = outcome.message.unwrap();
    assert!(
        message.contains("quá 300 mili giây") && message.contains("đã bị dừng"),
        "{message}"
    );
}

/// Bảng 3000 dòng dùng cho các câu chạy lâu hay tốn bộ nhớ.
fn big_sales() -> Dataset {
    Dataset::new(
        "sales",
        &["region", "amount"],
        "SELECT 'South' AS region, range AS amount FROM range(3000)",
    )
}

/// Câu sửa chạy quá giờ: dừng, failed, không có vòng sửa thứ hai.
#[tokio::test]
async fn fixed_sql_over_the_time_limit_is_failed() {
    let slow = "SELECT COUNT(*) AS n FROM sales a, sales b, sales c";
    let llm = ScriptedLlm::new(vec![Ok(reply(slow))]);
    let outcome = run(&llm, &big_sales(), BAD_SQL, Duration::from_millis(300)).await;
    assert_eq!(outcome.status, AskStatus::Failed, "{outcome:?}");
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
    assert_eq!(outcome.sql_generated.as_deref(), Some(slow));
    assert!(outcome.message.unwrap().contains("đã bị dừng"));
}

/// Câu chạm trần bộ nhớ lúc chạy: failed, không nhờ mô hình sửa. Ghép một chuỗi 1000 byte qua 9 triệu dòng (tích chéo
/// bảng 3000 dòng với chính nó) cần khoảng 9 GB, quá trần 1 GB; ghi tạm ra đĩa đã tắt nên DuckDB báo hết bộ nhớ.
#[tokio::test]
async fn query_over_the_memory_limit_is_failed() {
    let llm = ScriptedLlm::new(vec![]);
    let sql = format!(
        "SELECT length(string_agg(v, '')) AS n FROM (SELECT '{}' AS v FROM sales a, sales b) t",
        "y".repeat(1000)
    );
    let outcome = run(&llm, &big_sales(), &sql, Duration::from_secs(60)).await;
    assert_eq!(outcome.status, AskStatus::Failed, "{outcome:?}");
    assert_eq!(
        steps(&outcome),
        [(StepName::Guard, true), (StepName::Run, false)]
    );
    let message = outcome.message.unwrap();
    assert!(message.contains("bộ nhớ"), "{message}");
}

/// Câu sửa bị guard từ chối vì quá lớn: rejected, không có vòng sửa thứ hai dù Oversized bình thường vào vòng sửa.
#[tokio::test]
async fn fixed_sql_refused_as_oversized_is_rejected() {
    let five_levels =
        "SELECT v FROM (SELECT v FROM (SELECT v FROM (SELECT v FROM (SELECT region AS v FROM sales) a) b) c) d";
    let llm = ScriptedLlm::new(vec![Ok(reply(five_levels))]);
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
    assert!(outcome.message.unwrap().contains("tối đa 4"));
}

/// Hết lượt chạy (AppState::query_jobs có hai lượt) thì câu thứ ba chờ, không chạy, không bị từ chối. Test giữ cả hai
/// lượt, thấy câu chưa xong sau 300 ms, trả một lượt rồi thấy câu chạy xong. Không phụ thuộc tốc độ máy: lúc giữ lượt
/// câu đang chờ semaphore chứ không chạy, nên máy chậm chỉ làm nó chờ lâu hơn. Không so thời gian của bước run: khi các
/// test khác chạy song song, phần kiểm guard có thể tự mất quá 300 ms, và lúc đó thời gian chờ rơi vào bước guard.
#[tokio::test]
async fn third_query_waits_for_a_turn() {
    let data = Dataset::sales();
    let llm = ScriptedLlm::new(vec![]);
    let jobs = Arc::new(Semaphore::new(2));
    let first = Arc::clone(&jobs).acquire_owned().await.unwrap();
    let _second = Arc::clone(&jobs).acquire_owned().await.unwrap();
    let first_reply = reply(GOOD_SQL);
    let input = RunInput {
        question: "Doanh thu theo khu vực",
        prompt: PROMPT,
        reply: &first_reply,
        tables: &data.tables,
        max_rows: MAX_ROWS,
        timeout: Duration::from_secs(30),
        jobs: &jobs,
    };
    let mut third = std::pin::pin!(run_with_repair(&llm, input));
    let waited = tokio::time::timeout(Duration::from_millis(300), &mut third).await;
    assert!(
        waited.is_err(),
        "third query ran while both turns were held"
    );
    drop(first);
    let outcome = third.await.unwrap();
    assert_eq!(outcome.status, AskStatus::Answered);
}

/// Không lời báo nào, cho người dùng hay cho mô hình, chứa đường dẫn trên máy chủ: lỗi đọc ngày của DuckDB chạy trên
/// Parquet thật, rồi lời báo có cả đường dẫn file và phần LINE chép lại bản runnable.
#[tokio::test]
async fn error_texts_have_no_server_paths() {
    let data = Dataset::sales();
    let llm = ScriptedLlm::new(vec![Ok(reply(
        "SELECT strptime(ship_date, '%d/%m/%Y %H:%M') AS shipped FROM sales",
    ))]);
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
        json_schema: None,
    };
    let first_reply = mock.complete(&request).await.unwrap();
    let jobs = Arc::new(Semaphore::new(2));
    let input = RunInput {
        question,
        prompt: "",
        reply: &first_reply,
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

// Câu mẫu qua guard

/// Bảng dataco với các cột trong hồ sơ viết sẵn của mock.
fn dataco_table(answers: &[serde_json::Value]) -> Vec<AllowedTable> {
    let profile = answers
        .iter()
        .find(|e| e["task"] == "profile" && e["question"] == "dataco")
        .unwrap();
    let columns = profile["answer"]["columns"].as_array().unwrap();
    vec![AllowedTable {
        name: "dataco".to_string(),
        columns: columns
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect(),
        parquet_dir: PathBuf::from("/data/dataco"),
        row_filters: vec![],
    }]
}

/// Luật của guard (số tầng câu, kích thước hàm) không chặn các câu mẫu: câu SQL và câu sửa của mock, các câu SQL trong
/// response mẫu (ask_examples.rs), trừ câu DELETE cố ý của mock và của mẫu rejected.
#[test]
fn sample_sql_passes_the_guard() {
    use super::ask_examples::{build_failed_example, build_simple_example, build_why_example};
    use crate::engine::guard::build_guarded_sql;

    let answers: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../engine/llm/mock_answers.json")).unwrap();
    let tables = dataco_table(&answers);
    let mut samples: Vec<String> = answers
        .iter()
        .filter(|e| e["task"] == "sql" || e["task"] == "repair")
        // Câu này có để thử nhánh rejected trên giao diện: guard phải từ chối nó.
        .filter(|e| e["question"] != "Xóa các đơn hàng bị hủy")
        .map(|e| e["answer"]["sql"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(samples.len(), 15);
    for example in [
        build_simple_example(),
        build_why_example(),
        build_failed_example(),
    ] {
        samples.extend(example.sql_generated.clone());
        if let Some(explanation) = &example.explanation {
            samples.extend(explanation.dimensions.iter().map(|d| d.sql.clone()));
            samples.extend(explanation.volume_price.iter().map(|v| v.sql.clone()));
        }
    }
    for sql in &samples {
        if let Err(e) = build_guarded_sql(sql, &tables, MAX_ROWS) {
            panic!("guard rejected sample {sql:?}: {e:?}");
        }
    }
}
