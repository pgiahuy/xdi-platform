// SPDX-License-Identifier: Apache-2.0
/* engine/store/datasets.rs - nguồn, dataset và phiên bản dataset.
Mọi câu SQL lọc theo workspace_id lấy từ WorkspaceId. Lấy dataset bằng id của workspace khác trả None, không trả lỗi
quyền: người gọi không phân biệt được "không có" với "không được thấy", nên không dò được id của workspace khác. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::columns::{insert_columns, NewColumn};
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
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileStatus::Pending => "pending",
            ProfileStatus::Structure => "structure",
            ProfileStatus::Semantic => "semantic",
            ProfileStatus::Failed => "failed",
        }
    }
}

// Trường Option luôn có trong JSON (giá trị null khi trống), nên đánh dấu required trong đặc tả: thiếu dấu này thì
// utoipa coi là trường tùy chọn, kiểu TypeScript sinh ra thành `domain?: ...` và frontend phải xử lý cả hai trường hợp.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Dataset {
    pub id: Uuid,
    pub name: String,
    /// Table name used in SQL, derived from the dataset name.
    pub logical_table: String,
    #[schema(required)]
    pub domain: Option<String>,
    #[schema(required)]
    pub current_version_id: Option<Uuid>,
    /// Profile status of the current version (`pending`, `structure`, `semantic`, `failed`); null while the dataset
    /// has no current version. The UI polls it to know when the semantic profile is done.
    #[schema(required)]
    #[sqlx(default)]
    pub current_profile_status: Option<String>,
    /// Suggested questions of the current version, written by the semantic profile; empty while that step has not
    /// finished.
    #[schema(required)]
    // Câu hỏi gợi ý nằm trên dataset_versions (đi theo phiên bản): list_datasets và get_dataset nối sang phiên bản
    // hiện hành để lấy; create_dataset (RETURNING không nối được) nhờ `default` mà có mảng rỗng.
    #[sqlx(json, default)]
    pub suggested_questions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DatasetVersion {
    pub id: Uuid,
    pub dataset_id: Uuid,
    pub version: i32,
    pub source_kind: String,
    #[schema(required)]
    pub file_hash: Option<String>,
    #[schema(required)]
    pub parquet_path: Option<String>,
    #[schema(required)]
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

/// Phiên bản chưa lập hồ sơ cột, ở bất kỳ workspace nào (xem list_pending_versions).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingVersion {
    pub workspace_id: Uuid,
    pub id: Uuid,
    pub parquet_path: Option<String>,
    pub row_count: Option<i64>,
}

/// Phiên bản đã có hồ sơ cấu trúc mà chưa qua lớp ngữ nghĩa, ở bất kỳ workspace nào (xem list_semantic_candidates).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SemanticCandidate {
    pub workspace_id: Uuid,
    pub id: Uuid,
    pub dataset_id: Uuid,
    pub row_count: Option<i64>,
}

const DATASET_COLUMNS: &str = "id, name, logical_table, domain, current_version_id, created_at";
// Cùng danh sách đó kèm câu hỏi gợi ý, có tiền tố bảng, dùng khi câu SQL nối datasets với phiên bản hiện hành.
// suggested_questions là jsonb trên dataset_versions; dataset chưa có phiên bản hiện hành (mới tạo, chưa nạp xong)
// hay phiên bản chưa qua lớp ngữ nghĩa thì ra mảng rỗng.
const DATASET_COLUMNS_D: &str =
    "d.id, d.name, d.logical_table, d.domain, d.current_version_id, d.created_at, \
    v.profile_status AS current_profile_status, \
    COALESCE(v.suggested_questions, '[]'::jsonb) AS suggested_questions";
const CURRENT_VERSION_JOIN: &str =
    "FROM datasets d LEFT JOIN dataset_versions v ON v.workspace_id = d.workspace_id \
    AND v.id = d.current_version_id";
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
        let sql = format!(
            "SELECT {DATASET_COLUMNS_D} {CURRENT_VERSION_JOIN} WHERE d.workspace_id = $1 ORDER BY d.created_at DESC"
        );
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
        let sql = format!(
            "SELECT {DATASET_COLUMNS_D} {CURRENT_VERSION_JOIN} WHERE d.workspace_id = $1 AND d.id = $2"
        );
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

    /// Tạo phiên bản số kế tiếp cùng hồ sơ cột của nó, trong một giao dịch: hồ sơ ghi lỗi (giá trị Postgres từ chối,
    /// mất kết nối giữa chừng) thì phiên bản cũng không còn, nên nạp lại cùng bộ file không vấp phải một phiên bản trùng
    /// hash mà thiếu hồ sơ. columns rỗng thì phiên bản ở pending (chưa có hồ sơ), có cột thì ở structure.
    /// Số phiên bản được tính trong cùng câu lệnh; hai lần nạp cùng lúc mà trùng số thì một lần lỗi nhờ ràng buộc
    /// UNIQUE (dataset_id, version), không bao giờ ra hai phiên bản cùng số.
    pub async fn create_version(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        new: &NewVersion<'_>,
        columns: &[NewColumn],
    ) -> Result<DatasetVersion, AppError> {
        let status = if columns.is_empty() {
            ProfileStatus::Pending
        } else {
            ProfileStatus::Structure
        };
        let sql = format!(
            "INSERT INTO dataset_versions
                 (workspace_id, dataset_id, version, source_kind, file_hash, parquet_path, row_count, profile_status)
             SELECT $1, $2, COALESCE(MAX(version), 0) + 1, $3, $4, $5, $6, $7
             FROM dataset_versions WHERE workspace_id = $1 AND dataset_id = $2
             RETURNING {VERSION_COLUMNS}"
        );
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin create version", e))?;
        let version: DatasetVersion = sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .bind(new.source_kind.as_str())
            .bind(new.file_hash)
            .bind(new.parquet_path)
            .bind(new.row_count)
            .bind(status.as_str())
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_error("create version", e))?;
        // Lỗi ở đây trả về trước commit: tx bị hủy thì sqlx rollback, xóa luôn dòng phiên bản vừa thêm.
        insert_columns(&mut tx, ws, version.id, columns).await?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit create version", e))?;
        Ok(version)
    }

    /// Xóa một dataset chưa có phiên bản nào, kèm nguồn của nó, khi lần nạp tạo ra dataset đó thất bại trước khi có
    /// phiên bản. Dataset đã có phiên bản thì giữ nguyên, kể cả phiên bản một lần nạp khác thêm vào cùng lúc (danh sách
    /// dataset hiện cả dataset chưa có phiên bản, nên người khác có thể nạp vào nó theo id).
    ///
    /// Ba câu riêng trong một giao dịch, không gộp thành một DELETE ... WHERE NOT EXISTS: câu gộp chờ lần nạp kia commit
    /// rồi vẫn xét NOT EXISTS trên ảnh chụp cũ, xóa dataset và CASCADE xóa luôn phiên bản vừa commit. Ở đây, SELECT ...
    /// FOR UPDATE khóa dòng dataset trước. Khóa này xung đột với khóa mà khóa ngoại của dataset_versions lấy khi chèn
    /// phiên bản, nên một bên phải chờ bên kia: lần nạp kia đang chèn thì câu này chờ nó commit, và câu kiểm phiên bản
    /// sau đó (READ COMMITTED, ảnh chụp mới cho mỗi câu) thấy phiên bản đã commit; câu này khóa trước thì lần nạp kia
    /// chờ, rồi vấp khóa ngoại khi dataset đã bị xóa. Test: deleting_a_dataset_waits_for_a_concurrent_version.
    pub async fn delete_dataset_without_versions(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
    ) -> Result<(), AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin delete dataset", e))?;
        let source: Option<Uuid> = sqlx::query_scalar(
            "SELECT source_id FROM datasets WHERE workspace_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| db_error("lock dataset", e))?;
        let Some(source) = source else {
            return Ok(());
        };
        let has_versions: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM dataset_versions WHERE workspace_id = $1 AND dataset_id = $2)",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("check dataset versions", e))?;
        // Ra khỏi hàm mà chưa commit thì sqlx rollback, nhả khóa.
        if has_versions {
            return Ok(());
        }
        sqlx::query("DELETE FROM datasets WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("delete dataset", e))?;
        // Nguồn kiểu file tạo riêng cho từng dataset; khóa ngoại RESTRICT nên phải xóa sau dataset.
        sqlx::query(
            "DELETE FROM sources s WHERE s.workspace_id = $1 AND s.id = $2
               AND NOT EXISTS (SELECT 1 FROM datasets d WHERE d.source_id = s.id)",
        )
        .bind(ws.as_uuid())
        .bind(source)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("delete source", e))?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit delete dataset", e))
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

    /// Phiên bản còn profile_status = pending ở mọi workspace, cũ nhất trước. Ngoại lệ có chủ đích của quy tắc lọc theo
    /// WorkspaceId (xem store/mod.rs): việc nền lúc khởi động lập hồ sơ cho phiên bản nạp trước khi có bước hồ sơ, và
    /// lúc đó chưa có workspace nào để lọc. Chỉ auth::pending_profiles gọi, để dựng WorkspaceId từ chính các dòng này.
    pub async fn list_pending_versions(&self) -> Result<Vec<PendingVersion>, AppError> {
        sqlx::query_as(
            "SELECT workspace_id, id, parquet_path, row_count FROM dataset_versions
             WHERE profile_status = $1 ORDER BY created_at",
        )
        .bind(ProfileStatus::Pending.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list pending versions", e))
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

    /// Phiên bản đang ở structure và chưa thử lớp ngữ nghĩa với provider đang cấu hình, ở mọi workspace, cũ nhất
    /// trước. Ngoại lệ có chủ đích của quy tắc lọc theo WorkspaceId, như list_pending_versions: việc nền lúc khởi
    /// động lập hồ sơ ngữ nghĩa cho phiên bản nạp trước khi có bước này (#23), lúc đó chưa có workspace nào để lọc.
    /// Phiên bản đã thử với provider này mà lỗi (semantic_attempted_with trùng tên) không nằm trong danh sách; đổi
    /// LLM_PROVIDER thì được thử lại.
    pub async fn list_semantic_candidates(
        &self,
        provider: &str,
    ) -> Result<Vec<SemanticCandidate>, AppError> {
        sqlx::query_as(
            "SELECT workspace_id, id, dataset_id, row_count FROM dataset_versions
             WHERE profile_status = $1 AND (semantic_attempted_with IS NULL OR semantic_attempted_with <> $2)
             ORDER BY created_at",
        )
        .bind(ProfileStatus::Structure.as_str())
        .bind(provider)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list semantic candidates", e))
    }

    /// Phiên bản đã có hồ sơ (structure hay semantic) còn cột chưa đánh dấu nhạy cảm, ở mọi workspace, cũ nhất
    /// trước. Ngoại lệ có chủ đích của quy tắc lọc theo WorkspaceId, như list_pending_versions: việc nền lúc
    /// khởi động chạy lại luật đánh dấu nhạy cảm cho phiên bản nạp trước khi luật nằm ở lớp cấu trúc, lúc đó chưa có
    /// workspace nào để lọc.
    pub async fn list_versions_for_sensitive_scan(
        &self,
    ) -> Result<Vec<SemanticCandidate>, AppError> {
        sqlx::query_as(
            "SELECT v.workspace_id, v.id, v.dataset_id, v.row_count FROM dataset_versions v
             WHERE v.profile_status IN ($1, $2)
               AND EXISTS (SELECT 1 FROM columns c WHERE c.workspace_id = v.workspace_id AND c.version_id = v.id
                           AND NOT c.is_sensitive)
             ORDER BY v.created_at",
        )
        .bind(ProfileStatus::Structure.as_str())
        .bind(ProfileStatus::Semantic.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list versions for sensitive scan", e))
    }

    /// Đánh dấu đã thử lớp ngữ nghĩa với provider này (và câu trả lời không dùng được): việc nền lúc khởi động không
    /// chọn lại phiên bản này cho tới khi đổi LLM_PROVIDER. Chỉ đặt khi phiên bản còn ở structure: đã semantic thì
    /// không cần, và không ghi đè một lượt vừa thành công.
    pub async fn mark_semantic_attempted(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        provider: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE dataset_versions SET semantic_attempted_with = $4
             WHERE workspace_id = $1 AND id = $2 AND profile_status = $3",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(ProfileStatus::Structure.as_str())
        .bind(provider)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("mark semantic attempted", e))?;
        Ok(())
    }

    /// profile_status hiện tại của một phiên bản; None khi không có (workspace khác hay đã xóa). semantic.rs đọc lại
    /// sau khi lấy được permit, trước khi gọi mô hình: hai lượt xếp hàng cho cùng một phiên bản thì lượt sau thấy
    /// semantic và dừng, không gọi mô hình hai lần.
    pub async fn get_profile_status(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        sqlx::query_scalar(
            "SELECT profile_status FROM dataset_versions WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get profile status", e))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, fetch_count, store, temp_workspace};
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
            .create_version(&a, ds.id, &new_version("h1"), &[])
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
            .create_version(&ws, ds.id, &new_version("h1"), &[])
            .await
            .unwrap();
        let v2 = store
            .create_version(&ws, ds.id, &new_version("h2"), &[])
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
                store.create_version(&ws, ds.id, &new, &[]).await
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

    /// Lần nạp B đang chèn phiên bản (giao dịch chưa commit) vào dataset D thì lệnh xóa D của lần nạp A chờ, rồi thấy
    /// phiên bản của B và giữ D. Trước khi có FOR UPDATE, A xóa D và CASCADE xóa luôn phiên bản của B.
    #[tokio::test]
    async fn deleting_a_dataset_waits_for_a_concurrent_version() {
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

        let mut b = store.pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO dataset_versions (workspace_id, dataset_id, version, source_kind) VALUES ($1, $2, 1, 'csv')",
        )
        .bind(ws.as_uuid())
        .bind(ds.id)
        .execute(&mut *b)
        .await
        .unwrap();
        let a = {
            let store = store.clone();
            tokio::spawn(async move { store.delete_dataset_without_versions(&ws, ds.id).await })
        };
        // Cho A kịp tới chỗ chờ khóa. A chưa tới mà B đã commit thì A vẫn thấy phiên bản: test không sai theo hướng
        // ngược lại, chỉ mất tác dụng.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(!a.is_finished(), "delete must wait for the version insert");
        b.commit().await.unwrap();
        a.await.unwrap().unwrap();

        assert!(store.get_dataset(&ws, ds.id).await.unwrap().is_some());
        let versions = fetch_count(
            &store,
            &format!(
                "SELECT count(*) FROM dataset_versions WHERE dataset_id = '{}'",
                ds.id
            ),
        )
        .await;
        assert_eq!(versions, 1);

        drop_workspace(&store, ws).await;
    }
}
