// SPDX-License-Identifier: Apache-2.0
/* service/overview/sql.rs - chọn độ đo chính và cột thời gian của trang tổng quan, và dựng ba câu SQL tổng hợp: tóm tắt
(số dòng, ngày đầu, ngày cuối), độ đo chính theo kỳ, số dòng theo kỳ. Câu do code dựng từ hồ sơ cột và bảng metrics,
không qua mô hình; service/overview/query.rs cho qua guard rồi chạy. Test ở service/overview/tests.rs. */

use chrono::NaiveDate;
use sqlparser::ast::Expr;

use crate::engine::store::{ColumnUnit, MetricDefinition, ProfileColumn, ProfileRole};
use crate::service::explain::sql::{build_ident, build_string, build_time_expr};
use crate::service::overview::contract::{MainMeasure, TimeGrain};

/// Tên cột số dòng trong bảng của biểu đồ: người xem đọc nó ở chú giải và tooltip.
pub const ROWS_LABEL: &str = "Số dòng";

/// Khoảng dữ liệu tới chừng này ngày thì vẽ theo ngày (tối đa 93 điểm, vẫn thấy từng điểm), dài hơn thì theo tháng. 92
/// chọn ở đây (ba tháng), chưa đo.
const DAY_GRAIN_MAX_DAYS: i64 = 92;

/// Độ đo chính đã chọn, kèm thứ cần để dựng câu.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurePlan {
    pub main: MainMeasure,
    /// Biểu thức tổng hợp: biểu thức của chỉ số, hay SUM của cột số đo.
    pub expression: String,
    /// Điều kiện của chỉ số; None với cột số đo.
    pub filter: Option<String>,
    /// Cột thời gian chỉ số khai báo; None với cột số đo.
    pub time_column: Option<String>,
}

/// Chỉ số đã xác nhận đầu tiên (list_metrics xếp theo tên); chưa có thì cột số đo không nhạy cảm đơn vị tiền, rồi số
/// lượng, theo thứ tự trong bảng. Chỉ số nháp không được chọn: công thức chưa ai duyệt không nên là con số đầu tiên
/// người xem thấy. None khi không có gì hợp.
pub fn choose_measure(
    metrics: &[MetricDefinition],
    columns: &[ProfileColumn],
) -> Option<MeasurePlan> {
    if let Some(metric) = metrics.iter().find(|m| m.confirmed) {
        return Some(MeasurePlan {
            main: MainMeasure {
                label: metric.name.clone(),
                metric: Some(metric.name.clone()),
                column: None,
            },
            expression: metric.expression.clone(),
            filter: metric.filter.clone(),
            time_column: metric.time_column.clone(),
        });
    }
    let unit_column = |unit: ColumnUnit| {
        columns.iter().find(|c| {
            c.role == ProfileRole::Measure.as_str()
                && !c.is_sensitive
                && c.unit.as_deref() == Some(unit.as_str())
        })
    };
    let column = unit_column(ColumnUnit::Money).or_else(|| unit_column(ColumnUnit::Quantity))?;
    Some(MeasurePlan {
        main: MainMeasure {
            label: build_label(column),
            metric: None,
            column: Some(column.name.clone()),
        },
        expression: format!("SUM({})", build_ident(&column.name)),
        filter: None,
        time_column: None,
    })
}

/// Nhãn tiếng Việt của cột: bí danh đầu do lớp ngữ nghĩa ghi, chưa có thì tên cột.
pub fn build_label(column: &ProfileColumn) -> String {
    column
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| column.name.clone())
}

/// Cột thời gian của độ đo chính nếu nó có trong hồ sơ, không thì cột thời gian đầu tiên. Cột nhạy cảm không dùng, và
/// cột không đọc được thành ngày (build_time_expr None) cũng bỏ.
pub fn choose_time_column<'a>(
    columns: &'a [ProfileColumn],
    measure: Option<&MeasurePlan>,
) -> Option<&'a ProfileColumn> {
    let usable = |c: &&ProfileColumn| {
        c.role == ProfileRole::Time.as_str() && !c.is_sensitive && build_time_expr(c).is_some()
    };
    let wanted = measure.and_then(|m| m.time_column.as_deref());
    columns
        .iter()
        .filter(usable)
        .find(|c| Some(c.name.as_str()) == wanted)
        .or_else(|| columns.iter().find(usable))
}

/// Số dòng người xem đọc được, ngày đầu và ngày cuối của cột thời gian (nếu có), một dòng.
pub fn build_summary_sql(table: &str, time: Option<&Expr>) -> String {
    let mut outputs = vec!["COUNT(*) AS row_count".to_string()];
    if let Some(time) = time {
        outputs.push(format!("strftime(MIN({time}), '%Y-%m-%d') AS time_start"));
        outputs.push(format!("strftime(MAX({time}), '%Y-%m-%d') AS time_end"));
    }
    format!("SELECT {} FROM {}", outputs.join(", "), build_ident(table))
}

/// Số dòng theo kỳ, kỳ cũ trước. Dòng không có ngày không thuộc kỳ nào nên không đếm.
pub fn build_rows_over_time_sql(table: &str, time: &Expr, grain: TimeGrain) -> String {
    build_over_time_sql(table, time, grain, "COUNT(*)", ROWS_LABEL, None)
}

/// Độ đo chính theo kỳ, cùng điều kiện của chỉ số; tên cột độ đo là nhãn của nó.
pub fn build_measure_over_time_sql(
    table: &str,
    time: &Expr,
    grain: TimeGrain,
    plan: &MeasurePlan,
) -> String {
    build_over_time_sql(
        table,
        time,
        grain,
        &plan.expression,
        &plan.main.label,
        plan.filter.as_deref(),
    )
}

fn build_over_time_sql(
    table: &str,
    time: &Expr,
    grain: TimeGrain,
    aggregate: &str,
    label: &str,
    filter: Option<&str>,
) -> String {
    let mut conditions = vec![format!("{time} IS NOT NULL")];
    if let Some(filter) = filter {
        conditions.push(format!("({filter})"));
    }
    let (unit, period) = match grain {
        TimeGrain::Day => ("day", "Ngày"),
        TimeGrain::Month => ("month", "Tháng"),
    };
    format!(
        "SELECT date_trunc({}, {time}) AS {}, {aggregate} AS {} FROM {} WHERE {} GROUP BY 1 ORDER BY 1",
        build_string(unit),
        build_ident(period),
        build_ident(label),
        build_ident(table),
        conditions.join(" AND ")
    )
}

/// Theo ngày khi khoảng dữ liệu ngắn, theo tháng khi dài hơn hay khi không đọc được ngày.
pub fn build_grain(start: &str, end: &str) -> TimeGrain {
    let parse = |text: &str| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok();
    match (parse(start), parse(end)) {
        (Some(start), Some(end)) if (end - start).num_days() <= DAY_GRAIN_MAX_DAYS => {
            TimeGrain::Day
        }
        _ => TimeGrain::Month,
    }
}
