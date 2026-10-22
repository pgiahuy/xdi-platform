// SPDX-License-Identifier: Apache-2.0
/* api/system.rs - endpoint hệ thống: /api/health cho compose và CI, /api/version cho giao diện. Không cần đăng nhập. */

use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(health))
        .routes(routes!(version))
}

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    /// Always `ok` when the server answers.
    status: &'static str,
}

#[derive(Serialize, ToSchema)]
struct VersionResponse {
    name: &'static str,
    version: &'static str,
    // Nhà cung cấp LLM đang dùng, để giao diện hiện "LLM: mock" hay "LLM: gemini-2.5-flash".
    /// Language model provider in use, for example `mock` or `gemini-2.5-flash`.
    llm: String,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    operation_id = "health",
    summary = "Liveness check for compose and CI",
    security(()),
    responses((status = 200, body = HealthResponse, description = "OK"))
)]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[utoipa::path(
    get,
    path = "/version",
    tag = "system",
    operation_id = "version",
    summary = "Server name, version and language model provider",
    security(()),
    responses((status = 200, body = VersionResponse, description = "OK"))
)]
async fn version(State(state): State<AppState>) -> Json<VersionResponse> {
    // Lấy từ Cargo.toml lúc biên dịch, để số phiên bản chỉ đổi ở một chỗ.
    Json(VersionResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        llm: state.llm.name(),
    })
}
