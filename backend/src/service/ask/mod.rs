// SPDX-License-Identifier: Apache-2.0
/* service/ask/mod.rs - luồng một câu hỏi (docs/ARCHITECTURE.md mục 2): dataset và phiên bản đang dùng, ngữ cảnh
(ask/context.rs) cùng chỉ số, câu mẫu đã duyệt và gói ngành, mô hình viết SQL (prompt ở ask/prompt.rs), guard, chạy và sửa
một lần (ask/run.rs), ghi lịch sử (engine/store/queries.rs), rồi dựng AskResponse (ask/contract.rs). Mỗi bước đo thời
gian thành một AskStep. Lựa chọn của response dựng ở ask/choices.rs và ask/response_choices.rs; thẻ "Đã hiểu là", vai
trò cột và gợi ý biểu đồ ở ask/view.rs. Sau bước context, bước classify (ask/classify.rs) nhờ mô hình phân loại câu
hỏi, mô hình lỗi thì dùng luật từ khóa; câu hỏi tìm nguyên nhân rẽ sang ask/why.rs, dùng chung ngữ cảnh, bảng được
phép và cách ghi lịch sử ở đây.

Câu không trả lời được vẫn là Ok kèm status (ARCHITECTURE mục 8). Err chỉ cho request sai (400, 404), mô hình lỗi khi
chưa có câu SQL nào (502, 503; mock gặp câu lạ thì 400) và lỗi máy chủ. needs_choice không ghi lịch sử: câu chưa được
trả lời, người hỏi sẽ gửi lại kèm lựa chọn.

Cảnh báo kiểm hợp lý (rỗng, toàn null, giá trị lọc không có, độ lớn bất thường) dựng ở ask/checks.rs sau khi chạy;
chúng không đổi status. */

pub(crate) mod chart;
pub mod checks;
pub mod classify;
#[cfg(test)]
mod classify_tests;
// build_metric_mismatches chờ cách báo chỉ số dùng sai công thức: bốn luật kiểm hợp lý (#36) không có mã cảnh báo hợp,
// thêm mã mới là đổi API. Phần còn lại endpoint hỏi đã gọi. Gỡ allow khi hàm đó được gọi, để clippy bắt lại mã thừa.
#[cfg(test)]
mod checks_tests;
#[allow(dead_code)]
pub mod choices;
pub mod context;
#[cfg(test)]
mod context_tests;
pub mod contract;
pub mod examples;
#[cfg(test)]
mod examples_tests;
pub mod failures;
pub mod fanout;
mod fanout_grain;
#[cfg(test)]
mod fanout_tests;
pub mod filter_values;
pub mod group_prompt;
pub mod judge;
pub mod lessons;
#[cfg(test)]
mod lessons_tests;
pub mod magnitude;
pub mod matching;
pub mod narrate;
#[cfg(test)]
mod narrate_tests;
pub mod other_text;
pub mod phrases;
mod plain_insight;
pub mod prompt;
#[cfg(test)]
mod prompt_tests;
pub mod related;
#[cfg(test)]
mod related_tests;
pub mod repair;
pub mod reply;
mod response_choices;
pub mod result;
pub mod row_filters;
#[cfg(test)]
mod row_filters_tests;
pub mod run;
#[cfg(test)]
mod run_tests;
pub mod summary;
#[cfg(test)]
mod summary_tests;
pub mod unanswerable;
mod unsnapshotted;
pub mod value_retry;
#[cfg(test)]
mod value_retry_tests;
pub mod values;
pub(crate) mod view;
mod why;
pub mod why_prompt;

use std::time::Instant;

