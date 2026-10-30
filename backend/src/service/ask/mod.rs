// SPDX-License-Identifier: Apache-2.0
/* service/ask/mod.rs - luồng một câu hỏi (docs/ARCHITECTURE.md mục 2): dataset và phiên bản đang dùng, ngữ cảnh
(ask/context.rs) cùng chỉ số, câu đã đúng và gói ngành, mô hình viết SQL (prompt ở ask/prompt.rs), guard, chạy và sửa
một lần (ask/run.rs), ghi lịch sử (engine/store/queries.rs), rồi dựng AskResponse (ask/contract.rs). Mỗi bước đo thời
gian thành một AskStep. Lựa chọn của response dựng ở ask/choices.rs và ask/response_choices.rs; thẻ "Đã hiểu là", vai
trò cột và gợi ý biểu đồ ở ask/view.rs. Câu "tại sao" (build_question_kind của explain/form.rs) rẽ sang ask/why.rs sau
bước context, dùng chung ngữ cảnh, bảng được phép và cách ghi lịch sử ở đây.

Câu không trả lời được vẫn là Ok kèm status (ARCHITECTURE mục 8). Err chỉ cho request sai (400, 404), mô hình lỗi khi
chưa có câu SQL nào (502, 503; mock gặp câu lạ thì 400) và lỗi máy chủ. needs_choice không ghi lịch sử: câu chưa được
trả lời, người hỏi sẽ gửi lại kèm lựa chọn.

Cảnh báo kiểm hợp lý (rỗng, toàn null, giá trị lọc không có, độ lớn bất thường) dựng ở ask/checks.rs sau khi chạy;
chúng không đổi status. */

mod chart;
pub mod checks;
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
pub mod filter_values;
pub mod magnitude;
pub mod matching;
pub mod narrate;
#[cfg(test)]
mod narrate_tests;
pub mod other_text;
pub mod phrases;
pub mod prompt;
#[cfg(test)]
mod prompt_tests;
pub mod repair;
pub mod repeated;
#[cfg(test)]
mod repeated_tests;
pub mod reply;
mod response_choices;
pub mod result;
pub mod run;
#[cfg(test)]
mod run_tests;
pub mod summary;
#[cfg(test)]
mod summary_tests;
pub mod unanswerable;
pub mod values;
pub(crate) mod view;
mod why;
pub mod why_prompt;

use std::time::Instant;

use crate::auth::{Member, Role, WorkspaceId};
use crate::engine::domains::find_domain_package;
use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::llm::{LlmRequest, LlmTask};
use crate::engine::store::{Dataset, DatasetVersion, NewQuery, ProfileColumn, QueryStatus};
use crate::service::ask::checks::{build_result_warnings, CheckedQuery};
use crate::service::ask::choices::{build_assumption_choices, build_clarify_choice};
use crate::service::ask::context::{build_ask_context, AskContext};
use crate::service::ask::contract::{
    AskResponse, AskStatus, AskStep, ChosenOption, ResultTable, StepName, Understanding, Warning,
    WarningCode,
};
use crate::service::ask::prompt::{build_sql_prompt, PromptInput};
use crate::service::ask::repeated::build_repeated_columns;
use crate::service::ask::reply::{build_sql_reply_schema, SqlAnswer, Term, TermKind};
use crate::service::ask::response_choices::{
    build_ambiguity_choices, build_ambiguity_detail, build_ambiguity_message,
    build_unapplied_warnings, build_used_choices,
};
use crate::service::ask::run::{build_step, run_with_repair, RunInput, RunOutcome};
use crate::service::ask::summary::{summarize, SummaryInput, SummaryOutcome};
use crate::service::ask::unanswerable::build_unanswerable_message;
use crate::service::ask::view::{apply_answer_view, build_answer_view, RanQuery};
use crate::service::ask::why::{ask_why, build_narrate_timeout, WhyAsk};
use crate::service::explain::form::{build_question_kind, QuestionKind};
use crate::service::ingest::{dataset_not_found, parse_dataset_id};
use crate::{error::AppError, state::AppState};

