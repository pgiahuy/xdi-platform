// SPDX-License-Identifier: Apache-2.0
/* engine/store/data_roles.rs - vai dữ liệu của workspace (bảng data_roles) và vai của từng thành viên
(member_data_roles), migration 0020. Luật lọc dòng của từng vai ở policies.rs. Ai được gọi (admin), kiểm và chuẩn hóa
tên do service/data_roles.rs quyết định.

Luật giữ ở đây vì phải kiểm trong cùng giao dịch với lệnh ghi (khóa dòng workspace, lock_workspace ở policies.rs):
- Chỉ gán được vai đã có ít nhất một luật. Vai không có luật nào không giới hạn bảng nào; gán nhầm nó không làm ai
  thấy thêm dòng (xem service/ask/row_filters.rs), nhưng admin dễ tưởng người đó đã bị lọc.
- Không xóa được vai đang gán cho ai đó: người chỉ có vai đó sẽ thành không có vai nào và thấy mọi dòng. */

use serde::Serialize;
use uuid::Uuid;

use super::policies::lock_workspace;
use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

const UNIQUE_VIOLATION: &str = "23505";

// Một dòng của data_roles.
/// A data role of the workspace.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DataRoleRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

pub enum RoleWriteOutcome {
    Done(DataRoleRow),
    NotFound,
    /// Từ chối: workspace đã có vai cùng tên (không phân biệt hoa thường).
    NameTaken,
}

pub enum RoleDeleteOutcome {
    Done,
    NotFound,
    /// Từ chối: vai đang gán cho bấy nhiêu thành viên.
    Assigned(i64),
}

pub enum MemberRolesOutcome {
    Done,
    NotMember,
    /// Từ chối: có id không phải vai của workspace này.
    UnknownRole,
    /// Từ chối: các vai này chưa có luật nào.
    RolesWithoutRules(Vec<String>),
}

impl Store {
    /// Mọi vai của workspace, theo tên.
    pub async fn list_data_roles(&self, ws: &WorkspaceId) -> Result<Vec<DataRoleRow>, AppError> {
        sqlx::query_as(
            "SELECT id, name, description FROM data_roles WHERE workspace_id = $1 ORDER BY lower(name)",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list data roles", e))
    }

    pub async fn create_data_role(
        &self,
        ws: &WorkspaceId,
        name: &str,
        description: Option<&str>,
    ) -> Result<RoleWriteOutcome, AppError> {
        let row = sqlx::query_as(
            "INSERT INTO data_roles (workspace_id, name, description) VALUES ($1, $2, $3)
             RETURNING id, name, description",
        )
        .bind(ws.as_uuid())
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await;
        build_write_outcome(row.map(Some), "create data role")
    }

    /// Đổi tên và mô tả. Luật và người mang vai trỏ tới id nên không đổi theo.
    pub async fn update_data_role(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<RoleWriteOutcome, AppError> {
        let row = sqlx::query_as(
            "UPDATE data_roles SET name = $3, description = $4 WHERE workspace_id = $1 AND id = $2
             RETURNING id, name, description",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .bind(name)
        .bind(description)
        .fetch_optional(&self.pool)
        .await;
        build_write_outcome(row, "update data role")
    }

    /// Xóa vai và luật của nó (khóa ngoại). Vai đang gán thì từ chối.
    pub async fn delete_data_role(
        &self,
        ws: &WorkspaceId,
        id: Uuid,
    ) -> Result<RoleDeleteOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin delete data role", e))?;
        lock_workspace(&mut tx, ws).await?;
        let assigned: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM member_data_roles WHERE workspace_id = $1 AND data_role_id = $2",
        )
        .bind(ws.as_uuid())
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("count role members", e))?;
        if assigned > 0 {
            return Ok(RoleDeleteOutcome::Assigned(assigned));
        }
        let result = sqlx::query("DELETE FROM data_roles WHERE workspace_id = $1 AND id = $2")
            .bind(ws.as_uuid())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("delete data role", e))?;
        if result.rows_affected() == 0 {
            return Ok(RoleDeleteOutcome::NotFound);
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit delete data role", e))?;
        Ok(RoleDeleteOutcome::Done)
    }

    /// Mọi cặp (thành viên, vai) của workspace.
    pub async fn list_member_data_roles(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<(Uuid, Uuid)>, AppError> {
        sqlx::query_as(
            "SELECT user_id, data_role_id FROM member_data_roles WHERE workspace_id = $1
             ORDER BY user_id, data_role_id",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list member data roles", e))
    }

    /// Thay toàn bộ vai của một thành viên bằng role_ids (rỗng là gỡ hết), trong một giao dịch.
    pub async fn set_member_data_roles(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
        role_ids: &[Uuid],
    ) -> Result<MemberRolesOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin set member data roles", e))?;
        lock_workspace(&mut tx, ws).await?;
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM memberships WHERE workspace_id = $1 AND user_id = $2)",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_error("check member", e))?;
        if !is_member {
            return Ok(MemberRolesOutcome::NotMember);
        }
        // Tên và số luật của từng vai được chọn; vai của workspace khác không có trong kết quả.
        let roles: Vec<(String, i64)> = sqlx::query_as(
            "SELECT r.name, (SELECT count(*) FROM rls_policies p WHERE p.data_role_id = r.id)
             FROM data_roles r WHERE r.workspace_id = $1 AND r.id = ANY($2) ORDER BY lower(r.name)",
        )
        .bind(ws.as_uuid())
        .bind(role_ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_error("check data roles", e))?;
        if roles.len() != role_ids.len() {
            return Ok(MemberRolesOutcome::UnknownRole);
        }
        let without_rules: Vec<String> = roles
            .into_iter()
            .filter(|(_, rules)| *rules == 0)
            .map(|(name, _)| name)
            .collect();
        if !without_rules.is_empty() {
            return Ok(MemberRolesOutcome::RolesWithoutRules(without_rules));
        }
        sqlx::query("DELETE FROM member_data_roles WHERE workspace_id = $1 AND user_id = $2")
            .bind(ws.as_uuid())
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("clear member data roles", e))?;
        sqlx::query(
            "INSERT INTO member_data_roles (workspace_id, user_id, data_role_id)
             SELECT $1, $2, unnest($3::uuid[])",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .bind(role_ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("insert member data roles", e))?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit set member data roles", e))?;
        Ok(MemberRolesOutcome::Done)
    }
}

/// Kết quả ghi một dòng data_roles: trùng tên (chỉ mục data_roles_name_key) thành NameTaken, không có dòng thành NotFound.
fn build_write_outcome(
    row: Result<Option<DataRoleRow>, sqlx::Error>,
    action: &str,
) -> Result<RoleWriteOutcome, AppError> {
    match row {
        Ok(Some(row)) => Ok(RoleWriteOutcome::Done(row)),
        Ok(None) => Ok(RoleWriteOutcome::NotFound),
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some(UNIQUE_VIOLATION) => {
            Ok(RoleWriteOutcome::NameTaken)
        }
        Err(e) => Err(db_error(action, e)),
    }
}
