// SPDX-License-Identifier: Apache-2.0
/* engine/store/source_tables.rs - bảng của nguồn database: bảng mới có metadata (bảng source_metadata_tables, migration
0018) và bảng đã chụp thành dataset (đọc từ sources.config.tables). Logic chọn bảng nào chỉ lưu metadata ở
service/sources/plan.rs; luồng hỏi đọc list_metadata_tables để nêu đúng bảng chưa chụp (service/ask/unsnapshotted.rs). */

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// One column of a table kept as metadata only, read from the source catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MetadataColumn {
    pub name: String,
    /// Postgres type as `format_type` writes it, for example `numeric(10,2)`.
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    /// `COMMENT ON COLUMN`, cleaned; null when none.
    #[schema(required)]
    pub description: Option<String>,
}

/// Một bảng cần ghi metadata, như service/sources/ dựng lúc chụp.
#[derive(Debug, Clone)]
pub struct NewMetadataTable {
    pub table_name: String,
    /// `row_limit` hay `size_limit` (CHECK của migration 0018).
    pub cause: &'static str,
    pub reason: String,
    pub row_count: Option<i64>,
    pub size_bytes: Option<i64>,
    pub description: Option<String>,
    pub columns: Vec<MetadataColumn>,
}

/// Phần của một dòng source_metadata_tables mà danh sách bảng và luồng hỏi cần.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MetadataTableRow {
    pub table_name: String,
    pub reason: String,
    pub description: Option<String>,
    pub columns: Value,
}

impl MetadataTableRow {
    /// Cột đã lưu; jsonb hỏng (ghi tay bằng SQL) thì coi như không có cột, không làm hỏng cả danh sách.
    pub fn build_columns(&self) -> Vec<MetadataColumn> {
        serde_json::from_value(self.columns.clone()).unwrap_or_default()
    }
}

const METADATA_COLUMNS: &str = "table_name, reason, description, columns";

impl Store {
    /// Ghi (hay ghi đè) metadata của các bảng chưa chụp được của một nguồn.
    pub async fn upsert_metadata_tables(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
        tables: &[NewMetadataTable],
    ) -> Result<(), AppError> {
        for t in tables {
            let columns = serde_json::to_value(&t.columns)
                .map_err(|e| AppError::Internal(format!("encode metadata columns: {e}")))?;
            sqlx::query(
                "INSERT INTO source_metadata_tables
                     (workspace_id, source_id, table_name, cause, reason, row_count, size_bytes, description, columns)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (source_id, table_name) DO UPDATE SET
                     cause = EXCLUDED.cause, reason = EXCLUDED.reason, row_count = EXCLUDED.row_count,
                     size_bytes = EXCLUDED.size_bytes, description = EXCLUDED.description,
                     columns = EXCLUDED.columns, updated_at = now()",
            )
            .bind(ws.as_uuid())
            .bind(source_id)
            .bind(&t.table_name)
            .bind(t.cause)
            .bind(&t.reason)
            .bind(t.row_count)
            .bind(t.size_bytes)
            .bind(&t.description)
            .bind(columns)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("upsert metadata table", e))?;
        }
        Ok(())
    }

    /// Bảng vừa chụp được thành dataset: bỏ dòng metadata của nó (nếu có).
    pub async fn delete_metadata_table(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
        table_name: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "DELETE FROM source_metadata_tables WHERE workspace_id = $1 AND source_id = $2 AND table_name = $3",
        )
        .bind(ws.as_uuid())
        .bind(source_id)
        .bind(table_name)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("delete metadata table", e))?;
        Ok(())
    }

    /// Bảng mới có metadata của một nguồn, theo tên.
    pub async fn list_metadata_tables(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
    ) -> Result<Vec<MetadataTableRow>, AppError> {
        let sql = format!(
            "SELECT {METADATA_COLUMNS} FROM source_metadata_tables
             WHERE workspace_id = $1 AND source_id = $2 ORDER BY table_name"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(source_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list metadata tables", e))
    }

    /// Bảng đã chụp thành dataset của một nguồn mà dataset còn trong workspace: (dataset, schema.bảng), cũ trước. Đọc
    /// sources.config.tables; dataset đã xóa không còn dòng nên không có ở đây, và lần chụp lại cả nguồn bỏ qua nó.
    pub async fn list_source_datasets(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
    ) -> Result<Vec<(Uuid, String)>, AppError> {
        sqlx::query_as(
            "SELECT d.id, t.value FROM sources s
             CROSS JOIN LATERAL jsonb_each_text(COALESCE(s.config->'tables', '{}'::jsonb)) AS t(key, value)
             JOIN datasets d ON d.workspace_id = s.workspace_id AND d.source_id = s.id AND d.id::text = t.key
             WHERE s.workspace_id = $1 AND s.id = $2
             ORDER BY d.created_at, d.id",
        )
        .bind(ws.as_uuid())
        .bind(source_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list source datasets", e))
    }

    /// Mọi nguồn Postgres của mọi workspace, kèm một admin của workspace (người vào workspace sớm nhất trong số admin
    /// còn hoạt động) và slug, cho lịch chụp lại (service/sources/schedule.rs). Workspace không còn admin nào thì không
    /// có trong danh sách. Không lọc theo workspace: việc nền không gắn với người dùng nào, như list_pending_versions.
    pub async fn list_scheduled_sources(&self) -> Result<Vec<(Uuid, Uuid, String)>, AppError> {
        sqlx::query_as(
            "SELECT s.id, a.user_id, w.slug FROM sources s
             JOIN workspaces w ON w.id = s.workspace_id
             JOIN LATERAL (
                 SELECT m.user_id FROM memberships m JOIN users u ON u.id = m.user_id
                 WHERE m.workspace_id = s.workspace_id AND m.role = 'admin' AND u.is_active
                 ORDER BY m.created_at, m.user_id LIMIT 1
             ) a ON true
             WHERE s.kind = 'postgres'
             ORDER BY s.created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list scheduled sources", e))
    }
}