/// Số câu đã đúng gần nhất đọc từ lịch sử; prompt chỉ lấy tối đa EXAMPLES_MAX câu có từ trùng câu hỏi trong số này
/// (ask/prompt.rs). Đọc ít quá thì câu liên quan mà cũ không bao giờ được chọn; nhiều quá thì mỗi câu hỏi đọc Postgres
/// lâu hơn.
const EXAMPLE_POOL: i64 = 50;

/// Cùng lời với đường dự phòng của câu "tại sao" (explain/plain.rs), để hai đường báo một kiểu.
const CLARIFY_WITHOUT_OPTIONS: &str =
    "Mô hình hỏi lại nhưng không đưa phương án nào có trong dữ liệu. Thử hỏi lại theo cách khác.";

pub async fn ask(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    question: &str,
    choices: &[ChosenOption],
    history: &str,
) -> Result<AskResponse, AppError> {
    if question.trim().is_empty() {
        return Err(AppError::BadRequest("Câu hỏi đang trống".to_string()));
    }
    let ws = &member.workspace;
    let (dataset, version) = load_target(state, ws, dataset_id).await?;

    // Chỉ số và câu đã đúng là hai tầng của ngữ cảnh (ARCHITECTURE mục 5), nên tính vào bước context.
    let started = Instant::now();
    let context = build_ask_context(state, ws, &version, question, choices).await?;
    let metrics = state.store.list_metrics(ws, dataset.id).await?;
    let examples = state
        .store
        .list_correct_queries(ws, dataset.id, EXAMPLE_POOL)
        .await?;
    let columns = state.store.list_columns(ws, version.id).await?;
    let model = state.store.version_source_model(ws, version.id).await?;
    let repeated = build_repeated_columns(model.as_ref());
    let detail = build_ambiguity_detail(&context);
    let mut steps = vec![build_step(
        StepName::Context,
        started,
        true,
        detail.as_deref(),
    )];
    // Hỏi lại trước khi gọi mô hình: cột nào cũng đoán thì SQL có thể đọc sai cột mà không ai biết.
    if !context.ambiguous.is_empty() {
        let used = build_used_choices(&context, &[]);
        return Ok(AskResponse {
            message: Some(build_ambiguity_message(&context)),
            warnings: build_unapplied_warnings(choices, &used),
            choices: used,
            clarifications: build_ambiguity_choices(&context),
            steps,
            ..build_base_response(state, &version, question, AskStatus::NeedsChoice)
        });
    }

    let input = PromptInput {
        table: &dataset.logical_table,
        question,
        context: &context,
        metrics: &metrics,
        examples: &examples,
        domain: dataset.domain.as_deref().and_then(find_domain_package),
        choices,
        history,
    };
    let tables = [build_allowed_table(state, member, &dataset, &version, &columns).await?];
    if build_question_kind(question) == QuestionKind::Why {
        let why = WhyAsk {
            dataset: &dataset,
            version: &version,
            prompt: input,
            columns: &columns,
            tables: &tables,
            repeated: &repeated,
            steps,
        };
        return ask_why(state, member, why).await;
    }

    let prompt = build_sql_prompt(&input);
    let started = Instant::now();
    let request = LlmRequest {
        task: LlmTask::Sql,
        question,
        prompt: &prompt,
        json_schema: Some(build_sql_reply_schema()),
    };
    let reply = state.llm.complete(&request).await?;
    steps.push(build_step(StepName::Llm, started, true, None));

    let input = RunInput {
        question,
        prompt: &prompt,
        reply: &reply,
        tables: &tables,
        repeated: &repeated,
        max_rows: state.config.ask_max_rows,
        timeout: state.config.query_timeout,
        jobs: &state.query_jobs,
    };
    let mut outcome = run_with_repair(state.llm.as_ref(), input).await?;
    steps.append(&mut outcome.steps);
    if let Some(unanswerable) = &outcome.unanswerable {
        let names: Vec<&str> = context.columns.iter().map(|c| c.name.as_str()).collect();
        outcome.message = Some(build_unanswerable_message(unanswerable, &names));
    }
    // Giả định thành Choice một lần, dùng cho cả thẻ "Đã hiểu là" lẫn AskResponse.choices để hai chỗ cùng id.
    let answer = SqlAnswer {
        terms: outcome.terms.clone(),
        assumptions: outcome.assumptions.clone(),
        sql: outcome.sql_generated.clone().unwrap_or_default(),
    };
    let assumptions = build_assumption_choices(&answer, &context, &metrics);
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
        restricted: !tables[0].row_filters.is_empty(),
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
    let answer = build_answer_text(state, question, understanding.as_ref(), &outcome).await;
    let (answer_text, answer_text_by_model) = match answer {
        Some(answer) => {
            steps.push(answer.step);
            (Some(answer.text), Some(answer.by_model))
        }
        None => (None, None),
    };

    let recorded = Recorded {
        status: outcome.status,
        message: outcome.message.as_deref(),
        sql_generated: outcome.sql_generated.as_deref(),
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        explanation: None,
    };
    let target = (&dataset, &version);
    let query_id = record_query(state, member, target, question, recorded, &mut steps).await?;
    Ok(AskResponse {
        message: outcome.message,
        query_id: Some(query_id),
        choices: used,
        sql_generated: outcome.sql_generated,
        sql_guarded: outcome.sql_guarded,
        understanding,
        result: outcome.result,
        chart,
        warnings,
        answer_text,
        answer_text_by_model,
        steps,
        ..build_base_response(state, &version, question, outcome.status)
    })
}

