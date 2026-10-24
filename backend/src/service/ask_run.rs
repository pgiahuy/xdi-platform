// SPDX-License-Identifier: Apache-2.0
/* service/ask_run.rs - các bước guard, run, repair của luồng hỏi (docs/ARCHITECTURE.md mục 2, bước 5 và 6): kiểm câu SQL
mô hình viết, chạy trên DuckDB (engine/query.rs), đổi kết quả sang ResultTable (ask_result.rs), và nhờ mô hình sửa một
lần khi lỗi sửa được. service/ask.rs (#33) gọi run_with_repair sau bước llm rồi dựng AskResponse từ RunOutcome; nhận
mô hình và đầu vào của guard qua tham số nên test chạy được với mô hình giả, không cần Postgres.

Vào vòng sửa: DuckDB báo lỗi, hoặc guard từ chối vì bảng hay cột không có trong hồ sơ (GuardError::UnknownName) hay vì
câu có thể dựng giá trị quá lớn (GuardError::Oversized: tham số kích thước, số tầng, mảnh ghép, độ rộng). Không sửa:
guard từ chối vì lý do khác (câu ghi, hàm bị cấm), câu chạy quá giờ hay quá bộ nhớ. Lỗi guard và lỗi DuckDB chung một
vòng sửa, tối đa một vòng. Lời báo lỗi gửi mô hình và hiện cho người dùng đã bỏ đường dẫn file Parquet trên máy chủ
(ask_repair.rs, cùng prompt sửa). Test ở ask_run_tests.rs. */

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use super::ask_contract::{AskStatus, AskStep, ResultTable, StepName};
use super::ask_repair::{
    build_repair_prompt, build_safe_error, build_user_error, build_user_text, mentions_data_files,
    read_sql_reply,
};
use super::ask_result::{build_result_table, SENT_ROWS};
use crate::engine::guard::{build_guarded_sql, AllowedTable, GuardError};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::query::{run_query, QueryError, QueryOutput};
use crate::error::AppError;

/// Đầu vào của run_with_repair, trừ mô hình.
pub struct RunInput<'a> {
    /// Câu hỏi gốc; mock tra câu sửa theo câu này.
    pub question: &'a str,
    /// Prompt đã gửi ở bước llm. Prompt sửa ghép thêm câu SQL và lời báo lỗi vào sau, để mô hình có đủ hồ sơ bảng như
    /// lần đầu.
    pub prompt: &'a str,
    /// SQL mô hình viết ở bước llm.
    pub sql: String,
    /// Đầu vào của guard: các bảng được phép và trần số dòng.
    pub tables: &'a [AllowedTable],
    pub max_rows: u64,
    /// Thời gian tối đa mỗi lần chạy (Config::query_timeout).
    pub timeout: Duration,
    /// Lượt chạy DuckDB (AppState::query_jobs).
    pub jobs: &'a Arc<Semaphore>,
}

/// Phần của AskResponse do các bước guard, run, repair quyết định. status chỉ là answered, rejected hoặc failed.
#[derive(Debug)]
pub struct RunOutcome {
    pub status: AskStatus,
    /// Lời cho người dùng khi rejected hoặc failed; None khi answered.
    pub message: Option<String>,
    /// SQL mô hình viết; sau vòng sửa là câu đã sửa.
    pub sql_generated: String,
    /// Bản `shown` của sql_generated; None khi câu đó không qua guard.
    pub sql_guarded: Option<String>,
    pub result: Option<ResultTable>,
    /// Các bước theo đúng thứ tự đã chạy, ví dụ guard, run (lỗi), repair, guard, run.
    pub steps: Vec<AskStep>,
}

/// Kết quả một lượt guard rồi run của một câu.
enum Attempt {
    Answered {
        shown: String,
        result: ResultTable,
    },
    /// Guard từ chối. fixable khi lý do là sai tên bảng hay cột.
    Refused {
        reason: String,
        fixable: bool,
    },
    /// DuckDB báo lỗi; error đã bỏ đường dẫn, gửi được cho mô hình.
    RunFailed {
        shown: String,
        error: String,
    },
    /// Quá giờ hay quá bộ nhớ: dừng câu hỏi, không sửa.
    Stopped {
        shown: String,
        message: String,
    },
}

