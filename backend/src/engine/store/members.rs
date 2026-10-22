// SPDX-License-Identifier: Apache-2.0
/* engine/store/members.rs - thành viên của một workspace, đổi tên và xóa workspace; và phần quản trị hệ thống nhìn
mọi workspace. Ai được gọi các hàm này (kiểm vai của người gọi) do service/workspaces.rs quyết định.

Riêng luật luôn còn ít nhất một admin nằm ở đây, vì phải kiểm trong cùng giao dịch với lệnh ghi: đổi vai và xóa thành
viên khóa mọi dòng thành viên của workspace (FOR UPDATE) rồi mới đếm. Không khóa, hai admin hạ vai lẫn nhau cùng lúc
thì mỗi bên đều thấy còn người kia, và workspace mất hết admin.

Vai của người gọi được kiểm lúc nhận request, trước giao dịch. Một request đã qua bước kiểm vẫn chạy nốt dù người gọi
vừa bị hạ vai; luật còn ít nhất một admin không bị ảnh hưởng vì nó được kiểm lại trong giao dịch. */

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, Store};
use crate::auth::{Role, WorkspaceId};
use crate::error::AppError;

// Một thành viên, như admin của workspace nhìn thấy.
/// A member of a workspace.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    /// `admin` or `member`.
    pub role: String,
}

// Một workspace như quản trị hệ thống nhìn thấy: tên và thành viên, không có dữ liệu bên trong.
/// A workspace as the system administrator sees it: name and members, none of its data.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct WorkspaceOverview {
    pub workspace_id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub members: Vec<MemberRow>,
}

pub enum MemberChange {
    SetRole(Role),
    Remove,
}

pub enum ChangeOutcome {
    Done,
    NotMember,
    /// Từ chối: sau thay đổi workspace sẽ không còn admin nào.
    LastAdmin,
}

impl Store {
    pub async fn rename_workspace(&self, ws: &WorkspaceId, name: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE workspaces SET name = $2 WHERE id = $1")
            .bind(ws.as_uuid())
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("rename workspace", e))?;
        Ok(())
    }

    /// Xóa workspace; khóa ngoại ON DELETE CASCADE xóa theo thành viên, dataset, phiên bản, câu hỏi. File Parquet trên
    /// đĩa do service xóa.
    pub async fn delete_workspace(&self, ws: &WorkspaceId) -> Result<(), AppError> {
        sqlx::query("DELETE FROM workspaces WHERE id = $1")
            .bind(ws.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(|e| db_error("delete workspace", e))?;
        Ok(())
    }

    pub async fn list_members(&self, ws: &WorkspaceId) -> Result<Vec<MemberRow>, AppError> {
        sqlx::query_as(
            "SELECT u.id AS user_id, u.email, u.display_name, m.role
             FROM memberships m JOIN users u ON u.id = m.user_id
             WHERE m.workspace_id = $1 ORDER BY u.email",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list members", e))
    }

    /// Thêm thành viên. Trả false nếu người đó đã là thành viên.
    pub async fn add_member(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
        role: Role,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "INSERT INTO memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (workspace_id, user_id) DO NOTHING",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("add member", e))?;
        Ok(result.rows_affected() == 1)
    }

    /// Đổi vai hoặc xóa một thành viên, giữ luật luôn còn ít nhất một admin. Admin có tài khoản đã khóa vẫn được tính:
    /// workspace chỉ còn admin như vậy thì quản trị hệ thống chỉ định admin mới (make_workspace_admin).
    pub async fn change_member(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
        change: MemberChange,
    ) -> Result<ChangeOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin member change", e))?;
        let mut roles = lock_roles(&mut tx, ws.as_uuid()).await?;
        // Lấy người bị đổi ra khỏi danh sách; phần còn lại là những người khác trong workspace.
        let Some(current) = roles.remove(&user_id) else {
            return Ok(ChangeOutcome::NotMember);
        };
        let stays_admin = matches!(change, MemberChange::SetRole(Role::Admin));
        let others_have_admin = roles.values().any(|role| *role == Role::Admin);
        if current == Role::Admin && !stays_admin && !others_have_admin {
            return Ok(ChangeOutcome::LastAdmin);
        }
        let query = match change {
            MemberChange::SetRole(role) => sqlx::query(
                "UPDATE memberships SET role = $3 WHERE workspace_id = $1 AND user_id = $2",
            )
            .bind(ws.as_uuid())
            .bind(user_id)
            .bind(role.as_str()),
            MemberChange::Remove => {
                sqlx::query("DELETE FROM memberships WHERE workspace_id = $1 AND user_id = $2")
                    .bind(ws.as_uuid())
                    .bind(user_id)
            }
        };
        query
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("change member", e))?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit member change", e))?;
        Ok(ChangeOutcome::Done)
    }

    /// Mọi workspace kèm thành viên, cho quản trị hệ thống. Không chạm dataset hay câu hỏi.
    pub async fn list_workspaces_with_members(&self) -> Result<Vec<WorkspaceOverview>, AppError> {
        let workspaces: Vec<(Uuid, String, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, name, slug, created_at FROM workspaces WHERE kind <> 'demo_session' ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list workspaces", e))?;
        let members: Vec<(Uuid, Uuid, String, String, String)> = sqlx::query_as(
            "SELECT m.workspace_id, u.id, u.email, u.display_name, m.role
             FROM memberships m JOIN users u ON u.id = m.user_id ORDER BY u.email",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list all members", e))?;
        let mut by_workspace: BTreeMap<Uuid, Vec<MemberRow>> = BTreeMap::new();
        for (workspace_id, user_id, email, display_name, role) in members {
            by_workspace
                .entry(workspace_id)
                .or_default()
                .push(MemberRow {
                    user_id,
                    email,
                    display_name,
                    role,
                });
        }
        Ok(workspaces
            .into_iter()
            .map(|(workspace_id, name, slug, created_at)| WorkspaceOverview {
                members: by_workspace.remove(&workspace_id).unwrap_or_default(),
                workspace_id,
                name,
                slug,
                created_at,
            })
            .collect())
    }

    /// Quản trị hệ thống chỉ định admin cho workspace (ví dụ khi admin cuối nghỉ việc và bị khóa tài khoản): thêm
    /// người đó với vai admin, hoặc nâng lên admin nếu đã là thành viên. Trả false nếu không có workspace này, hoặc đó là
    /// workspace tạm của bản demo.
    pub async fn make_workspace_admin(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "INSERT INTO memberships (workspace_id, user_id, role)
             SELECT id, $2, 'admin' FROM workspaces WHERE id = $1 AND kind <> 'demo_session'
             ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = 'admin'",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("make workspace admin", e))?;
        Ok(result.rows_affected() == 1)
    }
}

