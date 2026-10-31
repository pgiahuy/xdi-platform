// SPDX-License-Identifier: Apache-2.0
/* engine/store/conversations.rs - cuộc trò chuyện trên một dataset (#164) và các lượt của nó (dòng queries có
conversation_id). Kiểm ai được đọc cuộc nào là việc của service/conversation/; ở đây mọi câu lọc theo workspace để id
của workspace khác không bao giờ đọc được. */

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một cuộc trò chuyện. created_by để service kiểm người đọc có phải người tạo không.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub dataset_id: Uuid,
    pub created_by: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

/// Một lượt đã ghi của cuộc trò chuyện: câu hỏi và cách nó được hiểu, không có dòng dữ liệu hay giá trị kết quả.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredTurn {
    pub query_id: Uuid,
    pub question: String,
    /// Chữ của QueryStatus::as_str.
    pub status: String,
    pub sql_generated: Option<String>,
    pub sql_guarded: Option<String>,
    /// Phần "Cách tính" dạng chữ ngắn (service/conversation/history.rs); None với câu không answered.
    pub understood: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Đánh giá của người hỏi (#212): chữ của queries.feedback, None khi chưa đánh giá; ba trường sau đi cùng nó.
    pub feedback: Option<String>,
    pub feedback_reason: Option<String>,
    pub feedback_note: Option<String>,
    pub feedback_at: Option<DateTime<Utc>>,
    /// Admin đã duyệt câu làm câu mẫu.
    pub example_approved: bool,
    /// Lời trả lời đã hiện cho người hỏi (queries.answer, migration 0016); None với câu không answered hay ghi trước đó.
    pub answer: Option<serde_json::Value>,
}

impl Store {
    pub async fn create_conversation(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        user_id: Uuid,
        title: &str,
    ) -> Result<Uuid, AppError> {
        sqlx::query_scalar(
            "INSERT INTO conversations (workspace_id, dataset_id, created_by, title)
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(user_id)
        .bind(title)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create conversation", e))
    }

    /// Một cuộc của workspace theo id; None nếu không có hay thuộc workspace khác.
    pub async fn get_conversation(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<Conversation>, AppError> {
        sqlx::query_as(
            "SELECT id, dataset_id, created_by, title, created_at FROM conversations
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get conversation", e))
    }

    /// Các cuộc của một người trên một dataset, mới nhất trước. Bỏ cuộc chưa có lượt nào: cuộc mở ra bằng một câu còn
    /// chờ chọn (needs_choice) mà người hỏi bỏ dở thì không có gì để mở lại.
    pub async fn list_conversations(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError> {
        sqlx::query_as(
            "SELECT c.id, c.dataset_id, c.created_by, c.title, c.created_at FROM conversations c
             WHERE c.workspace_id = $1 AND c.dataset_id = $2 AND c.created_by = $3
               AND EXISTS (SELECT 1 FROM queries q WHERE q.conversation_id = c.id)
             ORDER BY c.created_at DESC LIMIT $4",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list conversations", e))
    }

    /// Các cuộc của một người trên mọi bảng của dataset `source_id` (#257), mới nhất trước, cùng luật bỏ cuộc rỗng
    /// như list_conversations.
    pub async fn list_dataset_conversations(
        &self,
        ws: &WorkspaceId,
        source_id: Uuid,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Conversation>, AppError> {
        sqlx::query_as(
            "SELECT c.id, c.dataset_id, c.created_by, c.title, c.created_at FROM conversations c
             JOIN datasets d ON d.workspace_id = c.workspace_id AND d.id = c.dataset_id
             WHERE c.workspace_id = $1 AND d.source_id = $2 AND c.created_by = $3
               AND EXISTS (SELECT 1 FROM queries q WHERE q.conversation_id = c.id)
             ORDER BY c.created_at DESC LIMIT $4",
        )
        .bind(ws.as_uuid())
        .bind(source_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list dataset conversations", e))
    }

    /// Các lượt của một cuộc theo thứ tự hỏi. limit giữ các lượt mới nhất: ngữ cảnh gửi mô hình chỉ cần vài lượt cuối.
    pub async fn list_turns(
        &self,
        ws: &WorkspaceId,
        conversation_id: Uuid,
        limit: i64,
    ) -> Result<Vec<StoredTurn>, AppError> {
        sqlx::query_as(
            "SELECT * FROM (
                 SELECT id AS query_id, question, status, sql_generated, sql_guarded, understood, created_at,
                        feedback, feedback_reason, feedback_note, feedback_at,
                        example_approved_at IS NOT NULL AS example_approved, answer
                 FROM queries WHERE workspace_id = $1 AND conversation_id = $2
                 ORDER BY created_at DESC LIMIT $3
             ) last ORDER BY created_at",
        )
        .bind(ws.as_uuid())
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list turns", e))
    }

    /// Gắn một câu đã ghi vào cuộc trò chuyện, kèm phần đã hiểu dạng chữ và lời trả lời đã hiện. Gọi sau record_query
    /// của luồng hỏi, để hai nhánh câu thường và câu "tại sao" không phải mang thêm trường (service/conversation/mod.rs).
    pub async fn attach_turn(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        conversation_id: Uuid,
        understood: Option<&str>,
        answer: Option<&serde_json::Value>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE queries SET conversation_id = $3, understood = $4, answer = $5
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(query_id)
        .bind(conversation_id)
        .bind(understood)
        .bind(answer)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("attach turn", e))?;
        Ok(())
    }
}