/// Err chỉ cho lỗi của máy chủ (không mở được DuckDB, file Parquet mất). Mô hình lỗi khi sửa không phải Err: câu hỏi
/// đã có SQL, nên trả failed kèm các bước thay vì 502.
pub async fn run_with_repair(
    llm: &dyn LlmProvider,
    input: RunInput<'_>,
) -> Result<RunOutcome, AppError> {
    let mut steps = Vec::new();
    // first_shown: bản shown của câu đầu khi nó đã qua guard và chạy lỗi; None khi guard từ chối.
    let (error, first_shown) = match attempt(&input, &input.sql, &mut steps).await? {
        Attempt::Refused {
            reason,
            fixable: true,
        } => (reason, None),
        Attempt::RunFailed { shown, error } => (error, Some(shown)),
        other => return Ok(build_outcome(other, input.sql, steps)),
    };
    let started = Instant::now();
    let prompt = build_repair_prompt(input.prompt, &input.sql, &error);
    let request = LlmRequest {
        task: LlmTask::Repair,
        question: input.question,
        prompt: &prompt,
    };
    let reply = match llm.complete(&request).await {
        Ok(reply) => reply,
        Err(e) => {
            let model_error = build_user_text(&e);
            steps.push(build_step(
                StepName::Repair,
                started,
                false,
                Some(&model_error),
            ));
            // Câu hỏi dừng ở bước repair nên là failed; SQL giữ là câu đầu.
            let message = format!(
                "Câu SQL lỗi ({}) và mô hình không sửa được: {model_error}.",
                build_user_error(&error)
            );
            return Ok(RunOutcome {
                status: AskStatus::Failed,
                message: Some(message),
                sql_generated: input.sql,
                sql_guarded: first_shown,
                result: None,
                steps,
            });
        }
    };
    let detail = match first_shown {
        Some(_) => "Mô hình sửa SQL theo lỗi của lần chạy đầu",
        None => "Mô hình sửa SQL theo lý do guard từ chối",
    };
    steps.push(build_step(StepName::Repair, started, true, Some(detail)));
    let fixed = read_sql_reply(&reply);
    let second = attempt(&input, &fixed, &mut steps).await?;
    Ok(build_outcome(second, fixed, steps))
}

/// Guard rồi run một câu, ghi hai bước vào steps (chỉ bước guard nếu guard từ chối).
async fn attempt(
    input: &RunInput<'_>,
    sql: &str,
    steps: &mut Vec<AskStep>,
) -> Result<Attempt, AppError> {
    let started = Instant::now();
    let (reason, fixable) = match build_guarded_sql(sql, input.tables, input.max_rows) {
        Ok(guarded) => {
            let note = guarded.limit_note.as_deref();
            steps.push(build_step(StepName::Guard, started, true, note));
            return run_guarded(input, guarded.shown, guarded.runnable, steps).await;
        }
        Err(GuardError::Internal(e)) => return Err(e),
        Err(GuardError::UnknownName(reason) | GuardError::Oversized(reason)) => (reason, true),
        Err(GuardError::Rejected(reason)) => (reason, false),
    };
    steps.push(build_step(StepName::Guard, started, false, Some(&reason)));
    Ok(Attempt::Refused { reason, fixable })
}

