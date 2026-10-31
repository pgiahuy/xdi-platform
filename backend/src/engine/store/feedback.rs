// SPDX-License-Identifier: Apache-2.0
/* engine/store/feedback.rs - đánh giá câu trả lời ("Tốt", "Chưa đạt") và duyệt câu mẫu (#212), trên các cột của
queries thêm ở migration 0012. Ai được đánh giá câu nào, ai được duyệt là việc của service/feedback/; ở đây mọi câu
lọc theo workspace, và ghi đánh giá còn lọc theo người đã hỏi: một người chỉ đánh giá được câu của mình.

Trong database "Tốt" là feedback = 'correct', "Chưa đạt" là 'incorrect' (giá trị có từ migration 0001). */

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Đánh giá cần ghi. reason chỉ có khi feedback là 'incorrect' (ràng buộc của migration 0012).
pub struct NewFeedback<'a> {
    /// 'correct' hay 'incorrect'.
    pub feedback: &'a str,
    pub reason: Option<&'a str>,
    pub note: Option<&'a str>,
}

/// Đánh giá đã lưu của một câu hỏi.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct StoredFeedback {
    pub feedback: String,
    pub reason: Option<String>,
    pub note: Option<String>,
    pub rated_at: DateTime<Utc>,
    /// Admin đã duyệt câu làm câu mẫu.
    pub approved: bool,
}

/// Một câu "Tốt" trong danh sách admin duyệt: đủ để đọc lại câu đã được trả lời thế nào.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExampleRow {
    pub query_id: Uuid,
    pub dataset_id: Option<Uuid>,
    pub dataset_name: Option<String>,
    pub question: String,
    pub sql_generated: Option<String>,
    pub understood: Option<String>,
    pub row_count: Option<i64>,
    /// Câu hỏi tìm nguyên nhân: queries.explanation có form (service/ask/why.rs).
    pub is_why: bool,
    /// Tên hiển thị, không có thì email; None khi tài khoản đã xóa.
    pub rated_by: Option<String>,
    pub rated_at: DateTime<Utc>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
}

// Cột trả về của set_feedback, cùng tên với StoredFeedback.
const FEEDBACK_COLUMNS: &str =
    "feedback, feedback_reason AS reason, feedback_note AS note, feedback_at AS rated_at, \
                                example_approved_at IS NOT NULL AS approved";

impl Store {
    /// Ghi đánh giá của người đã hỏi câu này; None khi không có câu đó trong workspace, hoặc câu của người khác. Đổi
    /// sang khác "Tốt" thì xóa lần duyệt câu mẫu trong cùng câu lệnh: câu "Chưa đạt" không được ở lại trong prompt.
    pub async fn set_feedback(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        user_id: Uuid,
        new: &NewFeedback<'_>,
    ) -> Result<Option<StoredFeedback>, AppError> {
        let sql = format!(
            "UPDATE queries SET feedback = $4, feedback_reason = $5, feedback_note = $6, feedback_by = $3,
                 feedback_at = now(),
                 example_approved_by = CASE WHEN $4 = 'correct' THEN example_approved_by END,
                 example_approved_at = CASE WHEN $4 = 'correct' THEN example_approved_at END
             WHERE workspace_id = $1 AND id = $2 AND user_id = $3
             RETURNING {FEEDBACK_COLUMNS}"
        );
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(query_id)
            .bind(user_id)
            .bind(new.feedback)
            .bind(new.reason)
            .bind(new.note)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("set feedback", e))
    }

    /// Bỏ đánh giá (bấm lại nút đang chọn), kèm lần duyệt. false khi không có câu đó của người này.
    pub async fn clear_feedback(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, AppError> {
        let done = sqlx::query(
            "UPDATE queries SET feedback = NULL, feedback_reason = NULL, feedback_note = NULL, feedback_by = NULL,
                 feedback_at = NULL, example_approved_by = NULL, example_approved_at = NULL
             WHERE workspace_id = $1 AND id = $2 AND user_id = $3",
        )
        .bind(ws.as_uuid())
        .bind(query_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("clear feedback", e))?;
        Ok(done.rows_affected() > 0)
    }

    /// Các câu "Tốt" đã trả lời được của workspace, mới đánh giá trước; dataset_id None là mọi dataset.
    pub async fn list_good_answers(
        &self,
        ws: &WorkspaceId,
        dataset_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<ExampleRow>, AppError> {
        sqlx::query_as(
            "SELECT q.id AS query_id, q.dataset_id, d.name AS dataset_name, q.question, q.sql_generated,
                    q.understood, q.row_count, COALESCE(q.explanation ? 'form', false) AS is_why,
                    NULLIF(COALESCE(NULLIF(r.display_name, ''), r.email), '') AS rated_by,
                    q.feedback_at AS rated_at,
                    NULLIF(COALESCE(NULLIF(a.display_name, ''), a.email), '') AS approved_by,
                    q.example_approved_at AS approved_at
             FROM queries q
             LEFT JOIN datasets d ON d.workspace_id = q.workspace_id AND d.id = q.dataset_id
             LEFT JOIN users r ON r.id = q.feedback_by
             LEFT JOIN users a ON a.id = q.example_approved_by
             WHERE q.workspace_id = $1 AND q.feedback = 'correct' AND q.status = 'ok'
               AND q.feedback_at IS NOT NULL AND ($2::uuid IS NULL OR q.dataset_id = $2)
             ORDER BY q.feedback_at DESC LIMIT $3",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list good answers", e))
    }

    /// Duyệt (approver Some) hay bỏ duyệt (None) một câu "Tốt" đã trả lời được làm câu mẫu. false khi câu không có,
    /// chưa được đánh giá "Tốt", hay không trả lời được: chỉ câu đó mới làm ví dụ được.
    pub async fn set_example_approval(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        approver: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let done = sqlx::query(
            "UPDATE queries SET example_approved_by = $3,
                 example_approved_at = CASE WHEN $3::uuid IS NULL THEN NULL ELSE now() END
             WHERE workspace_id = $1 AND id = $2 AND feedback = 'correct' AND status = 'ok'
               AND sql_generated IS NOT NULL",
        )
        .bind(ws.as_uuid())
        .bind(query_id)
        .bind(approver)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set example approval", e))?;
        Ok(done.rows_affected() > 0)
    }
}
