// SPDX-License-Identifier: Apache-2.0
/* service/ask/plain_insight.rs - nối luồng hỏi câu thường (ask/mod.rs) với phần giải thích theo hình dạng kết quả
(service/insight/, #227): dựng đầu vào từ nhóm bảng của câu hỏi (ask/related.rs), cấu hình và câu đã chạy, và đóng gói
phân rã tự động để ghi vào queries.explanation cùng khuôn với câu "tại sao" (ask/why.rs), để truy dòng nguồn của một
dòng phân rã (service/drill/) chạy như nhau. */

use crate::error::AppError;
use crate::service::ask::contract::{AskStatus, Explanation, Understanding};
use crate::service::ask::fanout::JoinLink;
use crate::service::ask::related::TableGroup;
use crate::service::ask::run::RunOutcome;
use crate::service::ask::why::build_narrate_timeout;
use crate::service::explain::breakdown::{QueryEnv, ASK_STOP_HINT};
use crate::service::insight::plan::PlainTable;
use crate::service::insight::{build_insight, InsightInput, InsightOutcome};
use crate::state::AppState;

/// None khi câu chưa answered hay không có bảng kết quả.
pub(super) async fn run_insight(
    state: &AppState,
    group: &TableGroup,
    links: &[JoinLink],
    question: &str,
    outcome: &RunOutcome,
    understanding: Option<&Understanding>,
) -> Result<Option<InsightOutcome>, AppError> {
    let (Some(result), Some(sql_generated), Some(sql_guarded)) = (
        outcome.result.as_ref(),
        outcome.sql_generated.as_deref(),
        outcome.sql_guarded.as_deref(),
    ) else {
        return Ok(None);
    };
    if outcome.status != AskStatus::Answered {
        return Ok(None);
    }
    let tables: Vec<PlainTable> = group
        .tables
        .iter()
        .map(|t| PlainTable {
            table: &t.dataset.logical_table,
            columns: &t.columns,
            row_count: t.version.row_count,
        })
        .collect();
    let input = InsightInput {
        question,
        result,
        sql_generated,
        sql_guarded,
        limit_kept: outcome.limit_kept,
        understanding,
        tables: &tables,
        links,
        env: QueryEnv {
            tables: &group.allowed,
            max_rows: state.config.ask_max_rows,
            timeout: state.config.query_timeout,
            jobs: &state.query_jobs,
            stop_hint: ASK_STOP_HINT,
        },
        narrate: state.config.insight_text,
        narrate_timeout: build_narrate_timeout(&state.config),
    };
    build_insight(state.llm.as_ref(), &input).await
}

/// Object ghi vào queries.explanation: cùng khuôn {"form", "explanation"} với câu "tại sao", form là null vì không có
/// form nào do mô hình điền.
pub(super) fn build_stored_breakdown(
    explanation: Option<&Explanation>,
) -> Result<Option<serde_json::Value>, AppError> {
    let Some(explanation) = explanation else {
        return Ok(None);
    };
    let json = serde_json::json!({
        "form": null,
        "explanation": serde_json::to_value(explanation)
            .map_err(|e| AppError::Internal(format!("serialize insight breakdown: {e}")))?,
    });
    Ok(Some(json))
}
