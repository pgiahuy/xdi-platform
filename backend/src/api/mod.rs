// SPDX-License-Identifier: Apache-2.0
/* api/mod.rs - tầng HTTP: gom route của từng nhóm thành một router.
Handler chỉ đọc request, gọi service, trả response; không chứa logic nghiệp vụ.

Mọi route API nằm dưới /api, để cùng một server phục vụ được cả giao diện (các đường dẫn còn lại) mà không trùng
nhau. Đường dẫn dưới /api không tồn tại trả 404 JSON; nếu rơi xuống phần phục vụ giao diện, frontend sẽ nhận một
trang HTML thay vì lỗi và khó biết vì sao. Giao diện chỉ được phục vụ khi có STATIC_DIR (xem web.rs).

Route API đăng ký qua OpenApiRouter (utoipa_axum): cùng một chỗ dựng cả router lẫn đặc tả OpenAPI (openapi.rs), phục vụ
ở /api/openapi.json, Swagger UI ở /api/docs, thông báo giấy phép của Swagger UI ở /api/docs/licenses.txt.
Ngoại lệ của quy tắc 404 JSON: đường dẫn không có dưới /api/docs/ (ví dụ /api/docs/nope) trả 404 body rỗng, vì route
/api/docs/{*rest} của Swagger UI bắt trước fallback của nest. Frontend không gọi các đường này; bọc lại service của
Swagger UI chỉ để đổi body 404 thì không đáng.

Mọi response, cả giao diện và trang 404, đi qua security_headers.rs để nhận header bảo mật. */

mod admin;
#[cfg(test)]
mod ask_database_tests;
#[cfg(test)]
mod ask_fanout_tests;
#[cfg(test)]
mod ask_privacy_tests;
#[cfg(test)]
mod ask_related_tests;
#[cfg(test)]
mod ask_row_filter_error_tests;
#[cfg(test)]
mod ask_tests;
#[cfg(test)]
mod ask_why_database_tests;
#[cfg(test)]
mod ask_why_frames_tests;
#[cfg(test)]
mod ask_why_tests;
mod auth;
mod catalog;
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod conversation_tests;
mod conversations;
mod data_roles;
#[cfg(test)]
mod data_roles_tests;
mod datasets;
mod domains;
#[cfg(test)]
mod domains_tests;
#[cfg(test)]
mod drill_tests;
#[cfg(test)]
mod excel_tests;
mod extract;
mod feedback;
#[cfg(test)]
mod feedback_tests;
mod lessons;
#[cfg(test)]
mod lessons_tests;
mod metrics;
#[cfg(test)]
mod metrics_tests;
mod openapi;
#[cfg(test)]
mod overview_tests;
#[cfg(test)]
mod profile_tests;
mod query;
#[cfg(test)]
mod report_tests;
mod reports;
#[cfg(test)]
mod roles_tests;
#[cfg(test)]
mod row_filter_fk_tests;
mod security_headers;
#[cfg(test)]
mod source_comments_tests;
#[cfg(test)]
mod source_limits_tests;
#[cfg(test)]
mod source_relations_tests;
#[cfg(test)]
mod source_snapshot_all_tests;
#[cfg(test)]
mod source_snapshot_tests;
mod sources;
#[cfg(test)]
mod sources_tests;
mod system;
#[cfg(test)]
mod test_support;
mod web;
mod workspaces;

use axum::{
    http::header,
    middleware,
    routing::{any, get},
    Router,
};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

use crate::{error::AppError, state::AppState};

/// Mọi route API, đường dẫn chưa có tiền tố /api, kèm đặc tả của chúng. max_upload_bytes chỉ đổi giới hạn body của
/// route nạp file, không đổi đặc tả.
fn api_routes(max_upload_bytes: u64) -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(openapi::ApiDoc::openapi())
        .merge(system::routes())
        .merge(auth::routes())
        .merge(admin::routes())
        .merge(workspaces::routes())
        .merge(data_roles::routes())
        .merge(datasets::routes(max_upload_bytes))
        .merge(catalog::routes())
        .merge(query::routes())
        .merge(conversations::routes())
        .merge(feedback::routes())
        .merge(lessons::routes())
        .merge(metrics::routes())
        .merge(domains::routes())
        .merge(sources::routes())
        .merge(reports::routes())
}

/// Tách router API và đặc tả hoàn chỉnh (đã thêm phần chung ở openapi.rs). Server và lệnh in đặc tả cùng đi qua đây,
/// nên đặc tả phục vụ ở /api/openapi.json giống hệt openapi.json.
fn build_api(max_upload_bytes: u64) -> (Router<AppState>, utoipa::openapi::OpenApi) {
    let (api, mut spec) = api_routes(max_upload_bytes).split_for_parts();
    openapi::apply_common(&mut spec);
    (api, spec)
}

/// Đặc tả OpenAPI sinh từ router. Không cần AppState hay database, nên lệnh xdi-backend openapi chạy được không cần
/// Postgres.
pub fn build_openapi() -> utoipa::openapi::OpenApi {
    build_api(0).1
}

/// Đặc tả dạng JSON, đúng nội dung openapi.json ở gốc repo.
pub fn build_openapi_json() -> String {
    openapi::build_spec_json(&build_openapi())
}

// Thông báo giấy phép của Swagger UI và các thư viện gói trong nó, nhúng vào binary cùng Swagger UI (docs/LICENSES.md).
const SWAGGER_UI_LICENSES: &str = include_str!("../../third-party/swagger-ui-5.32.6-licenses.txt");

