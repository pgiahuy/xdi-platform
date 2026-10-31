// SPDX-License-Identifier: Apache-2.0
/* service/overview/contract.rs - hợp đồng của endpoint tổng quan dataset (GET .../datasets/{id}/overview, #198): tóm tắt,
hồ sơ cột, chỉ số, biểu đồ dựng sẵn và các phần chưa có. service/overview/mod.rs dựng, api/datasets/overview.rs trả. Mô
tả trong đặc tả viết tiếng Anh vì người đọc là người dùng API (AGENTS.md mục 1); chú thích // là cho đội.

Biểu đồ dùng lại đúng ResultTable và ChartSuggestion của câu hỏi (ask/contract.rs), nên giao diện vẽ bằng cùng component
với câu trả lời và cùng bộ luật IBCS ở ARCHITECTURE mục 6b. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::ask::contract::{ChartSuggestion, ResultTable};

/// Overview of a dataset for anyone who can ask it: what it holds and what to ask, computed without the language model.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DatasetOverviewResponse {
    pub dataset_id: Uuid,
    pub name: String,
    /// Description of the table written in the source database (`COMMENT ON TABLE`) for the current version; null for
    /// uploaded files and tables without a comment.
    #[schema(required)]
    pub description: Option<String>,
    /// Current version; null when the dataset has no data yet.
    #[schema(required)]
    pub version: Option<OverviewVersion>,
    pub summary: OverviewSummary,
    /// Columns in table order, without the ones the profile ignores (all empty, `_source_file`).
    pub columns: Vec<OverviewColumn>,
    /// Defined metrics, one per name: the latest confirmed definition, else the latest draft.
    pub metrics: Vec<OverviewMetric>,
    /// The measure drawn over time: a confirmed metric, else a measure column whose unit is money or quantity.
    #[schema(required)]
    pub main_measure: Option<MainMeasure>,
    /// Pre-built charts: the main measure over time, row count over time, then row count by value of each dimension.
    pub charts: Vec<OverviewChart>,
    /// Parts that are not there yet or cannot be shown, with the reason.
    pub missing: Vec<MissingPart>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewVersion {
    pub id: Uuid,
    /// Version number, from 1.
    pub number: i32,
    /// pending, structure, semantic or failed (see `Dataset.current_profile_status`).
    pub profile_status: String,
    pub created_at: DateTime<Utc>,
}

// Số dòng và khoảng thời gian tính bằng một câu SQL qua guard với vai dữ liệu của người xem, nên người bị lọc dòng chỉ
// thấy số của phần mình được xem.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewSummary {
    /// Rows the viewer can read, after the row filters of their data role; null when not counted yet or the count failed.
    #[schema(required)]
    pub row_count: Option<u64>,
    /// Time column of the time span and the time charts; null when the dataset has no time column.
    #[schema(required)]
    pub time_column: Option<String>,
    /// First and last day with data in `time_column` (YYYY-MM-DD), among the rows the viewer can read.
    #[schema(required)]
    pub time_start: Option<String>,
    #[schema(required)]
    pub time_end: Option<String>,
    /// True when the viewer's data role filters rows. Per-column counts and value frequencies are computed on every
    /// row, so they are left out: null `null_percent` and `distinct_count`, empty `values`, no `rows_by_value` charts.
    pub restricted: bool,
}

/// Role of a column in the profile; the rules are in docs/DATA.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverviewRole {
    Dimension,
    Measure,
    Time,
    Key,
    Text,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewColumn {
    pub name: String,
    /// Vietnamese label: the first alias written by the semantic profile; null until then.
    #[schema(required)]
    pub label: Option<String>,
    #[schema(required)]
    pub description: Option<String>,
    /// Who wrote `description`: `database` (a `COMMENT ON COLUMN` of the source database) or `model` (the semantic
    /// profile); null when there is no description.
    #[schema(required)]
    pub description_source: Option<String>,
    pub role: OverviewRole,
    /// money, quantity, percent, duration or other; null when unknown.
    #[schema(required)]
    pub unit: Option<String>,
    /// Marked sensitive: no values are returned for it.
    pub sensitive: bool,
    /// Share of empty cells, 0 to 100; null when `summary.restricted`.
    #[schema(required)]
    pub null_percent: Option<f64>,
    /// Exact number of different values; null when `summary.restricted`.
    #[schema(required)]
    pub distinct_count: Option<i64>,
    /// Values with their row counts, most rows first; only for dimension columns that are not sensitive, at most 200.
    pub values: Vec<OverviewValue>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewValue {
    pub value: String,
    /// First alias of the value; null when it has none.
    #[schema(required)]
    pub label: Option<String>,
    /// Rows holding this value, on every row of the version.
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewMetric {
    pub name: String,
    pub description: String,
    /// Aggregate over the columns, for example `SUM("Order Item Total")`.
    pub expression: String,
    #[schema(required)]
    pub filter: Option<String>,
    #[schema(required)]
    pub time_column: Option<String>,
    #[schema(required)]
    pub unit: Option<String>,
    /// False while an admin has not confirmed the definition.
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MainMeasure {
    /// Metric name, or the column label (else name) of a measure column; also the measure column of the chart.
    pub label: String,
    /// The metric of that name, or null for a measure column.
    #[schema(required)]
    pub metric: Option<String>,
    /// The measure column, or null for a metric.
    #[schema(required)]
    pub column: Option<String>,
}

/// Kind of pre-built chart:
/// - `measure_over_time`: the main measure by day or month.
/// - `rows_over_time`: number of rows by day or month, to see periods with little data or data that stops mid-period.
/// - `rows_by_value`: number of rows for each value of a dimension column, read from the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverviewChartKind {
    MeasureOverTime,
    RowsOverTime,
    RowsByValue,
}

/// Period of the time charts: by day when the data spans at most 92 days, else by month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeGrain {
    Day,
    Month,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewChart {
    pub kind: OverviewChartKind,
    /// Column of the profile the chart is about: the dimension for `rows_by_value`, the time column otherwise.
    pub column: String,
    /// Null for `rows_by_value`.
    #[schema(required)]
    pub grain: Option<TimeGrain>,
    pub result: ResultTable,
    pub chart: ChartSuggestion,
    /// SQL after the guard that computed the chart; null for `rows_by_value`, which no query computes.
    #[schema(required)]
    pub sql: Option<String>,
}

/// What is missing:
/// - `no_version`: the dataset has no data yet.
/// - `profile_pending`: the column profile is being built; no columns or charts yet.
/// - `profile_failed`: the profile failed; upload the file again.
/// - `semantic_pending`: labels, descriptions and units are being written.
/// - `semantic_unavailable`: the current model cannot write labels, descriptions and units.
/// - `restricted`: the viewer's data role filters rows, so per-column counts and value charts are left out.
/// - `no_time`: no time column, so no time span and no time charts.
/// - `no_measure`: no confirmed metric and no money or quantity measure column, so no measure chart.
/// - `chart_failed`: a chart query failed; the message says why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MissingCode {
    NoVersion,
    ProfilePending,
    ProfileFailed,
    SemanticPending,
    SemanticUnavailable,
    Restricted,
    NoTime,
    NoMeasure,
    ChartFailed,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MissingPart {
    pub code: MissingCode,
    /// Text for the end user (Vietnamese).
    pub message: String,
}
