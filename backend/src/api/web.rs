// SPDX-License-Identifier: Apache-2.0
/* api/web.rs - phục vụ file giao diện đã build (frontend/dist) ở mọi đường dẫn ngoài /api.
Đường dẫn không khớp file nào trả index.html với mã 200, để React Router phía trình duyệt chọn trang; nhờ vậy tải
lại trang ở /datasets/abc vẫn chạy. /api không đi qua đây: router chính gắn /api trước, có 404 JSON riêng.
Riêng /assets (JS, CSS do Vite build) không có fallback: file thiếu trả 404. Nếu trả index.html, trình duyệt còn giữ
trang cũ sau khi cập nhật sẽ báo lỗi MIME khó hiểu thay vì 404. */

use std::path::Path;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

pub fn routes(dir: &Path) -> Router<AppState> {
    let index = dir.join("index.html");
    // Cấu hình sai thì mọi trang đều 404; báo ngay lúc khởi động thay vì để người dùng tự đoán.
    if !index.is_file() {
        tracing::warn!(path = %index.display(), "STATIC_DIR has no index.html, the UI will not load");
    }
    Router::new()
        .nest_service("/assets", ServeDir::new(dir.join("assets")))
        .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use axum::{body::Body, http::Request, http::StatusCode, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::config::Config;
    use crate::state::AppState;

    /// Thư mục giả lập frontend/dist: index.html và một file trong assets/. Tự xóa khi biến ra khỏi phạm vi (Drop),
    /// kể cả khi test dừng giữa chừng vì assert thất bại.
    struct FakeDist(PathBuf);

    impl FakeDist {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("xdi-web-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(dir.join("assets")).unwrap();
            std::fs::write(dir.join("index.html"), "<!doctype html><p>test ui</p>").unwrap();
            std::fs::write(dir.join("assets/app.js"), "console.log('test')").unwrap();
            FakeDist(dir)
        }
    }

    impl Drop for FakeDist {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn app(dir: &Path) -> Router {
        let mut config = Config::from_env();
        config.static_dir = Some(dir.to_path_buf());
        let store = crate::engine::store::Store::lazy(&config.database_url);
        crate::api::router(AppState::new(config, store))
    }

    async fn get(app: Router, path: &str) -> (StatusCode, String, String) {
        let res = app
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let content_type = res
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            content_type,
            String::from_utf8_lossy(&body).into_owned(),
        )
    }

    #[tokio::test]
    async fn serves_index_and_assets() {
        let dist = FakeDist::new();
        let dir = &dist.0;
        let (status, content_type, body) = get(app(dir), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "got {content_type}");
        assert!(body.contains("test ui"));

        let (status, content_type, _) = get(app(dir), "/assets/app.js").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.contains("javascript"), "got {content_type}");
    }

    #[tokio::test]
    async fn missing_asset_is_404_not_the_page() {
        let dist = FakeDist::new();
        let (status, _, _) = get(app(&dist.0), "/assets/missing.js").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_page_falls_back_to_index() {
        let dist = FakeDist::new();
        let dir = &dist.0;
        let (status, _, body) = get(app(dir), "/datasets/abc").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("test ui"));
    }

    #[tokio::test]
    async fn api_paths_never_return_the_ui() {
        let dist = FakeDist::new();
        let dir = &dist.0;
        let (status, _, body) = get(app(dir), "/api/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, r#"{"status":"ok"}"#);

        for path in [
            "/api",
            "/api/",
            "/api/does-not-exist",
            "/api/datasets/abc/x",
        ] {
            let (status, content_type, _) = get(app(dir), path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "path {path}");
            assert!(
                content_type.starts_with("application/json"),
                "path {path}: got {content_type}"
            );
        }
    }
}
