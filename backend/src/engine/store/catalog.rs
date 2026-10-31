// SPDX-License-Identifier: Apache-2.0
/* engine/store/catalog.rs - dataset như người dùng thấy (#257, migration 0017): một database đã kết nối hay một file,
bảng nằm bên trong. Trong Postgres, dataset là một dòng `sources` (nguồn Postgres, hay nguồn local_file của đúng một
file), bảng là một dòng `datasets` nối qua datasets.source_id; tên bảng Postgres giữ như cũ để không phải sửa mọi câu
SQL. service/catalog/ dựng response từ đây và từ Store::list_datasets (bảng, phiên bản hiện hành). */

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một dataset, không có cấu hình kết nối (cấu hình chỉ admin đọc được qua /sources).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CatalogSourceRow {
    pub id: Uuid,
    /// `postgres` hay `local_file`.
    pub kind: String,
    pub name: String,
    pub domain: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Một cột của phiên bản hiện hành của một bảng, phần giao diện cần để xem schema.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CatalogColumnRow {
    /// Id bảng (datasets.id).
    pub table_id: Uuid,
    pub name: String,
    pub data_type: String,
    pub role: String,
    pub description: Option<String>,
    pub description_source: Option<String>,
    pub is_sensitive: bool,
    pub aliases: Vec<String>,
}

// Nguồn file không còn bảng nào (lần nạp lỗi giữa chừng để lại) thì không phải dataset; nguồn database chưa chụp bảng
// nào vẫn là dataset, để admin thấy và chụp.
const CATALOG_SELECT: &str = "SELECT s.id, s.kind, s.name, s.domain, s.created_at FROM sources s
     WHERE s.workspace_id = $1
       AND (s.kind <> 'local_file' OR EXISTS (SELECT 1 FROM datasets d WHERE d.source_id = s.id))";

impl Store {
    /// Mọi dataset của workspace, cũ trước, để thẻ trên giao diện không đổi chỗ khi có dataset mới.
    pub async fn list_catalog_sources(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<CatalogSourceRow>, AppError> {
        let sql = format!("{CATALOG_SELECT} ORDER BY s.created_at, s.id");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list catalog sources", e))
    }

    /// Một dataset của workspace; None khi không có hay thuộc workspace khác.
    pub async fn get_catalog_source(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<CatalogSourceRow>, AppError> {
        let sql = format!("{CATALOG_SELECT} AND s.id = $2");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("get catalog source", e))
    }

    /// Cột của phiên bản hiện hành của mọi bảng trong workspace, hay chỉ các bảng của dataset `source_id`, theo thứ tự
    /// cột trong bảng. Một câu cho mọi bảng, không hỏi Postgres một lần cho mỗi bảng.
    pub async fn list_catalog_columns(
        &self,
        ws: &WorkspaceId,
        source_id: Option<Uuid>,
    ) -> Result<Vec<CatalogColumnRow>, AppError> {
        sqlx::query_as(
            "SELECT d.id AS table_id, c.name, c.data_type, c.role, c.description, c.description_source,
                    c.is_sensitive, c.aliases
             FROM datasets d
             JOIN columns c ON c.workspace_id = d.workspace_id AND c.version_id = d.current_version_id
             WHERE d.workspace_id = $1 AND ($2::uuid IS NULL OR d.source_id = $2)
             ORDER BY d.id, c.position",
        )
        .bind(ws.as_uuid())
        .bind(source_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list catalog columns", e))
    }
}
