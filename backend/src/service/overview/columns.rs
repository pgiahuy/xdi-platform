// SPDX-License-Identifier: Apache-2.0
/* service/overview/columns.rs - phần hồ sơ của trang tổng quan: mỗi cột (nhãn, mô tả, vai trò, đơn vị, % ô trống, số giá
trị khác nhau, tần suất giá trị) và các chỉ số đã định nghĩa, đọc thẳng từ columns, column_values và metrics, không chạy
câu nào trên dữ liệu.

Cột nhạy cảm không bao giờ có giá trị: store chỉ được hỏi giá trị của cột chiều không nhạy cảm (mod.rs), và ở đây lọc
lại theo hồ sơ vừa đọc. Người xem bị lọc dòng theo vai dữ liệu (restricted) không nhận số đếm nào của hồ sơ, vì hồ sơ
đếm trên mọi dòng, kể cả dòng họ không được xem. */

use crate::engine::store::{ColumnValue, MetricDefinition, ProfileColumn, ProfileRole};
use crate::service::overview::contract::{
    OverviewColumn, OverviewMetric, OverviewRole, OverviewValue,
};

/// Cột theo thứ tự trong bảng, bỏ cột vai trò ignore. row_count là số dòng của cả phiên bản, mẫu số của % ô trống.
pub fn build_columns(
    columns: &[ProfileColumn],
    values: &[ColumnValue],
    row_count: Option<i64>,
    restricted: bool,
) -> Vec<OverviewColumn> {
    columns
        .iter()
        .filter_map(|column| {
            let role = build_role(&column.role)?;
            let counts_shown = !restricted;
            let values = if shows_values(column) && counts_shown {
                build_values(column, values)
            } else {
                Vec::new()
            };
            Some(OverviewColumn {
                name: column.name.clone(),
                label: column.aliases.first().cloned(),
                description: column.description.clone(),
                role,
                unit: column.unit.clone(),
                sensitive: column.is_sensitive,
                null_percent: counts_shown
                    .then(|| build_null_percent(column.null_count, row_count))
                    .flatten(),
                distinct_count: column.distinct_count.filter(|_| counts_shown),
                values,
            })
        })
        .collect()
}

/// Cột được trả giá trị: cột chiều không nhạy cảm, như danh sách giá trị gửi mô hình (store::list_value_rows).
pub fn shows_values(column: &ProfileColumn) -> bool {
    column.role == ProfileRole::Dimension.as_str() && !column.is_sensitive
}

/// Giá trị của một cột, giữ thứ tự của store (nhiều dòng nhất trước).
pub fn values_of<'a>(column: &ProfileColumn, values: &'a [ColumnValue]) -> Vec<&'a ColumnValue> {
    values.iter().filter(|v| v.column_id == column.id).collect()
}

fn build_values(column: &ProfileColumn, values: &[ColumnValue]) -> Vec<OverviewValue> {
    values_of(column, values)
        .into_iter()
        .map(|v| OverviewValue {
            value: v.value.clone(),
            label: v.aliases.first().cloned(),
            rows: v.frequency.unwrap_or(0),
        })
        .collect()
}

fn build_null_percent(null_count: Option<i64>, row_count: Option<i64>) -> Option<f64> {
    match (null_count, row_count) {
        (Some(nulls), Some(rows)) if rows > 0 => Some(nulls as f64 * 100.0 / rows as f64),
        _ => None,
    }
}

/// None với ignore và vai trò lạ: CHECK của columns.role (0001_init.sql) chỉ cho sáu vai trò.
fn build_role(role: &str) -> Option<OverviewRole> {
    let roles = [
        (ProfileRole::Dimension, OverviewRole::Dimension),
        (ProfileRole::Measure, OverviewRole::Measure),
        (ProfileRole::Time, OverviewRole::Time),
        (ProfileRole::Key, OverviewRole::Key),
        (ProfileRole::Text, OverviewRole::Text),
    ];
    roles
        .into_iter()
        .find(|(profile, _)| profile.as_str() == role)
        .map(|(_, overview)| overview)
}

pub fn build_metrics(metrics: &[MetricDefinition]) -> Vec<OverviewMetric> {
    metrics
        .iter()
        .map(|m| OverviewMetric {
            name: m.name.clone(),
            description: m.description.clone(),
            expression: m.expression.clone(),
            filter: m.filter.clone(),
            time_column: m.time_column.clone(),
            unit: m.unit.clone(),
            confirmed: m.confirmed,
        })
        .collect()
}
