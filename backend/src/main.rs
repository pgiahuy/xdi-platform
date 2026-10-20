// SPDX-License-Identifier: Apache-2.0
/* main.rs - điểm vào: đọc cấu hình, dựng AppState, gắn router của tầng api, lắng nghe.
Phụ thuộc đi một chiều: api gọi service, service gọi engine và auth. Xem docs/ARCHITECTURE.md mục 3. */

mod api;
mod auth;
mod config;
mod engine;
mod error;
mod service;
mod state;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::Config::from_env();
    let addr = format!("0.0.0.0:{}", config.port);
    let state = state::AppState::new(config);
    let app = api::router(state);

    tracing::info!(%addr, "backend listening");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("cannot bind port, check BACKEND_PORT");
    axum::serve(listener, app)
        .await
        .expect("server stopped unexpectedly");
}
