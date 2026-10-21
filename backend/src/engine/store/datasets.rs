// SPDX-License-Identifier: Apache-2.0
/* engine/store/datasets.rs - nguồn, dataset và phiên bản dataset.
Mọi câu SQL lọc theo workspace_id lấy từ WorkspaceId. Lấy dataset bằng id của workspace khác trả None, không trả lỗi
quyền: người gọi không phân biệt được "không có" với "không được thấy", nên không dò được id của workspace khác. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{db_error, is_foreign_key_violation, Store};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileStatus {
    Pending,
    Structure,
    Semantic,
    Failed,
}

impl ProfileStatus {
    fn as_str(self) -> &'static str {
        match self {
            ProfileStatus::Pending => "pending",
            ProfileStatus::Structure => "structure",
            ProfileStatus::Semantic => "semantic",
            ProfileStatus::Failed => "failed",
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
// Cùng danh sách cột, có tiền tố bảng, dùng khi câu SQL nối với datasets.
const VERSION_COLUMNS_V: &str =
    "v.id, v.dataset_id, v.version, v.source_kind, v.file_hash, v.parquet_path, \
    v.row_count, v.profile_status, v.created_at";

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

    /// Tên bảng logic đã có trong workspace chưa (so không phân biệt hoa thường, như chỉ mục datasets_logical_table_key).
    pub async fn logical_table_taken(
        &self,
        ws: &WorkspaceId,
        name: &str,
    ) -> Result<bool, AppError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM datasets WHERE workspace_id = $1 AND lower(logical_table) = lower($2))",
        )
        .bind(ws.as_uuid())
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("check logical table", e))
    }

    /// Phiên bản có cùng nội dung ở bất kỳ dataset nào của workspace: nạp lại đúng bộ file cũ dưới dạng dataset mới
    /// thì trả về cái đã có thay vì tạo bản trùng.
    pub async fn find_version_by_hash_in_workspace(
        &self,
        ws: &WorkspaceId,
        file_hash: &str,
    ) -> Result<Option<DatasetVersion>, AppError> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS} FROM dataset_versions
             WHERE workspace_id = $1 AND file_hash = $2 ORDER BY created_at LIMIT 1"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(file_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find version in workspace", e))
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

    /// Đặt phiên bản hiện hành. Phiên bản phải thuộc đúng dataset này: khóa ngoại trong database chặn nếu sai, và lỗi
    /// đó trả về là "không tìm thấy" chứ không phải lỗi nội bộ, vì id sai đến từ người gọi.
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
        .await;
        match result {
            Ok(r) if r.rows_affected() == 0 => {
                Err(AppError::NotFound("Không tìm thấy dataset".to_string()))
            }
            Ok(_) => Ok(()),
            Err(e) if is_foreign_key_violation(&e) => Err(AppError::NotFound(
                "Không tìm thấy phiên bản này trong dataset".to_string(),
            )),
            Err(e) => Err(db_error("set current version", e)),
        }
    }

    /// Phiên bản hiện hành của dataset, gồm đường dẫn Parquet để DuckDB đọc. None nếu dataset không có trong
    /// workspace này hoặc chưa có phiên bản hiện hành.
    pub async fn get_current_version(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
    ) -> Result<Option<DatasetVersion>, AppError> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS_V} FROM dataset_versions v
             JOIN datasets d ON d.workspace_id = v.workspace_id AND d.current_version_id = v.id
             WHERE d.workspace_id = $1 AND d.id = $2"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("get current version", e))
    }

    /// Ghi tiến độ lập hồ sơ của một phiên bản: pending, rồi structure khi xong lớp cấu trúc, semantic khi xong lớp
    /// ngữ nghĩa, hoặc failed.
    pub async fn set_profile_status(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        status: ProfileStatus,
    ) -> Result<(), AppError> {
        let r = sqlx::query(
            "UPDATE dataset_versions SET profile_status = $3 WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(status.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set profile status", e))?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound("Không tìm thấy phiên bản".to_string()));
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

        assert!(store
            .get_current_version(&ws, ds.id)
            .await
            .unwrap()
            .is_none());
        store.set_current_version(&ws, ds.id, v2.id).await.unwrap();
        let ds = store.get_dataset(&ws, ds.id).await.unwrap().unwrap();
        assert_eq!(ds.current_version_id, Some(v2.id));
        let current = store
            .get_current_version(&ws, ds.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (current.id, current.parquet_path.as_deref()),
            (v2.id, Some("/data/x.parquet"))
        );

        store
            .set_profile_status(&ws, v2.id, ProfileStatus::Structure)
            .await
            .unwrap();
        let current = store
            .get_current_version(&ws, ds.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.profile_status, "structure");

        drop_workspace(&store, ws).await;
    }

    #[tokio::test]
    async fn concurrent_versions_never_share_a_number() {
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

        // Tám lần tạo phiên bản cùng lúc. Lần nào thành công thì số phải khác nhau; lần nào thất bại thì phải là lỗi
        // trùng khóa (người gọi thử lại), không được ra hai phiên bản cùng số.
        let mut tasks = Vec::new();
        for i in 0..8 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                let hash = format!("h{i}");
                let new = NewVersion {
                    source_kind: SourceKind::Csv,
                    file_hash: &hash,
                    parquet_path: "/data/x.parquet",
                    row_count: 1,
                };
                store.create_version(&ws, ds.id, &new).await
            }));
        }
        let mut numbers = Vec::new();
        for t in tasks {
            match t.await.unwrap() {
                Ok(v) => numbers.push(v.version),
                Err(AppError::Internal(m)) => {
                    assert!(m.contains("duplicate key"), "unexpected error: {m}")
                }
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert!(!numbers.is_empty(), "at least one insert must succeed");
        let mut unique = numbers.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            numbers.len(),
            "duplicate version numbers: {numbers:?}"
        );

        drop_workspace(&store, ws).await;
    }
}