use crate::auth::{Member, WorkspaceId};
use crate::engine::guard::AllowedTable;
use crate::engine::llm::meter::{build_call_totals, CallLog};
use crate::engine::llm::{LlmRequest, LlmTask};
use crate::engine::store::{Dataset, DatasetVersion, NewQuery, ProfileColumn, QueryStatus};
use crate::service::ask::checks::{build_result_warnings, find_missing_values, CheckedQuery};
use crate::service::ask::choices::{build_assumption_choices, build_clarify_choice};
use crate::service::ask::classify::{build_classify_timeout, classify, ClassifyInput};
use crate::service::ask::context::{build_ask_context_with_keys, AskContext};
use crate::service::ask::contract::{
    AskResponse, AskStatus, AskStep, ChosenOption, Insight, QuestionType, ResultTable, StepName,
    Understanding, Warning, WarningCode,
};
use crate::service::ask::fanout::{build_join_links, JoinLink};
use crate::service::ask::group_prompt::{
    apply_question_columns, build_group_prompt, load_question_tables,
};
use crate::service::ask::lessons::pick_lessons;
use crate::service::ask::plain_insight::{build_stored_breakdown, run_insight};
use crate::service::ask::prompt::{
    build_examples_detail, build_sql_prompt, build_sql_prompt_detail, pick_examples, PromptInput,
};
use crate::service::ask::related::{build_group_related, load_group};
use crate::service::ask::repair::build_user_text;
use crate::service::ask::reply::{build_sql_reply_schema, SqlAnswer, Term, TermKind};
use crate::service::ask::response_choices::{
    build_pick_choices, build_unapplied_warnings, build_used_choices,
};
pub use crate::service::ask::row_filters::load_row_filters;
use crate::service::ask::run::{build_step, run_with_repair, RunInput, RunOutcome};
use crate::service::ask::summary::{build_answer, SummaryInput, SummaryOutcome};
use crate::service::ask::unanswerable::build_unanswerable_message;
use crate::service::ask::value_retry::{apply_full_values, build_resend_columns};
use crate::service::ask::values::load_value_index;
use crate::service::ask::view::{apply_answer_view, build_answer_view, RanQuery};
use crate::service::ask::why::{ask_why, WhyAsk};
use crate::service::domains::load_for_prompt;
use crate::service::explain::form::QuestionKind;
use crate::service::ingest::{dataset_not_found, parse_dataset_id};
use crate::service::lessons::{record_failures, FailedQuestion};
use crate::{error::AppError, state::AppState};

/// Số câu mẫu đã duyệt gần nhất đọc từ lịch sử; prompt chỉ lấy tối đa EXAMPLES_MAX câu có từ trùng câu hỏi trong số này
/// (ask/prompt.rs). Đọc ít quá thì câu liên quan mà cũ không bao giờ được chọn; nhiều quá thì mỗi câu hỏi đọc Postgres
/// lâu hơn.
const EXAMPLE_POOL: i64 = 50;

/// Số bài học đã bật đọc mỗi câu hỏi; prompt chỉ lấy tối đa LESSONS_MAX bài liên quan trong số này (ask/lessons.rs).
const LESSON_POOL: i64 = 50;

/// Cùng lời với đường dự phòng của câu "tại sao" (explain/plain.rs), để hai đường báo một kiểu.
const CLARIFY_WITHOUT_OPTIONS: &str =
    "Mô hình hỏi lại nhưng không đưa phương án nào có trong dữ liệu. Thử hỏi lại theo cách khác.";

/// Các lượt trước của cuộc trò chuyện, do service/conversation/ dựng; bộ đánh giá và câu mở đầu cuộc dùng
/// AskHistory::default().
#[derive(Debug, Default)]
pub struct AskHistory {
    /// Khối "Hội thoại trước" của prompt SQL và prompt form (conversation/history.rs); chuỗi rỗng khi chưa có lượt nào.
    pub section: String,
    /// Câu hỏi của các lượt đó, cũ trước. Bước classify chỉ cần câu hỏi: SQL và phần đã hiểu làm prompt dài hơn mà
    /// không giúp phân loại.
    pub questions: Vec<String>,
}

