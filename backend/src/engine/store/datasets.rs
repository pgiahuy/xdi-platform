// SPDX-License-Identifier: Apache-2.0
/* engine/store/datasets.rs - nguồn, dataset và phiên bản dataset.
Mọi câu SQL lọc theo workspace_id lấy từ WorkspaceId. Lấy dataset bằng id của workspace khác trả None, không trả lỗi
quyền: người gọi không phân biệt được "không có" với "không được thấy", nên không dò được id của workspace khác. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Csv,
    Parquet,
    Xlsx,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::Csv => "csv",
            SourceKind::Parquet => "parquet",
            SourceKind::Xlsx => "xlsx",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Dataset {
    pub id: Uuid,
    pub name: String,
    pub logical_table: String,
    pub domain: Option<String>,
    pub current_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DatasetVersion {
    pub id: Uuid,
    pub dataset_id: Uuid,
    pub version: i32,
    pub source_kind: String,
    pub file_hash: Option<String>,
    pub parquet_path: Option<String>,
    pub row_count: Option<i64>,
    pub profile_status: String,
    pub created_at: DateTime<Utc>,
}

/// Dữ liệu cho một phiên bản mới của dataset nạp từ file.
pub struct NewVersion<'a> {
    pub source_kind: SourceKind,
    pub file_hash: &'a str,
    pub parquet_path: &'a str,
    pub row_count: i64,
}

const DATASET_COLUMNS: &str = "id, name, logical_table, domain, current_version_id, created_at";
const VERSION_COLUMNS: &str =
    "id, dataset_id, version, source_kind, file_hash, parquet_path, row_count, profile_status, created_at";

impl Store {
    /// Nguồn kiểu file nạp lên. Nguồn database thêm khi có tính năng nối database.
    pub async fn create_local_source(&self, ws: &WorkspaceId) -> Result<Uuid, AppError> {
        sqlx::query_scalar(
            "INSERT INTO sources (workspace_id, kind) VALUES ($1, 'local_file') RETURNING id",
        )
        .bind(ws.as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create source", e))
    }

    pub async fn create_dataset(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
        name: &str,
        logical_table: &str,
        created_by: Option<Uuid>,
    ) -> Result<Dataset, AppError> {
        let sql = format!(
            "INSERT INTO datasets (workspace_id, source_id, name, logical_table, created_by)
             VALUES ($1, $2, $3, $4, $5) RETURNING {DATASET_COLUMNS}"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(source_id)
            .bind(name)
            .bind(logical_table)
            .bind(created_by)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| db_error("create dataset", e))
    }

    pub async fn list_datasets(&self, ws: &WorkspaceId) -> Result<Vec<Dataset>, AppError> {
        let sql =
            format!("SELECT {DATASET_COLUMNS} FROM datasets WHERE workspace_id = $1 ORDER BY created_at DESC");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list datasets", e))
    }

    pub async fn get_dataset(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Dataset>, AppError> {
        let sql =
            format!("SELECT {DATASET_COLUMNS} FROM datasets WHERE workspace_id = $1 AND id = $2");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("get dataset", e))
    }

    /// Phiên bản có cùng nội dung file, nếu đã nạp trước đó. Dùng để bỏ qua khi người dùng nạp lại đúng file cũ.
    pub async fn find_version_by_hash(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        file_hash: &str,
    ) -> Result<Option<DatasetVersion>, AppError> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS} FROM dataset_versions
             WHERE workspace_id = $1 AND dataset_id = $2 AND file_hash = $3"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .bind(file_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find version", e))
    }

    /// Tạo phiên bản số kế tiếp. Số được tính trong cùng câu lệnh; hai lần nạp cùng lúc mà trùng số thì một lần lỗi
    /// nhờ ràng buộc UNIQUE (dataset_id, version), không bao giờ ra hai phiên bản cùng số.
    pub async fn create_version(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        new: &NewVersion<'_>,
    ) -> Result<DatasetVersion, AppError> {
        let sql = format!(
            "INSERT INTO dataset_versions
                 (workspace_id, dataset_id, version, source_kind, file_hash, parquet_path, row_count)
             SELECT $1, $2, COALESCE(MAX(version), 0) + 1, $3, $4, $5, $6
             FROM dataset_versions WHERE workspace_id = $1 AND dataset_id = $2
             RETURNING {VERSION_COLUMNS}"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .bind(new.source_kind.as_str())
            .bind(new.file_hash)
            .bind(new.parquet_path)
            .bind(new.row_count)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| db_error("create version", e))
    }

    /// Đặt phiên bản hiện hành. Phiên bản phải thuộc đúng dataset này; khóa ngoại trong database chặn nếu sai.
    pub async fn set_current_version(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        version_id: Uuid,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE datasets SET current_version_id = $3 WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(version_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set current version", e))?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Không tìm thấy dataset".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, store, temp_workspace};
    use super::*;

    fn new_version(hash: &str) -> NewVersion<'_> {
        NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: hash,
            parquet_path: "/data/x.parquet",
            row_count: 10,
        }
    }

    #[tokio::test]
    async fn workspace_cannot_see_another_workspace_dataset() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let a = temp_workspace(&store).await;
        let b = temp_workspace(&store).await;
        let src = store.create_local_source(&a).await.unwrap();
        let ds = store
            .create_dataset(&a, src, "Sales", "sales", None)
            .await
            .unwrap();

        assert_eq!(store.list_datasets(&a).await.unwrap().len(), 1);
        assert!(store.list_datasets(&b).await.unwrap().is_empty());
        // Biết đúng id vẫn không lấy được từ workspace khác.
        assert!(store.get_dataset(&b, ds.id).await.unwrap().is_none());
        assert!(store.get_dataset(&a, ds.id).await.unwrap().is_some());
        // Không đặt được phiên bản hiện hành cho dataset của workspace khác.
        let v = store
            .create_version(&a, ds.id, &new_version("h1"))
            .await
            .unwrap();
        let err = store
            .set_current_version(&b, ds.id, v.id)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "unexpected error: {err:?}"
        );

        drop_workspace(&store, a).await;
        drop_workspace(&store, b).await;
    }

    #[tokio::test]
    async fn versions_are_numbered_and_found_by_hash() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws).await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "Sales", "sales", None)
            .await
            .unwrap();

        let v1 = store
            .create_version(&ws, ds.id, &new_version("h1"))
            .await
            .unwrap();
        let v2 = store
            .create_version(&ws, ds.id, &new_version("h2"))
            .await
            .unwrap();
        assert_eq!((v1.version, v2.version), (1, 2));

        let found = store.find_version_by_hash(&ws, ds.id, "h1").await.unwrap();
        assert_eq!(found.map(|v| v.id), Some(v1.id));
        assert!(store
            .find_version_by_hash(&ws, ds.id, "h3")
            .await
            .unwrap()
            .is_none());

        store.set_current_version(&ws, ds.id, v2.id).await.unwrap();
        let ds = store.get_dataset(&ws, ds.id).await.unwrap().unwrap();
        assert_eq!(ds.current_version_id, Some(v2.id));

        drop_workspace(&store, ws).await;
    }
}
