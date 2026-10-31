// SPDX-License-Identifier: Apache-2.0
/* service/understanding/why.rs - thẻ "Đã hiểu là" của câu "tại sao" (#38), dựng từ form đã kiểm (WhyPlan của
explain/check.rs) thay vì đoán nghĩa từ cây SQL: đọc theo cây thì vế `(<kỳ gốc> OR <kỳ so>)` của bảng tổng chỉ là một
điều kiện OR khó hiểu, còn plan biết đó là hai kỳ hay hai nhóm đang so. ask/view.rs gọi build_why_understanding khi
câu "tại sao" đã có bảng tổng; test ở understanding/tests.rs.

Mỗi UnderstoodItem.sql vẫn là đoạn nguyên văn của sql_guarded như ở understanding/mod.rs: đoạn được in từ đúng cây mà
explain/sql.rs dùng để dựng câu (build_why_conditions, aggregate của độ đo), và guard in lại câu bằng cùng sqlparser
nên chữ của các vế đó không đổi. Khóa nhóm và thứ tự vẫn đọc từ câu đã chạy bằng build_understanding như câu thường.
Các phần:
- measures: độ đo của form; chỉ số thì tên kèm mô tả của chỉ số.
- time_range: so hai kỳ thì "<kỳ so> so với <kỳ gốc>" trên vế hai lát cắt; so hai nhóm trong một kỳ thì kỳ đó.
- filters: bộ lọc của câu hỏi và của chỉ số; so hai nhóm thì "<cột>: <nhóm so> so với <nhóm gốc>" trên vế hai lát
  cắt; cuối cùng là điều kiện phân quyền guard thêm.
- assumptions: giả định đã dựng ở explain/choices.rs, chép nguyên để cùng id với AskResponse.choices.
Nhóm phân rã không có mục riêng: chúng không có trong sql_guarded, mà ở Explanation.dimensions cùng câu SQL của chúng. */

use sqlparser::ast::Expr;

use crate::engine::sql_shape::QueryShape;
use crate::engine::store::MetricDefinition;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{Choice, Understanding, UnderstoodItem};
use crate::service::explain::check::{PlanSlices, PlanTime, WhyPlan};
use crate::service::explain::measure::find_metric;
use crate::service::explain::period::Period;
use crate::service::explain::sql::build_why_conditions;
use crate::service::understanding::build_understanding;
use crate::service::understanding::labels::{
    build_column_label, build_filter_label, build_row_filter_label, lower_first,
};
use crate::service::understanding::metric_words::build_metric_label;

/// Thẻ của câu "tại sao" đã chạy bảng tổng.
/// - shape: hình dạng của sql_guarded (bảng tổng), cho khóa nhóm và thứ tự.
/// - row_filters: GuardedSql::row_filters của bảng tổng.
/// - assumptions: WhyOutcome::assumptions.
pub fn build_why_understanding(
    plan: &WhyPlan,
    shape: &QueryShape,
    profile: &[ColumnProfile],
    metrics: &[MetricDefinition],
    row_filters: &[String],
    assumptions: &[Choice],
) -> Understanding {
    let conditions = build_why_conditions(plan);
    let mut filters: Vec<UnderstoodItem> = conditions
        .filters
        .iter()
        .map(|f| build_item(build_filter_label(f, profile), f))
        .collect();
    let time_range = match &plan.slices {
        PlanSlices::Periods {
            time,
            base,
            compare,
            ..
        } => Some(build_item(
            build_periods_label(time, *base, *compare, profile),
            &conditions.slices,
        )),
        PlanSlices::Groups {
            column,
            base,
            compare,
            period,
        } => {
            let label = format!(
                "{}: {compare} so với {}",
                build_column_label(column, profile, false),
                base.build_label(column, &plan.labels)
            );
            filters.push(build_item(label, &conditions.slices));
            match (period, &conditions.period) {
                (Some((time, period)), Some(expr)) => Some(build_item(
                    format!(
                        "{}, theo {}",
                        period.label(),
                        build_column_label(&time.column, profile, true)
                    ),
                    expr,
                )),
                _ => None,
            }
        }
    };
    filters.extend(row_filters.iter().map(|sql| UnderstoodItem {
        label: build_row_filter_label(sql, profile),
        sql: sql.clone(),
        from_previous: false,
    }));
    // Khóa nhóm (kỳ hay nhóm của bảng tổng) và thứ tự đọc từ câu đã chạy như câu thường; phần còn lại theo plan.
    let read = build_understanding(shape, false, profile, metrics, row_filters, assumptions);
    Understanding {
        measures: vec![build_item(
            build_measure_label(plan, metrics),
            &plan.measure.aggregate,
        )],
        filters,
        group_by: read.group_by,
        order_by: read.order_by,
        limit: None,
        time_range,
        assumptions: assumptions.to_vec(),
    }
}

/// "Quý 3/2017 so với quý 2/2017, theo ngày đặt hàng (order date)".
fn build_periods_label(
    time: &PlanTime,
    base: Period,
    compare: Period,
    profile: &[ColumnProfile],
) -> String {
    format!(
        "{} so với {}, theo {}",
        compare.label(),
        lower_first(&base.label()),
        build_column_label(&time.column, profile, true)
    )
}

/// Chỉ số thì "<tên>: <mô tả hay cách tính>" như thẻ câu thường; cột thì nhãn của plan ("Tổng <cột>", "Trung bình
/// <cột>").
fn build_measure_label(plan: &WhyPlan, metrics: &[MetricDefinition]) -> String {
    let metric = plan
        .measure
        .metric
        .as_deref()
        .and_then(|name| find_metric(metrics, name));
    match metric {
        Some(metric) => build_metric_label(metric),
        None => plan.measure.label.clone(),
    }
}

fn build_item(label: String, expr: &Expr) -> UnderstoodItem {
    UnderstoodItem {
        label,
        sql: expr.to_string(),
        from_previous: false,
    }
}