/// Một câu hỏi, kèm danh sách lần gọi mô hình của nó (AskResponse.model_calls, #184): mọi bước bên dưới gọi mô hình
/// qua state_for_question, nên lần gọi nào cũng được ghi, kể cả lần gọi của ask/why.rs và service/insight/.
pub async fn ask(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    question: &str,
    choices: &[ChosenOption],
    history: &AskHistory,
) -> Result<AskResponse, AppError> {
    let log = CallLog::new();
    let state_for_question = state.with_call_log(&log);
    let asked = ask_steps(
        &state_for_question,
        member,
        dataset_id,
        question,
        choices,
        history,
    )
    .await;
    let calls = log.calls();
    let totals = build_call_totals(&calls);
    tracing::info!(
        total_ms = log.elapsed_ms(),
        model_calls = totals.calls,
        model_ms = totals.model_ms,
        input_tokens = totals.input_tokens,
        output_tokens = totals.output_tokens,
        ok = asked.is_ok(),
        "question done"
    );
    let mut response = asked?;
    response.model_calls = calls;
    Ok(response)
}

async fn ask_steps(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    question: &str,
    choices: &[ChosenOption],
    history: &AskHistory,
) -> Result<AskResponse, AppError> {
    if question.trim().is_empty() {
        return Err(AppError::BadRequest("Câu hỏi đang trống".to_string()));
    }
    let ws = &member.workspace;
    let (dataset, version) = load_target(state, ws, dataset_id).await?;

    // Chỉ số và câu mẫu đã duyệt là hai tầng của ngữ cảnh (ARCHITECTURE mục 5), nên tính vào bước context, cùng các bảng
    // liên quan (#213).
    let started = Instant::now();
    let columns = state.store.list_columns(ws, version.id).await?;
    let picked = build_allowed_table(state, member, &dataset, &version, &columns).await?;
    let group = load_group(state, member, (&dataset, &version, &columns), picked).await?;
    let join_keys = group.join_keys(dataset.id);
    // Chỉ số đọc trước ngữ cảnh: cột trong công thức chỉ số luôn được gửi đủ chi tiết (ask/context.rs).
    let metrics = state.store.list_metrics(ws, dataset.id).await?;
    let mut context =
        build_ask_context_with_keys(state, ws, &version, question, choices, &join_keys, &metrics)
            .await?;
    let examples = state
        .store
        .list_approved_examples(ws, dataset.id, EXAMPLE_POOL)
        .await?;
    // Gói ngành đọc từ bảng của workspace (#229), nên khái niệm admin vừa sửa có ngay ở câu này.
    let domain = match dataset.domain.as_deref() {
        Some(key) => load_for_prompt(state, ws, key).await?,
        None => None,
    };
    let active_lessons = state
        .store
        .list_active_lessons(ws, dataset.id, LESSON_POOL)
        .await?;
    let mut steps = vec![build_step(StepName::Context, started, true, None)];

    // Lượt phân loại nhận metadata cột của mọi bảng; cột nó chọn quyết định cột nào được gửi đủ danh sách giá trị ở
    // lượt viết SQL (ask/context.rs). Một cụm khớp nhiều cột không hỏi lại ở đây: ngữ cảnh đã chọn cột đầu và ghi các cột
    // khác thành giả định đổi được.
    let mut question_tables = load_question_tables(state, member, &group, question).await?;
    let table = dataset.logical_table.as_str();
    let restricted = group.is_restricted();
    apply_question_columns(&mut context, table, &mut question_tables, &[], restricted);
    // Phân loại sau bước ngữ cảnh: prompt phân loại cần metadata cột của các bảng vừa đọc.
    let classify_input = ClassifyInput {
        table,
        question,
        columns: &context.columns,
        related: &question_tables.related,
        earlier: &history.questions,
    };
    let timeout = build_classify_timeout(&state.config);
    let classified = classify(state.llm.as_ref(), &classify_input, timeout).await;
    steps.push(classified.step);
    apply_question_columns(
        &mut context,
        table,
        &mut question_tables,
        &classified.columns,
        restricted,
    );
    let group_prompt = build_group_prompt(&group, question_tables);
    // Bài học chọn sau khi ngữ cảnh đã đủ (cột khớp, cột lượt phân loại chọn), để khớp đúng các cột câu hỏi dùng.
    let lessons = pick_lessons(question, &context.columns, &active_lessons);
    let input = PromptInput {
        table: &dataset.logical_table,
        question,
        context: &context,
        metrics: &metrics,
        examples: &examples,
        domain: domain.as_ref(),
        choices,
        history: &history.section,
        group: group_prompt.as_ref(),
        lessons: &lessons,
    };
    let tables = &group.allowed;
    // Quan hệ giữa các bảng của câu hỏi, để chặn câu cộng lặp khi nối bảng một-nhiều (ask/fanout.rs) và để phân rã
    // nối bảng liên quan theo đúng chiều nhiều-một (explain/source.rs).
    let links = build_join_links(&group);
    if classified.kind == QuestionKind::Why {
        // Form của câu "tại sao" chỉ nêu cột của bảng đang chọn (#213); phân rã thì xét thêm cột của các bảng nối
        // nhiều-một vào nó (#227), nên guard nhận đủ các bảng của nhóm.
        let related = build_group_related(&group, &links);
        let why = WhyAsk {
            dataset: &dataset,
            version: &version,
            prompt: PromptInput {
                group: None,
                ..input
            },
            columns: &columns,
            tables,
            group: &group,
            related: &related,
            steps,
        };
        let mut response = ask_why(state, member, why).await?;
        response.question_type = Some(QuestionType::Causal);
        return Ok(response);
    }

    // Mức chi tiết của prompt: danh sách giá trị nào thật sự đã gửi (ask/value_retry.rs).
    let (prompt, detail) = build_sql_prompt_detail(&input);
    let started = Instant::now();
    let request = LlmRequest {
        task: LlmTask::Sql,
        question,
        prompt: &prompt,
        json_schema: Some(build_sql_reply_schema()),
    };
    let reply = state.llm.complete(&request).await?;
    let examples_used = build_examples_detail(&pick_examples(&input, false), input.lessons.len());
    steps.push(build_step(
        StepName::Llm,
        started,
        true,
        examples_used.as_deref(),
    ));

    let sensitive = group.sensitive_columns();
    let input = RunInput {
        question,
        prompt: &prompt,
        reply: &reply,
        tables,
        links: &links,
        max_rows: state.config.ask_max_rows,
        timeout: state.config.query_timeout,
        jobs: &state.query_jobs,
        sensitive: &sensitive,
    };
    let mut outcome = run_with_repair(state.llm.as_ref(), input).await?;
    steps.append(&mut outcome.steps);

    // Câu lọc theo giá trị cột không có, mà prompt chưa có đủ danh sách của cột: gửi lại một lần kèm đủ giá trị của cột
    // đó (ask/value_retry.rs). Người bị lọc dòng không nhận danh sách giá trị nào, nên không gửi lại.
    let checked = CheckedQuery {
        status: outcome.status,
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        row_filters: &outcome.row_filters,
        terms: &outcome.terms,
        columns: &columns,
        restricted: group.is_restricted(),
    };
    let (_, missing) = find_missing_values(state, ws, &version, &checked).await;
    let resend = build_resend_columns(&missing, &context.columns, detail);
    if !resend.is_empty() && !group.is_restricted() {
        let index = load_value_index(state, ws, &version).await?;
        let applied: Vec<String> = resend
            .into_iter()
            .filter(|name| apply_full_values(&mut context.columns, &index, name))
            .collect();
        if !applied.is_empty() {
            let retry_input = PromptInput {
                table: &dataset.logical_table,
                question,
                context: &context,
                metrics: &metrics,
                examples: &examples,
                domain: domain.as_ref(),
                choices,
                history: &history.section,
                group: group_prompt.as_ref(),
                lessons: &lessons,
            };
            let retry = ValueRetry {
                input: &retry_input,
                columns: &applied,
                tables,
                links: &links,
                sensitive: &sensitive,
            };
            run_value_retry(state, retry, &mut outcome, &mut steps).await;
        }
    }
    if let Some(unanswerable) = &outcome.unanswerable {
        let names: Vec<&str> = context.columns.iter().map(|c| c.name.as_str()).collect();
        outcome.message = Some(build_unanswerable_message(unanswerable, &names));
    }
    apply_needed_table_message(state, ws, &dataset, question, &mut outcome).await?;
    // Giả định thành Choice một lần, dùng cho cả thẻ "Đã hiểu là" lẫn AskResponse.choices để hai chỗ cùng id.
    let answer = SqlAnswer {
        terms: outcome.terms.clone(),
        assumptions: outcome.assumptions.clone(),
        sql: outcome.sql_generated.clone().unwrap_or_default(),
    };
    let mut assumptions = build_pick_choices(&context);
    assumptions.extend(build_assumption_choices(&answer, &context, &metrics));
    let used = build_used_choices(&context, &assumptions);
    let mut warnings = build_unapplied_warnings(choices, &used);
    // Gọi trước nhánh hỏi lại: câu chưa answered vẫn được báo giá trị mô hình nêu mà cột không có.
    let checked = CheckedQuery {
        status: outcome.status,
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        row_filters: &outcome.row_filters,
        terms: &outcome.terms,
        columns: &columns,
        restricted: group.is_restricted(),
    };
    warnings.extend(build_result_warnings(state, ws, &version, &checked).await);

    if let Some(clarify) = &outcome.clarify {
        match build_clarify_choice(clarify, &context, &metrics) {
            Some(choice) => {
                return Ok(AskResponse {
                    message: outcome.message,
                    choices: used,
                    warnings,
                    clarifications: vec![choice],
                    steps,
                    question_type: Some(QuestionType::Quantitative),
                    ..build_base_response(state, &version, question, AskStatus::NeedsChoice)
                });
            }
            // Hỏi lại mà không cho chọn gì thì người hỏi kẹt: coi như mô hình trả lời sai, ghi lịch sử như câu lỗi.
            None => {
                outcome.status = AskStatus::Failed;
                outcome.message = Some(CLARIFY_WITHOUT_OPTIONS.to_string());
            }
        }
    }

    // Chỉ câu answered có thẻ, vai trò cột và biểu đồ; câu khác nhận None.
    let ran = RanQuery {
        status: outcome.status,
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        limit_kept: outcome.limit_kept,
        row_filters: &outcome.row_filters,
        plan: None,
    };
    let view = build_answer_view(&ran, &context.columns, &metrics, &assumptions);
    let (understanding, chart) = apply_answer_view(view, outcome.result.as_mut());
    let mut failures = std::mem::take(&mut outcome.failures);

    // Phần giải thích theo hình dạng kết quả (#227): một lần gọi mô hình để viết lời, phân rã tự động do code.
    let insight = run_insight(
        state,
        &group,
        &links,
        question,
        &outcome,
        understanding.as_ref(),
        &lessons,
    )
    .await?;
    let mut read_names = outcome.tables_read.clone();
    let (insight, explanation) = match insight {
        Some(found) => {
            steps.extend(found.steps);
            failures.extend(found.failure);
            for name in found.tables_read {
                if !read_names.contains(&name) {
                    read_names.push(name);
                }
            }
            (Some(found.insight), found.explanation)
        }
        None => (None, None),
    };
    let answer = build_answer_text(state, understanding.as_ref(), &outcome, insight.as_ref());
    let answer_text_by_model = answer.as_ref().map(|a| a.by_model);
    let answer_text = answer.map(|a| a.text);
    let tables_read = group.build_tables_read(&read_names);
    let recorded = Recorded {
        status: outcome.status,
        message: outcome.message.as_deref(),
        sql_generated: outcome.sql_generated.as_deref(),
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        explanation: build_stored_breakdown(explanation.as_ref())?,
        read_versions: group.build_read_versions(&tables_read),
    };
    let target = (&dataset, &version);
    let query_id = record_query(state, member, target, question, recorded, &mut steps).await?;
    let asked = FailedQuestion {
        dataset_id: dataset.id,
        question,
        query_id: Some(query_id),
    };
    record_failures(state, member, &asked, &failures).await;
    Ok(AskResponse {
        message: outcome.message,
        query_id: Some(query_id),
        choices: used,
        sql_generated: outcome.sql_generated,
        sql_guarded: outcome.sql_guarded,
        tables_read,
        understanding,
        result: outcome.result,
        chart,
        warnings,
        explanation,
        insight,
        answer_text,
        answer_text_by_model,
        question_type: Some(QuestionType::Quantitative),
        steps,
        ..build_base_response(state, &version, question, outcome.status)
    })
}

