// SPDX-License-Identifier: Apache-2.0
/* service/metrics/contract.rs - kiểu request và response của API chỉ số (api/metrics.rs): danh sách chỉ số và cột cho
màn Chỉ số, chỉ số admin gửi lên, cách gộp của một cột. Mô tả trong đặc tả OpenAPI viết tiếng Anh như các kiểu khác
của API. */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::store::{ColumnAggregation, MetricRecord, ProfileColumn, ProfileRole};

/// A metric of the dataset: the version in use (the latest confirmed one).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Metric {
    /// Id of this version; editing or deleting by it acts on the metric with this name.
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// Aggregate SQL expression over the dataset's columns, for example `SUM("Order Item Total")`.
    pub expression: String,
    /// Row condition applied with the expression, for example `"Order Status" <> 'CANCELED'`.
    #[schema(required)]
    pub filter: Option<String>,
    /// Column that dates the rows for this metric.
    #[schema(required)]
    pub time_column: Option<String>,
    #[schema(required)]
    pub unit: Option<String>,
    /// Starts at 1; each edit adds a version and keeps the earlier ones.
    pub version: i32,
    /// `ui` (written on the metrics screen), `sample`, `formula_table` or `llm_suggested`.
    pub origin: String,
    /// False for a draft nobody approved; the SQL prompt marks it as such.
    pub confirmed: bool,
}

impl From<MetricRecord> for Metric {
    fn from(r: MetricRecord) -> Metric {
        Metric {
            id: r.id,
            name: r.name,
            description: r.description,
            expression: r.expression,
            filter: r.filter,
            time_column: r.time_column,
            unit: r.unit,
            version: r.version,
            origin: r.origin,
            confirmed: r.confirmed,
        }
    }
}

/// A column of the current version, for the column picker and the default aggregation of measure columns.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricColumn {
    pub name: String,
    /// DuckDB type, for example `DOUBLE`, `BIGINT`, `VARCHAR`.
    pub data_type: String,
    /// `dimension`, `measure`, `time`, `key`, `text` or `ignore`.
    pub role: String,
    #[schema(required)]
    pub description: Option<String>,
    /// Default aggregation; null unless the role is `measure`.
    #[schema(required)]
    pub aggregation: Option<ColumnAggregation>,
}

impl From<ProfileColumn> for MetricColumn {
    fn from(c: ProfileColumn) -> MetricColumn {
        let aggregation = if c.role == ProfileRole::Measure.as_str() {
            ColumnAggregation::parse(&c.aggregation)
        } else {
            None
        };
        MetricColumn {
            name: c.name,
            data_type: c.data_type,
            role: c.role,
            description: c.description,
            aggregation,
        }
    }
}

/// Metrics of a dataset and the columns of its current version.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricsResponse {
    /// One entry per metric name, sorted by name.
    pub metrics: Vec<Metric>,
    /// Columns of the current version in table order; empty while the dataset has no profiled version.
    pub columns: Vec<MetricColumn>,
}

/// A metric to create, or the new version of one. Errors name the wrong field in `field`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MetricRequest {
    /// Name users ask with, unique in the dataset ignoring case and accents. Cannot change when editing.
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// One aggregate SQL expression over the dataset's columns, column names in double quotes.
    pub expression: String,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub time_column: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
}

/// New default aggregation of one measure column of the current version.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ColumnAggregationRequest {
    pub column: String,
    pub aggregation: ColumnAggregation,
}
