// SPDX-License-Identifier: Apache-2.0
/* service/accounts.rs - thiết lập lần đầu, đăng nhập, đăng xuất, phiên hiện tại.

Phiên: mã ngẫu nhiên gửi cho trình duyệt qua cookie, database giữ bản băm (auth/token.rs). Mọi thông báo lỗi đăng
nhập giống nhau, không nói email có tồn tại hay không, để người ngoài không dò được ai có tài khoản. */

use std::sync::OnceLock;

use chrono::{Duration, Utc};

use crate::auth::{password, token};
use crate::engine::store::{Membership, NewAdmin, User};
use crate::error::AppError;
use crate::state::AppState;

/// Phiên hết hạn sau 30 ngày; đăng xuất hay khóa tài khoản thì hết ngay.
pub const SESSION_DAYS: i64 = 30;

const WRONG_LOGIN: &str = "Email hoặc mật khẩu không đúng";
const NOT_LOGGED_IN: &str = "Chưa đăng nhập hoặc phiên đã hết hạn";

pub struct SetupInput {
    pub setup_code: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub workspace_name: String,
}

/// Còn cần thiết lập không: còn mã thiết lập trong bộ nhớ nghĩa là lúc khởi động chưa có tài khoản nào.
pub fn setup_needed(state: &AppState) -> bool {
    state.setup_code.lock().expect("setup code lock").is_some()
}

/// Tạo tài khoản quản trị đầu tiên và đăng nhập luôn. Trả người dùng và mã phiên để đặt vào cookie.
pub async fn setup(state: &AppState, input: SetupInput) -> Result<(User, String), AppError> {
    let expected = state.setup_code.lock().expect("setup code lock").clone();
    let Some(expected) = expected else {
        return Err(AppError::Forbidden(
            "Bản cài này đã được thiết lập".to_string(),
        ));
    };
    // Người dùng gõ tay: bỏ khoảng trắng, không phân biệt hoa thường.
    if input.setup_code.trim().to_uppercase() != expected {
        return Err(AppError::Forbidden(
            "Mã thiết lập không đúng. Mã nằm trong log của backend lúc khởi động".to_string(),
        ));
    }

    let email = clean_email(&input.email)?;
    let display_name = input.display_name.trim();
    password::validate(&input.password)?;
    let workspace_name = match input.workspace_name.trim() {
        "" => "Workspace đầu tiên",
        name => name,
    };
    let password_hash = hash_password(input.password).await?;

    let user = state
        .store
        .create_first_admin(&NewAdmin {
            email: &email,
            display_name,
            password_hash: &password_hash,
            workspace_name,
            workspace_slug: &slug_from_name(workspace_name),
        })
        .await?
        .ok_or_else(|| AppError::Forbidden("Bản cài này đã được thiết lập".to_string()))?;
    *state.setup_code.lock().expect("setup code lock") = None;
    tracing::info!(user_id = %user.id, "administrator account created");

    let session = start_session(state, &user).await?;
    Ok((user, session))
}

pub async fn login(
    state: &AppState,
    email: &str,
    password: String,
) -> Result<(User, String), AppError> {
    let credentials = state.store.find_credentials(email.trim()).await?;
    // Không có tài khoản thì vẫn kiểm mật khẩu với một chuỗi băm giả, để thời gian trả lời như khi email có thật.
    let (user_id, stored_hash) = match &credentials {
        Some(c) if c.is_active => (Some(c.id), c.password_hash.clone()),
        _ => (None, None),
    };
    let hash = stored_hash.unwrap_or_else(|| dummy_hash().to_string());
    let matches = tokio::task::spawn_blocking(move || password::verify(&password, &hash))
        .await
        .map_err(|e| AppError::Internal(format!("verify password task: {e}")))?;

    let Some(user_id) = user_id.filter(|_| matches) else {
        return Err(AppError::Unauthorized(WRONG_LOGIN.to_string()));
    };
    let user = state
        .store
        .find_user(user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized(WRONG_LOGIN.to_string()))?;
    let session = start_session(state, &user).await?;
    Ok((user, session))
}

pub async fn logout(state: &AppState, session_token: &str) -> Result<(), AppError> {
    state
        .store
        .delete_session(&token::hash_session_token(session_token))
        .await
}

/// Người dùng của phiên trong cookie; không có hoặc hết hạn thì 401.
pub async fn current_user(state: &AppState, session_token: Option<&str>) -> Result<User, AppError> {
    let Some(session_token) = session_token else {
        return Err(AppError::Unauthorized(NOT_LOGGED_IN.to_string()));
    };
    state
        .store
        .find_session_user(&token::hash_session_token(session_token))
        .await?
        .ok_or_else(|| AppError::Unauthorized(NOT_LOGGED_IN.to_string()))
}

pub async fn memberships(state: &AppState, user: &User) -> Result<Vec<Membership>, AppError> {
    state.store.list_memberships(user.id).await
}

async fn start_session(state: &AppState, user: &User) -> Result<String, AppError> {
    let session = token::new_session_token()?;
    let expires_at = Utc::now() + Duration::days(SESSION_DAYS);
    state
        .store
        .create_session(user.id, &token::hash_session_token(&session), expires_at)
        .await?;
    Ok(session)
}

// Băm tốn CPU (auth/password.rs): chạy trên luồng riêng để không chặn các request khác.
async fn hash_password(plain: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || password::hash(&plain))
        .await
        .map_err(|e| AppError::Internal(format!("hash password task: {e}")))?
}

/// Chuỗi băm của một mật khẩu không ai biết, tính một lần rồi dùng lại.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        password::hash("not a real password, only for timing").expect("hash dummy password")
    })
}

fn clean_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim();
    // Kiểm sơ bộ; định dạng đầy đủ để lúc gửi thư mời (sau này) xác nhận.
    let valid = email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty());
    if !valid {
        return Err(AppError::BadRequest("Email không hợp lệ".to_string()));
    }
    Ok(email.to_string())
}

/// "Cửa hàng Online" thành "cua-hang-online". Tên không có chữ cái hay số nào thì dùng "workspace".
pub fn slug_from_name(name: &str) -> String {
    let ascii = deunicode::deunicode(name).to_lowercase();
    let slug = ascii
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "workspace".to_string()
    } else {
        slug.chars()
            .take(48)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_folds_vietnamese() {
        assert_eq!(slug_from_name("Cửa hàng Online"), "cua-hang-online");
        assert_eq!(slug_from_name("Chi tiêu  gia đình!"), "chi-tieu-gia-dinh");
        assert_eq!(slug_from_name("???"), "workspace");
    }

    #[test]
    fn email_needs_local_part_and_domain() {
        assert_eq!(clean_email("  an@example.com ").unwrap(), "an@example.com");
        assert!(clean_email("an@").is_err());
        assert!(clean_email("@example.com").is_err());
        assert!(clean_email("an.example.com").is_err());
        // Bản cài trên máy cá nhân hay dùng tên miền không có dấu chấm.
        assert!(clean_email("admin@localhost").is_ok());
    }
}
