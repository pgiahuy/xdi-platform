// SPDX-License-Identifier: Apache-2.0
/* service/explain_run.rs - đường của câu "tại sao" (#38), từ câu hỏi tới bảng, phân rã và lời: mô hình điền form
(ask_why_prompt.rs), code kiểm form (explain_check.rs), dựng SQL (explain_sql.rs), chạy qua guard, phân rã
(explain_breakdown.rs) và nhờ mô hình viết lời (ask_narrate.rs). Endpoint hỏi gọi run_why qua ask_why.rs khi
build_question_kind ra Why, thay cho bước llm và run_with_repair của câu thường, rồi dựng AskResponse từ WhyOutcome.
Nhận mô hình và dữ liệu đã đọc qua tham số, như run_with_repair, nên test chạy được với mô hình giả và Parquet tạm,
không cần Postgres; test ở explain_run_tests.rs.

Vòng sửa: form sai (không đọc được, cột lạ, vai trò sai, giá trị không có, kỳ không có dòng) thì gửi lỗi từng trường
cho mô hình sửa một lần. Sai lần hai thì chạy như câu thường (explain_plain.rs) kèm cảnh báo
explanation_unavailable: người hỏi vẫn có bảng số, câu vẫn answered nếu SQL thường chạy được. Câu SQL do code dựng mà
guard từ chối hay DuckDB lỗi thì không sửa: mô hình không viết câu đó, sửa form không đổi được nó. Cũng không sửa khi
kỳ so sánh do người hỏi chọn (why:base) không có dòng nào: lựa chọn đè lên base của form, mô hình sửa form cũng không
đổi được kỳ đó, nên trả ngay bảng tổng kèm explanation_unavailable; giả định why:base vẫn trên thẻ để chọn kỳ khác.

Hỏi lại (needs_choice) trước khi gọi mô hình khi một cụm khớp nhiều cột gần nhau (AskContext::ambiguous, #28; endpoint
có thể đã hỏi lại trước, kiểm lại ở đây để run_why đúng khi gọi riêng), và sau khi gọi khi form không nêu độ đo mà
dataset có chỉ số để chọn. Giả định đổi được (kỳ so sánh, chỉ số chưa xác nhận) dựng ở explain_choices.rs. */

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use super::ask_choices::build_clarify_choice;
use super::ask_contract::{
    AskStatus, AskStep, Choice, ChoiceOption, Explanation, ResultTable, StepName, Warning,
    WarningCode,
};
use super::ask_narrate::narrate;
use super::ask_prompt::PromptInput;
use super::ask_reply::{build_sql_reply_schema, ChoiceKind, Clarify, Term};
use super::ask_run::build_step;
use super::ask_values::ValueIndex;
use super::ask_why_prompt::{build_why_form_schema, build_why_prompt};
use super::explain_breakdown::{
    build_breakdown, read_slice_totals, run_code_query, Breakdown, CodeQuery, QueryEnv,
};
use super::explain_check::{build_chosen_base, build_why_check, PlanSlices, WhyPlan, WhyProfile};
use super::explain_choices::{build_why_assumptions, BASE_CHOICE_ID};
use super::explain_form::{build_form_from_reply, build_why_repair_prompt, FieldError, ReadForm};
use super::explain_plain::run_plain;
use super::explain_sql::{build_slices_sql, build_why_sql};
use crate::engine::guard::AllowedTable;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::ProfileColumn;
use crate::error::AppError;

/// Lời cho needs_choice, như mẫu needs_choice của ask_examples.rs.
const NEEDS_CHOICE_MESSAGE: &str = "Câu hỏi hiểu được theo nhiều cách. Chọn một cách rồi hỏi lại.";

