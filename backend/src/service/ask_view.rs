// SPDX-License-Identifier: Apache-2.0
/* service/ask_view.rs - phần trình bày của một câu đã trả lời: thẻ "Đã hiểu là", vai trò từng cột kết quả và gợi ý
biểu đồ. service/ask.rs gọi sau run_with_repair (ask_run.rs), ask_why.rs sau run_why (explain_run.rs). Thẻ và vai trò
dựng bằng code từ sql_guarded (engine/sql_shape.rs, understanding.rs), không nhờ mô hình, nên luôn khớp câu đã chạy;
biểu đồ theo vai trò cột. Câu "tại sao" có form đã kiểm thì thẻ dựng từ form (understanding_why.rs), vai trò cột và
biểu đồ vẫn theo cùng luật. */

use super::ask_context::ColumnProfile;
use super::ask_contract::{
    AskStatus, ChartKind, ChartSuggestion, Choice, ColumnRole, ResultTable, Understanding,
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
    let names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    let chart = build_chart(&names, &roles);
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

/// Luật ở ARCHITECTURE mục 6: một chiều một độ đo thì cột, có thời gian thì đường. Trục x phải là cột khóa duy nhất của
/// kết quả: hai khóa trở lên là bảng chéo, một biểu đồ hai trục không vẽ đúng được. Đường vẽ độ đo đầu, như mẫu why
/// của ask_examples.rs: các độ đo thường khác đơn vị (doanh thu và số đơn), vẽ chung một trục thì độ đo nhỏ thành đường
/// phẳng. Cột vai trò other không ảnh hưởng. None thì giao diện chỉ hiện bảng.
fn build_chart(names: &[&str], roles: &[ColumnRole]) -> Option<ChartSuggestion> {
    let keys: Vec<usize> = (0..roles.len())
        .filter(|&i| matches!(roles[i], ColumnRole::Dimension | ColumnRole::Time))
        .collect();
    let measures: Vec<usize> = (0..roles.len())
        .filter(|&i| roles[i] == ColumnRole::Measure)
        .collect();
    let ([key], [first_measure, ..]) = (keys.as_slice(), measures.as_slice()) else {
        return None;
    };
    let kind = match roles[*key] {
        ColumnRole::Time => ChartKind::Line,
        // Biểu đồ cột theo luật cần đúng một độ đo; không vẽ độ đo đầu như đường, vì người đọc bảng cột sẽ nghĩ các độ
        // đo kia bị bỏ sót.
        _ if measures.len() == 1 => ChartKind::Bar,
        _ => return None,
    };
    Some(ChartSuggestion {
        kind,
        x: names[*key].to_string(),
        y: vec![names[*first_measure].to_string()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ColumnRole::{Dimension, Measure, Other, Time};

    fn chart(roles: &[ColumnRole]) -> Option<(ChartKind, String, Vec<String>)> {
        let names: Vec<String> = (0..roles.len()).map(|i| format!("c{i}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        build_chart(&names, roles).map(|c| (c.kind, c.x, c.y))
    }

    #[test]
    fn one_dimension_and_one_measure_is_a_bar() {
        assert_eq!(
            chart(&[Dimension, Measure]),
            Some((ChartKind::Bar, "c0".to_string(), vec!["c1".to_string()]))
        );
        // Cột other (ví dụ một cột chữ) không đổi luật.
        assert_eq!(
            chart(&[Other, Measure, Dimension]),
            Some((ChartKind::Bar, "c2".to_string(), vec!["c1".to_string()]))
        );
    }

    #[test]
    fn time_axis_is_a_line_of_the_first_measure() {
        assert_eq!(
            chart(&[Time, Measure, Measure]),
            Some((ChartKind::Line, "c0".to_string(), vec!["c1".to_string()]))
        );
    }

    #[test]
    fn other_shapes_are_table_only() {
        assert_eq!(chart(&[Dimension, Measure, Measure]), None);
        assert_eq!(chart(&[Dimension, Time, Measure]), None);
        assert_eq!(chart(&[Dimension, Dimension]), None);
        assert_eq!(chart(&[Measure]), None);
        assert_eq!(chart(&[Other, Other]), None);
    }
}