/// Phần của lần gửi lại kèm đủ giá trị (ask/value_retry.rs) mà run_value_retry cần.
struct ValueRetry<'a> {
    input: &'a PromptInput<'a>,
    /// Cột được gửi lại đủ danh sách giá trị.
    columns: &'a [String],
    tables: &'a [AllowedTable],
    links: &'a [JoinLink],
    sensitive: &'a [String],
}

/// Gọi lại lượt viết SQL một lần. Lần gửi lại chỉ là bước thêm: chỉ khi nó answered thì kết quả của nó thay kết quả lần
/// đầu. Lỗi mô hình hay lỗi chạy, hay kết quả không answered, thì giữ kết quả lần đầu và ghi một bước ok = false.
/// failures (nhật ký lỗi, #228) của cả hai lần đều được giữ. Lỗi gọi nhà cung cấp mô hình (mạng, hạn mức) không phải chỗ
/// mô hình sai nên chỉ ghi log máy chủ và bước, không vào nhật ký lỗi.
async fn run_value_retry(
    state: &AppState,
    retry: ValueRetry<'_>,
    outcome: &mut RunOutcome,
    steps: &mut Vec<AskStep>,
) {
    let names = retry.columns.join(", ");
    let prompt = build_sql_prompt(retry.input);
    let started = Instant::now();
    let request = LlmRequest {
        task: LlmTask::Sql,
        question: retry.input.question,
        prompt: &prompt,
        json_schema: Some(build_sql_reply_schema()),
    };
    let reply = match state.llm.complete(&request).await {
        Ok(reply) => reply,
        Err(e) => {
            tracing::warn!(error = ?e, "value retry: the model call failed, keeping the first answer");
            let detail = format!(
                "Gửi lại kèm đủ giá trị của cột {names} không được ({}): giữ kết quả lần đầu",
                build_user_text(&e, "llm")
            );
            steps.push(build_step(StepName::Llm, started, false, Some(&detail)));
            return;
        }
    };
    let detail =
        format!("Gửi lại kèm đủ giá trị của cột {names}: câu trước lọc theo giá trị cột không có");
    steps.push(build_step(StepName::Llm, started, true, Some(&detail)));
    let input = RunInput {
        question: retry.input.question,
        prompt: &prompt,
        reply: &reply,
        tables: retry.tables,
        links: retry.links,
        max_rows: state.config.ask_max_rows,
        timeout: state.config.query_timeout,
        jobs: &state.query_jobs,
        sensitive: retry.sensitive,
    };
    let started = Instant::now();
    match run_with_repair(state.llm.as_ref(), input).await {
        Ok(mut retried) => {
            steps.append(&mut retried.steps);
            if retried.status == AskStatus::Answered {
                let mut failures = std::mem::take(&mut outcome.failures);
                failures.append(&mut retried.failures);
                retried.failures = failures;
                *outcome = retried;
            } else {
                outcome.failures.append(&mut retried.failures);
                let detail = "Lần gửi lại không ra câu trả lời: giữ kết quả lần đầu";
                steps.push(build_step(StepName::Llm, started, false, Some(detail)));
            }
        }
        Err(e) => {
            tracing::warn!(error = ?e, "value retry: running the new SQL failed, keeping the first answer");
            let detail = format!(
                "Lần gửi lại lỗi ({}): giữ kết quả lần đầu",
                build_user_text(&e, "llm")
            );
            steps.push(build_step(StepName::Llm, started, false, Some(&detail)));
        }
    }
}

