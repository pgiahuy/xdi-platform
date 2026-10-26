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
    pub fn as_str(self) -> &'static str {
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
    /// Phần giải thích của câu "tại sao" (#38), gồm điều kiện hai lát cắt cho truy dòng nguồn (#34); None với câu thường.
    pub explanation: Option<serde_json::Value>,
}

/// Phần của một dòng queries mà truy dòng nguồn (#34) cần.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredQuery {
    /// None khi câu không gắn dataset, hay dataset đã xóa.
    pub dataset_id: Option<Uuid>,
    /// None khi phiên bản đã xóa.
    pub version_id: Option<Uuid>,
    /// Chữ của QueryStatus::as_str.
    pub status: String,
    pub sql_guarded: Option<String>,
    /// Form và phân rã của câu "tại sao" (khuôn ở service/ask_why.rs); None với câu thường.
    pub explanation: Option<serde_json::Value>,
}

/// Một câu hỏi đã được đánh dấu trả lời đúng, làm ví dụ trong prompt sinh SQL.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PromptExample {
    pub question: String,
    pub sql: String,
}

impl Store {
    /// Tối đa `limit` câu đã đánh dấu đúng của dataset, mới nhất trước. Lấy SQL mô hình viết (sql_generated), không lấy
    /// bản sau guard: bản đó có LIMIT và điều kiện phân quyền theo vai của người đã hỏi, đưa vào prompt thì mô hình sẽ
    /// chép điều kiện của người khác vào câu mới.
    pub async fn list_correct_queries(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        limit: i64,
    ) -> Result<Vec<PromptExample>, AppError> {
        sqlx::query_as(
            "SELECT question, sql_generated AS sql FROM queries
             WHERE workspace_id = $1 AND dataset_id = $2 AND feedback = 'correct' AND status = 'ok'
               AND sql_generated IS NOT NULL
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list correct queries", e))
    }

    /// Một câu hỏi của workspace theo id; None nếu không có hay thuộc workspace khác.
    pub async fn get_query(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<StoredQuery>, AppError> {
        sqlx::query_as(
            "SELECT dataset_id, version_id, status, sql_guarded, explanation FROM queries
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get query", e))
    }

    pub async fn record_query(&self, ws: &WorkspaceId, q: &NewQuery<'_>) -> Result<Uuid, AppError> {
        // Tách cặp thành hai giá trị cùng có hoặc cùng không: kiểu Option<(Uuid, Uuid)> làm cho việc ghi lệch một
        // trong hai không thể xảy ra từ phía code.
        let (dataset_id, version_id) = match q.target {
            Some((d, v)) => (Some(d), Some(v)),
            None => (None, None),
        };
        sqlx::query_scalar(
            "INSERT INTO queries (workspace_id, dataset_id, version_id, user_id, question, sql_generated,
                                  sql_guarded, status, error, timings, row_count, explanation)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
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
        .bind(&q.explanation)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("record query", e))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, exec, store, temp_workspace};
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
            explanation: Some(serde_json::json!({"base": {"filter": "x = 1"}})),
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

    #[tokio::test]
    async fn correct_queries_are_examples_newest_first() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws).await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "DataCo", "dataco", None)
            .await
            .unwrap();
        let v = store
            .create_version(
                &ws,
                ds.id,
                &NewVersion {
                    source_kind: SourceKind::Csv,
                    file_hash: "h",
                    parquet_path: "x",
                    row_count: 1,
                },
                &[],
            )
            .await
            .unwrap();
        // (câu hỏi, trạng thái, đánh giá, số giờ trước): chỉ hai câu đầu là ví dụ.
        let rows = [
            ("old correct", QueryStatus::Ok, Some("correct"), 2),
            ("new correct", QueryStatus::Ok, Some("correct"), 1),
            ("marked wrong", QueryStatus::Ok, Some("incorrect"), 1),
            ("not marked", QueryStatus::Ok, None, 1),
            ("rejected", QueryStatus::Rejected, Some("correct"), 1),
        ];
        for (question, status, feedback, hours_ago) in rows {
            let sql = format!("SELECT '{question}'");
            let id = store
                .record_query(
                    &ws,
                    &NewQuery {
                        target: Some((ds.id, v.id)),
                        user_id: None,
                        question,
                        sql_generated: Some(&sql),
                        sql_guarded: Some("SELECT 1 LIMIT 100000"),
                        status,
                        error: None,
                        timings: serde_json::json!({}),
                        row_count: None,
                        explanation: None,
                    },
                )
                .await
                .unwrap();
            // Chưa có endpoint đánh dấu đúng sai, nên đặt thẳng bằng SQL; created_at đặt rõ để thứ tự không phụ thuộc
            // tốc độ chèn.
            let feedback = feedback.map_or("NULL".to_string(), |f| format!("'{f}'"));
            exec(
                &store,
                &format!(
                    "UPDATE queries SET feedback = {feedback}, created_at = now() - interval '{hours_ago} hours'
                     WHERE id = '{id}'"
                ),
            )
            .await;
        }

        let examples = store.list_correct_queries(&ws, ds.id, 10).await.unwrap();
        let got: Vec<(&str, &str)> = examples
            .iter()
            .map(|e| (e.question.as_str(), e.sql.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("new correct", "SELECT 'new correct'"),
                ("old correct", "SELECT 'old correct'"),
            ]
        );
        assert_eq!(
            store
                .list_correct_queries(&ws, ds.id, 1)
                .await
                .unwrap()
                .len(),
            1
        );
        let other = temp_workspace(&store).await;
        assert!(store
            .list_correct_queries(&other, ds.id, 10)
            .await
            .unwrap()
            .is_empty());

        drop_workspace(&store, ws).await;
        drop_workspace(&store, other).await;
    }
}
