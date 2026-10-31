// SPDX-License-Identifier: Apache-2.0
/* service/data_roles/mod.rs - vai dữ liệu của workspace (#45, #167): admin tạo, đổi tên, xóa vai, gán vai cho thành viên;
luật lọc dòng của từng vai ở policies.rs. Lưu ở engine/store/data_roles.rs và engine/store/policies.rs; luồng hỏi, truy
dòng nguồn và tổng quan đọc luật qua service/ask/row_filters.rs mỗi request, nên sửa ở đây có hiệu lực ngay request kế
tiếp.

Mọi việc ở đây chỉ admin làm được (member nhận 403), trừ mine(): ai cũng xem được vai của chính mình. Admin không bị
lọc dòng dù mang vai nào: họ tự sửa được luật, lọc với họ không bảo vệ được gì. */

pub mod contract;
pub mod policies;

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{
    DataRoleRow, MemberRolesOutcome, RoleDeleteOutcome, RoleWriteOutcome, StoredPolicy,
};
use crate::{error::AppError, state::AppState};
use contract::{DataRoleRequest, DataRoleResponse};

pub use policies::{filter_columns, set_policies};

const NAME_MAX: usize = 60;
const DESCRIPTION_MAX: usize = 500;

/// Mọi vai của workspace kèm luật và người mang vai, cho màn quản lý vai.
pub async fn list(state: &AppState, member: &Member) -> Result<Vec<DataRoleResponse>, AppError> {
    member.require_admin()?;
    let ws = &member.workspace;
    let roles = state.store.list_data_roles(ws).await?;
    let policies = state.store.list_role_policies(ws).await?;
    let assignments = state.store.list_member_data_roles(ws).await?;
    Ok(roles
        .into_iter()
        .map(|role| build_response(role, &policies, &assignments))
        .collect())
}

/// Vai của chính người gọi; ai trong workspace cũng xem được, chỉ tên và mô tả, không có luật.
pub async fn mine(state: &AppState, member: &Member) -> Result<Vec<DataRoleRow>, AppError> {
    let ws = &member.workspace;
    let roles = state.store.list_data_roles(ws).await?;
    let assignments = state.store.list_member_data_roles(ws).await?;
    Ok(roles
        .into_iter()
        .filter(|role| assignments.contains(&(member.user.id, role.id)))
        .collect())
}

pub async fn create(
    state: &AppState,
    member: &Member,
    req: &DataRoleRequest,
) -> Result<DataRoleRow, AppError> {
    member.require_admin()?;
    let (name, description) = clean_request(req)?;
    let outcome = state
        .store
        .create_data_role(&member.workspace, name, description)
        .await?;
    finish_write(outcome, name)
}

pub async fn update(
    state: &AppState,
    member: &Member,
    role_id: Uuid,
    req: &DataRoleRequest,
) -> Result<DataRoleRow, AppError> {
    member.require_admin()?;
    let (name, description) = clean_request(req)?;
    let outcome = state
        .store
        .update_data_role(&member.workspace, role_id, name, description)
        .await?;
    finish_write(outcome, name)
}

/// Xóa vai cùng luật của nó. Vai đang gán thì từ chối: người chỉ mang vai đó sẽ thành không có vai và thấy mọi dòng.
pub async fn delete(state: &AppState, member: &Member, role_id: Uuid) -> Result<(), AppError> {
    member.require_admin()?;
    match state
        .store
        .delete_data_role(&member.workspace, role_id)
        .await?
    {
        RoleDeleteOutcome::Done => Ok(()),
        RoleDeleteOutcome::NotFound => Err(role_not_found()),
        RoleDeleteOutcome::Assigned(count) => Err(AppError::BadRequest(format!(
            "Vai đang gán cho {count} thành viên; gỡ vai khỏi họ trước khi xóa"
        ))),
    }
}

/// Thay toàn bộ vai của một thành viên; danh sách rỗng là gỡ hết (người đó thấy mọi dòng).
pub async fn set_member_roles(
    state: &AppState,
    member: &Member,
    user_id: Uuid,
    role_ids: &[Uuid],
) -> Result<(), AppError> {
    member.require_admin()?;
    // Bỏ id lặp: store so số vai tìm được với số id gửi lên để phát hiện id lạ.
    let unique: Vec<Uuid> = role_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    match state
        .store
        .set_member_data_roles(&member.workspace, user_id, &unique)
        .await?
    {
        MemberRolesOutcome::Done => Ok(()),
        MemberRolesOutcome::NotMember => Err(AppError::NotFound(
            "Người này không phải thành viên của workspace".to_string(),
        )),
        MemberRolesOutcome::UnknownRole => Err(AppError::BadRequest(
            "Có vai không thuộc workspace này; tải lại trang rồi chọn lại".to_string(),
        )),
        MemberRolesOutcome::RolesWithoutRules(names) => Err(AppError::BadRequest(format!(
            "Vai {} chưa có luật lọc dòng nào; thêm luật trước khi gán",
            names.join(", ")
        ))),
    }
}

fn build_response(
    role: DataRoleRow,
    policies: &[StoredPolicy],
    assignments: &[(Uuid, Uuid)],
) -> DataRoleResponse {
    DataRoleResponse {
        policies: policies
            .iter()
            .filter(|p| p.data_role_id == role.id)
            .map(|p| crate::engine::store::RolePolicy {
                table_id: p.table_id,
                column: p.column.clone(),
                allowed_values: p.allowed_values.clone(),
            })
            .collect(),
        member_ids: assignments
            .iter()
            .filter(|(_, role_id)| *role_id == role.id)
            .map(|(user_id, _)| *user_id)
            .collect(),
        id: role.id,
        name: role.name,
        description: role.description,
    }
}

/// Tên bỏ khoảng trắng hai đầu, 1 tới NAME_MAX ký tự; mô tả rỗng thành None.
fn clean_request(req: &DataRoleRequest) -> Result<(&str, Option<&str>), AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::InvalidField {
            field: "name".to_string(),
            message: format!("Tên vai cần từ 1 đến {NAME_MAX} ký tự"),
        });
    }
    let description = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if description.is_some_and(|d| d.chars().count() > DESCRIPTION_MAX) {
        return Err(AppError::InvalidField {
            field: "description".to_string(),
            message: format!("Mô tả tối đa {DESCRIPTION_MAX} ký tự"),
        });
    }
    Ok((name, description))
}

fn finish_write(outcome: RoleWriteOutcome, name: &str) -> Result<DataRoleRow, AppError> {
    match outcome {
        RoleWriteOutcome::Done(row) => Ok(row),
        RoleWriteOutcome::NotFound => Err(role_not_found()),
        RoleWriteOutcome::NameTaken => Err(AppError::InvalidField {
            field: "name".to_string(),
            message: format!("Workspace đã có vai tên {name}"),
        }),
    }
}

fn role_not_found() -> AppError {
    AppError::NotFound("Không có vai dữ liệu này trong workspace".to_string())
}