/// Khóa mọi dòng thành viên của workspace tới hết giao dịch, trả vai của từng người.
async fn lock_roles(
    tx: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<BTreeMap<Uuid, Role>, AppError> {
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT user_id, role FROM memberships WHERE workspace_id = $1 FOR UPDATE")
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| db_error("lock members", e))?;
    let mut roles = BTreeMap::new();
    for (user_id, role) in rows {
        // Ràng buộc CHECK của cột role chỉ cho hai giá trị, nên không parse được nghĩa là schema và code lệch nhau.
        let role = Role::parse(&role)
            .ok_or_else(|| AppError::Internal(format!("unknown role in database: {role}")))?;
        roles.insert(user_id, role);
    }
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::engine::store::test_support::fresh_database;
    use crate::engine::store::WorkspaceKind;

    async fn add_admin(store: &Store, ws: &WorkspaceId, email: &str) -> Uuid {
        let id: Uuid = sqlx::query_scalar("INSERT INTO users (email) VALUES ($1) RETURNING id")
            .bind(email)
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert!(store.add_member(ws, id, Role::Admin).await.unwrap());
        id
    }

    /// Một lần đổi vai khác đang giữ khóa thì change_member phải chờ nó xong rồi mới đếm admin. Gửi hai request
    /// cùng lúc qua API không bắt được lỗi này một cách chắc chắn (khe giữa SELECT và UPDATE quá ngắn), nên test tự giữ
    /// một giao dịch đang dở. Bỏ FOR UPDATE trong lock_roles thì test hỏng: change_member đọc thấy hai admin từ
    /// trước, và workspace mất hết admin.
    #[tokio::test]
    async fn change_member_waits_for_a_concurrent_change() {
        let Some(db) = fresh_database().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let store = db.store.clone();
        let ws_id = store
            .create_workspace("W", "w", WorkspaceKind::Team)
            .await
            .unwrap();
        let ws = WorkspaceId::for_test(ws_id);
        let lan = add_admin(&store, &ws, "lan@example.com").await;
        let minh = add_admin(&store, &ws, "minh@example.com").await;

        // Giao dịch kia: khóa các dòng thành viên, hạ vai Minh, chưa commit.
        let mut other = store.pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM memberships WHERE workspace_id = $1 FOR UPDATE")
            .bind(ws_id)
            .execute(&mut *other)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE memberships SET role = 'member' WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(ws_id)
        .bind(minh)
        .execute(&mut *other)
        .await
        .unwrap();

        // Trong lúc đó hạ vai Lan: phải chờ giao dịch kia, rồi thấy Lan là admin cuối cùng.
        let demote_lan = tokio::spawn({
            let store = store.clone();
            async move {
                store
                    .change_member(&ws, lan, MemberChange::SetRole(Role::Member))
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !demote_lan.is_finished(),
            "change_member must wait for the lock"
        );
        other.commit().await.unwrap();
        let outcome = demote_lan.await.unwrap().unwrap();
        assert!(matches!(outcome, ChangeOutcome::LastAdmin));

        db.drop().await;
    }
}
