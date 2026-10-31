// SPDX-License-Identifier: Apache-2.0
/* service/explain/run.rs - đường của câu "tại sao" (#38), từ câu hỏi tới bảng, phân rã và lời: mô hình điền form
(ask/why_prompt.rs), code kiểm form (explain/check.rs), dựng SQL (explain/sql.rs), chạy qua guard, phân rã
(explain/breakdown.rs) và nhờ mô hình viết lời (ask/narrate.rs). Endpoint hỏi gọi run_why qua ask/why.rs khi bước
classify (ask/classify.rs) ra câu hỏi tìm nguyên nhân, thay cho bước llm và run_with_repair của câu thường, rồi dựng
AskResponse từ WhyOutcome.
Nhận mô hình và dữ liệu đã đọc qua tham số, như run_with_repair, nên test chạy được với mô hình giả và Parquet tạm,
không cần Postgres; test ở explain/run_tests.rs.

Vòng sửa: form sai (không đọc được, cột lạ, vai trò sai, giá trị không có, kỳ không có dòng) thì gửi lỗi từng trường
cho mô hình sửa một lần. Sai lần hai thì chạy như câu thường (explain/plain.rs) kèm cảnh báo
explanation_unavailable: người hỏi vẫn có bảng số, câu vẫn answered nếu SQL thường chạy được. Câu SQL do code dựng mà
guard từ chối hay DuckDB lỗi thì không sửa: mô hình không viết câu đó, sửa form không đổi được nó. Cũng không sửa khi
kỳ so sánh do người hỏi chọn (why:base) không có dòng nào: lựa chọn đè lên base của form, mô hình sửa form cũng không
đổi được kỳ đó, nên trả ngay bảng tổng kèm explanation_unavailable; giả định why:base vẫn trên thẻ để chọn kỳ khác.

Hỏi lại (needs_choice) trước khi gọi mô hình khi một cụm khớp nhiều cột gần nhau (AskContext::ambiguous, #28; endpoint
có thể đã hỏi lại trước, kiểm lại ở đây để run_why đúng khi gọi riêng), và sau khi gọi khi form không nêu độ đo mà
dataset có chỉ số để chọn. Form báo dữ liệu không có thông tin (unanswerable, #164) thì trả rejected ngay, không kiểm
form, không chạy SQL. Giả định đổi được (kỳ so sánh, chỉ số chưa xác nhận) dựng ở explain/choices.rs. */

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use crate::engine::guard::AllowedTable;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::ProfileColumn;
use crate::error::AppError;
use crate::service::ask::choices::build_clarify_choice;
use crate::service::ask::contract::{
    AskStatus, AskStep, Choice, ChoiceOption, Explanation, ResultTable, StepName, Warning,
    WarningCode,
};
use crate::service::ask::failures::{FailedStep, StepFailure};
use crate::service::ask::narrate::narrate;
use crate::service::ask::prompt::{build_examples_detail, pick_examples, PromptInput};
use crate::service::ask::reply::{build_sql_reply_schema, ChoiceKind, Clarify, Term};
use crate::service::ask::run::build_step;
use crate::service::ask::unanswerable::build_unanswerable_message;
use crate::service::ask::values::ValueIndex;
use crate::service::ask::why_prompt::{build_why_form_schema, build_why_prompt};
use crate::service::explain::breakdown::{
    build_breakdown, read_slice_totals, run_code_query, Breakdown, CodeQuery, QueryEnv,
    SliceTotals, ASK_STOP_HINT,
};
use crate::service::explain::check::{
    build_chosen_base, build_why_check, PlanSlices, WhyPlan, WhyProfile,
};
use crate::service::explain::choices::{build_why_assumptions, BASE_CHOICE_ID};
use crate::service::explain::form::{
    build_form_from_reply, build_why_repair_prompt, FieldError, ReadForm,
};
use crate::service::explain::latest::{apply_latest, find_latest, LatestPeriod, COMPARE_CHOICE_ID};
use crate::service::explain::plain::run_plain;
use crate::service::explain::premise::{build_premise_labels, build_premise_warning};
use crate::service::explain::sql::{build_slices_sql, build_why_sql};
use crate::service::explain::summary::apply_summary_parts;

/// Lời cho needs_choice, như mẫu needs_choice của ask/examples.rs.
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
    /// Form đã chạy, nguyên object JSON của mô hình, để ask/why.rs lưu vào queries.explanation cùng Explanation.
    /// None khi chạy như câu thường.
    pub form: Option<serde_json::Value>,
    /// Form đã kiểm, cho thẻ "Đã hiểu là" (understanding/why.rs). None khi chạy như câu thường.
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
    /// Form sai, lời diễn giải bị bộ kiểm chặn, và lỗi của đường dự phòng câu thường, theo thứ tự xảy ra; ask/why.rs ghi
    /// vào nhật ký lỗi (#228).
    pub failures: Vec<StepFailure>,
}

