// SPDX-License-Identifier: Apache-2.0
/* engine/store/relations.rs - quan hệ khóa ngoại giữa các dataset (bảng dataset_relations, migration 0009).
service/sources/ ghi lúc chụp một nhóm bảng từ database; service/ingest.rs đọc để trả Dataset.relations ra API; "Chụp
lại" đọc để tìm cả nhóm bảng nối với nhau.

Phía from là bảng giữ khóa ngoại (phía nhiều), phía to là bảng có khóa chính hay UNIQUE (phía một). Mọi hàm nhận
&WorkspaceId và lọc theo nó; khóa ngoại theo cặp (workspace, dataset) chặn thêm ở database. */

use serde::Serialize;
use uuid::Uuid;

use super::{db_error, is_foreign_key_violation, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một quan hệ cần ghi: khóa ngoại của database nguồn, đã đổi bảng thành dataset.
#[derive(Debug, Clone)]
pub struct NewRelation {
    pub from_dataset_id: Uuid,
    pub from_columns: Vec<String>,
    pub to_dataset_id: Uuid,
    pub to_columns: Vec<String>,
    pub constraint_name: String,
    /// `many_to_one` hay `one_to_one` (CHECK của migration 0009).
    pub cardinality: &'static str,
}

/// A foreign-key relation between two datasets of the workspace, copied from the source database.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DatasetRelation {
    /// Dataset holding the foreign key (the many side).
    pub from_dataset_id: Uuid,
    pub from_dataset_name: String,
    pub from_columns: Vec<String>,
    /// Dataset whose primary key or UNIQUE columns the foreign key points to (the one side).
    pub to_dataset_id: Uuid,
    pub to_dataset_name: String,
    pub to_columns: Vec<String>,
    /// Foreign-key constraint name in the source database.
    pub constraint_name: String,
    /// `many_to_one`, or `one_to_one` when the foreign-key columns are also unique.
    pub cardinality: String,
}

const RELATION_SELECT: &str =
    "SELECT r.from_dataset_id, f.name AS from_dataset_name, r.from_columns,
        r.to_dataset_id, t.name AS to_dataset_name, r.to_columns, r.constraint_name, r.cardinality
     FROM dataset_relations r
     JOIN datasets f ON f.workspace_id = r.workspace_id AND f.id = r.from_dataset_id
     JOIN datasets t ON t.workspace_id = r.workspace_id AND t.id = r.to_dataset_id";

impl Store {
    /// Thay mọi quan hệ giữa các dataset trong `group` bằng `relations`, trong một giao dịch: lần chụp lại thấy khóa
    /// ngoại đã bỏ ở database nguồn thì quan hệ cũ cũng mất, và không ai đọc được nhóm đang thay dở. Quan hệ nối một
    /// dataset của nhóm với dataset ngoài nhóm giữ nguyên: lần chụp này không đọc bảng kia nên không biết nó còn hay
    /// không. Dataset của workspace khác (hay đã xóa) làm khóa ngoại từ chối, cả giao dịch hủy.
    pub async fn replace_relations(
        &self,
        ws: &WorkspaceId,
        group: &[Uuid],
        relations: &[NewRelation],
    ) -> Result<(), AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin replace relations", e))?;
        sqlx::query(
            "DELETE FROM dataset_relations
             WHERE workspace_id = $1 AND from_dataset_id = ANY($2) AND to_dataset_id = ANY($2)",
        )
        .bind(ws.as_uuid())
        .bind(group)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("delete relations", e))?;
        for r in relations {
            // ON CONFLICT: cùng ràng buộc của cùng bảng trước đây trỏ tới một dataset ngoài nhóm (ví dụ bản chụp cũ của
            // bảng đích); lần chụp này thay nó bằng dataset vừa chụp.
            sqlx::query(
                "INSERT INTO dataset_relations (workspace_id, from_dataset_id, from_columns, to_dataset_id, to_columns,
                     constraint_name, cardinality)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (from_dataset_id, constraint_name) DO UPDATE SET
                     from_columns = EXCLUDED.from_columns, to_dataset_id = EXCLUDED.to_dataset_id,
                     to_columns = EXCLUDED.to_columns, cardinality = EXCLUDED.cardinality",
            )
            .bind(ws.as_uuid())
            .bind(r.from_dataset_id)
            .bind(&r.from_columns)
            .bind(r.to_dataset_id)
            .bind(&r.to_columns)
            .bind(&r.constraint_name)
            .bind(r.cardinality)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if is_foreign_key_violation(&e) {
                    AppError::BadRequest("Dataset của quan hệ không có trong workspace".to_string())
                } else {
                    db_error("insert relation", e)
                }
            })?;
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit replace relations", e))
    }

    /// Quan hệ của workspace; có `dataset_id` thì chỉ những quan hệ mà dataset đó ở một trong hai đầu (cả hai chiều).
    /// Thứ tự cố định (tên dataset, tên ràng buộc) để danh sách trên giao diện không nhảy chỗ.
    pub async fn list_relations(
        &self,
        ws: &WorkspaceId,
        dataset_id: Option<Uuid>,
    ) -> Result<Vec<DatasetRelation>, AppError> {
        let sql = format!(
            "{RELATION_SELECT}
             WHERE r.workspace_id = $1 AND ($2::uuid IS NULL OR r.from_dataset_id = $2 OR r.to_dataset_id = $2)
             ORDER BY f.name, r.constraint_name"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list relations", e))
    }
}

/// A dataset linked to this one by a foreign key of the source database.
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct RelatedDataset {
    pub dataset_id: Uuid,
    pub name: String,
    /// `many_to_one` when this dataset holds the foreign key, `one_to_many` when the related dataset holds it.
    pub direction: String,
    /// Key columns of this dataset.
    pub columns: Vec<String>,
    /// Key columns of the related dataset, in the same order.
    pub related_columns: Vec<String>,
}

/// Các dataset nối với `dataset_id`, nhìn từ phía nó. Khóa ngoại trỏ về chính bảng đó (cột quản lý trỏ tới nhân viên)
/// chỉ ghi một lần, theo chiều nhiều-một.
pub fn build_related(dataset_id: Uuid, relations: &[DatasetRelation]) -> Vec<RelatedDataset> {
    let mut related = Vec::new();
    for r in relations {
        if r.from_dataset_id == dataset_id {
            related.push(RelatedDataset {
                dataset_id: r.to_dataset_id,
                name: r.to_dataset_name.clone(),
                direction: "many_to_one".to_string(),
                columns: r.from_columns.clone(),
                related_columns: r.to_columns.clone(),
            });
        } else if r.to_dataset_id == dataset_id {
            related.push(RelatedDataset {
                dataset_id: r.from_dataset_id,
                name: r.from_dataset_name.clone(),
                direction: "one_to_many".to_string(),
                columns: r.to_columns.clone(),
                related_columns: r.from_columns.clone(),
            });
        }
    }
    related
}