/// Đầu vào của run_why, trừ mô hình. #33 đọc từ Postgres và AppState rồi truyền vào.
pub struct WhyInput<'a> {
    /// Phần chung với prompt câu thường: bảng, câu hỏi, ngữ cảnh (#28), chỉ số, ví dụ, gói ngành, lựa chọn của người
    /// hỏi. Prompt form không dùng ví dụ; prompt SQL của đường dự phòng dùng.
    pub prompt: PromptInput<'a>,
    /// Hồ sơ cột của phiên bản (Store::list_columns).
    pub columns: &'a [ProfileColumn],
    /// Giá trị cột chiều của phiên bản (load_value_index).
    pub values: &'a ValueIndex,
    /// DatasetVersion::row_count.
    pub row_count: Option<i64>,
    /// Bảng được phép, kèm bộ lọc dòng theo vai dữ liệu của người hỏi.
    pub tables: &'a [AllowedTable],
    pub max_rows: u64,
    /// Thời gian tối đa mỗi câu SQL (Config::query_timeout).
    pub timeout: Duration,
    pub jobs: &'a Arc<Semaphore>,
    /// Thời gian tối đa chờ mô hình viết lời (bước narrate).
    pub narrate_timeout: Duration,
}

/// Phần của AskResponse do đường câu "tại sao" quyết định, cùng nghĩa các trường như RunOutcome.
#[derive(Debug)]
pub struct WhyOutcome {
    pub status: AskStatus,
    pub message: Option<String>,
    /// Bảng tổng theo kỳ do code dựng; câu của mô hình khi chạy như câu thường.
    pub sql_generated: Option<String>,
    pub sql_guarded: Option<String>,
    pub result: Option<ResultTable>,
    pub explanation: Option<Explanation>,
    /// Form đã chạy, nguyên object JSON của mô hình, để ask_why.rs lưu vào queries.explanation cùng Explanation.
    /// None khi chạy như câu thường.
    pub form: Option<serde_json::Value>,
    /// Form đã kiểm, cho thẻ "Đã hiểu là" (understanding_why.rs). None khi chạy như câu thường.
    pub plan: Option<WhyPlan>,
    /// Như RunOutcome::limit_kept; câu do code dựng không có LIMIT nên luôn false.
    pub limit_kept: bool,
    /// Như RunOutcome::row_filters: điều kiện phân quyền guard thêm vào sql_guarded.
    pub row_filters: Vec<String>,
    pub terms: Vec<Term>,
    /// Giả định đã chọn sẵn: kỳ so sánh (why:base), chỉ số chưa xác nhận.
    pub assumptions: Vec<Choice>,
    /// Câu hỏi lại; chỉ có khi needs_choice.
    pub clarifications: Vec<Choice>,
    pub warnings: Vec<Warning>,
    pub steps: Vec<AskStep>,
}

/// Một lượt đọc và chạy form.
enum Round {
    Done(Box<WhyOutcome>),
    /// Form sai; lỗi gửi lại cho mô hình.
    Invalid(Vec<FieldError>),
}

/// Err như run_with_repair: lỗi máy chủ, và lỗi của mô hình khi chưa có kết quả nào (502, 503).
pub async fn run_why(llm: &dyn LlmProvider, input: WhyInput<'_>) -> Result<WhyOutcome, AppError> {
    if let Some(outcome) = build_ambiguous_outcome(&input) {
        return Ok(outcome);
    }
    let mut steps = Vec::new();
    let prompt = build_why_prompt(&input.prompt);
    let started = Instant::now();
    let reply = complete(llm, &input, LlmTask::Why, &prompt).await?;
    steps.push(build_step(StepName::Llm, started, true, None));
    let errors = match run_round(llm, &input, &reply, &mut steps).await? {
        Round::Done(outcome) => return Ok(*outcome),
        Round::Invalid(errors) => errors,
    };
    log_errors(&errors, "why form has errors, asking the model to fix it");
    let started = Instant::now();
    let repair_prompt = build_why_repair_prompt(&prompt, &reply, &errors);
    let reply = complete(llm, &input, LlmTask::Why, &repair_prompt).await?;
    let detail = "Mô hình sửa form theo lỗi của từng trường";
    steps.push(build_step(StepName::Repair, started, true, Some(detail)));
    match run_round(llm, &input, &reply, &mut steps).await? {
        Round::Done(outcome) => Ok(*outcome),
        Round::Invalid(errors) => {
            log_errors(
                &errors,
                "why form still has errors, answering as a plain question",
            );
            run_plain(llm, &input, steps).await
        }
    }
}

