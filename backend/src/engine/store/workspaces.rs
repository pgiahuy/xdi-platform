// SPDX-License-Identifier: Apache-2.0
/* engine/store/workspaces.rs - tạo và tìm workspace.
Đây là chỗ duy nhất của store không nhận WorkspaceId, vì lúc tạo hay tìm workspace thì chưa có workspace để lọc.
Hàm trả Uuid thô; đổi nó thành WorkspaceId là việc của auth sau khi kiểm người dùng có quyền vào workspace đó. */

use uuid::Uuid;

use super::{db_error, Store};
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceKind {
    Team,
    Personal,
    DemoSession,
}

impl WorkspaceKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            WorkspaceKind::Team => "team",
            WorkspaceKind::Personal => "personal",
            WorkspaceKind::DemoSession => "demo_session",
        }
    }
}

impl Store {
    /// Tạo workspace thường hoặc cá nhân. Workspace tạm của phiên demo có hạn, tạo bằng hàm riêng khi làm chế độ demo.
    pub async fn create_workspace(
        &self,
        name: &str,
        slug: &str,
        kind: WorkspaceKind,
    ) -> Result<Uuid, AppError> {
        sqlx::query_scalar(
            "INSERT INTO workspaces (name, slug, kind) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(name)
        .bind(slug)
        .bind(kind.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_error("create workspace", e))
    }

    pub async fn find_workspace_by_slug(&self, slug: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar("SELECT id FROM workspaces WHERE slug = $1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_error("find workspace", e))
    }
}
