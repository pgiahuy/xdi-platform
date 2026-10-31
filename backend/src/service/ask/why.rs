// SPDX-License-Identifier: Apache-2.0
/* service/ask/why.rs - nhánh câu "tại sao" của endpoint hỏi (#38). ask/mod.rs gọi ask_why khi bước classify
(ask/classify.rs) ra câu hỏi tìm nguyên nhân, thay cho bước llm và run_with_repair của câu thường. Ở đây: gọi run_why
(explain/run.rs: form, kiểm form, SQL do code dựng, phân rã, lời), dựng thẻ "Đã hiểu là", vai trò cột và biểu đồ
(ask/view.rs), ghi lịch sử kèm form và Explanation, rồi dựng AskResponse. Trạng thái và lỗi theo cùng quy tắc với câu
thường (đầu file ask/mod.rs); test qua route ở api/ask_why_tests.rs.

Dataset chụp từ database: form chỉ nêu cột của bảng đang hỏi, còn phân rã xét thêm cột của các bảng nối nhiều-một vào
nó (explain/source.rs, #227). tables_read và queries.read_versions ghi bảng đang hỏi cộng các bảng mà một chiều của phân
rã đã đọc.

queries.explanation lưu object {"form": <form mô hình điền>, "explanation": <Explanation>}. drill/mod.rs đọc điều kiện lát
cắt và câu SQL từng chiều trong "explanation" để truy dòng nguồn của một dòng phân rã; "form" giữ lại để xem câu hỏi
đã được hiểu thế nào. "explanation" là null khi có bảng tổng mà không phân rã được (cảnh báo explanation_unavailable).
Câu chạy như câu thường (form sai hai lần) không có form, nên cột để null như câu thường. */

use std::time::Duration;

use crate::auth::Member;
use crate::config::Config;
use crate::engine::guard::AllowedTable;
use crate::engine::store::{Dataset, DatasetVersion, ProfileColumn};
use crate::service::ask::checks::build_empty_warning;
use crate::service::ask::contract::{AskResponse, AskStatus, AskStep, Explanation, TableRead};
use crate::service::ask::prompt::PromptInput;
use crate::service::ask::related::TableGroup;
use crate::service::ask::response_choices::{build_unapplied_warnings, build_used_choices};
use crate::service::ask::values::load_value_index;
use crate::service::ask::view::{apply_answer_view, build_answer_view, RanQuery};
use crate::service::ask::{build_base_response, build_value_warnings, record_query, Recorded};
use crate::service::explain::run::{run_why, WhyInput, WhyOutcome};
use crate::service::explain::source::{build_tables_used, RelatedTables};
use crate::{error::AppError, state::AppState};

/// Những gì ask/mod.rs đã đọc và dựng trước khi rẽ nhánh.
pub(super) struct WhyAsk<'a> {
    pub dataset: &'a Dataset,
    pub version: &'a DatasetVersion,
    /// Phần chung với prompt câu thường; run_why dựng prompt form từ đây, đường dự phòng dựng prompt SQL.
    pub prompt: PromptInput<'a>,
    /// Hồ sơ cột của phiên bản (Store::list_columns).
    pub columns: &'a [ProfileColumn],
    /// Bảng được phép, đã có lọc dòng theo vai dữ liệu của người hỏi: mọi câu của run_why đi qua guard với nó.
    /// Gồm bảng đang hỏi và các bảng liên quan của nhóm (ask/related.rs).
    pub tables: &'a [AllowedTable],
    pub group: &'a TableGroup,
    /// Bảng liên quan nối nhiều-một vào bảng đang hỏi, cho phân rã (#227).
    pub related: &'a RelatedTables,
    /// Các bước đã chạy (context).
    pub steps: Vec<AskStep>,
}

