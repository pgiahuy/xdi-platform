// SPDX-License-Identifier: Apache-2.0
/* engine/store/sources.rs - nguồn database của workspace (bảng sources, migration 0001). Logic (mã hóa, thử kết nối,
chụp bảng) ở service/sources/.

config (jsonb) chỉ giữ phần không bí mật: máy chủ, cổng, database, tài khoản, sslmode, và bảng nào đã chụp thành
dataset nào (tables: {"<dataset id>": "schema.table"}). Mật khẩu chỉ nằm trong secret_encrypted, đã mã hóa; không câu
SELECT nào ở đây trả secret_encrypted ra ngoài, trừ source_secret dùng lúc kết nối.

SourceModel là mô hình dữ liệu của một phiên bản chụp từ database (dataset_versions.source_model, migration 0009):
bảng chính và các bảng nối nhiều-một. Nó nằm ở engine/store vì Dataset (datasets.rs) trả nó ra API; service/sources/
dựng nó lúc chụp. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// How a dataset version snapshotted from a database was built: the main (fact) table and the tables joined
/// many-to-one along foreign keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SourceModel {
    /// `schema.table` whose rows become the rows of the dataset.
    pub fact_table: String,
    /// Rows of the main table and of the joined result, counted in the same snapshot; always equal (a snapshot whose
    /// join would change the row count is refused).
    pub fact_rows: i64,
    pub joined_rows: i64,
    /// Joins in the order they appear in the SQL; empty when only the main table was copied.
    pub joins: Vec<ModelJoin>,
}

/// One many-to-one join of a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ModelJoin {
    /// Relation id from the relation list; a snapshot again resolves the same ids.
    pub relation: String,
    /// Foreign-key constraint name in the source database.
    pub constraint: String,
    /// `schema.table` holding the foreign key: the main table, or a table joined before (two hops).
    pub from_table: String,
    pub from_columns: Vec<String>,
    /// `schema.table` the foreign key points to.
    pub to_table: String,
    pub to_columns: Vec<String>,
    /// Prefix of the dataset columns copied from `to_table`: a column `c` of it is named `<prefix>.c`.
    pub prefix: String,
    /// Dataset columns copied from `to_table`, as named in the dataset.
    pub columns: Vec<String>,
    /// Dataset columns that identify one row of `to_table` (the foreign-key values pointing to it).
    #[serde(default)]
    pub key_columns: Vec<String>,
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

    /// Mô hình của phiên bản hiện hành của dataset, để "Chụp lại" nối đúng các bảng của lần trước. None khi dataset
    /// không có phiên bản hiện hành hay phiên bản đó không ghi mô hình (chụp trước khi có migration 0009).
    pub async fn current_source_model(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
    ) -> Result<Option<SourceModel>, AppError> {
        let model: Option<Option<sqlx::types::Json<SourceModel>>> = sqlx::query_scalar(
            "SELECT v.source_model FROM datasets d
             JOIN dataset_versions v ON v.workspace_id = d.workspace_id AND v.id = d.current_version_id
             WHERE d.workspace_id = $1 AND d.id = $2",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get source model", e))?;
        Ok(model.flatten().map(|m| m.0))
    }

    /// Mô hình của một phiên bản (luồng hỏi đọc để chặn SUM trên cột lặp lại, service/ask/repeated.rs). None với
    /// phiên bản nạp từ file hay thuộc workspace khác.
    pub async fn version_source_model(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
    ) -> Result<Option<SourceModel>, AppError> {
        let model: Option<Option<sqlx::types::Json<SourceModel>>> = sqlx::query_scalar(
            "SELECT source_model FROM dataset_versions WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get version source model", e))?;
        Ok(model.flatten().map(|m| m.0))
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
