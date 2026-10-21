// SPDX-License-Identifier: Apache-2.0
/* api/extract.rs - lấy người dùng, workspace, body JSON và tham số đường dẫn từ request, dùng như tham số của handler.

Handler khai báo `user: CurrentUser` thì chỉ chạy khi đã đăng nhập (không thì 401). Khai báo `member: Member` thì chỉ
chạy khi đã đăng nhập và là thành viên workspace {ws} trong đường dẫn (không thì 401 hoặc 404). Kiểm một lần ở đây,
nên không handler nào quên kiểm.

JsonBody và PathParams thay cho Json và Path của axum: hai cái đó trả lỗi dạng chữ thường (400, 415), còn ở đây lỗi
luôn là JSON {code, message} như mọi lỗi khác của API, để giao diện đọc được. */

use axum::{
    extract::{
        rejection::JsonRejection, FromRequest, FromRequestParts, Path, RawPathParams, Request,
    },
    http::request::Parts,
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use serde::de::DeserializeOwned;

use super::auth::session_user;
use crate::auth::Member;
use crate::engine::store::User;
use crate::{auth, error::AppError, state::AppState};

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
        // Đọc lỗi (ví dụ %FF không phải UTF-8) thì coi như không có workspace đó.
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|_| auth::workspace_not_found())?;
        let slug = params
            .iter()
            .find(|(key, _)| *key == "ws")
            .map(|(_, value)| value.to_string())
            .ok_or_else(|| {
                AppError::Internal("route uses Member but has no {ws} segment".to_string())
            })?;
        auth::member_of(&state.store, user, &slug).await
    }
}

/// Body JSON của request. Sai cú pháp, thiếu trường, thiếu header Content-Type đều thành 400 JSON.
pub struct JsonBody<T>(pub T);

impl<T: DeserializeOwned> FromRequest<AppState> for JsonBody<T> {
    type Rejection = AppError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(JsonBody(value)),
            Err(rejection) => {
                tracing::debug!(error = %rejection.body_text(), "rejected json body");
                let message = match rejection {
                    JsonRejection::MissingJsonContentType(_) => {
                        "Cần gửi body JSON với header Content-Type: application/json"
                    }
                    JsonRejection::JsonDataError(_) => {
                        "Body JSON thiếu trường hoặc sai kiểu dữ liệu"
                    }
                    _ => "Body không phải JSON hợp lệ",
                };
                Err(AppError::BadRequest(message.to_string()))
            }
        }
    }
}

/// Tham số đường dẫn. Không đọc được (ví dụ %FF không phải UTF-8) thì trả 404 JSON như đường dẫn không tồn tại.
pub struct PathParams<T>(pub T);

impl<T: DeserializeOwned + Send> FromRequestParts<AppState> for PathParams<T> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| PathParams(value))
            .map_err(|_| AppError::NotFound("Không tìm thấy".to_string()))
    }
}
