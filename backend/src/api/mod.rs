// SPDX-License-Identifier: Apache-2.0
/* api/mod.rs - tầng HTTP: gom route của từng nhóm thành một router.
Handler chỉ đọc request, gọi service, trả response; không chứa logic nghiệp vụ. */

mod auth;
mod datasets;
mod query;
mod system;

use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(system::routes())
        .merge(auth::routes())
        .merge(datasets::routes())
        .merge(query::routes())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{body::Body, http::Request, http::StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app() -> Router {
        let config = Config::from_env();
        let store = crate::engine::store::Store::lazy(&config.database_url);
        router(AppState::new(config, store))
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let res = app()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], br#"{"status":"ok"}"#);
    }

    #[tokio::test]
    async fn unfinished_endpoint_returns_501() {
        let res = app()
            .oneshot(Request::get("/datasets").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
