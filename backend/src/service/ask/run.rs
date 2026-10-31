// SPDX-License-Identifier: Apache-2.0
/* service/ask/run.rs - các bước guard, run, repair của luồng hỏi (docs/ARCHITECTURE.md mục 2, bước 5 và 6): kiểm câu SQL
mô hình viết, chạy trên DuckDB (engine/query.rs), đổi kết quả sang ResultTable (ask/result.rs), và nhờ mô hình sửa một
lần khi lỗi sửa được. service/ask/mod.rs (#33) gọi run_with_repair với câu trả lời thô của bước llm rồi dựng AskResponse
từ RunOutcome; nhận mô hình và đầu vào của guard qua tham số nên test chạy được với mô hình giả, không cần Postgres.

Câu trả lời của mô hình là object JSON {terms, assumptions, clarify, sql} (ask/reply.rs). Mô hình hỏi lại ở lần đầu thì
trả needs_choice, không chạy gì; báo dữ liệu không có thông tin (unanswerable, #164) thì trả rejected, không chạy gì,
ask/mod.rs dựng lời. Câu trả lời không đọc được (không phải JSON, thiếu sql) cũng vào vòng sửa.

Vào vòng sửa: DuckDB báo lỗi, guard từ chối vì bất kỳ lý do nào (trừ lỗi máy chủ GuardError::Internal), câu cộng lặp
khi nối bảng một-nhiều (ask/fanout.rs, ghi như một bước guard không qua; câu sửa vẫn cộng lặp thì người hỏi nhận lời
nói rõ lý do), hoặc câu trả lời không đọc được. Guard từ chối là lỗi của hệ thống chứ không phải của người hỏi: câu
sửa lại qua guard từ đầu nên gửi mọi lý do cho mô hình không mở thêm đường nào tới dữ liệu. Không sửa: câu chạy quá
giờ hay quá bộ nhớ. Lỗi guard, lỗi DuckDB và câu trả lời không đọc được chung một vòng sửa, cả câu hỏi tối đa một
vòng. Câu sửa vẫn bị từ chối thì người hỏi nhận lời của hệ thống (REFUSED_MESSAGE); lý do của guard nằm trong chi
tiết bước guard. Lời báo lỗi gửi mô hình và hiện cho người dùng đã bỏ đường dẫn file Parquet trên máy chủ
(ask/repair.rs, cùng prompt sửa). Test ở ask/run_tests.rs. */

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use crate::engine::guard::{build_guarded_sql, AllowedTable, GuardError, GuardedSql};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::query::{run_query, QueryError, QueryOutput};
use crate::error::AppError;
use crate::service::ask::contract::{AskStatus, AskStep, ResultTable, StepName};
use crate::service::ask::failures::{FailedStep, StepFailure};
use crate::service::ask::fanout::{
    build_hint, build_refused_message, build_step_detail, find_double_count, JoinLink,
};
use crate::service::ask::repair::{
    build_model_error, build_repair_prompt, build_safe_error, build_user_error, build_user_text,
    mentions_data_files,
};
use crate::service::ask::reply::{
    build_answer_from_reply, build_sql_reply_schema, Assumption, Clarify, ModelAnswer, SqlAnswer,
    Term, Unanswerable,
};
use crate::service::ask::result::{build_result_table, SENT_ROWS};

/// Lời cho người hỏi khi câu SQL của vòng sửa vẫn bị guard từ chối. Lỗi nằm ở câu hệ thống viết, không ở câu hỏi, nên
/// lời không đưa lý do kỹ thuật mà gợi ý cách hỏi lại.
pub const REFUSED_MESSAGE: &str =
    "Hệ thống chưa viết được câu truy vấn hợp lệ cho câu hỏi này, kể cả sau khi tự sửa một \
     lần. Thử hỏi lại theo cách khác, ví dụ tách thành các câu hỏi nhỏ hơn.";

