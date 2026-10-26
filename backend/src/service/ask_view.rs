// SPDX-License-Identifier: Apache-2.0
/* service/ask_view.rs - phần trình bày của một câu thường đã trả lời: thẻ "Đã hiểu là", vai trò từng cột kết quả và gợi ý
biểu đồ. service/ask.rs gọi sau run_with_repair (ask_run.rs). Thẻ và vai trò dựng bằng code từ sql_guarded
(engine/sql_shape.rs, understanding.rs), không nhờ mô hình, nên luôn khớp câu đã chạy; biểu đồ chọn ở ask_chart.rs
theo vai trò cột cùng kiểu và giá trị của cột.

Câu "tại sao" không đi qua đây: thẻ của nó dựng từ form đã kiểm (#38). */

use super::ask_chart::build_chart;
use super::ask_context::ColumnProfile;
use super::ask_contract::{AskStatus, ChartSuggestion, Choice, ColumnRole, Understanding};
use super::ask_run::RunOutcome;
use super::understanding::{build_column_roles, build_understanding};
use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::MetricDefinition;

/// Thẻ, vai trò cột (theo thứ tự cột của result) và biểu đồ của một câu đã trả lời.
pub struct AnswerView {
    pub understanding: Understanding,
    pub roles: Vec<ColumnRole>,
    pub chart: Option<ChartSuggestion>,
}

/// None khi câu chưa answered, hoặc khi không đọc được cây cú pháp của sql_guarded. Trường hợp sau là lỗi của code (guard
/// vừa dựng chính câu đó), nhưng thẻ chỉ để đọc: mất thẻ không làm sai số liệu, nên câu vẫn trả lời, kèm log để sửa.
/// - profile: hồ sơ cột trong ngữ cảnh (AskContext::columns), cho nhãn tiếng Việt của thẻ.
/// - assumptions: build_assumption_choices của ask_choices.rs; thẻ chép nguyên để cùng id với AskResponse.choices.
pub fn build_answer_view(
    outcome: &RunOutcome,
    profile: &[ColumnProfile],
    metrics: &[MetricDefinition],
    assumptions: &[Choice],
) -> Option<AnswerView> {
    if outcome.status != AskStatus::Answered {
        return None;
    }
    let (Some(sql_guarded), Some(result)) = (&outcome.sql_guarded, &outcome.result) else {
        return None;
    };
    let shape = match build_query_shape(sql_guarded) {
        Ok(shape) => shape,
        Err(e) => {
            tracing::warn!(error = ?e, "cannot read the guarded sql, answering without the understanding card");
            return None;
        }
    };
    let understanding = build_understanding(
        &shape,
        outcome.limit_kept,
        profile,
        metrics,
        &outcome.row_filters,
        assumptions,
    );
    let roles = build_column_roles(&shape, profile, result.columns.len());
    let chart = build_chart(result, &roles);
    Some(AnswerView {
        understanding,
        roles,
        chart,
    })
}
