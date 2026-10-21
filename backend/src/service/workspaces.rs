// SPDX-License-Identifier: Apache-2.0
/* service/workspaces.rs - workspace của người dùng: liệt kê, tạo mới, và kiểm một người có thuộc workspace không.

Workspace đang làm việc nằm trong đường dẫn (/api/workspaces/{ws}/...), không lưu trong phiên: mỗi tab trình duyệt
làm việc độc lập, không có chuyện tab này đổi workspace của tab kia rồi nạp nhầm dữ liệu. */

use crate::auth::Member;
use crate::engine::store::{Membership, User};
use crate::error::AppError;
use crate::service::accounts::slug_from_name;
use crate::state::AppState;

const NAME_MAX: usize = 100;

pub async fn list(state: &AppState, user: &User) -> Result<Vec<Membership>, AppError> {
    state.store.list_memberships(user.id).await
}

/// Ai đăng nhập cũng tạo được workspace; người tạo là admin của nó.
pub async fn create(state: &AppState, user: &User, name: &str) -> Result<Membership, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(AppError::BadRequest(format!(
            "Tên workspace cần từ 1 đến {NAME_MAX} ký tự"
        )));
    }
    let membership = state
        .store
        .create_workspace_for(user.id, name, &slug_from_name(name))
        .await?;
    tracing::info!(user_id = %user.id, slug = %membership.slug, "workspace created");
    Ok(membership)
}

/// Người dùng có thuộc workspace slug không. Không có workspace và không phải thành viên trả cùng một lỗi 404,
/// để người ngoài không dò được tên workspace nào tồn tại.
pub async fn member(state: &AppState, user: User, slug: &str) -> Result<Member, AppError> {
    let membership = state
        .store
        .find_membership(user.id, slug)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(
                "Không có workspace này hoặc bạn không thuộc workspace này".to_string(),
            )
        })?;
    Ok(Member::from_membership(user, membership))
}