/// Một lượt đọc và chạy form.
enum Round {
    Done(Box<WhyOutcome>),
    /// Form sai; lỗi gửi lại cho mô hình.
    Invalid(Vec<FieldError>),
}

/// Err như run_with_repair: lỗi máy chủ, và lỗi của mô hình khi chưa có kết quả nào (502, 503).
pub async fn run_why(llm: &dyn LlmProvider, input: WhyInput<'_>) -> Result<WhyOutcome, AppError> {
    // Như run_with_repair (ask/run.rs): các vòng form ghi lỗi vào failures, lỗi của bước sau (narrate, đường dự phòng)
    // nằm sẵn trong outcome.failures; ghép lại theo thứ tự xảy ra.
    let mut failures = Vec::new();
    let mut outcome = run_why_rounds(llm, input, &mut failures).await?;
    failures.append(&mut outcome.failures);
    outcome.failures = failures;
    Ok(outcome)
}

async fn run_why_rounds(
    llm: &dyn LlmProvider,
    input: WhyInput<'_>,
    failures: &mut Vec<StepFailure>,
) -> Result<WhyOutcome, AppError> {
    if let Some(outcome) = build_ambiguous_outcome(&input) {
        return Ok(outcome);
    }
    let mut steps = Vec::new();
    let prompt = build_why_prompt(&input.prompt);
    let started = Instant::now();
    let reply = complete(llm, &input, LlmTask::Why, &prompt).await?;
    let examples_used = build_examples_detail(
        &pick_examples(&input.prompt, true),
        input.prompt.lessons.len(),
    );
    steps.push(build_step(
        StepName::Llm,
        started,
        true,
        examples_used.as_deref(),
    ));
    let errors = match run_round(llm, &input, &reply, &mut steps).await? {
        Round::Done(outcome) => return Ok(*outcome),
        Round::Invalid(errors) => errors,
    };
    log_errors(&errors, "why form has errors, asking the model to fix it");
    failures.push(build_form_failure(&errors, &reply));
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
            failures.push(build_form_failure(&errors, &reply));
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

/// Dòng nhật ký cho một lần form sai: lỗi từng trường, và nguyên câu trả lời của mô hình.
fn build_form_failure(errors: &[FieldError], reply: &str) -> StepFailure {
    let fields: Vec<String> = errors
        .iter()
        .map(|e| format!("{}: {}", e.field, e.message))
        .collect();
    StepFailure::new(FailedStep::WhyForm, &fields.join("; "), Some(reply))
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
    let mut read = match build_form_from_reply(reply) {
        Ok(read) => read,
        Err(error) => return Ok(Round::Invalid(vec![error])),
    };
    if let Some(outcome) = build_unanswerable_outcome(input, &read, steps) {
        return Ok(Round::Done(Box::new(outcome)));
    }
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
    // Câu không nêu kỳ: code chọn kỳ gần nhất có dữ liệu và ghi vào form trước khi kiểm (explain/latest.rs).
    let chosen_compare = find_chosen(input, COMPARE_CHOICE_ID);
    let latest = find_latest(
        &build_env(input),
        input.prompt.table,
        input.columns,
        &read.form,
        chosen_compare,
    )
    .await?;
    if let Some(latest) = &latest {
        apply_latest(&mut read, latest);
    }
    let plan = match build_why_check(&read.form, &profile, find_base_choice(input)) {
        Ok(plan) => plan,
        Err(errors) => return Ok(Round::Invalid(errors)),
    };
    run_plan(llm, input, read, &plan, latest.as_ref(), steps).await
}

/// Phương án người hỏi gửi cho lựa chọn kỳ so sánh (explain/choices.rs), nếu có.
fn find_base_choice<'a>(input: &WhyInput<'a>) -> Option<&'a str> {
    find_chosen(input, BASE_CHOICE_ID)
}

fn find_chosen<'a>(input: &WhyInput<'a>, choice_id: &str) -> Option<&'a str> {
    input
        .prompt
        .choices
        .iter()
        .find(|c| c.choice_id == choice_id)
        .map(|c| c.option_id.as_str())
}

fn build_env<'a>(input: &WhyInput<'a>) -> QueryEnv<'a> {
    QueryEnv {
        tables: input.tables,
        max_rows: input.max_rows,
        timeout: input.timeout,
        jobs: input.jobs,
        stop_hint: ASK_STOP_HINT,
    }
}

