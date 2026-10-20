// SPDX-License-Identifier: Apache-2.0
/* api/auth.rs - đăng nhập demo theo vai và thông tin người dùng hiện tại. Logic ở auth/. */

use axum::{
    routing::{get, post},
    Router,
};

use crate::{error::AppError, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/demo-login", post(demo_login))
        .route("/auth/me", get(me))
}

async fn demo_login() -> Result<(), AppError> {
    Err(AppError::NotImplemented("đăng nhập demo theo vai"))
}

async fn me() -> Result<(), AppError> {
    Err(AppError::NotImplemented("thông tin người dùng hiện tại"))
}