/// Đầu vào của run_with_repair, trừ mô hình.
pub struct RunInput<'a> {
    /// Câu hỏi gốc; mock tra câu sửa theo câu này.
    pub question: &'a str,
    /// Prompt đã gửi ở bước llm. Prompt sửa ghép thêm câu SQL và lời báo lỗi vào sau, để mô hình có đủ hồ sơ bảng như
    /// lần đầu.
    pub prompt: &'a str,
    /// Câu trả lời thô của mô hình ở bước llm, chưa đọc.
    pub reply: &'a str,
    /// Đầu vào của guard: các bảng được phép và trần số dòng.
    pub tables: &'a [AllowedTable],
    /// Quan hệ giữa các bảng: guard chỉ nhận phép nối đúng khóa ngoại (#256), và câu SQL cộng lặp khi nối bảng
    /// một-nhiều (ask/fanout.rs) thì không chạy mà đi vòng sửa. Rỗng khi câu hỏi chỉ có một bảng.
    pub links: &'a [JoinLink],
    pub max_rows: u64,
    /// Thời gian tối đa mỗi lần chạy (Config::query_timeout).
    pub timeout: Duration,
    /// Lượt chạy DuckDB (AppState::query_jobs).
    pub jobs: &'a Arc<Semaphore>,
    /// Tên các cột đánh dấu nhạy cảm của các bảng được phép: giá trị của chúng bị ẩn khỏi lời báo lỗi gửi mô hình
    /// (ask/repair.rs, build_model_error).
    pub sensitive: &'a [String],
}

/// Phần của AskResponse do các bước guard, run, repair quyết định. status là answered, needs_choice, rejected hoặc
/// failed.
#[derive(Debug)]
pub struct RunOutcome {
    pub status: AskStatus,
    /// Lời cho người dùng khi rejected hoặc failed, câu hỏi lại khi needs_choice; None khi answered.
    pub message: Option<String>,
    /// SQL mô hình viết; sau vòng sửa là câu đã sửa. None khi chưa đọc được câu SQL nào: mô hình hỏi lại, hoặc câu trả
    /// lời không đọc được cả hai lần.
    pub sql_generated: Option<String>,
    /// Bản `shown` của sql_generated; None khi câu đó không qua guard.
    pub sql_guarded: Option<String>,
    pub result: Option<ResultTable>,
    /// Guard giữ nguyên LIMIT của câu mô hình viết (GuardedSql::limit_note là None), nên LIMIT trong sql_guarded là ý
    /// của câu hỏi chứ không phải trần guard thêm; thẻ "Đã hiểu là" (understanding/mod.rs) chỉ hiện mục limit khi đó. Chỉ có
    /// nghĩa khi answered, còn lại là false.
    pub limit_kept: bool,
    /// Điều kiện phân quyền guard đã thêm vào sql_guarded (GuardedSql::row_filters), để thẻ gắn nhãn phân quyền; rỗng
    /// khi không lọc hoặc khi chưa answered.
    pub row_filters: Vec<String>,
    /// Bảng câu answered đã đọc (GuardedSql::tables_read); rỗng khi chưa answered.
    pub tables_read: Vec<String>,
    /// Cách hiểu của mô hình trong câu trả lời có sql_generated (câu sửa nếu đã sửa, vì câu sửa có thể hiểu lại câu
    /// hỏi). #33 kiểm terms với hồ sơ và dựng giả định từ assumptions.
    pub terms: Vec<Term>,
    pub assumptions: Vec<Assumption>,
    /// Câu hỏi lại; chỉ có khi needs_choice.
    pub clarify: Option<Clarify>,
    /// Mô hình báo dữ liệu không có thông tin; status là rejected, message để ask/mod.rs dựng (cần tên cột).
    pub unanswerable: Option<Unanswerable>,
    /// Các bước theo đúng thứ tự đã chạy, ví dụ guard, run (lỗi), repair, guard, run.
    pub steps: Vec<AskStep>,
    /// Lần guard từ chối, câu cộng lặp bị chặn, lần DuckDB báo lỗi, kèm câu SQL của mô hình, theo thứ tự xảy ra; ask/mod.rs
    /// ghi vào nhật ký lỗi (#228).
    pub failures: Vec<StepFailure>,
}

/// Kết quả một lượt guard rồi run của một câu.
enum Attempt {
    Answered {
        shown: String,
        result: ResultTable,
        limit_kept: bool,
        row_filters: Vec<String>,
        tables_read: Vec<String>,
    },
    /// Guard từ chối; reason là lý do của guard, gửi cho mô hình ở vòng sửa.
    Refused { reason: String },
    /// Guard nhận câu nhưng câu cộng lặp khi nối bảng một-nhiều (ask/fanout.rs). reason gửi mô hình như lý do của
    /// guard; message cho người hỏi khi câu sửa vẫn cộng lặp.
    DoubleCounted { reason: String, message: String },
    /// DuckDB báo lỗi; error đã bỏ đường dẫn, gửi được cho mô hình.
    RunFailed { shown: String, error: String },
    /// Quá giờ hay quá bộ nhớ: dừng câu hỏi, không sửa.
    Stopped { shown: String, message: String },
}

