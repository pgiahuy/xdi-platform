// SPDX-License-Identifier: Apache-2.0
/* engine/store/metric_edits.rs - đọc và ghi chỉ số từ màn Chỉ số (#185, service/metrics/). Đọc cho prompt nằm ở
metrics.rs (list_metrics); file này thêm id, phiên bản, nguồn gốc cho giao diện, và ba phép ghi: tạo, sửa, xóa.

Sửa là thêm một dòng cùng tên với version lớn hơn (UNIQUE (source_id, name, version), migration 0017: tên chỉ số không
trùng trong một dataset), không ghi đè:
bản cũ còn trong bảng để biết công thức đã đổi thế nào. Hai lần sửa cùng lúc tính ra cùng một version; lần sau vấp
UNIQUE và trả lỗi BadRequest để người dùng tải lại, không ghi đè lặng lẽ bản của người kia. Xóa là xóa mọi phiên bản
của tên đó: câu hỏi cũ lưu nguyên câu SQL đã chạy (bảng queries), không đọc lại bảng metrics. */

use uuid::Uuid;

use super::{db_error, NewMetric, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Bản đang dùng của một chỉ số (cùng cách chọn như list_metrics), kèm phần giao diện cần.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MetricRecord {
    /// Id của dòng phiên bản này; sửa hay xóa theo id nào của cùng tên cũng như nhau.
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub expression: String,
    pub filter: Option<String>,
    pub time_column: Option<String>,
    pub unit: Option<String>,
    pub confirmed: bool,
    /// formula_table, ui, llm_suggested, sample.
    pub origin: String,
    pub version: i32,
}

/// Mã lỗi Postgres của vi phạm UNIQUE.
const UNIQUE_VIOLATION: &str = "23505";

impl Store {
    /// Chỉ số của một dataset cho màn Chỉ số, mỗi tên một dòng, chọn bản như list_metrics: bản đã xác nhận mới nhất,
    /// không có thì bản nháp mới nhất. Dataset của workspace khác trả rỗng.
    pub async fn list_metric_records(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
    ) -> Result<Vec<MetricRecord>, AppError> {
        sqlx::query_as(
            "SELECT DISTINCT ON (name)
                    id, name, description, expression, filter, time_column, unit,
                    status = 'confirmed' AS confirmed, origin, version
             FROM metrics
             WHERE workspace_id = $1 AND dataset_id = $2
             ORDER BY name, status = 'confirmed' DESC, version DESC",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list metric records", e))
    }

    /// Tên của chỉ số có dòng id này trong dataset; None khi không có hay thuộc workspace khác.
    pub async fn find_metric_name(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        id: Uuid,
    ) -> Result<Option<String>, AppError> {
        sqlx::query_scalar(
            "SELECT name FROM metrics WHERE workspace_id = $1 AND dataset_id = $2 AND id = $3",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("find metric name", e))
    }

    /// Ghi một phiên bản mới của chỉ số do admin viết: origin 'ui', đã xác nhận, version lớn nhất của tên đó cộng 1
    /// (tên mới thì 1). Trả id của dòng vừa ghi.
    pub async fn add_metric_version(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        metric: &NewMetric,
        created_by: Uuid,
    ) -> Result<Uuid, AppError> {
        // INSERT ... SELECT tính version trong cùng câu. Không khóa: hai lần ghi cùng lúc cùng tên thì lần sau vấp
        // UNIQUE (source_id, name, version) và nhận lời báo bên dưới. Version tính theo dataset của bảng (cùng phạm vi
        // với ràng buộc đó); tên đã dùng ở một bảng khác của dataset thì service/metrics/ đã từ chối trước.
        let inserted = sqlx::query_scalar(
            "INSERT INTO metrics (workspace_id, dataset_id, source_id, name, description, expression, filter,
                                  time_column, unit, status, origin, version, created_by)
             SELECT $1, d.id, d.source_id, $3, $4, $5, $6, $7, $8, 'confirmed', 'ui',
                    COALESCE((SELECT MAX(m.version) FROM metrics m WHERE m.source_id = d.source_id AND m.name = $3), 0)
                        + 1,
                    $9
             FROM datasets d WHERE d.workspace_id = $1 AND d.id = $2
             RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(&metric.name)
        .bind(&metric.description)
        .bind(&metric.expression)
        .bind(&metric.filter)
        .bind(&metric.time_column)
        .bind(&metric.unit)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await;
        match inserted {
            Ok(id) => Ok(id),
            Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some(UNIQUE_VIOLATION) => {
                Err(AppError::BadRequest(
                    "Chỉ số này vừa được sửa ở nơi khác; tải lại trang rồi sửa lại".to_string(),
                ))
            }
            Err(e) => Err(db_error("add metric version", e)),
        }
    }

    /// Tên bảng khác của cùng dataset đã có chỉ số tên `name`; None khi tên còn trống trong dataset hay chỉ có ở chính
    /// bảng này. Tên chỉ số không trùng trong một dataset (migration 0017), vì câu hỏi trên dataset thấy chỉ số của mọi
    /// bảng nó đọc.
    pub async fn find_metric_name_elsewhere(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        name: &str,
    ) -> Result<Option<String>, AppError> {
        sqlx::query_scalar(
            "SELECT o.name FROM metrics m
             JOIN datasets o ON o.id = m.dataset_id
             JOIN datasets t ON t.source_id = m.source_id
             WHERE m.workspace_id = $1 AND t.id = $2 AND m.dataset_id <> $2 AND m.name = $3
             LIMIT 1",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("find metric name elsewhere", e))
    }

    /// Xóa mọi phiên bản của một chỉ số theo tên. Trả số dòng đã xóa; 0 khi không có tên đó trong dataset.
    pub async fn delete_metric(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        name: &str,
    ) -> Result<u64, AppError> {
        let deleted = sqlx::query(
            "DELETE FROM metrics WHERE workspace_id = $1 AND dataset_id = $2 AND name = $3",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("delete metric", e))?
        .rows_affected();
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, exec, store, temp_workspace};
    use super::*;

    fn metric(name: &str, expression: &str) -> NewMetric {
        NewMetric {
            name: name.to_string(),
            description: String::new(),
            expression: expression.to_string(),
            filter: None,
            time_column: None,
            unit: None,
        }
    }

    #[tokio::test]
    async fn edits_add_versions_and_delete_removes_all() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws, "Test").await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "Bán hàng", "ban_hang", None)
            .await
            .unwrap();
        let user: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash) VALUES ($1, 'x') RETURNING id",
        )
        .bind(format!("{}@example.com", Uuid::new_v4()))
        .fetch_one(&store.pool)
        .await
        .unwrap();

        let first = store
            .add_metric_version(&ws, ds.id, &metric("revenue", "SUM(\"amount\")"), user)
            .await
            .unwrap();
        store
            .add_metric_version(&ws, ds.id, &metric("revenue", "SUM(\"net\")"), user)
            .await
            .unwrap();
        let records = store.list_metric_records(&ws, ds.id).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].expression, "SUM(\"net\")");
        assert_eq!((records[0].version, records[0].origin.as_str()), (2, "ui"));
        assert!(records[0].confirmed);
        // Id của bản cũ vẫn trỏ về đúng tên.
        assert_eq!(
            store.find_metric_name(&ws, ds.id, first).await.unwrap(),
            Some("revenue".to_string())
        );

        // Workspace khác không thấy, không xóa được.
        let other = temp_workspace(&store).await;
        assert!(store
            .list_metric_records(&other, ds.id)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            store.delete_metric(&other, ds.id, "revenue").await.unwrap(),
            0
        );

        assert_eq!(store.delete_metric(&ws, ds.id, "revenue").await.unwrap(), 2);
        assert!(store.list_metrics(&ws, ds.id).await.unwrap().is_empty());

        exec(&store, &format!("DELETE FROM users WHERE id = '{user}'")).await;
        drop_workspace(&store, ws).await;
        drop_workspace(&store, other).await;
    }
}