/// Đoạn trả lời ngắn (ask/summary.rs), chỉ cho câu answered có kết quả: lời của phần giải thích hay câu khuôn, không
/// gọi mô hình.
fn build_answer_text(
    state: &AppState,
    understanding: Option<&Understanding>,
    outcome: &RunOutcome,
    insight: Option<&Insight>,
) -> Option<SummaryOutcome> {
    let result = outcome.result.as_ref()?;
    if outcome.status != AskStatus::Answered {
        return None;
    }
    build_answer(&SummaryInput {
        understanding,
        result,
        insight,
        enabled: state.config.answer_text,
    })
}

/// Câu không trả lời được mà cần một bảng cùng nguồn chưa có dữ liệu (chỉ metadata, #247): lời nêu đúng bảng đó
/// (ask/unsnapshotted.rs). Dataset nạp từ file không có nguồn database, không đọc gì thêm.
async fn apply_needed_table_message(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    question: &str,
    outcome: &mut RunOutcome,
) -> Result<(), AppError> {
    let (AskStatus::Rejected | AskStatus::Failed, Some(source_id)) =
        (outcome.status, dataset.source_id)
    else {
        return Ok(());
    };
    let tables = state.store.list_metadata_tables(ws, source_id).await?;
    let sql = outcome.sql_generated.as_deref();
    if let Some(table) = unsnapshotted::find_needed_table(question, sql, &tables) {
        outcome.message = Some(unsnapshotted::build_needed_table_message(table));
    }
    Ok(())
}

