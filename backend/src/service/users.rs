// SPDX-License-Identifier: Apache-2.0
/* service/users.rs - quản lý tài khoản: người dùng tự đổi mật khẩu; quản trị hệ thống tạo, khóa, mở tài khoản và
đặt lại mật khẩu cho người khác.

Quản trị hệ thống chỉ quản lý tài khoản, không vì vậy mà đọc được dữ liệu workspace (docs/ARCHITECTURE.md mục 4b).
Đặt lại mật khẩu sinh mật khẩu tạm hiện một lần, không gửi link: bản đầu chưa gửi thư. */

use uuid::Uuid;

use crate::auth::{password, token};
use crate::engine::store::{Deactivated, User, UserSummary};
use crate::error::AppError;
use crate::service::accounts::{clean_email, hash_password};
use crate::state::AppState;

fn require_system_admin(actor: &User) -> Result<(), AppError> {
    if actor.is_system_admin {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Chỉ quản trị hệ thống làm được việc này".to_string(),
        ))
    }
}

/// Người dùng tự đổi mật khẩu. Các phiên khác của họ bị đăng xuất (phòng khi mật khẩu cũ đã lộ), phiên đang dùng giữ lại.
pub async fn change_password(
    state: &AppState,
    user: &User,
    current_session: &str,
    current_password: String,
    new_password: String,
) -> Result<(), AppError> {
    password::validate(&new_password)?;
    let stored = state
        .store
        .password_hash_of(user.id)
        .await?
        .unwrap_or_default();
    let verified = stored.clone();
    let matches = tokio::task::spawn_blocking(move || password::verify(&current_password, &stored))
        .await
        .map_err(|e| AppError::Internal(format!("verify password task: {e}")))?;
    if !matches {
        return Err(AppError::BadRequest(
            "Mật khẩu hiện tại không đúng".to_string(),
        ));
    }
    let keep = token::hash_session_token(current_session);
    // false: mật khẩu vừa bị quản trị đặt lại trong lúc kiểm; không ghi đè mật khẩu tạm của quản trị.
    if !set_password(state, user.id, new_password, Some(&verified), Some(&keep)).await? {
        return Err(AppError::Unauthorized(
            "Mật khẩu vừa được đặt lại, hãy đăng nhập lại".to_string(),
        ));
    }
    Ok(())
}

pub async fn list(state: &AppState, actor: &User) -> Result<Vec<UserSummary>, AppError> {
    require_system_admin(actor)?;
    state.store.list_users().await
}

/// Tạo tài khoản thường. Người mới chưa thuộc workspace nào cho tới khi admin của workspace thêm họ vào (#44).
pub async fn create(
    state: &AppState,
    actor: &User,
    email: &str,
    display_name: &str,
    initial_password: String,
) -> Result<User, AppError> {
    require_system_admin(actor)?;
    let email = clean_email(email)?;
    password::validate(&initial_password)?;
    let hash = hash_password(initial_password).await?;
    let user = state
        .store
        .create_user(&email, display_name.trim(), &hash)
        .await?;
    tracing::info!(actor = %actor.id, user_id = %user.id, "account created");
    Ok(user)
}

pub async fn deactivate(state: &AppState, actor: &User, user_id: Uuid) -> Result<(), AppError> {
    require_system_admin(actor)?;
    // Tự khóa mình thì không ai mở lại được qua giao diện.
    if user_id == actor.id {
        return Err(AppError::BadRequest(
            "Không tự khóa tài khoản của chính mình".to_string(),
        ));
    }
    match state.store.deactivate_user(user_id).await? {
        Deactivated::Done => {
            tracing::info!(actor = %actor.id, user_id = %user_id, "account deactivated");
            Ok(())
        }
        Deactivated::NotFound => Err(not_found()),
        Deactivated::LastSystemAdmin => Err(AppError::BadRequest(
            "Không khóa được quản trị hệ thống cuối cùng còn hoạt động".to_string(),
        )),
    }
}

pub async fn activate(state: &AppState, actor: &User, user_id: Uuid) -> Result<(), AppError> {
    require_system_admin(actor)?;
    if !state.store.activate_user(user_id).await? {
        return Err(not_found());
    }
    tracing::info!(actor = %actor.id, user_id = %user_id, "account activated");
    Ok(())
}

/// Quản trị hệ thống đặt lại mật khẩu cho người khác. Trả mật khẩu tạm để hiện một lần.
pub async fn reset_password(
    state: &AppState,
    actor: &User,
    user_id: Uuid,
) -> Result<String, AppError> {
    require_system_admin(actor)?;
    let temporary = reset_password_unchecked(state, user_id).await?;
    tracing::info!(actor = %actor.id, user_id = %user_id, "password reset");
    Ok(temporary)
}

/// Dùng cho lệnh `xdi-backend reset-password` chạy trên máy chủ: ai vào được máy chủ là chủ bản cài, không cần đăng
/// nhập. Cũng là cách cứu khi quản trị hệ thống duy nhất quên mật khẩu.
pub async fn reset_password_by_email(state: &AppState, email: &str) -> Result<String, AppError> {
    let user_id = state
        .store
        .find_user_id_by_email(email.trim())
        .await?
        .ok_or_else(not_found)?;
    reset_password_unchecked(state, user_id).await
}

async fn reset_password_unchecked(state: &AppState, user_id: Uuid) -> Result<String, AppError> {
    let temporary = token::new_temporary_password()?;
    // Ai đang giữ phiên cũ (có thể chính là người lấy được mật khẩu cũ) bị đăng xuất ngay.
    if !set_password(state, user_id, temporary.clone(), None, None).await? {
        return Err(not_found());
    }
    Ok(temporary)
}

/// Đổi mật khẩu và đăng xuất mọi phiên trừ keep, trong một giao dịch. Xem Store::set_password_and_revoke về expected.
async fn set_password(
    state: &AppState,
    user_id: Uuid,
    plain: String,
    expected: Option<&str>,
    keep: Option<&str>,
) -> Result<bool, AppError> {
    let hash = hash_password(plain).await?;
    state
        .store
        .set_password_and_revoke(user_id, &hash, expected, keep)
        .await
}

fn not_found() -> AppError {
    AppError::NotFound("Không có tài khoản này".to_string())
}
