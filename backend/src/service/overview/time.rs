// SPDX-License-Identifier: Apache-2.0
/* service/overview/time.rs - phần của trang tổng quan tính bằng SQL: tóm tắt (số dòng người xem đọc được, ngày đầu,
ngày cuối), rồi độ đo chính và số dòng theo ngày hay theo tháng tùy khoảng dữ liệu. Câu dựng ở overview/sql.rs, chạy qua
guard và cache ở overview/query.rs, biểu đồ chọn ở overview/charts.rs. Câu bị guard từ chối, câu DuckDB báo lỗi, quá giờ
hay quá bộ nhớ chỉ thành một MissingPart, phần còn lại của trang vẫn trả. Lỗi máy chủ (ví dụ file Parquet của phiên bản
không còn trên đĩa) thì trả Err và cả trang tổng quan báo lỗi 500. */

use uuid::Uuid;

use crate::engine::guard::AllowedTable;
use crate::engine::store::{MetricDefinition, ProfileColumn};
use crate::error::AppError;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{CellValue, ResultTable};
use crate::service::ask::result::SENT_ROWS;
use crate::service::explain::breakdown::QueryEnv;
use crate::service::explain::sql::build_time_expr;
use crate::service::overview::charts::build_time_chart;
use crate::service::overview::contract::{
    DatasetOverviewResponse, MissingCode, OverviewChartKind, TimeGrain,
};
use crate::service::overview::missing::{build_chart_failed, build_missing};
use crate::service::overview::query::{run_cached, Ran};
use crate::service::overview::sql::{
    build_grain, build_measure_over_time_sql, build_rows_over_time_sql, build_summary_sql,
    choose_measure, choose_time_column,
};
use crate::state::AppState;

/// Lời khuyên khi một câu của trang bị dừng vì quá giờ hay quá bộ nhớ. Người xem không hỏi gì nên không khuyên "hỏi
/// hẹp hơn" như luồng hỏi; phần này bỏ trống, muốn xem thì hỏi riêng về một khoảng ngắn hơn.
const OVERVIEW_STOP_HINT: &str =
    "Phần này để trống; muốn xem thì đặt câu hỏi cho một khoảng thời gian ngắn hơn.";

/// Thứ phần SQL của trang cần, đã đọc xong từ Postgres.
pub struct TimeInput<'a> {
    pub state: &'a AppState,
    pub version_id: Uuid,
    /// Tên bảng logic của dataset.
    pub table: &'a str,
    /// Bảng duy nhất được đọc, row_filters theo người xem.
    pub tables: &'a [AllowedTable],
    pub columns: &'a [ProfileColumn],
    pub metrics: &'a [MetricDefinition],
    pub profile: &'a [ColumnProfile],
}

pub async fn add_time_parts(
    overview: &mut DatasetOverviewResponse,
    input: &TimeInput<'_>,
) -> Result<(), AppError> {
    let env = QueryEnv {
        tables: input.tables,
        // Tổng quan chỉ đọc một bảng.
        links: &[],
        max_rows: SENT_ROWS as u64,
        timeout: input.state.config.query_timeout,
        jobs: &input.state.query_jobs,
        stop_hint: OVERVIEW_STOP_HINT,
    };
    let plan = choose_measure(input.metrics, input.columns);
    overview.main_measure = plan.as_ref().map(|p| p.main.clone());
    let time_column = choose_time_column(input.columns, plan.as_ref());
    let time = time_column.and_then(build_time_expr);
    overview.summary.time_column = time_column.map(|c| c.name.clone());

    let summary_sql = build_summary_sql(input.table, time.as_ref());
    let summary = match run(input, &env, &summary_sql).await? {
        Ran::Done { result, .. } => result,
        Ran::Failed(reason) => {
            overview
                .missing
                .push(build_chart_failed("số dòng", &reason));
            return Ok(());
        }
    };
    let row = summary.rows.first();
    let cell = |i: usize| row.and_then(|r| r.get(i)).and_then(|c| c.as_ref());
    overview.summary.row_count = cell(0).and_then(read_count);
    overview.summary.time_start = cell(1).and_then(read_text);
    overview.summary.time_end = cell(2).and_then(read_text);

    let (Some(column), Some(time)) = (time_column, time) else {
        overview.missing.push(build_missing(MissingCode::NoTime));
        return Ok(());
    };
    let (Some(start), Some(end)) = (&overview.summary.time_start, &overview.summary.time_end)
    else {
        // Người xem không có dòng nào có ngày: không có kỳ để vẽ, số dòng 0 đã nói đủ.
        return Ok(());
    };
    let grain = build_grain(start, end);
    let period = match grain {
        TimeGrain::Day => "ngày",
        TimeGrain::Month => "tháng",
    };
    if plan.is_none() {
        overview.missing.push(build_missing(MissingCode::NoMeasure));
    }
    let measure = plan.as_ref().map(|plan| ChartQuery {
        kind: OverviewChartKind::MeasureOverTime,
        column: &column.name,
        grain,
        sql: build_measure_over_time_sql(input.table, &time, grain, plan),
        what: format!("{} theo {period}", plan.main.label),
    });
    let rows = ChartQuery {
        kind: OverviewChartKind::RowsOverTime,
        column: &column.name,
        grain,
        sql: build_rows_over_time_sql(input.table, &time, grain),
        what: format!("số dòng theo {period}"),
    };
    for query in measure.into_iter().chain([rows]) {
        add_chart(overview, input, &env, query).await?;
    }
    Ok(())
}

/// Một biểu đồ theo kỳ cần chạy; what là tên của nó trong lời báo khi câu lỗi ("số dòng theo tháng").
struct ChartQuery<'a> {
    kind: OverviewChartKind,
    column: &'a str,
    grain: TimeGrain,
    sql: String,
    what: String,
}

async fn add_chart(
    overview: &mut DatasetOverviewResponse,
    input: &TimeInput<'_>,
    env: &QueryEnv<'_>,
    query: ChartQuery<'_>,
) -> Result<(), AppError> {
    match run(input, env, &query.sql).await? {
        Ran::Done { shown, result } => {
            // Bảng trong cache dùng chung giữa các lần mở; chép ra để ghi vai trò cột vào bản của lần này.
            let result = ResultTable::clone(&result);
            let chart = build_time_chart(
                query.kind,
                query.column,
                query.grain,
                &shown,
                result,
                input.profile,
            );
            overview.charts.extend(chart);
        }
        Ran::Failed(reason) => overview
            .missing
            .push(build_chart_failed(&query.what, &reason)),
    }
    Ok(())
}

async fn run(input: &TimeInput<'_>, env: &QueryEnv<'_>, sql: &str) -> Result<Ran, AppError> {
    run_cached(env, &input.state.overview_cache, input.version_id, sql).await
}

/// COUNT(*) là BIGINT, gửi dạng decimal (chuỗi chữ số).
fn read_count(cell: &CellValue) -> Option<u64> {
    match cell {
        CellValue::String(text) => text.parse().ok(),
        CellValue::Number(n) if *n >= 0.0 => Some(*n as u64),
        _ => None,
    }
}

fn read_text(cell: &CellValue) -> Option<String> {
    match cell {
        CellValue::String(text) => Some(text.clone()),
        _ => None,
    }
}
