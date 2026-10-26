// SPDX-License-Identifier: Apache-2.0
/* service/ask_view.rs - phần trình bày của một câu đã trả lời: thẻ "Đã hiểu là", vai trò từng cột kết quả và gợi ý
biểu đồ. service/ask.rs gọi sau run_with_repair (ask_run.rs), ask_why.rs sau run_why (explain_run.rs). Thẻ và vai trò
dựng bằng code từ sql_guarded (engine/sql_shape.rs, understanding.rs), không nhờ mô hình, nên luôn khớp câu đã chạy;
biểu đồ theo bộ luật ở ask_chart.rs. Câu "tại sao" có form đã kiểm thì thẻ dựng từ form (understanding_why.rs), vai trò
cột và biểu đồ vẫn theo cùng luật. */

use super::ask_chart::build_chart;
use super::ask_context::ColumnProfile;
use super::ask_contract::{
    AskStatus, ChartSuggestion, Choice, ColumnRole, ResultTable, Understanding,
};
use super::explain_check::WhyPlan;
use super::understanding::{build_column_roles, build_understanding};
use super::understanding_why::build_why_understanding;
use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::MetricDefinition;

/// Thẻ, vai trò cột (theo thứ tự cột của result) và biểu đồ của một câu đã trả lời.
pub struct AnswerView {
    pub understanding: Understanding,
    pub roles: Vec<ColumnRole>,
    pub chart: Option<ChartSuggestion>,
}

/// Câu đã chạy mà phần trình bày đọc: các trường cùng tên của RunOutcome (câu thường) hay WhyOutcome (câu "tại sao").
pub struct RanQuery<'a> {
    pub status: AskStatus,
    pub sql_guarded: Option<&'a str>,
    pub result: Option<&'a ResultTable>,
    pub limit_kept: bool,
    pub row_filters: &'a [String],
    /// Form đã kiểm của câu "tại sao"; có thì thẻ dựng từ form. None với câu thường và câu "tại sao" chạy như câu thường.
    pub plan: Option<&'a WhyPlan>,
}

/// None khi câu chưa answered, hoặc khi không đọc được cây cú pháp của sql_guarded. Trường hợp sau là lỗi của code (guard
/// vừa dựng chính câu đó), nhưng thẻ chỉ để đọc: mất thẻ không làm sai số liệu, nên câu vẫn trả lời, kèm log để sửa.
/// - profile: hồ sơ cột trong ngữ cảnh (AskContext::columns), cho nhãn tiếng Việt của thẻ.
/// - assumptions: giả định đã dựng (build_assumption_choices của ask_choices.rs, hay WhyOutcome::assumptions); thẻ
///   chép nguyên để cùng id với AskResponse.choices.
pub fn build_answer_view(
    ran: &RanQuery,
    profile: &[ColumnProfile],
    metrics: &[MetricDefinition],
    assumptions: &[Choice],
) -> Option<AnswerView> {
    if ran.status != AskStatus::Answered {
        return None;
    }
    let (Some(sql_guarded), Some(result)) = (ran.sql_guarded, ran.result) else {
        return None;
    };
    let shape = match build_query_shape(sql_guarded) {
        Ok(shape) => shape,
        Err(e) => {
            tracing::warn!(error = ?e, "cannot read the guarded sql, answering without the understanding card");
            return None;
        }
    };
    let understanding = match ran.plan {
        Some(plan) => {
            build_why_understanding(plan, &shape, profile, metrics, ran.row_filters, assumptions)
        }
        None => build_understanding(
            &shape,
            ran.limit_kept,
            profile,
            metrics,
            ran.row_filters,
            assumptions,
        ),
    };
    let roles = build_column_roles(&shape, profile, result.columns.len());
    let chart = build_chart(&shape, result, &roles, profile);
    Some(AnswerView {
        understanding,
        roles,
        chart,
    })
}

/// Ghi vai trò vào từng cột của result, trả thẻ và biểu đồ cho AskResponse; view None thì cả hai None.
pub fn apply_answer_view(
    view: Option<AnswerView>,
    result: Option<&mut ResultTable>,
) -> (Option<Understanding>, Option<ChartSuggestion>) {
    let Some(view) = view else {
        return (None, None);
    };
    if let Some(result) = result {
        for (column, role) in result.columns.iter_mut().zip(&view.roles) {
            column.role = *role;
        }
    }
    (Some(view.understanding), view.chart)
}
