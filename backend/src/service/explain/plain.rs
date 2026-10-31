// SPDX-License-Identifier: Apache-2.0
/* service/explain/plain.rs - đường dự phòng của câu "tại sao": form sai cả sau một lần sửa thì hỏi như câu thường
(prompt SQL của ask/prompt.rs, run_with_repair của ask/run.rs), để người hỏi vẫn có bảng số. explain/run.rs gọi
run_plain; kết quả kèm cảnh báo explanation_unavailable, không có phân rã. Vòng sửa SQL của câu thường vẫn có (tối đa
một lần), vì đó là câu khác với form vừa sai. Test ở explain/run_tests.rs. */

use std::time::Instant;

use crate::engine::llm::{LlmProvider, LlmTask};
use crate::error::AppError;
use crate::service::ask::choices::{build_assumption_choices, build_clarify_choice};
use crate::service::ask::contract::{AskStatus, AskStep, StepName, Warning, WarningCode};
use crate::service::ask::prompt::{build_examples_detail, build_sql_prompt, pick_examples};
use crate::service::ask::reply::SqlAnswer;
use crate::service::ask::run::{build_step, run_with_repair, RunInput, RunOutcome};
use crate::service::ask::unanswerable::build_unanswerable_message;
use crate::service::explain::run::{build_empty, complete, WhyInput, WhyOutcome};

/// Form sai cả hai lần: hỏi như câu thường, kèm cảnh báo explanation_unavailable.
pub async fn run_plain(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    mut steps: Vec<AskStep>,
) -> Result<WhyOutcome, AppError> {
    let prompt = build_sql_prompt(&input.prompt);
    let started = Instant::now();
    let reply = complete(llm, input, LlmTask::Sql, &prompt).await?;
    let examples_used = build_examples_detail(
        &pick_examples(&input.prompt, false),
        input.prompt.lessons.len(),
    );
    steps.push(build_step(
        StepName::Llm,
        started,
        true,
        examples_used.as_deref(),
    ));
    let sensitive: Vec<String> = input
        .columns
        .iter()
        .filter(|c| c.is_sensitive)
        .map(|c| c.name.clone())
        .collect();
    let run = run_with_repair(
        llm,
        RunInput {
            question: input.prompt.question,
            prompt: &prompt,
            reply: &reply,
            // Prompt dự phòng chỉ có cột của bảng đang chọn (input.tables[0]): bảng liên quan chỉ dùng cho phân rã do
            // code dựng, nên câu của mô hình chỉ được đọc bảng đó và không có quan hệ nào để kiểm cộng lặp.
            tables: &input.tables[..1],
            links: &[],
            max_rows: input.max_rows,
            timeout: input.timeout,
            jobs: input.jobs,
            sensitive: &sensitive,
        },
    )
    .await?;
    Ok(build_plain_outcome(input, run, steps))
}

fn build_plain_outcome(
    input: &WhyInput<'_>,
    run: RunOutcome,
    mut steps: Vec<AskStep>,
) -> WhyOutcome {
    steps.extend(run.steps);
    let context = input.prompt.context;
    let metrics = input.prompt.metrics;
    let mut outcome = WhyOutcome {
        message: run.message,
        sql_guarded: run.sql_guarded,
        result: run.result,
        limit_kept: run.limit_kept,
        row_filters: run.row_filters,
        steps,
        failures: run.failures,
        ..build_empty(run.status)
    };
    if let Some(unanswerable) = &run.unanswerable {
        let names: Vec<&str> = context.columns.iter().map(|c| c.name.as_str()).collect();
        outcome.message = Some(build_unanswerable_message(unanswerable, &names));
    }
    if let Some(clarify) = &run.clarify {
        // Phương án nào cũng không có thật thì câu hỏi lại vô dụng: coi như mô hình trả lời sai.
        match build_clarify_choice(clarify, context, metrics) {
            Some(choice) => outcome.clarifications.push(choice),
            None => {
                outcome.status = AskStatus::Failed;
                outcome.message = Some(
                    "Mô hình hỏi lại nhưng không đưa phương án nào có trong dữ liệu. Thử hỏi lại theo cách khác."
                        .to_string(),
                );
            }
        }
    }
    if let Some(sql) = run.sql_generated {
        let answer = SqlAnswer {
            terms: run.terms.clone(),
            assumptions: run.assumptions,
            sql: sql.clone(),
        };
        outcome.assumptions = build_assumption_choices(&answer, context, metrics);
        outcome.sql_generated = Some(sql);
    }
    outcome.terms = run.terms;
    outcome.warnings.push(Warning {
        code: WarningCode::ExplanationUnavailable,
        message: "Không hiểu được câu hỏi thành hai kỳ hay hai nhóm để so sánh, nên kết quả chưa có phần giải thích."
            .to_string(),
    });
    outcome
}
