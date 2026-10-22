// SPDX-License-Identifier: Apache-2.0
/* api/security_headers.rs - header bảo mật gắn vào mọi response: giao diện, API, Swagger UI, cả trang 404.
router() trong api/mod.rs bọc toàn bộ router bằng middleware này. Backend tự gửi, không trông vào reverse proxy của người
cài, vì backend phục vụ cả giao diện và người cài có thể chạy không qua proxy.

HSTS (Strict-Transport-Security) không gửi ở đây: chỉ proxy biết chắc trình duyệt đang nói chuyện qua HTTPS. Backend gửi
HSTS khi chạy HTTP thường thì vô hại nhưng gây hiểu lầm; proxy gửi HSTS sai thì trình duyệt khóa tên miền vào HTTPS
nhiều tháng. Mẫu Caddy và nginx trong docs/INSTALL.md mục 3. */

use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};

/// CSP cho giao diện và API. Script chỉ từ chính máy chủ: giao diện build bằng Vite không có script inline, nên một lỗi
/// chèn HTML không chạy được JavaScript lạ. Style cho phép 'unsafe-inline' vì Radix UI (hộp thoại, menu, ngăn kéo)
/// chèn thẻ <style> lúc chạy để khóa cuộn trang, nội dung đổi theo bề rộng thanh cuộn nên không ghi trước được mã băm;
/// chặn thì trang vẫn chạy nhưng nền cuộn được khi hộp thoại mở và console báo lỗi CSP. Style không chạy được mã.
/// frame-ancestors 'none' cùng X-Frame-Options: DENY chặn trang khác nhúng giao diện vào iframe để lừa bấm.
const APP_CSP: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; \
                       base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// CSP riêng cho Swagger UI (/api/docs): như APP_CSP, thêm ảnh data: vì CSS của Swagger UI nhúng icon dạng data: URI.
/// Script vẫn chỉ từ chính máy chủ: Swagger UI gói sẵn trong binary, không tải gì từ ngoài.
const DOCS_CSP: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
                        object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// Middleware: chạy handler rồi ghi đè header bảo mật lên response. Ghi đè (insert) chứ không thêm, để handler nào lỡ
/// đặt giá trị lỏng hơn cũng không lọt ra ngoài.
pub async fn apply_security_headers(req: Request, next: Next) -> Response {
    let csp = if is_swagger_ui(req.uri().path()) {
        DOCS_CSP
    } else {
        APP_CSP
    };
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(csp),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    // Trình duyệt không đoán kiểu file khác với Content-Type: file nạp lên không bị hiểu thành script.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // Đường dẫn trong giao diện có tên workspace (/w/<slug>/...): không gửi sang trang khác khi người dùng bấm link ra
    // ngoài.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    res
}

fn is_swagger_ui(path: &str) -> bool {
    path == "/api/docs" || path.starts_with("/api/docs/")
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request, Router};
    use tower::ServiceExt;

    use super::{APP_CSP, DOCS_CSP};
    use crate::config::Config;
    use crate::state::AppState;

    /// Router thật, có hoặc không có thư mục giao diện.
    fn app(static_dir: Option<std::path::PathBuf>) -> Router {
        let mut config = Config::from_env();
        config.static_dir = static_dir;
        let store = crate::engine::store::Store::lazy(&config.database_url);
        crate::api::router(AppState::new(config, store))
    }

    async fn headers_of(app: Router, path: &str) -> axum::http::HeaderMap {
        let res = app
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        res.headers().clone()
    }

    fn assert_security_headers(headers: &axum::http::HeaderMap, csp: &str, path: &str) {
        assert_eq!(headers["content-security-policy"], csp, "{path}");
        assert_eq!(headers["x-frame-options"], "DENY", "{path}");
        assert_eq!(headers["x-content-type-options"], "nosniff", "{path}");
        assert_eq!(headers["referrer-policy"], "same-origin", "{path}");
        assert!(
            headers.get("strict-transport-security").is_none(),
            "{path}: HSTS is the proxy's job"
        );
    }

    /// API: route có thật, 404 JSON, đặc tả OpenAPI.
    #[tokio::test]
    async fn api_responses_have_security_headers() {
        for path in ["/api/health", "/api/does-not-exist", "/api/openapi.json"] {
            let headers = headers_of(app(None), path).await;
            assert_security_headers(&headers, APP_CSP, path);
        }
    }

    /// Swagger UI nhận CSP riêng, cả trang lẫn file JS, CSS của nó.
    #[tokio::test]
    async fn swagger_ui_has_its_own_csp() {
        for path in [
            "/api/docs",
            "/api/docs/",
            "/api/docs/swagger-ui.css",
            "/api/docs/licenses.txt",
        ] {
            let headers = headers_of(app(None), path).await;
            assert_security_headers(&headers, DOCS_CSP, path);
        }
    }

    /// Giao diện: trang chính, trang con (fallback về index.html), file trong assets/ và file thiếu.
    #[tokio::test]
    async fn ui_responses_have_security_headers() {
        let dist = crate::api::web::tests::FakeDist::new();
        for path in [
            "/",
            "/w/abc/datasets",
            "/assets/app.js",
            "/assets/missing.js",
        ] {
            let headers = headers_of(app(Some(dist.0.clone())), path).await;
            assert_security_headers(&headers, APP_CSP, path);
        }
    }
}
