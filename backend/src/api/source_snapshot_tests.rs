// SPDX-License-Identifier: Apache-2.0
/* api/source_snapshot_tests.rs - SOURCE_SNAPSHOT_MODE=direct qua route thật (#246): extension postgres của DuckDB
thiếu hay hỏng thì lần chụp vẫn xong bằng COPY, cùng số dòng và kiểu cột. So dữ liệu hai cách chép trên cùng một bảng
ở service/sources/direct_tests.rs. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::scripted;
use super::sources_tests::{start_with_snapshot_mode, KEY, SHOP};
use crate::api::test_support::call;
use crate::config::SnapshotMode;

/// Chụp public.orders (bảng của sources_tests::start) với SOURCE_SNAPSHOT_MODE=direct và extension ở đường dẫn cho sẵn.
async fn snapshot_orders(extension: std::path::PathBuf) -> Option<Value> {
    let mode = Some((SnapshotMode::Direct, extension));
    let s = start_with_snapshot_mode(Some(KEY), scripted(&[]), None, mode).await?;
    let url = format!("{SHOP}/sources");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let id = source["id"].as_str().unwrap();
    let (status, _, result) = call(
        &s.app,
        "POST",
        &format!("{url}/{id}/snapshot"),
        Some(&s.lan),
        Some(json!({"table": "public.orders"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    s.finish().await;
    Some(result)
}

fn assert_orders_copied(result: &Value) {
    let dataset = &result["datasets"][0];
    assert_eq!(dataset["version"]["row_count"], 3, "{result}");
    assert_eq!(dataset["version"]["source_kind"], "postgres", "{result}");
    let types: Vec<String> = dataset["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| format!("{} {}", c["name"], c["data_type"]))
        .collect();
    assert!(
        types.contains(&"\"amount\" \"DECIMAL(10,2)\"".to_string()),
        "{types:?}"
    );
    assert!(
        types.contains(&"\"ordered_at\" \"DATE\"".to_string()),
        "{types:?}"
    );
}

#[tokio::test]
async fn missing_extension_falls_back_to_copy() {
    let missing = std::env::temp_dir().join(format!("xdi-no-extension-{}", uuid::Uuid::new_v4()));
    let Some(result) = snapshot_orders(missing).await else {
        return;
    };
    assert_orders_copied(&result);
}

/// File không phải extension DuckDB (ví dụ tải dở, hay extension của bản DuckDB khác): LOAD lỗi, vẫn chụp bằng COPY.
#[tokio::test]
async fn broken_extension_falls_back_to_copy() {
    let broken =
        std::env::temp_dir().join(format!("xdi-broken-extension-{}", uuid::Uuid::new_v4()));
    std::fs::write(&broken, b"not a duckdb extension").unwrap();
    let result = snapshot_orders(broken.clone()).await;
    let _ = std::fs::remove_file(&broken);
    let Some(result) = result else {
        return;
    };
    assert_orders_copied(&result);
}
