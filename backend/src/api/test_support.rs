// SPDX-License-Identifier: Apache-2.0
/* api/test_support.rs - phần dùng chung cho test gọi API qua router thật: dựng app, gửi request, đọc cookie phiên. */

use std::sync::Arc;

use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::auth::SESSION_COOKIE;
use crate::config::Config;
use crate::engine::llm::LlmProvider;
use crate::engine::store::Store;
use crate::state::AppState;

pub const CODE: &str = "ABCD-EFGH-JKMN";

pub fn app(store: Store, setup_code: Option<&str>) -> axum::Router {
    app_with(store, setup_code, |_| {})
}

/// Như app, cho test đổi cấu hình trước (ví dụ thư mục dữ liệu, giới hạn dung lượng nạp).
pub fn app_with(
    store: Store,
    setup_code: Option<&str>,
    configure: impl FnOnce(&mut Config),
) -> axum::Router {
    crate::api::router(build_state(store, setup_code, configure))
}

/// Như app_with, thay mô hình ngôn ngữ của cấu hình bằng llm: mô hình giả trả câu viết sẵn mà mock không có.
pub fn app_with_llm(
    store: Store,
    setup_code: Option<&str>,
    configure: impl FnOnce(&mut Config),
    llm: Arc<dyn LlmProvider>,
) -> axum::Router {
    let mut state = build_state(store, setup_code, configure);
    state.llm = llm;
    crate::api::router(state)
}

fn build_state(
    store: Store,
    setup_code: Option<&str>,
    configure: impl FnOnce(&mut Config),
) -> AppState {
    let mut config = Config::from_env();
    config.static_dir = None;
    // Binary của test không phải xdi-backend, nên không chạy được tiến trình con excel-worker.
    config.excel_worker = crate::engine::excel_worker::ExcelWorker::InProcess;
    configure(&mut config);
    let state = AppState::new(config, store);
    *state.setup_code.lock().unwrap() = setup_code.map(str::to_string);
    state
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

/// Body multipart/form-data: các trường chữ và các file (tên file, nội dung). Trả (content-type, body).
pub fn multipart(fields: &[(&str, &str)], files: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "xdi-test-boundary-7d1f";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    for (file_name, bytes) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// Gửi body thô (multipart) kèm cookie phiên. Trả mã và body JSON.
pub async fn send_raw(
    app: &axum::Router,
    path: &str,
    cookie: &str,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("cookie", format!("{SESSION_COOKIE}={cookie}"))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