/// Dataset của workspace người hỏi và phiên bản đang dùng. Id của workspace khác trả 404 như id không có.
async fn load_target(
    state: &AppState,
    ws: &WorkspaceId,
    dataset_id: &str,
) -> Result<(Dataset, DatasetVersion), AppError> {
    let id = parse_dataset_id(dataset_id)?;
    let dataset = state
        .store
        .get_dataset(ws, id)
        .await?
        .ok_or_else(dataset_not_found)?;
    let version = state
        .store
        .get_current_version(ws, id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("Dataset chưa có dữ liệu nào; nạp file trước khi hỏi".to_string())
        })?;
    Ok((dataset, version))
}

/// Một bảng câu hỏi được đọc (dataset đang hỏi hay bảng liên quan, ask/related.rs), trên file Parquet của phiên bản
/// đang dùng, lọc dòng theo vai dữ liệu của người hỏi. columns là mọi cột của phiên bản, không chỉ cột ngữ cảnh gửi mô
/// hình: dataset nhiều cột thì ngữ cảnh chỉ gửi cột khớp câu hỏi, mà guard từ chối cột không có trong danh sách nó
/// nhận, nên câu SQL dùng một cột khác có thật sẽ bị chặn oan.
pub(super) async fn build_allowed_table(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
    version: &DatasetVersion,
    columns: &[ProfileColumn],
) -> Result<AllowedTable, AppError> {
    // Phiên bản nạp từ file luôn có đường dẫn; nguồn database (sau này) thì không, và cần cách chạy khác.
    let Some(relative) = &version.parquet_path else {
        return Err(AppError::Internal(
            "dataset version has no parquet path".to_string(),
        ));
    };
    Ok(AllowedTable {
        name: dataset.logical_table.clone(),
        columns: columns.iter().map(|c| c.name.clone()).collect(),
        parquet_dir: state.config.data_dir.join(relative),
        row_filters: load_row_filters(state, member, dataset.id).await?,
    })
}

