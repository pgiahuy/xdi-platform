// SPDX-License-Identifier: Apache-2.0
/* service/overview/charts.rs - biểu đồ dựng sẵn của trang tổng quan, chọn bằng đúng bộ luật của câu hỏi
(service/ask/chart.rs, IBCS, ARCHITECTURE mục 6b): vai trò cột kết quả đọc từ cây cú pháp của câu (understanding/mod.rs),
rồi build_chart chọn loại biểu đồ, số nhóm tối đa và gộp "Khác".

Biểu đồ số dòng theo giá trị không chạy câu nào: số dòng của từng giá trị đã có trong column_values từ lúc lập hồ sơ.
Bảng dựng tay từ các dòng đó, còn luật biểu đồ đọc câu SQL lẽ ra tính ra bảng ấy (GROUP BY cột, COUNT(*)), để thanh
ngang, cắt 15 nhóm lớn nhất và gộp "Khác" giống hệt câu hỏi "số dòng theo cột". */

use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::{ColumnValue, ProfileColumn};
use crate::service::ask::chart::build_chart;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{CellValue, ColumnRole, ResultColumn, ResultTable, ValueKind};
use crate::service::explain::sql::build_ident;
use crate::service::overview::contract::{OverviewChart, OverviewChartKind, TimeGrain};
use crate::service::overview::sql::ROWS_LABEL;
use crate::service::understanding::build_column_roles;

/// Số dòng theo giá trị của một cột chiều, từ column_values của cột (nhiều dòng nhất trước). None khi cột có dưới hai
/// giá trị: biểu đồ một thanh không thêm gì.
pub fn build_value_chart(
    table: &str,
    column: &ProfileColumn,
    values: &[&ColumnValue],
    profile: &[ColumnProfile],
) -> Option<OverviewChart> {
    let rows = values
        .iter()
        .map(|v| {
            // COUNT(*) của DuckDB là BIGINT, kết quả câu hỏi gửi dạng decimal (chuỗi chữ số); giữ cùng dạng.
            let count = v.frequency.unwrap_or(0).to_string();
            vec![
                Some(CellValue::String(v.value.clone())),
                Some(CellValue::String(count)),
            ]
        })
        .collect::<Vec<_>>();
    let total_rows = rows.len() as u64;
    let result = ResultTable {
        columns: vec![
            build_column(&column.name, ValueKind::Text),
            build_column(ROWS_LABEL, ValueKind::Decimal),
        ],
        rows,
        total_rows,
        truncated: false,
        limit_reached: false,
    };
    let ident = build_ident(&column.name);
    let sql = format!(
        "SELECT {ident}, COUNT(*) AS {} FROM {} GROUP BY {ident}",
        build_ident(ROWS_LABEL),
        build_ident(table)
    );
    build_overview_chart(
        OverviewChartKind::RowsByValue,
        &column.name,
        None,
        &sql,
        None,
        result,
        profile,
    )
}

/// Biểu đồ của một câu theo kỳ đã chạy; sql là bản đã qua guard. None khi luật không vẽ (dưới hai kỳ).
pub fn build_time_chart(
    kind: OverviewChartKind,
    column: &str,
    grain: TimeGrain,
    sql: &str,
    result: ResultTable,
    profile: &[ColumnProfile],
) -> Option<OverviewChart> {
    build_overview_chart(kind, column, Some(grain), sql, Some(sql), result, profile)
}

fn build_overview_chart(
    kind: OverviewChartKind,
    column: &str,
    grain: Option<TimeGrain>,
    shape_sql: &str,
    shown: Option<&str>,
    mut result: ResultTable,
    profile: &[ColumnProfile],
) -> Option<OverviewChart> {
    let shape = match build_query_shape(shape_sql) {
        Ok(shape) => shape,
        Err(e) => {
            // Câu do chính code này dựng: không đọc được là lỗi của code, mất một biểu đồ chứ không mất cả trang.
            tracing::warn!(error = ?e, "cannot read an overview chart query");
            return None;
        }
    };
    let roles = build_column_roles(&shape, profile, result.columns.len());
    let chart = build_chart(&shape, &result, &roles, profile)?;
    for (column, role) in result.columns.iter_mut().zip(&roles) {
        column.role = *role;
    }
    Some(OverviewChart {
        kind,
        column: column.to_string(),
        grain,
        result,
        chart,
        sql: shown.map(str::to_string),
    })
}

fn build_column(name: &str, kind: ValueKind) -> ResultColumn {
    ResultColumn {
        name: name.to_string(),
        kind,
        role: ColumnRole::Other,
    }
}