pub(super) async fn ask_why(
    state: &AppState,
    member: &Member,
    ask: WhyAsk<'_>,
) -> Result<AskResponse, AppError> {
    let WhyAsk {
        dataset,
        version,
        prompt,
        columns,
        tables,
        group,
        related,
        mut steps,
    } = ask;
    // Các trường của prompt là tham chiếu; chép ra trước khi prompt chuyển vào WhyInput để dùng tiếp sau run_why.
    let (question, context, metrics, choices) = (
        prompt.question,
        prompt.context,
        prompt.metrics,
        prompt.choices,
    );
    let values = load_value_index(state, &member.workspace, version).await?;
    let input = WhyInput {
        prompt,
        columns,
        values: &values,
        row_count: version.row_count,
        tables,
        max_rows: state.config.ask_max_rows,
        timeout: state.config.query_timeout,
        jobs: &state.query_jobs,
        narrate_timeout: build_narrate_timeout(&state.config),
        related,
    };
    let mut outcome = run_why(state.llm.as_ref(), input).await?;
    steps.append(&mut outcome.steps);
    let used = build_used_choices(context, &outcome.assumptions);
    let mut warnings = build_unapplied_warnings(choices, &used);
    warnings.extend(build_value_warnings(context, &outcome.terms));
    // Trong các luật kiểm hợp lý (ask/checks.rs), câu "tại sao" chỉ nhận empty_result: SQL do code dựng từ form mà giá
    // trị lọc đã được kiểm với danh sách giá trị (explain/check.rs), và phân rã không được thì đã có
    // explanation_unavailable nói lý do. Bảng rỗng vẫn gặp được, nhất là khi câu chạy như câu thường.
    if outcome.status == AskStatus::Answered {
        warnings.extend(outcome.result.as_ref().and_then(build_empty_warning));
    }
    warnings.append(&mut outcome.warnings);

    if outcome.status == AskStatus::NeedsChoice {
        return Ok(AskResponse {
            message: outcome.message,
            choices: used,
            sql_generated: outcome.sql_generated,
            warnings,
            clarifications: outcome.clarifications,
            steps,
            ..build_base_response(state, version, question, AskStatus::NeedsChoice)
        });
    }

    let ran = RanQuery {
        status: outcome.status,
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        limit_kept: outcome.limit_kept,
        row_filters: &outcome.row_filters,
        plan: outcome.plan.as_ref(),
    };
    let view = build_answer_view(&ran, &context.columns, metrics, &outcome.assumptions);
    let (understanding, chart) = apply_answer_view(view, outcome.result.as_mut());

    let tables_read = build_why_tables_read(group, &dataset.logical_table, &outcome);
    let recorded = Recorded {
        status: outcome.status,
        message: outcome.message.as_deref(),
        sql_generated: outcome.sql_generated.as_deref(),
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        explanation: build_stored_explanation(&outcome)?,
        read_versions: group.build_read_versions(&tables_read),
    };
    let target = (dataset, version);
    let query_id = record_query(state, member, target, question, recorded, &mut steps).await?;
    Ok(AskResponse {
        message: outcome.message,
        query_id: Some(query_id),
        choices: used,
        sql_generated: outcome.sql_generated,
        sql_guarded: outcome.sql_guarded,
        tables_read: if outcome.status == AskStatus::Answered {
            tables_read
        } else {
            Vec::new()
        },
        understanding,
        result: outcome.result,
        chart,
        warnings,
        explanation: outcome.explanation,
        steps,
        ..build_base_response(state, version, question, outcome.status)
    })
}

/// Bảng câu "tại sao" đã đọc: bảng đang hỏi, cộng bảng liên quan mà một chiều của phân rã đã đọc (explain/source.rs).
fn build_why_tables_read(group: &TableGroup, table: &str, outcome: &WhyOutcome) -> Vec<TableRead> {
    let columns: Vec<&str> = outcome
        .explanation
        .iter()
        .flat_map(|e| e.dimensions.iter().map(|d| d.column.as_str()))
        .collect();
    let mut names = vec![table.to_string()];
    if let Some(plan) = &outcome.plan {
        names.extend(build_tables_used(&plan.source, &columns));
    }
    group.build_tables_read(&names)
}

/// Object lưu vào queries.explanation (khuôn ở đầu file); None khi câu chạy như câu thường.
fn build_stored_explanation(outcome: &WhyOutcome) -> Result<Option<serde_json::Value>, AppError> {
    let Some(form) = &outcome.form else {
        return Ok(None);
    };
    let stored = StoredWhy {
        form,
        explanation: outcome.explanation.as_ref(),
    };
    serde_json::to_value(stored)
        .map(Some)
        .map_err(|e| AppError::Internal(format!("serialize why explanation: {e}")))
}

#[derive(serde::Serialize)]
struct StoredWhy<'a> {
    form: &'a serde_json::Value,
    explanation: Option<&'a Explanation>,
}

/// Mỗi lần gọi mô hình đã bị giới hạn bằng timeout của nhà cung cấp (engine/llm). Bước narrate dùng đúng mức đó, để lời
/// diễn giải không bị cắt sớm hơn một lần gọi bình thường; quá giờ thì narrate dùng câu khuôn, câu hỏi vẫn answered.
pub(super) fn build_narrate_timeout(config: &Config) -> Duration {
    match config.llm_provider.as_str() {
        "ollama" => config.ollama_timeout,
        _ => config.gemini_timeout,
    }
}