/// Err cho lỗi của máy chủ (không mở được DuckDB, file Parquet mất), và cho lỗi của mô hình khi chưa có câu SQL nào
/// (câu trả lời lần đầu không đọc được, rồi mô hình lỗi khi sửa): như mô hình lỗi ở bước llm, request nhận 502 hay 503
/// (xem AskStatus). Mô hình lỗi khi sửa một câu SQL đã có thì không phải Err: trả failed kèm các bước.
pub async fn run_with_repair(
    llm: &dyn LlmProvider,
    input: RunInput<'_>,
) -> Result<RunOutcome, AppError> {
    // Các lượt guard, run ghi lỗi vào failures qua tham chiếu &mut; mọi nhánh return của run_rounds dựng RunOutcome với
    // failures rỗng, rồi ở đây mới gắn danh sách đã gom, để không nhánh nào quên.
    let mut failures = Vec::new();
    let mut outcome = run_rounds(llm, input, &mut failures).await?;
    outcome.failures = failures;
    Ok(outcome)
}

async fn run_rounds(
    llm: &dyn LlmProvider,
    input: RunInput<'_>,
    failures: &mut Vec<StepFailure>,
) -> Result<RunOutcome, AppError> {
    let mut steps = Vec::new();
    // first: câu trả lời đầu khi đọc được. first_shown: bản shown của câu SQL đầu khi nó đã qua guard và chạy lỗi;
    // None khi guard từ chối hay câu trả lời không đọc được.
    let (first, error, first_shown) = match build_answer_from_reply(input.reply) {
        Ok(ModelAnswer::Clarify { terms, clarify }) => {
            return Ok(build_clarify_outcome(terms, clarify));
        }
        Ok(ModelAnswer::Unanswerable {
            terms,
            unanswerable,
        }) => return Ok(build_unanswerable_outcome(terms, unanswerable, steps)),
        Ok(ModelAnswer::Sql(answer)) => {
            match attempt(&input, &answer.sql, &mut steps, failures).await? {
                Attempt::Refused { reason } | Attempt::DoubleCounted { reason, .. } => {
                    (Some(answer), reason, None)
                }
                Attempt::RunFailed { shown, error } => {
                    let error = build_model_error(&error, &answer.sql, input.sensitive);
                    (Some(answer), error, Some(shown))
                }
                other => return Ok(build_outcome(other, answer, steps)),
            }
        }
        Err(unreadable) => {
            tracing::warn!(reason = %unreadable.reason, "model reply is unreadable, asking the model to fix it");
            (None, unreadable.reason, None)
        }
    };
    let started = Instant::now();
    let first_sql = first.as_ref().map(|answer| answer.sql.as_str());
    let prompt = build_repair_prompt(input.prompt, first_sql, &error);
    let request = LlmRequest {
        task: LlmTask::Repair,
        question: input.question,
        prompt: &prompt,
        json_schema: Some(build_sql_reply_schema()),
    };
    let reply = match llm.complete(&request).await {
        Ok(reply) => reply,
        Err(e) => {
            let Some(first) = first else {
                return Err(e);
            };
            let model_error = build_user_text(&e, "repair");
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
            return Ok(build_failed(message, Some(first), first_shown, steps));
        }
    };
    let fixed = match build_answer_from_reply(&reply) {
        Ok(ModelAnswer::Sql(answer)) => answer,
        // Lần sửa mới thấy dữ liệu không có thông tin: vẫn là câu trả lời dùng được, không phải lỗi.
        Ok(ModelAnswer::Unanswerable {
            terms,
            unanswerable,
        }) => {
            let detail = "Mô hình báo dữ liệu không có thông tin để trả lời";
            steps.push(build_step(StepName::Repair, started, true, Some(detail)));
            return Ok(build_unanswerable_outcome(terms, unanswerable, steps));
        }
        // Câu sửa không đọc được, hay hỏi lại thay vì sửa: đã hết vòng sửa nên câu hỏi dừng ở đây.
        other => {
            let reason = match other {
                Err(unreadable) => unreadable.reason,
                _ => "the model asked a question instead of fixing the SQL".to_string(),
            };
            tracing::warn!(%reason, "repaired reply has no readable SQL");
            let detail = "Câu trả lời sửa của mô hình không có câu SQL đọc được";
            steps.push(build_step(StepName::Repair, started, false, Some(detail)));
            let message = "Mô hình không trả câu SQL đọc được, kể cả sau khi sửa một lần. Thử hỏi lại theo cách khác."
                .to_string();
            return Ok(build_failed(message, first, first_shown, steps));
        }
    };
    let detail = match (&first, &first_shown) {
        (None, _) => "Mô hình viết lại câu trả lời không đọc được của lần đầu",
        (Some(_), Some(_)) => "Mô hình sửa SQL theo lỗi của lần chạy đầu",
        (Some(_), None) => "Mô hình sửa SQL theo lý do guard từ chối",
    };
    steps.push(build_step(StepName::Repair, started, true, Some(detail)));
    let second = attempt(&input, &fixed.sql, &mut steps, failures).await?;
    Ok(build_outcome(second, fixed, steps))
}