/// Giá trị mô hình nói là có trong một cột (terms kind value) mà danh sách đủ của cột đó không có: câu SQL lọc theo nó
/// sẽ ra rỗng hay thiếu dòng. Chỉ xét cột ngữ cảnh gửi đủ giá trị; danh sách chỉ là mẫu thì không biết chắc.
pub(super) fn build_value_warnings(context: &AskContext, terms: &[Term]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for term in terms.iter().filter(|t| t.kind == TermKind::Value) {
        let (Some(column_name), Some(value)) = (&term.column, &term.value) else {
            continue;
        };
        let Some(column) = context
            .columns
            .iter()
            .find(|c| c.name == *column_name && c.values_complete)
        else {
            continue;
        };
        if column.values.iter().all(|entry| entry.value != *value) {
            warnings.push(Warning {
                code: WarningCode::FilterValueNotFound,
                message: format!(
                    "Cột {column_name} không có giá trị \"{value}\"; kết quả có thể rỗng hoặc thiếu dòng"
                ),
            });
        }
    }
    warnings
}

/// Phần của một câu đã có kết quả cuối được ghi vào lịch sử: các trường cùng tên của RunOutcome hay WhyOutcome.
pub(super) struct Recorded<'a> {
    pub status: AskStatus,
    pub message: Option<&'a str>,
    pub sql_generated: Option<&'a str>,
    pub sql_guarded: Option<&'a str>,
    pub result: Option<&'a ResultTable>,
    /// Form và Explanation của câu "tại sao" (ask/why.rs), hay phân rã tự động của câu thường (ask/plain_insight.rs).
    pub explanation: Option<serde_json::Value>,
    /// Cặp (dataset, phiên bản) câu đã đọc, ghi vào queries.read_versions; rỗng thì ghi cặp của dataset đang hỏi.
    pub read_versions: Vec<(uuid::Uuid, uuid::Uuid)>,
}