/// Chạy bảng tổng, rồi bước explain và narrate.
async fn run_plan(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    read: ReadForm,
    plan: &WhyPlan,
    latest: Option<&LatestPeriod>,
    steps: &mut Vec<AskStep>,
) -> Result<Round, AppError> {
    let env = build_env(input);
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
            latest,
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
    // Kiểm tiền đề trước phân rã: chênh lệch bằng 0 thì không có phân rã, nhưng người hỏi "vì sao giảm" vẫn cần biết.
    let labels = build_premise_labels(plan);
    // Độ đo tỷ lệ: base, compare của totals là tử số; so hướng phải trên tỷ lệ, vì tử số có thể tăng khi tỷ lệ giảm.
    let premise = match build_premise_values(&totals) {
        Some((base, compare)) => {
            build_premise_warning(input.prompt.question, &labels, base, compare)
        }
        None => None,
    };
    outcome.warnings.extend(premise);
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
    apply_summary_parts(&mut explanation);
    let narrated = narrate(
        llm,
        input.prompt.question,
        &explanation,
        input.narrate_timeout,
        input.prompt.lessons,
    )
    .await;
    outcome.failures.extend(narrated.failure);
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
        failures: Vec::new(),
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

/// rejected khi form báo dữ liệu không có thông tin. form để None: không có form nào chạy, queries.explanation trống
/// như câu bị từ chối của đường câu thường.
fn build_unanswerable_outcome(
    input: &WhyInput<'_>,
    read: &ReadForm,
    steps: &mut Vec<AskStep>,
) -> Option<WhyOutcome> {
    let unanswerable = read.form.unanswerable.as_ref().filter(|u| u.is_given())?;
    // Form đã nêu độ đo hay chỉ số thì câu trả lời được bằng phân rã: mô hình đôi khi vẫn báo unanswerable vì dữ liệu
    // "không ghi lý do", trong khi câu "tại sao" ở đây không cần cột lý do. Bỏ qua để chạy phân rã như thường.
    let has_metric = read
        .form
        .metric
        .as_deref()
        .is_some_and(|m| !m.trim().is_empty());
    if has_metric || read.form.measure.is_some() {
        return None;
    }
    let names: Vec<&str> = input
        .prompt
        .context
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    Some(WhyOutcome {
        message: Some(build_unanswerable_message(unanswerable, &names)),
        terms: read.form.terms.clone(),
        steps: std::mem::take(steps),
        ..build_empty(AskStatus::Rejected)
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

/// Giá trị hai lát cắt để kiểm tiền đề: tổng như đã đọc, hay tỷ lệ tử trên mẫu làm tròn RATE_SCALE chữ số khi độ đo là
/// tỷ lệ. None khi một mẫu số bằng 0 (không có tỷ lệ để so).
fn build_premise_values(
    totals: &SliceTotals,
) -> Option<(rust_decimal::Decimal, rust_decimal::Decimal)> {
    match totals.denominators {
        None => Some((totals.base, totals.compare)),
        Some((d0, d1)) => {
            let r0 = totals
                .base
                .checked_div(d0)?
                .round_dp(crate::service::explain::ratio::RATE_SCALE);
            let r1 = totals
                .compare
                .checked_div(d1)?
                .round_dp(crate::service::explain::ratio::RATE_SCALE);
            Some((r0, r1))
        }
    }
}

#[cfg(test)]
mod premise_values_tests {
    use super::*;
    use rust_decimal::Decimal;

    fn totals(base: &str, compare: &str, denominators: Option<(&str, &str)>) -> SliceTotals {
        let d = |t: &str| t.parse::<Decimal>().unwrap();
        SliceTotals {
            sql: String::new(),
            base: d(base),
            compare: d(compare),
            base_rows: d("1"),
            compare_rows: d("1"),
            volume: None,
            denominators: denominators.map(|(a, b)| (d(a), d(b))),
        }
    }

    #[test]
    fn totals_are_compared_as_read() {
        let values = build_premise_values(&totals("100", "150", None)).unwrap();
        assert_eq!(values, ("100".parse().unwrap(), "150".parse().unwrap()));
    }

    // Tử số tăng (40 lên 45) nhưng tỷ lệ giảm (0.4 xuống 0.3): kiểm tiền đề phải thấy tỷ lệ giảm.
    #[test]
    fn ratios_are_compared_not_numerators() {
        let (base, compare) =
            build_premise_values(&totals("40", "45", Some(("100", "150")))).unwrap();
        assert!(compare < base);
    }

    #[test]
    fn zero_denominator_has_no_values() {
        assert!(build_premise_values(&totals("1", "2", Some(("0", "5")))).is_none());
    }
}
