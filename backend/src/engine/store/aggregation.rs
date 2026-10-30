// SPDX-License-Identifier: Apache-2.0
/* engine/store/aggregation.rs - cách gộp mặc định của cột số đo (columns.aggregation, 0010_column_aggregation.sql):
cộng hay lấy trung bình. Lớp ngữ nghĩa ghi qua apply_semantic_profile (columns.rs); admin đổi ở màn Chỉ số qua
set_column_aggregation dưới đây. Người đọc: prompt sinh SQL (service/ask/prompt.rs), form câu "tại sao"
(service/explain/measure.rs). Tách khỏi columns.rs vì file đó đã dài. */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{db_error, ProfileRole, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// How the values of a measure column are combined when no metric says otherwise: `sum` for amounts and quantities,
/// `avg` for values that describe one row (age, score, credits, days).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnAggregation {
    Sum,
    Avg,
}

impl ColumnAggregation {
    pub fn as_str(self) -> &'static str {
        match self {
            ColumnAggregation::Sum => "sum",
            ColumnAggregation::Avg => "avg",
        }
    }

    pub fn parse(text: &str) -> Option<ColumnAggregation> {
        match text {
            "sum" => Some(ColumnAggregation::Sum),
            "avg" => Some(ColumnAggregation::Avg),
            _ => None,
        }
    }
}

impl Store {
    /// Đổi cách gộp của một cột measure trong một phiên bản. Trả false khi không có cột measure tên đó trong phiên bản
    /// (hay phiên bản thuộc workspace khác): cột khác vai trò không có cách gộp để đổi.
    pub async fn set_column_aggregation(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        column: &str,
        aggregation: ColumnAggregation,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query(
            "UPDATE columns SET aggregation = $4
             WHERE workspace_id = $1 AND version_id = $2 AND name = $3 AND role = $5",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(column)
        .bind(aggregation.as_str())
        .bind(ProfileRole::Measure.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set column aggregation", e))?
        .rows_affected();
        Ok(updated > 0)
    }
}
