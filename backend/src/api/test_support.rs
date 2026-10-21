// SPDX-License-Identifier: Apache-2.0
/* api/test_support.rs - phần dùng chung cho test gọi API qua router thật: dựng app, gửi request, đọc cookie phiên. */

use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::auth::SESSION_COOKIE;
use crate::config::Config;
use crate::engine::store::Store;
use crate::state::AppState;

pub const CODE: &str = "ABCD-EFGH-JKMN";

pub fn app(store: Store, setup_code: Option<&str>) -> axum::Router {
    let mut config = Config::from_env();
    config.static_dir = None;
    let state = AppState::new(config, store);
    *state.setup_code.lock().unwrap() = setup_code.map(str::to_string);
    crate::api::router(state)
}

/// Gửi một request, trả mã, cookie phiên mới (nếu có) và body JSON (nếu có).
pub async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Option<String>, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(c) = cookie {
        req = req.header("cookie", format!("{SESSION_COOKIE}={c}"));
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let set_cookie = res
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().to_string());
    if let Some(sc) = &set_cookie {
        if !sc.contains("Max-Age=0") {
            assert!(sc.contains("HttpOnly"), "cookie must be httpOnly: {sc}");
            assert!(sc.contains("SameSite=Lax"), "got {sc}");
        }
    }
    let session = set_cookie
        .and_then(|sc| sc.split(';').next().map(str::to_string))
        .and_then(|kv| kv.split_once('=').map(|(_, v)| v.to_string()))
        .filter(|v| !v.is_empty());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, session, json)
}
