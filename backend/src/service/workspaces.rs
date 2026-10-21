// SPDX-License-Identifier: Apache-2.0
/* service/workspaces.rs - workspace của người dùng: liệt kê, tạo, đổi tên, xóa; thành viên: xem, thêm, đổi vai, xóa;
phần của quản trị hệ thống: xem mọi workspace, chỉ định admin. Kiểm thành viên nằm ở auth::member_of, kiểm vai ở
Member::require_admin. Bảng quyền: docs/ARCHITECTURE.md mục 4b.

Workspace đang làm việc nằm trong đường dẫn (/api/workspaces/{ws}/...), không lưu trong phiên: mỗi tab trình duyệt
làm việc độc lập, không có chuyện tab này đổi workspace của tab kia rồi nạp nhầm dữ liệu. */

use uuid::Uuid;

use crate::auth::{Member, Role};
use crate::engine::store::{
    ChangeOutcome, MemberChange, MemberRow, Membership, User, WorkspaceOverview,
};
use crate::error::AppError;
use crate::service::accounts::slug_from_name;
use crate::service::users::require_system_admin;
use crate::state::AppState;

const NAME_MAX: usize = 100;

pub async fn list(state: &AppState, user: &User) -> Result<Vec<Membership>, AppError> {
    state.store.list_memberships(user.id).await
}

/// Ai đăng nhập cũng tạo được workspace; người tạo là admin của nó.
pub async fn create(state: &AppState, user: &User, name: &str) -> Result<Membership, AppError> {
    let name = clean_name(name)?;
    let membership = state
        .store
        .create_workspace_for(user.id, name, &slug_from_name(name))
        .await?;
    tracing::info!(user_id = %user.id, slug = %membership.slug, "workspace created");
    Ok(membership)
}

/// Đổi tên hiển thị. Slug giữ nguyên để đường dẫn đã chia sẻ không hỏng.
pub async fn rename(state: &AppState, member: &Member, name: &str) -> Result<(), AppError> {
    member.require_admin()?;
    let name = clean_name(name)?;
    state.store.rename_workspace(&member.workspace, name).await
}

/// Xóa workspace cùng mọi dữ liệu: dòng trong Postgres (khóa ngoại xóa theo) và thư mục Parquet trên đĩa.
pub async fn delete(state: &AppState, member: &Member) -> Result<(), AppError> {
    member.require_admin()?;
    state.store.delete_workspace(&member.workspace).await?;
    let dir = state
        .config
        .data_dir
        .join(member.workspace.as_uuid().to_string());
    match tokio::fs::remove_dir_all(&dir).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        // Database đã xóa xong; file còn sót chỉ tốn chỗ, không lộ dữ liệu qua ứng dụng. Ghi log để dọn tay.
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "workspace deleted but its files remain")
        }
    }
    tracing::info!(user_id = %member.user.id, slug = %member.slug, "workspace deleted");
    Ok(())
}

/// Ai trong workspace cũng xem được danh sách thành viên.
pub async fn members(state: &AppState, member: &Member) -> Result<Vec<MemberRow>, AppError> {
    state.store.list_members(&member.workspace).await
}

/// Thêm một tài khoản có sẵn (theo email) vào workspace. Tài khoản mới do quản trị hệ thống tạo.
///
/// Ai cũng tạo được workspace và là admin của nó, nên mọi người đăng nhập đều dò được một email có tài khoản hay
/// không (thêm thành công hay báo không có), và thêm được người khác mà không cần họ đồng ý; người bị thêm tự rời được.
/// Chấp nhận vì một bản cài phục vụ một tổ chức, tài khoản do quản trị hệ thống tạo cho người trong tổ chức.
pub async fn add_member(
    state: &AppState,
    member: &Member,
    email: &str,
    role: &str,
) -> Result<(), AppError> {
    member.require_admin()?;
    let role = parse_role(role)?;
    let user_id = state
        .store
        .find_active_user_id_by_email(email.trim())
        .await?
        .ok_or_else(|| {
            AppError::NotFound(
                "Không có tài khoản đang hoạt động với email này; nhờ quản trị hệ thống tạo tài khoản trước"
                    .to_string(),
            )
        })?;
    if !state
        .store
        .add_member(&member.workspace, user_id, role)
        .await?
    {
        return Err(AppError::BadRequest(
            "Người này đã là thành viên của workspace".to_string(),
        ));
    }
    tracing::info!(actor = %member.user.id, user_id = %user_id, slug = %member.slug, role = role.as_str(), "member added");
    Ok(())
}

pub async fn set_role(
    state: &AppState,
    member: &Member,
    user_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    member.require_admin()?;
    let role = parse_role(role)?;
    apply_change(state, member, user_id, MemberChange::SetRole(role)).await
}

/// Admin xóa thành viên, hoặc một người tự rời workspace.
pub async fn remove_member(
    state: &AppState,
    member: &Member,
    user_id: Uuid,
) -> Result<(), AppError> {
    if user_id != member.user.id {
        member.require_admin()?;
    }
    apply_change(state, member, user_id, MemberChange::Remove).await
}

async fn apply_change(
    state: &AppState,
    member: &Member,
    user_id: Uuid,
    change: MemberChange,
) -> Result<(), AppError> {
    match state
        .store
        .change_member(&member.workspace, user_id, change)
        .await?
    {
        ChangeOutcome::Done => {
            tracing::info!(actor = %member.user.id, user_id = %user_id, slug = %member.slug, "membership changed");
            Ok(())
        }
        ChangeOutcome::NotMember => Err(AppError::NotFound(
            "Người này không thuộc workspace".to_string(),
        )),
        ChangeOutcome::LastAdmin => Err(AppError::BadRequest(
            "Workspace phải còn ít nhất một admin: chỉ định admin khác trước".to_string(),
        )),
    }
}

/// Quản trị hệ thống xem mọi workspace và thành viên, không xem được dữ liệu bên trong.
pub async fn overview(state: &AppState, actor: &User) -> Result<Vec<WorkspaceOverview>, AppError> {
    require_system_admin(actor)?;
    state.store.list_workspaces_with_members().await
}

/// Quản trị hệ thống chỉ định admin cho một workspace bất kỳ, lúc nào cũng được, kể cả chính mình. Mỗi lần ghi log mức
/// cảnh báo (log của máy chủ, chưa lưu vào database), vì đây là cách duy nhất để quản trị hệ thống vào được workspace
/// không phải của mình.
pub async fn assign_admin(
    state: &AppState,
    actor: &User,
    workspace_id: Uuid,
    email: &str,
) -> Result<(), AppError> {
    require_system_admin(actor)?;
    let user_id = state
        .store
        .find_active_user_id_by_email(email.trim())
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Không có tài khoản đang hoạt động với email này".to_string())
        })?;
    if !state
        .store
        .make_workspace_admin(workspace_id, user_id)
        .await?
    {
        return Err(AppError::NotFound("Không có workspace này".to_string()));
    }
    tracing::warn!(actor = %actor.id, workspace_id = %workspace_id, user_id = %user_id, "system admin assigned a workspace admin");
    Ok(())
}

fn parse_role(raw: &str) -> Result<Role, AppError> {
    Role::parse(raw.trim())
        .ok_or_else(|| AppError::BadRequest("Vai phải là admin hoặc member".to_string()))
}

fn clean_name(name: &str) -> Result<&str, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest(format!(
            "Tên workspace cần từ 1 đến {NAME_MAX} ký tự"
        )));
    }
    Ok(name)
}