async fn run_guarded(
    input: &RunInput<'_>,
    shown: String,
    runnable: String,
    steps: &mut Vec<AskStep>,
) -> Result<Attempt, AppError> {
    // Thời gian của bước run gồm cả lúc chờ lượt: người hỏi chờ đúng chừng đó.
    let started = Instant::now();
    let (outcome, detail) = match run_in_turn(input, runnable).await {
        Ok(output) => {
            let result = build_result_table(output, input.max_rows);
            (Attempt::Answered { shown, result }, None)
        }
        // File Parquet hỏng: lỗi máy chủ như IO Error, không gửi mô hình (xem mentions_data_files).
        Err(QueryError::Sql(raw)) if mentions_data_files(&raw, input.tables) => {
            return Err(AppError::Internal(format!("read dataset files: {raw}")));
        }
        Err(QueryError::Sql(raw)) => {
            let error = build_safe_error(&raw, input.tables);
            let detail = build_user_error(&error);
            (Attempt::RunFailed { shown, error }, Some(detail))
        }
        Err(QueryError::Timeout) => {
            let message = format!(
                "Câu SQL chạy quá {} giây nên đã bị dừng. Thử hỏi hẹp hơn, ví dụ thêm khoảng thời gian hoặc bớt chiều \
                 nhóm",
                input.timeout.as_secs()
            );
            (stopped(shown, &message), Some(message))
        }
        Err(QueryError::OutOfMemory) => {
            let message = "Câu SQL cần nhiều bộ nhớ hơn mức cho phép nên đã bị dừng. Thử hỏi hẹp hơn, ví dụ thêm \
                           khoảng thời gian hoặc bớt chiều nhóm"
                .to_string();
            (stopped(shown, &message), Some(message))
        }
        Err(QueryError::Internal(e)) => return Err(e),
    };
    let ok = matches!(outcome, Attempt::Answered { .. });
    steps.push(build_step(StepName::Run, started, ok, detail.as_deref()));
    Ok(outcome)
}

fn stopped(shown: String, message: &str) -> Attempt {
    Attempt::Stopped {
        shown,
        message: message.to_string(),
    }
}

/// Chờ lượt (AppState::query_jobs) rồi chạy trong spawn_blocking vì DuckDB chặn luồng. Lượt chuyển vào closure cùng
/// câu: request bị hủy giữa chừng thì DuckDB vẫn chạy tới khi xong hoặc hết giờ, và lượt chỉ được trả lúc đó, nên số
/// câu chạy cùng lúc không vượt số lượt.
async fn run_in_turn(input: &RunInput<'_>, runnable: String) -> Result<QueryOutput, QueryError> {
    let turn = Arc::clone(input.jobs)
        .acquire_owned()
        .await
        .map_err(|e| internal(format!("query semaphore closed: {e}")))?;
    let timeout = input.timeout;
    tokio::task::spawn_blocking(move || {
        let output = run_query(&runnable, SENT_ROWS, timeout);
        drop(turn);
        output
    })
    .await
    .map_err(|e| internal(format!("query task failed: {e}")))?
}

fn internal(message: String) -> QueryError {
    QueryError::Internal(AppError::Internal(message))
}

/// RunOutcome từ lượt cuối cùng: lượt đầu khi không sửa, lượt sau khi đã sửa. Lượt đầu không bao giờ là RunFailed hay
/// Refused sửa được (đã vào vòng sửa), nên RunFailed ở đây là câu đã sửa vẫn lỗi.
fn build_outcome(attempt: Attempt, sql: String, steps: Vec<AskStep>) -> RunOutcome {
    let (status, message, sql_guarded, result) = match attempt {
        Attempt::Answered { shown, result } => (AskStatus::Answered, None, Some(shown), Some(result)),
        Attempt::Refused { reason, .. } => (
            AskStatus::Rejected,
            Some(format!("Câu SQL bị từ chối: {reason}.")),
            None,
            None,
        ),
        Attempt::RunFailed { shown, error } => (
            AskStatus::Failed,
            Some(format!(
                "Không chạy được câu SQL, kể cả sau khi mô hình sửa một lần: {}. Thử hỏi lại theo cách khác.",
                build_user_error(&error)
            )),
            Some(shown),
            None,
        ),
        Attempt::Stopped { shown, message } => {
            (AskStatus::Failed, Some(format!("{message}.")), Some(shown), None)
        }
    };
    RunOutcome {
        status,
        message,
        sql_generated: sql,
        sql_guarded,
        result,
        steps,
    }
}

fn build_step(name: StepName, started: Instant, ok: bool, detail: Option<&str>) -> AskStep {
    AskStep {
        name,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        ok,
        detail: detail.map(str::to_string),
    }
}