/// Guard rồi run một câu, ghi hai bước vào steps (chỉ bước guard nếu guard từ chối).
async fn attempt(
    input: &RunInput<'_>,
    sql: &str,
    steps: &mut Vec<AskStep>,
    failures: &mut Vec<StepFailure>,
) -> Result<Attempt, AppError> {
    let started = Instant::now();
    let reason = match build_guarded_sql(sql, input.tables, input.links, input.max_rows) {
        Ok(guarded) => {
            // Kiểm cộng lặp trên câu mô hình viết, sau guard (câu đã hợp lệ) và trước khi chạy.
            if let Some(found) = find_double_count(sql, input.tables, input.links) {
                let detail = build_step_detail(&found);
                steps.push(build_step(StepName::Guard, started, false, Some(&detail)));
                failures.push(StepFailure::new(FailedStep::Fanout, &detail, Some(sql)));
                return Ok(Attempt::DoubleCounted {
                    reason: build_hint(&found),
                    message: build_refused_message(&found),
                });
            }
            let note = guarded.limit_note.as_deref();
            steps.push(build_step(StepName::Guard, started, true, note));
            let attempt = run_guarded(input, guarded, steps).await?;
            if let Attempt::RunFailed { error, .. } = &attempt {
                failures.push(StepFailure::new(FailedStep::Run, error, Some(sql)));
            }
            return Ok(attempt);
        }
        Err(GuardError::Internal(e)) => return Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => reason,
    };
    steps.push(build_step(StepName::Guard, started, false, Some(&reason)));
    failures.push(StepFailure::new(FailedStep::Guard, &reason, Some(sql)));
    Ok(Attempt::Refused { reason })
}