/// Đoạn trả lời bằng lời (ask/summary.rs), chỉ cho câu answered có kết quả.
async fn build_answer_text(
    state: &AppState,
    question: &str,
    understanding: Option<&Understanding>,
    outcome: &RunOutcome,
) -> Option<SummaryOutcome> {
    let result = outcome.result.as_ref()?;
    if outcome.status != AskStatus::Answered {
        return None;
    }
    let input = SummaryInput {
        question,
        understanding,
        result,
    };
    let timeout = build_narrate_timeout(&state.config);
    summarize(
        state.llm.as_ref(),
        &input,
        state.config.answer_text,
        timeout,
    )
    .await
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

/// Bảng duy nhất câu hỏi được đọc: dataset đang hỏi, trên file Parquet của phiên bản đang dùng, lọc dòng theo vai dữ
/// liệu của người hỏi. columns là mọi cột của phiên bản, không chỉ cột ngữ cảnh gửi mô hình: dataset nhiều cột thì ngữ
/// cảnh chỉ gửi cột khớp câu hỏi, mà guard từ chối cột không có trong danh sách nó nhận, nên câu SQL dùng một cột khác
/// có thật sẽ bị chặn oan.
async fn build_allowed_table(
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

/// Điều kiện lọc dòng theo vai dữ liệu (rls_policies) của một thành viên trên một dataset; luồng hỏi và truy dòng nguồn
/// (drill/mod.rs) cùng gọi để mọi SQL chạy trên dữ liệu bị lọc như nhau. Admin không bị lọc: họ tự sửa được chính sách,
/// lọc với họ không bảo vệ được gì (#45).
pub(super) async fn load_row_filters(
    state: &AppState,
    member: &Member,
    dataset_id: uuid::Uuid,
) -> Result<Vec<RowFilter>, AppError> {
    match member.role {
        Role::Admin => Ok(Vec::new()),
        Role::Member => {
            state
                .store
                .list_row_filters(&member.workspace, dataset_id, member.user.id)
                .await
        }
    }
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
    /// Form và Explanation của câu "tại sao" (ask/why.rs); None với câu thường.
    pub explanation: Option<serde_json::Value>,
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
    let timings = serde_json::to_value(&*steps)
        .map_err(|e| AppError::Internal(format!("serialize ask steps: {e}")))?;
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
    };
    let id = state.store.record_query(&member.workspace, &query).await?;
    steps.push(build_step(StepName::Record, started, true, None));
    Ok(id)
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
        understanding: None,
        result: None,
        chart: None,
        warnings: vec![],
        explanation: None,
        answer_text: None,
        answer_text_by_model: None,
        clarifications: vec![],
        steps: vec![],
    }
}
