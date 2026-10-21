// SPDX-License-Identifier: Apache-2.0
/* engine/store/workspaces.rs - tạo và tìm workspace, và người dùng thuộc workspace nào với vai gì.
Đây là chỗ của store không nhận WorkspaceId, vì lúc tạo hay tìm workspace thì chưa có workspace để lọc.
Hàm trả Uuid hay Membership thô; đổi thành WorkspaceId là việc của auth sau khi đã kiểm người dùng thuộc workspace đó. */

use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, Membership, Store};
use crate::error::AppError;

// Khóa tư vấn khi tạo workspace: hai người tạo cùng một tên thì người sau chờ, rồi nhận slug có hậu tố.
const WORKSPACE_LOCK: i64 = 0x7864_6903;

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

    /// Workspace của người này có slug này, kèm vai của họ. None nếu workspace không có hoặc họ không thuộc nó:
    /// hai trường hợp gộp làm một để người ngoài không dò được slug nào tồn tại.
    pub async fn find_membership(
        &self,
        user_id: Uuid,
        slug: &str,
    ) -> Result<Option<Membership>, AppError> {
        sqlx::query_as(
            "SELECT w.id AS workspace_id, w.name, w.slug, m.role
             FROM memberships m JOIN workspaces w ON w.id = m.workspace_id
             WHERE m.user_id = $1 AND w.slug = $2",
        )
        .bind(user_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_error("find membership", e))
    }

    /// Tạo workspace mới, người tạo là admin của nó.
    pub async fn create_workspace_for(
        &self,
        user_id: Uuid,
        name: &str,
        base_slug: &str,
    ) -> Result<Membership, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin workspace", e))?;
        let membership = insert_with_admin(&mut tx, name, base_slug, user_id).await?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit workspace", e))?;
        Ok(membership)
    }
}

/// Thêm workspace và cho user làm admin, trong giao dịch của người gọi. Slug trùng thì thêm hậu tố -2, -3 cho tới khi
/// trống. Khóa tư vấn giữ tới hết giao dịch, nên hai lần tạo cùng tên không cùng lấy một slug.
pub(super) async fn insert_with_admin(
    tx: &mut PgConnection,
    name: &str,
    base_slug: &str,
    user_id: Uuid,
) -> Result<Membership, AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(WORKSPACE_LOCK)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("lock workspace", e))?;
    let mut slug = base_slug.to_string();
    let mut n = 2;
    while sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM workspaces WHERE slug = $1)")
        .bind(&slug)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("check slug", e))?
    {
        slug = format!("{base_slug}-{n}");
        n += 1;
    }
    let workspace_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workspaces (name, slug, kind) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(name)
    .bind(&slug)
    .bind(WorkspaceKind::Team.as_str())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_error("create workspace", e))?;
    sqlx::query("INSERT INTO memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(workspace_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("create membership", e))?;
    Ok(Membership {
        workspace_id,
        name: name.to_string(),
        slug,
        role: "admin".to_string(),
    })
}