/// Ghi câu hỏi đã có kết quả cuối (answered, rejected, failed) rồi thêm bước record vào steps. Lỗi ghi là lỗi máy chủ
/// (500), kể cả khi đã có kết quả: answered bắt buộc có query_id để truy dòng nguồn (#34) và lịch sử dùng lại được.
pub(super) async fn record_query(
    state: &AppState,
    member: &Member,
    (dataset, version): (&Dataset, &DatasetVersion),
    question: &str,
    recorded: Recorded<'_>,
    steps: &mut Vec<AskStep>,
) -> Result<uuid::Uuid, AppError> {
    let started = Instant::now();
    let status = match recorded.status {
        AskStatus::Answered => QueryStatus::Ok,
        AskStatus::Rejected => QueryStatus::Rejected,
        AskStatus::Failed => QueryStatus::Error,
        AskStatus::NeedsChoice => {
            return Err(AppError::Internal(
                "a question that needs a choice is not recorded".to_string(),
            ))
        }
    };
    let timings = build_timings(state, steps)?;
    let row_count = recorded
        .result
        .map(|r| i64::try_from(r.total_rows).unwrap_or(i64::MAX));
    let query = NewQuery {
        target: Some((dataset.id, version.id)),
        user_id: Some(member.user.id),
        question,
        sql_generated: recorded.sql_generated,
        sql_guarded: recorded.sql_guarded,
        status,
        error: recorded.message,
        timings,
        row_count,
        explanation: recorded.explanation,
        read_versions: if recorded.read_versions.is_empty() {
            vec![(dataset.id, version.id)]
        } else {
            recorded.read_versions
        },
    };
    let id = state.store.record_query(&member.workspace, &query).await?;
    steps.push(build_step(StepName::Record, started, true, None));
    Ok(id)
}

/// queries.timings: các bước, các lần gọi mô hình tới lúc ghi và thời gian từ đầu câu hỏi (#184), để lệnh
/// `xdi-backend llm-stats` tính thời gian, số lần gọi và token theo loại câu. Mọi lần gọi mô hình của câu hỏi đều xảy
/// ra trước bước record.
fn build_timings(state: &AppState, steps: &[AskStep]) -> Result<serde_json::Value, AppError> {
    let (calls, elapsed_ms) = match &state.call_log {
        Some(log) => (log.calls(), Some(log.elapsed_ms())),
        None => (Vec::new(), None),
    };
    let timings = serde_json::json!({
        "steps": steps,
        "model_calls": calls,
        "elapsed_ms": elapsed_ms,
    });
    Ok(timings)
}

/// Phần chung của mọi response: mỗi nhánh của ask chỉ ghi các trường của nó.
pub(super) fn build_base_response(
    state: &AppState,
    version: &DatasetVersion,
    question: &str,
    status: AskStatus,
) -> AskResponse {
    AskResponse {
        status,
        question: question.to_string(),
        message: None,
        query_id: None,
        // service/conversation/ điền id thật sau khi luồng hỏi trả về; luồng hỏi không biết gì về cuộc trò chuyện ngoài
        // khối lịch sử trong prompt.
        conversation_id: uuid::Uuid::nil(),
        version_id: version.id,
        model: state.llm.name(),
        choices: vec![],
        sql_generated: None,
        sql_guarded: None,
        tables_read: vec![],
        understanding: None,
        result: None,
        chart: None,
        warnings: vec![],
        explanation: None,
        insight: None,
        answer_text: None,
        answer_text_by_model: None,
        question_type: None,
        clarifications: vec![],
        steps: vec![],
        model_calls: vec![],
    }
}