pub fn router(state: AppState) -> Router {
    // with_state ở cuối nhận luôn state (move), nên lấy static_dir ra trước.
    let static_dir = state.config.static_dir.clone();
    let (api, spec) = build_api(state.config.max_upload_bytes);
    let api = api.fallback(api_not_found);
    let app = Router::new()
        .nest("/api", api)
        // nest bắt "/api" và "/api/<gì đó>", nhưng không bắt "/api/" (đuôi phải có ít nhất một ký tự).
        // Thiếu dòng này, "/api/" sẽ rơi xuống phần phục vụ giao diện và trả trang HTML.
        .route("/api/", any(api_not_found))
        // Swagger UI gắn ở router ngoài với đường dẫn đầy đủ, không gắn trong nest("/api"): nó tự chuyển /docs sang
        // /docs/ bằng đường dẫn tuyệt đối, nằm trong nest thì trình duyệt bị đưa ra /docs/ ngoài /api. Route tĩnh
        // /api/docs và /api/openapi.json được chọn trước route bắt-tất-cả của nest. Gói Swagger UI kèm sẵn trong
        // crate (feature vendored), build không tải gì từ Internet.
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", spec))
        // Swagger UI nhúng trong binary mà không kèm file giấy phép của nó: phục vụ ngay cạnh, để image Docker hay
        // binary chép đi đâu cũng mang theo thông báo bản quyền. Route tĩnh này được chọn trước /api/docs/{*rest}.
        .route(
            "/api/docs/licenses.txt",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    SWAGGER_UI_LICENSES,
                )
            }),
        );
    // Không có STATIC_DIR (chạy dev với Vite) thì chỉ phục vụ API.
    let app = match static_dir {
        Some(dir) => app.merge(web::routes(&dir)),
        None => app,
    };
    // Gắn sau cùng để bọc mọi route đã có, kể cả fallback trả giao diện và 404: layer chỉ bọc các route thêm trước nó.
    app.layer(middleware::from_fn(
        security_headers::apply_security_headers,
    ))
    .with_state(state)
}

async fn api_not_found() -> AppError {
    AppError::NotFound("Không có đường dẫn API này".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{body::Body, http::Request, http::StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app() -> Router {
        let mut config = Config::from_env();
        config.static_dir = None;
        let store = crate::engine::store::Store::lazy(&config.database_url);
        router(AppState::new(config, store))
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let res = app()
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], br#"{"status":"ok"}"#);
    }

    /// /api/openapi.json phục vụ đúng đặc tả sinh từ router, không cần đăng nhập.
    #[tokio::test]
    async fn openapi_json_is_served() {
        let res = app()
            .oneshot(
                Request::get("/api/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let served: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&build_openapi_json()).unwrap();
        assert_eq!(served, expected);
    }

    /// Swagger UI: /api/docs chuyển sang /api/docs/ (vẫn trong /api), trang đó trả HTML.
    #[tokio::test]
    async fn swagger_ui_is_served_under_api() {
        let res = app()
            .oneshot(Request::get("/api/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(res.status().is_redirection(), "status {}", res.status());
        assert_eq!(res.headers()["location"], "/api/docs/");

        let res = app()
            .oneshot(Request::get("/api/docs/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let content_type = res.headers()["content-type"].to_str().unwrap().to_string();
        assert!(content_type.starts_with("text/html"), "got {content_type}");

        // Thông báo giấy phép của Swagger UI đi cùng binary.
        let res = app()
            .oneshot(
                Request::get("/api/docs/licenses.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("SmartBear Software Inc.") && text.contains("DOMPurify"));
    }

    /// Mọi thao tác đặc tả ghi là cần cookie phiên (không khai security(()) trên handler) phải trả 401 khi gọi không
    /// cookie, trước khi chạm tới database. Duyệt chính đặc tả, nên route mới tự được kiểm: quên extractor CurrentUser
    /// hay Member thì test đỏ, thay vì đặc tả ghi "cần đăng nhập" còn server thì mở.
    #[tokio::test]
    async fn every_session_operation_requires_login() {
        let spec: serde_json::Value = serde_json::from_str(&build_openapi_json()).unwrap();
        let mut checked = 0;
        for (path, item) in spec["paths"].as_object().unwrap() {
            for (method, operation) in item.as_object().unwrap() {
                // security: [{}] là thao tác công khai; không có trường security thì theo mặc định là cookie phiên.
                let public = operation["security"] == serde_json::json!([{}]);
                if public {
                    continue;
                }
                // Tham số đường dẫn nào cũng được: chưa đăng nhập thì bị chặn trước khi đọc chúng.
                let uri = format!(
                    "/api{}",
                    path.replace("{ws}", "any")
                        .replace("{id}", "x")
                        .replace("{user_id}", "x")
                );
                let res = app()
                    .oneshot(
                        Request::builder()
                            .method(method.to_uppercase().as_str())
                            .uri(&uri)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{method} {uri}");
                checked += 1;
            }
        }
        // Chặn trường hợp test không duyệt được gì (ví dụ đặc tả đổi dạng) mà vẫn qua.
        assert!(checked >= 20, "only {checked} operations checked");
    }

    #[tokio::test]
    async fn unknown_api_paths_return_json_404() {
        for path in ["/api/does-not-exist", "/api", "/api/", "/api/health/"] {
            let res = app()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "path {path}");
            let content_type = res.headers()["content-type"].to_str().unwrap().to_string();
            assert!(
                content_type.starts_with("application/json"),
                "path {path}: got {content_type}"
            );
            let body = res.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["code"], "not_found", "path {path}");
        }
    }

    #[tokio::test]
    async fn old_paths_without_prefix_are_gone() {
        let res = app()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
