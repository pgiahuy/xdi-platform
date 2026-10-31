// SPDX-License-Identifier: Apache-2.0
/* api/source_snapshot_tests.rs - hai cách chép bảng khi chụp (SOURCE_SNAPSHOT_MODE, #246) qua route thật. Cả copy lẫn
direct: cùng số dòng, kiểu cột, comment của bảng thành mô tả (#245), chụp lại bảng không đổi không tạo phiên bản, chỉ
sửa comment thì có phiên bản mới (comment nằm trong hash). direct mà extension postgres của DuckDB thiếu hay hỏng thì
lần chụp vẫn xong bằng COPY. So từng dòng của hai cách ở service/sources/direct_tests.rs. Không có DATABASE_URL thì bỏ
qua; test direct với extension thật bỏ qua khi chưa chạy make duckdb-extension. */

use std::path::PathBuf;

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::scripted;
use super::sources_tests::{start_with_snapshot_mode, KEY, SHOP};
use crate::api::test_support::call;
use crate::config::SnapshotMode;
use crate::engine::store::test_support::exec;

/// Chụp public.orders (bảng của sources_tests::start) ba lần: lần đầu, lần chụp lại không đổi gì, và lần chụp lại
/// sau khi sửa comment của bảng. Trả response của ba lần.
async fn snapshot_orders(mode: SnapshotMode, extension: PathBuf) -> Option<Vec<Value>> {
    let s =
        start_with_snapshot_mode(Some(KEY), scripted(&[]), None, Some((mode, extension))).await?;
    exec(&s.db.store, "COMMENT ON TABLE orders IS 'Orders by region'").await;
    let url = format!("{SHOP}/sources");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let snapshot_url = format!("{url}/{}/snapshot", source["id"].as_str().unwrap());
    let mut results = Vec::new();
    for step in 0..3 {
        if step == 2 {
            exec(
                &s.db.store,
                "COMMENT ON TABLE orders IS 'Orders, one row each'",
            )
            .await;
        }
        let body = json!({"table": "public.orders"});
        let (status, _, result) =
            call(&s.app, "POST", &snapshot_url, Some(&s.lan), Some(body)).await;
        // 201 khi có phiên bản mới, 200 khi bản chụp trùng phiên bản đã có.
        assert!(
            status == StatusCode::CREATED || status == StatusCode::OK,
            "{status} {result}"
        );
        results.push(result);
    }
    s.finish().await;
    Some(results)
}

fn assert_orders_copied(results: &[Value]) {
    let dataset = &results[0]["datasets"][0];
    assert_eq!(dataset["version"]["row_count"], 3, "{dataset}");
    assert_eq!(dataset["version"]["source_kind"], "postgres", "{dataset}");
    assert_eq!(
        dataset["version"]["description"], "Orders by region",
        "{dataset}"
    );
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
    // Bảng và comment không đổi: cùng hash, không có phiên bản mới.
    assert_eq!(
        results[1]["datasets"][0]["created"], false,
        "{}",
        results[1]
    );
    // Chỉ sửa comment: hash có comment nên ra phiên bản mới mang comment mới.
    let changed = &results[2]["datasets"][0];
    assert_eq!(changed["created"], true, "{changed}");
    assert_eq!(
        changed["version"]["description"], "Orders, one row each",
        "{changed}"
    );
}

#[tokio::test]
async fn copy_mode_keeps_types_comments_and_hash() {
    let Some(results) = snapshot_orders(SnapshotMode::Copy, PathBuf::new()).await else {
        return;
    };
    assert_orders_copied(&results);
}

/// Có file extension thật (make duckdb-extension): chụp bằng DuckDB, comment vẫn đọc trong giao dịch chụp và vào hash.
#[tokio::test]
async fn direct_mode_keeps_types_comments_and_hash() {
    let extension = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../duckdb-extensions/postgres_scanner.duckdb_extension");
    if !extension.is_file() {
        eprintln!("skipping: no DuckDB postgres extension (make duckdb-extension)");
        return;
    }
    let Some(results) = snapshot_orders(SnapshotMode::Direct, extension).await else {
        return;
    };
    assert_orders_copied(&results);
}

#[tokio::test]
async fn missing_extension_falls_back_to_copy() {
    let missing = std::env::temp_dir().join(format!("xdi-no-extension-{}", uuid::Uuid::new_v4()));
    let Some(results) = snapshot_orders(SnapshotMode::Direct, missing).await else {
        return;
    };
    assert_orders_copied(&results);
}

/// File không phải extension DuckDB (ví dụ tải dở, hay extension của bản DuckDB khác): LOAD lỗi, vẫn chụp bằng COPY.
#[tokio::test]
async fn broken_extension_falls_back_to_copy() {
    let broken =
        std::env::temp_dir().join(format!("xdi-broken-extension-{}", uuid::Uuid::new_v4()));
    std::fs::write(&broken, b"not a duckdb extension").unwrap();
    let results = snapshot_orders(SnapshotMode::Direct, broken.clone()).await;
    let _ = std::fs::remove_file(&broken);
    let Some(results) = results else {
        return;
    };
    assert_orders_copied(&results);
}
