// SPDX-License-Identifier: Apache-2.0
/* api/extract.rs - lấy người dùng và workspace từ request, dùng như tham số của handler.

Handler khai báo `user: CurrentUser` thì chỉ chạy khi đã đăng nhập (không thì 401). Khai báo `member: Member` thì chỉ
chạy khi đã đăng nhập và là thành viên workspace {ws} trong đường dẫn (không thì 401 hoặc 404). Kiểm một lần ở đây,
nên không handler nào quên kiểm. */

use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::request::Parts,
};
use axum_extra::extract::cookie::CookieJar;

use super::auth::session_user;
use crate::auth::Member;
use crate::engine::store::User;
use crate::service::workspaces;
use crate::{error::AppError, state::AppState};

/// Người dùng của phiên đăng nhập.
pub struct CurrentUser(pub User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        Ok(CurrentUser(session_user(state, &jar).await?))
    }
}

impl FromRequestParts<AppState> for Member {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        // RawPathParams đọc tham số đường dẫn mà không giành mất Path của handler (handler vẫn đọc {id} như thường).
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::Internal(format!("read path params: {e}")))?;
        let slug = params
            .iter()
            .find(|(key, _)| *key == "ws")
            .map(|(_, value)| value.to_string())
            .ok_or_else(|| {
                AppError::Internal("route uses Member but has no {ws} segment".to_string())
            })?;
        workspaces::member(state, user, &slug).await
    }
}