async fn run_guarded(
    input: &RunInput<'_>,
    guarded: GuardedSql,
    steps: &mut Vec<AskStep>,
) -> Result<Attempt, AppError> {
    let GuardedSql {
        shown,
        runnable,
        limit_note,
        row_filters,
        tables_read,
    } = guarded;
    // Thời gian của bước run gồm cả lúc chờ lượt: người hỏi chờ đúng chừng đó.
    let started = Instant::now();
    let (outcome, detail) = match run_in_turn(input, runnable).await {
        Ok(output) => {
            let answered = Attempt::Answered {
                shown,
                result: build_result_table(output, input.max_rows),
                limit_kept: limit_note.is_none(),
                row_filters,
                tables_read,
            };
            (answered, None)
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
                "Câu SQL chạy quá {} nên đã bị dừng. Thử hỏi hẹp hơn, ví dụ thêm khoảng thời gian hoặc bớt chiều nhóm",
                build_duration_text(input.timeout)
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

/// "30 giây"; dưới một giây thì mili giây ("300 mili giây"), không in "0 giây".
pub(crate) fn build_duration_text(duration: Duration) -> String {
    if duration < Duration::from_secs(1) {
        format!("{} mili giây", duration.as_millis())
    } else {
        format!("{} giây", duration.as_secs())
    }
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
    run_query_in_turn(input.jobs, runnable, input.timeout).await
}

/// Phần của run_in_turn không cần RunInput, để câu SQL do code dựng (câu "tại sao", explain/breakdown.rs) chờ cùng
/// một hàng lượt với câu của mô hình.
pub(crate) async fn run_query_in_turn(
    jobs: &Arc<Semaphore>,
    runnable: String,
    timeout: Duration,
) -> Result<QueryOutput, QueryError> {
    let turn = Arc::clone(jobs)
        .acquire_owned()
        .await
        .map_err(|e| internal(format!("query semaphore closed: {e}")))?;
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

/// RunOutcome từ lượt cuối cùng: lượt đầu khi không sửa, lượt sau khi đã sửa. Lượt đầu không bao giờ là RunFailed,
/// Refused hay DoubleCounted (cả ba đã vào vòng sửa), nên ba nhánh đó ở đây là câu đã sửa vẫn lỗi. answer là câu trả
/// lời của lượt đó.
fn build_outcome(attempt: Attempt, answer: SqlAnswer, steps: Vec<AskStep>) -> RunOutcome {
    // Nhánh answered dựng RunOutcome ngay vì chỉ nó có kết quả và ghi chú của guard; các nhánh còn lại chỉ khác nhau
    // ở status, lời báo và câu đã qua guard.
    let (status, message, sql_guarded) = match attempt {
        Attempt::Answered {
            shown,
            result,
            limit_kept,
            row_filters,
            tables_read,
        } => {
            return RunOutcome {
                status: AskStatus::Answered,
                message: None,
                sql_generated: Some(answer.sql),
                sql_guarded: Some(shown),
                result: Some(result),
                limit_kept,
                row_filters,
                tables_read,
                terms: answer.terms,
                assumptions: answer.assumptions,
                clarify: None,
                unanswerable: None,
                steps,
                failures: Vec::new(),
            };
        }
        // Đây luôn là câu của vòng sửa: guard từ chối câu đầu thì câu hỏi vào vòng sửa. Lý do của guard viết cho mô
        // hình và đã nằm trong chi tiết bước guard; người hỏi không sửa được câu SQL nên nhận lời của hệ thống.
        Attempt::Refused { .. } => (AskStatus::Rejected, Some(REFUSED_MESSAGE.to_string()), None),
        Attempt::DoubleCounted { message, .. } => (AskStatus::Rejected, Some(message), None),
        Attempt::RunFailed { shown, error } => (
            AskStatus::Failed,
            Some(format!(
                "Không chạy được câu SQL, kể cả sau khi mô hình sửa một lần: {}. Thử hỏi lại theo cách khác.",
                build_user_error(&error)
            )),
            Some(shown),
        ),
        Attempt::Stopped { shown, message } => {
            (AskStatus::Failed, Some(format!("{message}.")), Some(shown))
        }
    };
    RunOutcome {
        status,
        message,
        sql_generated: Some(answer.sql),
        sql_guarded,
        result: None,
        limit_kept: false,
        row_filters: Vec::new(),
        tables_read: Vec::new(),
        terms: answer.terms,
        assumptions: answer.assumptions,
        clarify: None,
        unanswerable: None,
        steps,
        failures: Vec::new(),
    }
}

/// Câu hỏi dừng ở bước repair (mô hình lỗi, hay câu sửa không đọc được). answer là câu trả lời đầu nếu đọc được, để
/// người dùng vẫn thấy câu SQL đã lỗi và cách mô hình hiểu câu hỏi.
fn build_failed(
    message: String,
    answer: Option<SqlAnswer>,
    sql_guarded: Option<String>,
    steps: Vec<AskStep>,
) -> RunOutcome {
    let (sql_generated, terms, assumptions) = match answer {
        Some(answer) => (Some(answer.sql), answer.terms, answer.assumptions),
        None => (None, Vec::new(), Vec::new()),
    };
    RunOutcome {
        status: AskStatus::Failed,
        message: Some(message),
        sql_generated,
        sql_guarded,
        result: None,
        limit_kept: false,
        row_filters: Vec::new(),
        tables_read: Vec::new(),
        terms,
        assumptions,
        clarify: None,
        unanswerable: None,
        steps,
        failures: Vec::new(),
    }
}

/// Mô hình hỏi lại ở lần đầu: chưa có SQL, chưa chạy bước nào. message là câu hỏi lại, như AskResponse.message của
/// needs_choice.
fn build_clarify_outcome(terms: Vec<Term>, clarify: Clarify) -> RunOutcome {
    RunOutcome {
        status: AskStatus::NeedsChoice,
        message: Some(clarify.question.clone()),
        sql_generated: None,
        sql_guarded: None,
        result: None,
        limit_kept: false,
        row_filters: Vec::new(),
        tables_read: Vec::new(),
        terms,
        assumptions: Vec::new(),
        clarify: Some(clarify),
        unanswerable: None,
        steps: Vec::new(),
        failures: Vec::new(),
    }
}

/// Mô hình báo dữ liệu không có thông tin: rejected, không SQL. Lời do ask/mod.rs dựng bằng build_unanswerable_message.
fn build_unanswerable_outcome(
    terms: Vec<Term>,
    unanswerable: Unanswerable,
    steps: Vec<AskStep>,
) -> RunOutcome {
    RunOutcome {
        status: AskStatus::Rejected,
        message: None,
        sql_generated: None,
        sql_guarded: None,
        result: None,
        limit_kept: false,
        row_filters: Vec::new(),
        tables_read: Vec::new(),
        terms,
        assumptions: Vec::new(),
        clarify: None,
        unanswerable: Some(unanswerable),
        steps,
        failures: Vec::new(),
    }
}

pub(crate) fn build_step(
    name: StepName,
    started: Instant,
    ok: bool,
    detail: Option<&str>,
) -> AskStep {
    AskStep {
        name,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        ok,
        detail: detail.map(str::to_string),
    }
}
