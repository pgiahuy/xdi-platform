// SPDX-License-Identifier: Apache-2.0
/* engine/store/reports.rs - báo cáo và mục của báo cáo (migration 0021). Mỗi mục trỏ tới một dòng queries; store chỉ
đọc và ghi dòng, không chạy lại câu SQL nào (service/report/ làm việc đó). Ai được đọc báo cáo nào là việc của
service/report/; ở đây mọi câu lọc theo workspace để id của workspace khác không bao giờ đọc được. */

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Một báo cáo kèm số mục. created_by để service kiểm người đọc có phải người tạo không.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportRow {
    pub id: Uuid,
    pub created_by: Uuid,
    pub title: String,
    pub intro: String,
    /// `manual` hay `automatic`.
    pub kind: String,
    pub source_id: Option<Uuid>,
    pub model_calls: Option<i32>,
    pub item_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Báo cáo mới. source_id và model_calls chỉ có với báo cáo tự động.
pub struct NewReport<'a> {
    pub created_by: Uuid,
    pub title: &'a str,
    pub intro: &'a str,
    pub source_id: Option<Uuid>,
    pub model_calls: Option<i32>,
}

/// Một mục của báo cáo cùng phần đã lưu của câu hỏi nó trỏ tới: đủ để chạy lại câu SQL và hiện lời đã trả lời. Không
/// có dòng kết quả nào (queries không lưu bảng kết quả).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportItemRow {
    pub id: Uuid,
    pub query_id: Uuid,
    pub question: String,
    pub asked_at: DateTime<Utc>,
    pub dataset_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub sql_generated: Option<String>,
    pub sql_guarded: Option<String>,
    pub row_count: Option<i64>,
    pub explanation: Option<serde_json::Value>,
    pub answer: Option<serde_json::Value>,
    pub read_versions: Option<serde_json::Value>,
}

/// Câu hỏi muốn thêm vào báo cáo: ai hỏi và đã trả lời chưa, để service kiểm trước khi thêm.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReportableQuery {
    pub user_id: Option<Uuid>,
    /// Chữ của QueryStatus::as_str.
    pub status: String,
}

/// Một phiên bản của một bảng, để hiện "bảng nào, phiên bản mấy, chụp lúc nào" cạnh mỗi mục.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VersionLabel {
    pub version_id: Uuid,
    pub dataset_id: Uuid,
    pub table_name: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub current_version_id: Option<Uuid>,
}

const REPORT_SELECT: &str = "SELECT r.id, r.created_by, r.title, r.intro, r.kind, r.source_id, r.model_calls,
        (SELECT COUNT(*) FROM report_items i WHERE i.report_id = r.id) AS item_count, r.created_at, r.updated_at
     FROM reports r";

impl Store {
    pub async fn create_report(
        &self,
        ws: &WorkspaceId,
        r: &NewReport<'_>,
    ) -> Result<Uuid, AppError> {
        let kind = if r.source_id.is_some() {
            "automatic"
        } else {
            "manual"
        };
        sqlx::query_scalar(
            "INSERT INTO reports (workspace_id, created_by, title, intro, kind, source_id, model_calls)
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(r.created_by)
        .bind(r.title)
        .bind(r.intro)
        .bind(kind)
        .bind(r.source_id)
        .bind(r.model_calls)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create report", e))
    }

    /// Báo cáo của một người, sửa gần nhất trước.
    pub async fn list_reports(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
    ) -> Result<Vec<ReportRow>, AppError> {
        sqlx::query_as(&format!(
            "{REPORT_SELECT} WHERE r.workspace_id = $1 AND r.created_by = $2 ORDER BY r.updated_at DESC"
        ))
        .bind(ws.as_uuid())
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list reports", e))
    }

    /// Một báo cáo của workspace; None nếu không có hay thuộc workspace khác.
    pub async fn get_report(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<Option<ReportRow>, AppError> {
        sqlx::query_as(&format!(
            "{REPORT_SELECT} WHERE r.workspace_id = $1 AND r.id = $2"
        ))
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("get report", e))
    }

    /// Đổi tiêu đề và đoạn mở đầu; trường None giữ nguyên.
    pub async fn update_report(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
        title: Option<&str>,
        intro: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE reports SET title = COALESCE($3, title), intro = COALESCE($4, intro), updated_at = now()
             WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .bind(title)
        .bind(intro)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("update report", e))?;
        Ok(())
    }

