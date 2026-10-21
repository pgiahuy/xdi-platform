// SPDX-License-Identifier: Apache-2.0
/* main.rs - điểm vào: đọc cấu hình, dựng AppState, gắn router của tầng api, lắng nghe.
Phụ thuộc đi một chiều: api gọi service, service gọi engine và auth. Xem docs/ARCHITECTURE.md mục 3. */

mod api;
mod auth;
mod cli;
mod config;
mod engine;
mod error;
mod service;
mod state;

#[tokio::main]
async fn main() {
    // Có tham số là chạy lệnh quản trị (cli.rs): kết quả in ra stdout, nên log chỉ hiện từ mức cảnh báo, để không lẫn
    // vào kết quả (compose đặt RUST_LOG=info cho server). Log luôn ghi ra stderr.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = if args.is_empty() {
        // Không đặt RUST_LOG thì in mức info: lần chạy đầu phải thấy được mã thiết lập và dòng "backend listening".
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    } else {
        tracing_subscriber::EnvFilter::new("warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let config = config::Config::from_env();
    let addr = format!("0.0.0.0:{}", config.port);

    // Migration chạy trước khi nhận request, để mọi request đều thấy schema đúng.
    let store = engine::store::Store::connect(&config.database_url, config.db_max_connections)
        .await
        .expect("cannot connect to postgres, check DATABASE_URL");
    store
        .migrate()
        .await
        .expect("cannot apply database migrations");
    tracing::info!("database migrations applied");

    // Có tham số thì chạy lệnh quản trị (cli.rs) rồi thoát, không mở server.
    if !args.is_empty() {
        let state = state::AppState::new(config, store);
        std::process::exit(cli::run(&state, &args).await);
    }

    // Chưa có tài khoản nào: in mã thiết lập ra log. Chỉ người đọc được log của máy chủ mới tạo được tài khoản quản trị
    // đầu tiên, nên người ngoài không chiếm được bản cài vừa đưa lên mạng.
    let setup_code = if store.has_any_user().await.expect("cannot read users table") {
        None
    } else {
        let code = auth::token::new_setup_code().expect("cannot generate setup code");
        tracing::warn!(
            setup_code = %code,
            "no account yet: open the web UI and enter this setup code to create the administrator"
        );
        Some(code)
    };

    service::ingest::clear_staging(&config.data_dir);

    let state = state::AppState::new(config, store);
    *state.setup_code.lock().expect("setup code lock") = setup_code;
    let app = api::router(state);

    tracing::info!(%addr, "backend listening");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("cannot bind port, check BACKEND_PORT");
    axum::serve(listener, app)
        .await
        .expect("server stopped unexpectedly");
}
