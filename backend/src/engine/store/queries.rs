// SPDX-License-Identifier: Apache-2.0
/* engine/store/queries.rs - lịch sử câu hỏi.
dataset_id và version_id luôn ghi cùng nhau trong một câu lệnh, vì khóa ngoại theo cặp đòi phiên bản thuộc đúng
dataset của câu hỏi. */

use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    Ok,
    Rejected,
    Error,
}

impl QueryStatus {
    fn as_str(self) -> &'static str {
        match self {
            QueryStatus::Ok => "ok",
            QueryStatus::Rejected => "rejected",
            QueryStatus::Error => "error",
        }
    }
}

pub struct NewQuery<'a> {
    /// Cặp dataset và phiên bản đã dùng để trả lời; None khi câu hỏi không gắn với dataset nào.
    pub target: Option<(Uuid, Uuid)>,
    pub user_id: Option<Uuid>,
    pub question: &'a str,
    pub sql_generated: Option<&'a str>,
    pub sql_guarded: Option<&'a str>,
    pub status: QueryStatus,
    pub error: Option<&'a str>,
    pub timings: serde_json::Value,
    pub row_count: Option<i64>,
}

impl Store {
    pub async fn record_query(&self, ws: &WorkspaceId, q: &NewQuery<'_>) -> Result<Uuid, AppError> {
        // Tách cặp thành hai giá trị cùng có hoặc cùng không: kiểu Option<(Uuid, Uuid)> làm cho việc ghi lệch một
        // trong hai không thể xảy ra từ phía code.
        let (dataset_id, version_id) = match q.target {
            Some((d, v)) => (Some(d), Some(v)),
            None => (None, None),
        };
        sqlx::query_scalar(
            "INSERT INTO queries (workspace_id, dataset_id, version_id, user_id, question, sql_generated,
                                  sql_guarded, status, error, timings, row_count)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(version_id)
        .bind(q.user_id)
        .bind(q.question)
        .bind(q.sql_generated)
        .bind(q.sql_guarded)
        .bind(q.status.as_str())
        .bind(q.error)
        .bind(&q.timings)
        .bind(q.row_count)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("record query", e))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, store, temp_workspace};
    use super::super::{NewVersion, SourceKind};
    use super::*;

    #[tokio::test]
    async fn records_query_with_dataset_and_version() {
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
        let v = store
            .create_version(
                &ws,
                ds.id,
                &NewVersion {
                    source_kind: SourceKind::Csv,
                    file_hash: "h",
                    parquet_path: "/data/x.parquet",
                    row_count: 1,
                },
                &[],
            )
            .await
            .unwrap();
        let q = NewQuery {
            target: Some((ds.id, v.id)),
            user_id: None,
            question: "doanh thu quý 3",
            sql_generated: Some("SELECT 1"),
            sql_guarded: Some("SELECT 1 LIMIT 10000"),
            status: QueryStatus::Ok,
            error: None,
            timings: serde_json::json!({"llm_ms": 1}),
            row_count: Some(1),
        };
        store.record_query(&ws, &q).await.unwrap();

        // Câu hỏi trỏ tới dataset của workspace khác bị database từ chối.
        let other = temp_workspace(&store).await;
        let err = store.record_query(&other, &q).await.unwrap_err();
        assert!(
            matches!(err, AppError::Internal(ref m) if m.contains("foreign key")),
            "unexpected error: {err:?}"
        );

        drop_workspace(&store, ws).await;
        drop_workspace(&store, other).await;
    }
}
