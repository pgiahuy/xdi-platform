// SPDX-License-Identifier: Apache-2.0
/* api/system.rs - endpoint hệ thống: /api/health cho compose và CI, /api/version cho giao diện. */

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct VersionResponse {
    name: &'static str,
    version: &'static str,
    /// Nhà cung cấp LLM đang dùng, để giao diện hiện "LLM: mock" hay "LLM: gemini-2.5-flash".
    llm: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn version(State(state): State<AppState>) -> Json<VersionResponse> {
    // Lấy từ Cargo.toml lúc biên dịch, để số phiên bản chỉ đổi ở một chỗ.
    Json(VersionResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        llm: state.llm.name(),
    })
}