    pub async fn delete_report(&self, ws: &WorkspaceId, id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM reports WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("delete report", e))?;
        Ok(())
    }

    /// Người hỏi và trạng thái của một câu hỏi; None nếu không có trong workspace.
    pub async fn get_reportable_query(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
    ) -> Result<Option<ReportableQuery>, AppError> {
        sqlx::query_as("SELECT user_id, status FROM queries WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(query_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("get reportable query", e))
    }

    /// Thêm một câu vào cuối báo cáo. Trả false khi câu đã có trong báo cáo (UNIQUE report_id, query_id).
    pub async fn add_report_item(
        &self,
        ws: &WorkspaceId,
        report_id: Uuid,
        query_id: Uuid,
    ) -> Result<bool, AppError> {
        let added = sqlx::query(
            "INSERT INTO report_items (workspace_id, report_id, query_id, position)
             SELECT $1, $2, $3, COALESCE(MAX(position) + 1, 0) FROM report_items WHERE report_id = $2
             ON CONFLICT (report_id, query_id) DO NOTHING",
        )
        .bind(ws.as_uuid())
        .bind(report_id)
        .bind(query_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("add report item", e))?;
        self.touch_report(ws, report_id).await?;
        Ok(added.rows_affected() == 1)
    }

    /// Bỏ một mục; trả false khi báo cáo không có mục đó.
    pub async fn remove_report_item(
        &self,
        ws: &WorkspaceId,
        report_id: Uuid,
        item_id: Uuid,
    ) -> Result<bool, AppError> {
        let removed = sqlx::query(
            "DELETE FROM report_items WHERE workspace_id = $1 AND report_id = $2 AND id = $3",
        )
        .bind(ws.as_uuid())
        .bind(report_id)
        .bind(item_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("remove report item", e))?;
        self.touch_report(ws, report_id).await?;
        Ok(removed.rows_affected() == 1)
    }

    /// Ghi lại thứ tự: mục thứ i của item_ids nhận position i. Service đã kiểm item_ids đúng là mọi mục của báo cáo.
    pub async fn reorder_report_items(
        &self,
        ws: &WorkspaceId,
        report_id: Uuid,
        item_ids: &[Uuid],
    ) -> Result<(), AppError> {
        // unnest WITH ORDINALITY đánh số từ 1; một câu lệnh nên thứ tự không bao giờ ghi dở dang.
        sqlx::query(
            "UPDATE report_items i SET position = o.n - 1
             FROM unnest($3::uuid[]) WITH ORDINALITY AS o(id, n)
             WHERE i.workspace_id = $1 AND i.report_id = $2 AND i.id = o.id",
        )
        .bind(ws.as_uuid())
        .bind(report_id)
        .bind(item_ids)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("reorder report items", e))?;
        self.touch_report(ws, report_id).await
    }

    /// Các mục của báo cáo theo thứ tự, kèm phần đã lưu của câu hỏi.
    pub async fn list_report_items(
        &self,
        ws: &WorkspaceId,
        report_id: Uuid,
    ) -> Result<Vec<ReportItemRow>, AppError> {
        sqlx::query_as(
            "SELECT i.id, q.id AS query_id, q.question, q.created_at AS asked_at, q.dataset_id,
                    q.version_id, q.sql_generated, q.sql_guarded, q.row_count, q.explanation, q.answer,
                    q.read_versions
             FROM report_items i JOIN queries q ON q.workspace_id = i.workspace_id AND q.id = i.query_id
             WHERE i.workspace_id = $1 AND i.report_id = $2
             ORDER BY i.position, i.created_at",
        )
        .bind(ws.as_uuid())
        .bind(report_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list report items", e))
    }

    /// Tên bảng, số phiên bản và lúc chụp của các phiên bản trong danh sách; phiên bản đã xóa không có dòng.
    pub async fn list_version_labels(
        &self,
        ws: &WorkspaceId,
        version_ids: &[Uuid],
    ) -> Result<Vec<VersionLabel>, AppError> {
        sqlx::query_as(
            "SELECT v.id AS version_id, v.dataset_id, d.name AS table_name, v.version, v.created_at,
                    d.current_version_id
             FROM dataset_versions v JOIN datasets d ON d.workspace_id = v.workspace_id AND d.id = v.dataset_id
             WHERE v.workspace_id = $1 AND v.id = ANY($2)",
        )
        .bind(ws.as_uuid())
        .bind(version_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list version labels", e))
    }

    /// Ghi phần đã hiểu dạng chữ và lời trả lời cho một câu mà báo cáo tự động vừa ghi; cùng hai cột luồng hỏi ghi qua
    /// attach_turn, nhưng câu của báo cáo không thuộc cuộc trò chuyện nào.
    pub async fn set_query_answer(
        &self,
        ws: &WorkspaceId,
        query_id: Uuid,
        understood: Option<&str>,
        answer: Option<&serde_json::Value>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE queries SET understood = $3, answer = $4 WHERE workspace_id = $1 AND id = $2",
        )
        .bind(ws.as_uuid())
        .bind(query_id)
        .bind(understood)
        .bind(answer)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("set query answer", e))?;
        Ok(())
    }

    async fn touch_report(&self, ws: &WorkspaceId, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE reports SET updated_at = now() WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("touch report", e))?;
        Ok(())
    }
}
