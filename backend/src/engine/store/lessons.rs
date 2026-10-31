// SPDX-License-Identifier: Apache-2.0
/* engine/store/lessons.rs - nhật ký lỗi và bài học (#228, migration 0013). Ai được đọc, ai được duyệt là việc của
service/lessons/; ở đây mọi câu lọc theo workspace. Nhật ký chỉ thêm, không sửa; bài học tạo là bản nháp, bật thì ghi
người duyệt và thời điểm. */

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một dòng nhật ký cần ghi. step là một giá trị của ràng buộc ở migration 0013.
pub struct NewErrorEntry<'a> {
    pub dataset_id: Option<Uuid>,
    pub query_id: Option<Uuid>,
    pub question: &'a str,
    pub step: &'a str,
    pub error: &'a str,
    pub model_output: Option<&'a str>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ErrorEntryRow {
    pub id: Uuid,
    pub dataset_id: Option<Uuid>,
    pub dataset_name: Option<String>,
    pub query_id: Option<Uuid>,
    pub question: String,
    pub step: String,
    pub error: String,
    pub model_output: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LessonRow {
    pub id: Uuid,
    pub dataset_id: Option<Uuid>,
    pub dataset_name: Option<String>,
    pub text: String,
    pub enabled: bool,
    pub error_id: Option<Uuid>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
}

/// Bài học đang dùng được cho prompt: đã duyệt và đang bật.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ActiveLesson {
    pub text: String,
    /// None là bài của cả workspace.
    pub dataset_id: Option<Uuid>,
}

pub struct NewLesson<'a> {
    pub dataset_id: Option<Uuid>,
    pub text: &'a str,
    pub error_id: Option<Uuid>,
    pub created_by: Uuid,
}

// Cột của LessonRow, kèm tên người tạo, người duyệt và dataset; l là bảng lessons.
const LESSON_SELECT: &str =
    "SELECT l.id, l.dataset_id, d.name AS dataset_name, l.text, l.enabled, l.error_id,
        NULLIF(COALESCE(NULLIF(c.display_name, ''), c.email), '') AS created_by, l.created_at,
        NULLIF(COALESCE(NULLIF(a.display_name, ''), a.email), '') AS approved_by, l.approved_at
    FROM lessons l
    LEFT JOIN datasets d ON d.workspace_id = l.workspace_id AND d.id = l.dataset_id
    LEFT JOIN users c ON c.id = l.created_by
    LEFT JOIN users a ON a.id = l.approved_by";

impl Store {
    pub async fn record_error(
        &self,
        ws: &WorkspaceId,
        entry: &NewErrorEntry<'_>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO error_log (workspace_id, dataset_id, query_id, question, step, error, model_output)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(ws.as_uuid())
        .bind(entry.dataset_id)
        .bind(entry.query_id)
        .bind(entry.question)
        .bind(entry.step)
        .bind(entry.error)
        .bind(entry.model_output)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("record error", e))?;
        Ok(())
    }

    /// Ghi một dòng "Chưa đạt" từ chính dòng lịch sử: câu hỏi, dataset và SQL mô hình viết lấy từ queries.
    pub async fn record_feedback_error(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        error: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO error_log (workspace_id, dataset_id, query_id, question, step, error, model_output)
             SELECT workspace_id, dataset_id, id, question, 'feedback', $3, sql_generated FROM queries
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(query_id)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("record feedback error", e))?;
        Ok(())
    }

    /// Các dòng nhật ký mới nhất trước.
    pub async fn list_errors(
        &self,
        ws: &WorkspaceId,
        limit: i64,
    ) -> Result<Vec<ErrorEntryRow>, AppError> {
        sqlx::query_as(
            "SELECT e.id, e.dataset_id, d.name AS dataset_name, e.query_id, e.question, e.step, e.error,
                    e.model_output, e.created_at
             FROM error_log e
             LEFT JOIN datasets d ON d.workspace_id = e.workspace_id AND d.id = e.dataset_id
             WHERE e.workspace_id = $1 ORDER BY e.created_at DESC LIMIT $2",
        )
        .bind(ws.as_uuid())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list errors", e))
    }

    /// true khi dòng nhật ký có trong workspace.
    pub async fn error_exists(&self, ws: &WorkspaceId, id: Uuid) -> Result<bool, AppError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM error_log WHERE workspace_id = $1 AND id = $2)",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("check error entry", e))
    }

    pub async fn list_lessons(&self, ws: &WorkspaceId) -> Result<Vec<LessonRow>, AppError> {
        let sql = format!("{LESSON_SELECT} WHERE l.workspace_id = $1 ORDER BY l.created_at DESC");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_error("list lessons", e))
    }

    pub async fn get_lesson(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<LessonRow>, AppError> {
        let sql = format!("{LESSON_SELECT} WHERE l.workspace_id = $1 AND l.id = $2");
        sqlx::query_as(&sql)
            .bind(ws.as_uuid())
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("get lesson", e))
    }

    /// Tạo bản nháp: chưa duyệt, chưa bật.
    pub async fn create_lesson(
        &self,
        ws: &WorkspaceId,
        new: &NewLesson<'_>,
    ) -> Result<Uuid, AppError> {
        sqlx::query_scalar(
            "INSERT INTO lessons (workspace_id, dataset_id, text, error_id, created_by)
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(new.dataset_id)
        .bind(new.text)
        .bind(new.error_id)
        .bind(new.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create lesson", e))
    }

    /// Bật (approver Some: ghi người duyệt và thời điểm) hay tắt (None: giữ lần duyệt cũ để biết ai đã duyệt). false khi
    /// không có bài học đó.
    pub async fn set_lesson_enabled(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
        approver: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let done = sqlx::query(
            "UPDATE lessons SET enabled = $3::uuid IS NOT NULL,
                 approved_by = COALESCE($3, approved_by),
                 approved_at = CASE WHEN $3::uuid IS NULL THEN approved_at ELSE now() END
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .bind(approver)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set lesson enabled", e))?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn delete_lesson(&self, ws: &WorkspaceId, id: Uuid) -> Result<bool, AppError> {
        let done = sqlx::query("DELETE FROM lessons WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("delete lesson", e))?;
        Ok(done.rows_affected() > 0)
    }

    /// Bài học đã duyệt, đang bật, của dataset hay của cả workspace, mới duyệt trước. limit chặn số bài đọc mỗi câu hỏi;
    /// prompt chỉ lấy vài bài liên quan trong số này (service/lessons/pick.rs).
    pub async fn list_active_lessons(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ActiveLesson>, AppError> {
        sqlx::query_as(
            "SELECT text, dataset_id FROM lessons
             WHERE workspace_id = $1 AND enabled AND approved_at IS NOT NULL
               AND (dataset_id IS NULL OR dataset_id = $2)
             ORDER BY approved_at DESC LIMIT $3",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list active lessons", e))
    }
}
