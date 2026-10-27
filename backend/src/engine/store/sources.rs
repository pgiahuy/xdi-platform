// SPDX-License-Identifier: Apache-2.0
/* engine/store/sources.rs - nguồn database của workspace (bảng sources, migration 0001). Logic (mã hóa, thử kết nối,
chụp bảng) ở service/sources/.

config (jsonb) chỉ giữ phần không bí mật: máy chủ, cổng, database, tài khoản, sslmode, và bảng nào đã chụp thành
dataset nào (tables: {"<dataset id>": "schema.table"}). Mật khẩu chỉ nằm trong secret_encrypted, đã mã hóa; không câu
SELECT nào ở đây trả secret_encrypted ra ngoài, trừ source_secret dùng lúc kết nối. */

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một nguồn database như lưu trong bảng, không có phần bí mật.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SourceRow {
    pub id: Uuid,
    pub kind: String,
    pub config: Value,
    pub created_at: DateTime<Utc>,
}

impl Store {
    pub async fn create_source(
        &self,
        ws: &WorkspaceId,
        kind: &str,
        config: &Value,
        secret_encrypted: &[u8],
    ) -> Result<SourceRow, AppError> {
        sqlx::query_as(
            "INSERT INTO sources (workspace_id, kind, config, secret_encrypted) VALUES ($1, $2, $3, $4)
             RETURNING id, kind, config, created_at",
        )
        .bind(ws.as_uuid())
        .bind(kind)
        .bind(config)
        .bind(secret_encrypted)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create source", e))
    }

    /// Nguồn database của workspace (nguồn kiểu file tạo riêng cho từng dataset, không liệt kê).
    pub async fn list_sources(&self, ws: &WorkspaceId) -> Result<Vec<SourceRow>, AppError> {
        sqlx::query_as(
            "SELECT id, kind, config, created_at FROM sources
             WHERE workspace_id = $1 AND kind <> 'local_file' ORDER BY created_at",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list sources", e))
    }

    pub async fn get_source(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<SourceRow>, AppError> {
        sqlx::query_as(
            "SELECT id, kind, config, created_at FROM sources
             WHERE workspace_id = $1 AND id = $2 AND kind <> 'local_file'",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get source", e))
    }

    /// Phần mã hóa của nguồn, chỉ dùng để kết nối. Không đưa vào SourceRow, để không lỡ trả nó ra API.
    pub async fn source_secret(&self, ws: &WorkspaceId, id: Uuid) -> Result<Vec<u8>, AppError> {
        let secret: Option<Option<Vec<u8>>> = sqlx::query_scalar(
            "SELECT secret_encrypted FROM sources WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get source secret", e))?;
        secret
            .flatten()
            .ok_or_else(|| AppError::NotFound("Không có nguồn này".to_string()))
    }

    /// Ghi nhớ dataset_id được chụp từ bảng nào của nguồn, để lần chụp lại không phải hỏi lại tên bảng.
    pub async fn record_source_table(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
        dataset_id: Uuid,
        table: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE sources SET config = jsonb_set(config, '{tables}',
                 COALESCE(config->'tables', '{}'::jsonb) || jsonb_build_object($3::text, $4::text))
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .bind(dataset_id.to_string())
        .bind(table)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("record source table", e))?;
        Ok(())
    }

    /// Xóa nguồn. Còn dataset chụp từ nguồn thì khóa ngoại RESTRICT chặn; trả false để báo người dùng xóa dataset
    /// trước, không xóa dataset thay họ.
    pub async fn delete_source(&self, ws: &WorkspaceId, id: Uuid) -> Result<bool, AppError> {
        let in_use: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM datasets WHERE workspace_id = $1 AND source_id = $2)",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("delete source", e))?;
        if in_use {
            return Ok(false);
        }
        sqlx::query(
            "DELETE FROM sources WHERE workspace_id = $1 AND id = $2 AND kind <> 'local_file'",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("delete source", e))?;
        Ok(true)
    }
}
