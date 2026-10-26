// SPDX-License-Identifier: Apache-2.0
/* service/explain_plain.rs - đường dự phòng của câu "tại sao": form sai cả sau một lần sửa thì hỏi như câu thường
(prompt SQL của ask_prompt.rs, run_with_repair của ask_run.rs), để người hỏi vẫn có bảng số. explain_run.rs gọi
run_plain; kết quả kèm cảnh báo explanation_unavailable, không có phân rã. Vòng sửa SQL của câu thường vẫn có (tối đa
một lần), vì đó là câu khác với form vừa sai. Test ở explain_run_tests.rs. */

use std::time::Instant;

use super::ask_choices::{build_assumption_choices, build_clarify_choice};
use super::ask_contract::{AskStatus, AskStep, StepName, Warning, WarningCode};
use super::ask_prompt::build_sql_prompt;
use super::ask_reply::SqlAnswer;
use super::ask_run::{build_step, run_with_repair, RunInput, RunOutcome};
use super::explain_run::{build_empty, complete, WhyInput, WhyOutcome};
use crate::engine::llm::{LlmProvider, LlmTask};
use crate::error::AppError;

/// Form sai cả hai lần: hỏi như câu thường, kèm cảnh báo explanation_unavailable.
pub async fn run_plain(
    llm: &dyn LlmProvider,
    input: &WhyInput<'_>,
    mut steps: Vec<AskStep>,
) -> Result<WhyOutcome, AppError> {
    let prompt = build_sql_prompt(&input.prompt);
    let started = Instant::now();
    let reply = complete(llm, input, LlmTask::Sql, &prompt).await?;
    steps.push(build_step(StepName::Llm, started, true, None));
    let run = run_with_repair(
        llm,
        RunInput {
            question: input.prompt.question,
            prompt: &prompt,
            reply: &reply,
            tables: input.tables,
            max_rows: input.max_rows,
            timeout: input.timeout,
            jobs: input.jobs,
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
        ..build_empty(run.status)
    };
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