pub(super) async fn complete(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    task: LlmTask,
    prompt: &str,
) -> Result<String, AppError> {
    let json_schema = match task {
        LlmTask::Why => build_why_form_schema(),
        _ => build_sql_reply_schema(),
    };
    llm.complete(&LlmRequest {
        task,
        question: input.prompt.question,
        prompt,
        json_schema: Some(json_schema),
    })
    .await
}

// Chỉ ghi trường và lời tiếng Anh của code, không ghi câu hỏi (chữ người dùng gõ).
fn log_errors(errors: &[FieldError], message: &str) {
    let fields: Vec<String> = errors
        .iter()
        .map(|e| format!("{}: {}", e.field, e.message))
        .collect();
    tracing::warn!(errors = ?fields, "{message}");
}

/// Đọc, kiểm, chạy một câu trả lời form. Invalid khi form sai hoặc một lát cắt không có dòng nào.
async fn run_round(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    reply: &str,
    steps: &mut Vec<AskStep>,
) -> Result<Round, AppError> {
    let read = match build_form_from_reply(reply) {
        Ok(read) => read,
        Err(error) => return Ok(Round::Invalid(vec![error])),
    };
    if let Some(outcome) = build_measure_choice(input, &read, steps) {
        return Ok(Round::Done(Box::new(outcome)));
    }
    let profile = WhyProfile {
        table: input.prompt.table,
        columns: input.columns,
        values: input.values,
        metrics: input.prompt.metrics,
        row_count: input.row_count,
    };
    let plan = match build_why_check(&read.form, &profile, find_base_choice(input)) {
        Ok(plan) => plan,
        Err(errors) => return Ok(Round::Invalid(errors)),
    };
    run_plan(llm, input, read, &plan, steps).await
}

/// Phương án người hỏi gửi cho lựa chọn kỳ so sánh (explain_choices.rs), nếu có.
fn find_base_choice<'a>(input: &WhyInput<'a>) -> Option<&'a str> {
    input
        .prompt
        .choices
        .iter()
        .find(|c| c.choice_id == BASE_CHOICE_ID)
        .map(|c| c.option_id.as_str())
}

