// SPDX-License-Identifier: Apache-2.0
/* api/mod.rs - tầng HTTP: gom route của từng nhóm thành một router.
Handler chỉ đọc request, gọi service, trả response; không chứa logic nghiệp vụ.

Mọi route API nằm dưới /api, để cùng một server phục vụ được cả giao diện (các đường dẫn còn lại) mà không trùng
nhau. Đường dẫn dưới /api không tồn tại trả 404 JSON; nếu rơi xuống phần phục vụ giao diện, frontend sẽ nhận một
trang HTML thay vì lỗi và khó biết vì sao. Giao diện chỉ được phục vụ khi có STATIC_DIR (xem web.rs). */

mod auth;
mod datasets;
mod query;
mod system;
mod web;

use axum::{routing::any, Router};

use crate::{error::AppError, state::AppState};

pub fn router(state: AppState) -> Router {
    // with_state ở cuối nhận luôn state (move), nên lấy static_dir ra trước.
    let static_dir = state.config.static_dir.clone();
    let api = Router::new()
        .merge(system::routes())
        .merge(auth::routes())
        .merge(datasets::routes())
        .merge(query::routes())
        .fallback(api_not_found);
    let app = Router::new()
        .nest("/api", api)
        // nest bắt "/api" và "/api/<gì đó>", nhưng không bắt "/api/" (đuôi phải có ít nhất một ký tự).
        // Thiếu dòng này, "/api/" sẽ rơi xuống phần phục vụ giao diện và trả trang HTML.
        .route("/api/", any(api_not_found));
    // Không có STATIC_DIR (chạy dev với Vite) thì chỉ phục vụ API.
    let app = match static_dir {
        Some(dir) => app.merge(web::routes(&dir)),
        None => app,
    };
    app.with_state(state)
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

    #[tokio::test]
    async fn unfinished_endpoint_returns_501() {
        let res = app()
            .oneshot(Request::get("/api/datasets").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
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
