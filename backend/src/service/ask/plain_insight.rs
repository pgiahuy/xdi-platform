// SPDX-License-Identifier: Apache-2.0
/* service/ask/plain_insight.rs - nối luồng hỏi câu thường (ask/mod.rs) với phần giải thích theo hình dạng kết quả
(service/insight/, #227): dựng đầu vào từ nhóm bảng của câu hỏi (ask/related.rs), cấu hình và câu đã chạy, và đóng gói
phân rã tự động để ghi vào queries.explanation cùng khuôn với câu "tại sao" (ask/why.rs), để truy dòng nguồn của một
dòng phân rã (service/drill/) chạy như nhau. */

use crate::error::AppError;
use crate::service::ask::context::AskContext;
use crate::service::ask::contract::{AskStatus, Explanation, Understanding};
use crate::service::ask::fanout::build_join_links;
use crate::service::ask::related::TableGroup;
use crate::service::ask::run::RunOutcome;
use crate::service::ask::why::build_narrate_timeout;
use crate::service::explain::breakdown::{QueryEnv, ASK_STOP_HINT};
use crate::service::explain::lower_first;
use crate::service::insight::plan::PlainTable;
use crate::service::insight::{build_insight, ColumnReading, InsightInput, InsightOutcome};
use crate::service::understanding::labels::build_display_name;
use crate::state::AppState;

/// None khi câu chưa answered hay không có bảng kết quả. lessons là bài học đã duyệt liên quan câu hỏi, cho prompt
/// viết lời; context cho biết cụm nào ngữ cảnh đã tự chọn cột, để lời nói rõ đã tính theo cột nào.
pub(super) async fn run_insight(
    state: &AppState,
    group: &TableGroup,
    question: &str,
    outcome: &RunOutcome,
    understanding: Option<&Understanding>,
    context: &AskContext,
    lessons: &[String],
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
    // Quan hệ giữa các bảng, dựng lại từ nhóm như ask/mod.rs: rẻ, và hàm bớt một tham số.
    let links = build_join_links(group);
    let input = InsightInput {
        question,
        result,
        sql_generated,
        sql_guarded,
        limit_kept: outcome.limit_kept,
        understanding,
        tables: &tables,
        links: &links,
        env: QueryEnv {
            tables: &group.allowed,
            links: &links,
            max_rows: state.config.ask_max_rows,
            timeout: state.config.query_timeout,
            jobs: &state.query_jobs,
            stop_hint: ASK_STOP_HINT,
        },
        narrate: state.config.insight_text,
        narrate_timeout: build_narrate_timeout(&state.config),
        lessons,
        readings: &build_readings(context),
    };
    build_insight(state.llm.as_ref(), &input).await
}

/// Cụm ngữ cảnh đã tự chọn một trong nhiều cột (còn cột khác để đổi, người hỏi chưa chọn), với tên tiếng Việt của cột:
/// bí danh có dấu của hồ sơ, không có thì phần đầu của mô tả (build_display_name), cuối cùng là tên cột.
fn build_readings(context: &AskContext) -> Vec<ColumnReading> {
    context
        .picks
        .iter()
        .filter(|pick| !pick.others.is_empty() && !pick.chosen_by_user)
        .map(|pick| {
            let profile = context.columns.iter().find(|c| c.name == pick.column);
            let label = profile
                .and_then(|c| build_display_name(&c.name, &c.aliases, c.description.as_deref()))
                .unwrap_or_else(|| pick.column.clone());
            ColumnReading {
                phrase: pick.phrase.clone(),
                label: lower_first(&label),
            }
        })
        .collect()
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