/// Chạy bảng tổng, rồi bước explain và narrate.
async fn run_plan(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    read: ReadForm,
    plan: &WhyPlan,
    steps: &mut Vec<AskStep>,
) -> Result<Round, AppError> {
    let env = QueryEnv {
        tables: input.tables,
        max_rows: input.max_rows,
        timeout: input.timeout,
        jobs: input.jobs,
    };
    let sql = build_why_sql(plan);
    let mut outcome = WhyOutcome {
        sql_generated: Some(sql.total_sql.clone()),
        form: Some(read.json),
        plan: Some(plan.clone()),
        assumptions: build_why_assumptions(
            input.prompt.context,
            input.prompt.metrics,
            &read.form.terms,
            plan,
        ),
        terms: read.form.terms,
        ..build_empty(AskStatus::Answered)
    };
    match run_code_query(&env, &sql.total_sql, Some(steps)).await? {
        CodeQuery::Ran {
            shown,
            result,
            row_filters,
        } => {
            outcome.sql_guarded = Some(shown);
            outcome.result = Some(result);
            outcome.row_filters = row_filters;
        }
        CodeQuery::Refused(reason) => {
            outcome.status = AskStatus::Rejected;
            outcome.message = Some(format!("Câu SQL bị từ chối: {reason}."));
            return Ok(Round::Done(Box::new(build_finished(outcome, steps))));
        }
        CodeQuery::Failed(message) => {
            outcome.status = AskStatus::Failed;
            outcome.message = Some(format!("Không chạy được câu SQL: {message}."));
            return Ok(Round::Done(Box::new(build_finished(outcome, steps))));
        }
    }
    let started = Instant::now();
    let totals = match run_code_query(&env, &build_slices_sql(plan), None).await? {
        CodeQuery::Ran { shown, result, .. } => read_slice_totals(shown, &result),
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(%reason, "slice totals query failed");
            None
        }
    };
    let Some(totals) = totals else {
        let reason = "Không tính được tổng của hai lát cắt".to_string();
        return Ok(build_unavailable(outcome, steps, started, reason));
    };
    // Lát cắt không có dòng nào: kỳ thủng trong dữ liệu, hay bộ lọc bỏ hết dòng. Mô hình sửa được (chọn kỳ khác, bỏ
    // bộ lọc), nên là lỗi của form, không phải phân rã rỗng.
    let empty = build_empty_slice_errors(
        plan,
        totals.base_rows.is_zero(),
        totals.compare_rows.is_zero(),
    );
    if let Some(reason) = build_chosen_base_empty(input, plan, &empty) {
        return Ok(build_unavailable(outcome, steps, started, reason));
    }
    if !empty.is_empty() {
        let detail = "Một lát cắt không có dòng dữ liệu nào";
        steps.push(build_step(StepName::Explain, started, false, Some(detail)));
        return Ok(Round::Invalid(empty));
    }
    let filters = (sql.base_filter, sql.compare_filter);
    let mut explanation = match build_breakdown(&env, plan, &totals, filters).await? {
        Breakdown::Ready(explanation) => *explanation,
        Breakdown::Unavailable(reason) => {
            return Ok(build_unavailable(outcome, steps, started, reason));
        }
    };
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    let detail = format!(
        "Phân rã theo {} chiều: {}",
        columns.len(),
        columns.join(", ")
    );
    steps.push(build_step(StepName::Explain, started, true, Some(&detail)));
    let narrated = narrate(
        llm,
        input.prompt.question,
        &explanation,
        input.narrate_timeout,
    )
    .await;
    explanation.summary = narrated.summary;
    explanation.summary_by_model = narrated.summary_by_model;
    steps.push(narrated.step);
    outcome.explanation = Some(explanation);
    Ok(Round::Done(Box::new(build_finished(outcome, steps))))
}

fn build_empty_slice_errors(
    plan: &WhyPlan,
    base_empty: bool,
    compare_empty: bool,
) -> Vec<FieldError> {
    let (base_field, compare_field, base, compare) = match &plan.slices {
        PlanSlices::Periods { base, compare, .. } => {
            ("base", "compare", base.text(), compare.text())
        }
        PlanSlices::Groups { base, compare, .. } => (
            "groups.base",
            "groups.compare",
            base.clone(),
            compare.clone(),
        ),
    };
    let mut errors = Vec::new();
    for (empty, field, slice) in [
        (base_empty, base_field, base),
        (compare_empty, compare_field, compare),
    ] {
        if empty {
            errors.push(FieldError::new(
                field,
                format!("no rows match {slice} with the filters of the form; choose a slice that has data"),
            ));
        }
    }
    errors
}

/// Lý do cho cảnh báo khi lỗi duy nhất là kỳ so sánh trống và kỳ đó do người hỏi chọn (why:base đè base của form, xem
/// build_why_check): gọi lại mô hình không đổi được kỳ đó. None khi phải để mô hình sửa form như mọi lỗi khác.
fn build_chosen_base_empty(
    input: &WhyInput<'_>,
    plan: &WhyPlan,
    errors: &[FieldError],
) -> Option<String> {
    let PlanSlices::Periods { base, compare, .. } = &plan.slices else {
        return None;
    };
    let only_base = matches!(errors, [error] if error.field == "base");
    let chosen = build_chosen_base(*compare, find_base_choice(input)) == Some(*base);
    (only_base && chosen).then(|| {
        format!(
            "{}, kỳ so sánh đã chọn, không có dòng dữ liệu nào",
            base.label()
        )
    })
}

