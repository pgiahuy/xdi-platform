// SPDX-License-Identifier: Apache-2.0
/* main.rs - điểm vào: đọc cấu hình, dựng AppState, gắn router của tầng api, lắng nghe. Có tham số thì chạy lệnh quản
trị (cli.rs), hoặc tiến trình con đọc file Excel (engine/excel_worker.rs).
Phụ thuộc đi một chiều: api gọi service, service gọi engine và auth. Xem docs/ARCHITECTURE.md mục 3. */

mod api;
mod auth;
mod cli;
mod config;
mod engine;
mod error;
mod service;
mod state;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // In đặc tả OpenAPI (make openapi ghi nó ra openapi.json): rẽ ra trước khi đọc cấu hình hay kết nối database, để
    // sinh được trên máy chưa có Postgres, như trong CI.
    if args == ["openapi"] {
        print!("{}", api::build_openapi_json());
        return;
    }
    // Response mẫu của ask cho giao diện (make openapi ghi ra frontend/src/api/examples/ask.json), cũng không cần
    // database.
    if args == ["ask-examples"] {
        print!("{}", service::ask_examples::build_examples_json());
        return;
    }

    // Tiến trình con đọc file Excel: rẽ ra trước khi dựng runtime tokio. Runtime nhiều luồng giữ sẵn hàng GB vùng nhớ
    // ảo (mỗi luồng một arena của glibc), tính vào trần RAM của tiến trình con và làm file hợp lệ cũng bị từ chối
    // trên máy nhiều nhân. Tiến trình con chỉ cần một luồng, log theo RUST_LOG như server.
    if args.first().map(String::as_str) == Some(engine::excel_worker::WORKER_ARG) {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .init();
        std::process::exit(engine::excel_worker::serve());
    }

    // Tương đương #[tokio::main], viết tay để nhánh trên chạy được mà không có runtime.
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("cannot start tokio runtime")
        .block_on(run(args));
}

async fn run(args: Vec<String>) {
    // Có tham số là chạy lệnh quản trị (cli.rs): kết quả in ra stdout, nên log chỉ hiện từ mức cảnh báo, để không lẫn
    // vào kết quả (compose đặt RUST_LOG=info cho server). Log luôn ghi ra stderr.
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
    let store = engine::store::Store::connect(
        &config.database_url,
        config.db_max_connections,
        config.db_acquire_timeout,
    )
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

    config.log_warnings();

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

    // Phiên bản nạp trước khi có hồ sơ cột: lấy danh sách trước khi nhận request (phiên bản nạp sau đó có hồ sơ ngay
    // lúc nạp), rồi lập hồ sơ nền để không chặn khởi động. Không đọc được danh sách thì chỉ cảnh báo.
    let pending = auth::pending_profiles(&store).await.unwrap_or_else(|e| {
        tracing::warn!(error = ?e, "cannot list dataset versions waiting for a column profile");
        Vec::new()
    });

    let state = state::AppState::new(config, store);
    *state.setup_code.lock().expect("setup code lock") = setup_code;
    // Quét đánh dấu nhạy cảm cho phiên bản nạp trước khi luật nằm ở lớp cấu trúc: đồng bộ, trước khi nhận request,
    // để ngữ cảnh hỏi (#28) không gửi giá trị cột nhạy cảm của phiên bản cũ trong khởi đầu phiên chạy. Rẻ: hai câu
    // SELECT mỗi phiên bản.
    service::profile::mark_sensitive_columns_at_start(&state).await;

    // Việc nền còn lại trong một task: lớp cấu trúc cho phiên bản pending xong trước, rồi lớp ngữ nghĩa cho phiên bản
    // structure chưa từng thử (gồm cả các phiên bản vừa lên structure ở bước trước).
    tokio::spawn({
        let state = state.clone();
        async move {
            service::profile::profile_pending_versions(state.clone(), pending).await;
            service::semantic::profile_remaining_versions(state).await;
        }
    });
    let app = api::router(state);

    tracing::info!(%addr, "backend listening");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("cannot bind port, check BACKEND_PORT");
    axum::serve(listener, app)
        .await
        .expect("server stopped unexpectedly");
}