/// Có bảng tổng mà không có phân rã: câu vẫn answered, cảnh báo nói lý do.
fn build_unavailable(
    mut outcome: WhyOutcome,
    steps: &mut Vec<AskStep>,
    started: Instant,
    reason: String,
) -> Round {
    steps.push(build_step(StepName::Explain, started, false, Some(&reason)));
    outcome.warnings.push(Warning {
        code: WarningCode::ExplanationUnavailable,
        message: format!("{reason}, nên kết quả chưa có phần giải thích."),
    });
    Round::Done(Box::new(build_finished(outcome, steps)))
}

fn build_finished(mut outcome: WhyOutcome, steps: &mut Vec<AskStep>) -> WhyOutcome {
    outcome.steps = std::mem::take(steps);
    outcome
}

pub(super) fn build_empty(status: AskStatus) -> WhyOutcome {
    WhyOutcome {
        status,
        message: None,
        sql_generated: None,
        sql_guarded: None,
        result: None,
        explanation: None,
        form: None,
        plan: None,
        limit_kept: false,
        row_filters: Vec::new(),
        terms: Vec::new(),
        assumptions: Vec::new(),
        clarifications: Vec::new(),
        warnings: Vec::new(),
        steps: Vec::new(),
    }
}

/// needs_choice trước khi gọi mô hình: cụm khớp nhiều cột gần nhau mà người hỏi chưa chọn. Id là id của #28 (cụm
/// đã bỏ dấu), để lần hỏi sau build_ask_context áp đúng lựa chọn; phương án là các cột #28 đã tìm thấy trong hồ sơ.
fn build_ambiguous_outcome(input: &WhyInput<'_>) -> Option<WhyOutcome> {
    let clarifications: Vec<Choice> = input
        .prompt
        .context
        .ambiguous
        .iter()
        .map(|word| Choice {
            id: word.choice_id.clone(),
            label: format!("\"{}\" là cột nào?", word.phrase),
            selected: None,
            options: word
                .columns
                .iter()
                .map(|column| ChoiceOption {
                    id: column.clone(),
                    label: column.clone(),
                })
                .collect(),
        })
        .collect();
    if clarifications.is_empty() {
        return None;
    }
    Some(WhyOutcome {
        message: Some(NEEDS_CHOICE_MESSAGE.to_string()),
        clarifications,
        ..build_empty(AskStatus::NeedsChoice)
    })
}

/// needs_choice sau khi gọi mô hình: form không nêu độ đo ("vì sao giảm?") mà dataset có chỉ số. Không chỉ số nào thì
/// để kiểm form báo lỗi trường measure như mọi lỗi khác.
fn build_measure_choice(
    input: &WhyInput<'_>,
    read: &ReadForm,
    steps: &mut Vec<AskStep>,
) -> Option<WhyOutcome> {
    let form = &read.form;
    let has_metric = form.metric.as_deref().is_some_and(|m| !m.trim().is_empty());
    if has_metric || form.measure.is_some() {
        return None;
    }
    let clarify = Clarify {
        kind: ChoiceKind::Metric,
        phrase: None,
        question: "Bạn muốn giải thích chỉ số nào?".to_string(),
        options: input
            .prompt
            .metrics
            .iter()
            .map(|m| m.name.clone())
            .collect(),
    };
    let choice = build_clarify_choice(&clarify, input.prompt.context, input.prompt.metrics)?;
    Some(WhyOutcome {
        message: Some(clarify.question),
        clarifications: vec![choice],
        terms: form.terms.clone(),
        steps: std::mem::take(steps),
        ..build_empty(AskStatus::NeedsChoice)
    })
}
