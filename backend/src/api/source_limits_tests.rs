// SPDX-License-Identifier: Apache-2.0
/* api/source_limits_tests.rs - trần của một lần chụp bảng (service/sources/plan.rs) qua route thật, trên schema shop
của api/source_relations_tests.rs: không đặt SOURCE_MAX_ROWS thì không có trần dòng; bảng quá SOURCE_TABLE_MAX_MB chỉ
lưu metadata mà không ghi file nào; quá SOURCE_TIMEOUT_SECS thì bỏ cả lần chụp, câu đang đọc trên nguồn bị dừng và
không còn file tạm. Trần dòng khi đặt ở source_relations_tests.rs (snapshot_limits_and_roles), trần tổng ở
source_snapshot_all_tests.rs. Không có DATABASE_URL thì bỏ qua. */

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::scripted;
use super::source_relations_tests::{add_source, snapshot};
use super::sources_tests::{start_configured, Setup, KEY, SHOP};
use crate::api::test_support::call;
use crate::config::SnapshotMode;
use crate::engine::store::test_support::{exec, fetch_count};

async fn listed(s: &Setup, id: &str, table: &str) -> Value {
    let url = format!("{SHOP}/sources/{id}/tables");
    let (status, _, body) = call(&s.app, "GET", &url, Some(&s.lan), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array()
        .unwrap()
        .iter()
        .find(|t| t["table"] == table)
        .unwrap_or_else(|| panic!("no table {table}"))
        .clone()
}

/// Số file trong DATA_DIR/staging (file tạm của các lần chụp chưa nạp xong).
fn staged_files(data: &Path) -> usize {
    let Ok(dirs) = std::fs::read_dir(data.join("staging")) else {
        return 0;
    };
    dirs.flatten()
        .filter_map(|d| std::fs::read_dir(d.path()).ok())
        .map(|files| files.count())
        .sum()
}

/// Không có trần dòng: order_items (5 dòng) chụp đủ dòng, không bảng nào bị giữ lại, danh sách bảng không gắn nhãn.
#[tokio::test]
async fn no_row_limit_copies_every_row() {
    let Some(s) = start_configured(Some(KEY), scripted(&[]), |c| c.source_max_rows = None).await
    else {
        return;
    };
    let id = add_source(&s).await;
    assert_eq!(
        listed(&s, &id, "shop.order_items").await["over_limit"],
        Value::Null
    );
    let (status, body) = snapshot(&s, &id, json!({"tables": ["shop.order_items"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["datasets"][0]["version"]["row_count"], 5, "{body}");
    assert_eq!(body["metadata_only"], json!([]));
    s.finish().await;
}

/// Trần dòng khi đặt: nhãn ở danh sách bảng theo ước lượng của Postgres (sau ANALYZE).
#[tokio::test]
async fn row_limit_when_set_labels_the_table() {
    let Some(s) = start_configured(Some(KEY), scripted(&[]), |c| c.source_max_rows = Some(4)).await
    else {
        return;
    };
    let id = add_source(&s).await;
    exec(&s.db.store, "ANALYZE shop.order_items").await;
    assert_eq!(
        listed(&s, &id, "shop.order_items").await["over_limit"],
        "row_limit"
    );
    let (status, body) = snapshot(&s, &id, json!({"tables": ["shop.order_items"]})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["metadata_only"][0]["cause"], "row_limit", "{body}");
    s.finish().await;
}

/// Trần một bảng 1 byte: mọi bảng có dung lượng bị giữ lại trước khi chép (không có file tạm nào), lời báo nêu
/// SOURCE_TABLE_MAX_MB; danh sách bảng gắn nhãn size_limit.
#[tokio::test]
async fn table_size_is_checked_before_copying() {
    let Some(s) =
        start_configured(Some(KEY), scripted(&[]), |c| c.source_table_max_bytes = 1).await
    else {
        return;
    };
    let id = add_source(&s).await;
    assert_eq!(
        listed(&s, &id, "shop.order_items").await["over_limit"],
        "size_limit"
    );
    let body = json!({"tables": ["shop.order_items", "shop.customers"]});
    let (status, result) = snapshot(&s, &id, body).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["datasets"], json!([]), "{result}");
    let skipped = result["metadata_only"].as_array().unwrap();
    assert_eq!(skipped.len(), 2, "{result}");
    for t in skipped {
        assert_eq!(t["cause"], "size_limit", "{t}");
        let reason = t["reason"].as_str().unwrap();
        assert!(reason.contains("SOURCE_TABLE_MAX_MB"), "{reason}");
    }
    assert_eq!(staged_files(&s.data), 0);
    s.finish().await;
}

/// Quá SOURCE_TIMEOUT_SECS khi DuckDB đang đọc (view chờ 30 giây): lần chụp báo lỗi sau khoảng 1 giây, không tạo dataset,
/// không còn file tạm, và câu đọc trên nguồn đã dừng chứ không chạy tiếp tới hết. Cần extension thật (make
/// duckdb-extension) để đi đúng đường direct.rs; không có thì bỏ qua.
#[tokio::test]
async fn timeout_cancels_the_copy_and_leaves_nothing() {
    let extension = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../duckdb-extensions/postgres_scanner.duckdb_extension");
    if !extension.is_file() {
        eprintln!("skipping: no DuckDB postgres extension (make duckdb-extension)");
        return;
    }
    let Some(s) = start_configured(Some(KEY), scripted(&[]), move |c| {
        c.source_timeout = Duration::from_secs(1);
        c.source_snapshot_mode = SnapshotMode::Direct;
        c.source_postgres_extension = extension;
    })
    .await
    else {
        return;
    };
    let id = add_source(&s).await;
    exec(
        &s.db.store,
        "CREATE VIEW shop.slow AS SELECT 1 AS n FROM pg_sleep(30)",
    )
    .await;
    exec(
        &s.db.store,
        &format!("GRANT SELECT ON shop.slow TO {}", s.user),
    )
    .await;

    let started = Instant::now();
    let (status, body) = snapshot(&s, &id, json!({"tables": ["shop.slow"]})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let message = body["message"].as_str().unwrap();
    assert!(message.contains("SOURCE_TIMEOUT_SECS"), "{message}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );

    // Câu của tài khoản nguồn trên Postgres dừng ngay sau đó (ngắt từ DuckDB hay statement_timeout), không chờ 30 giây.
    let query = format!(
        "SELECT count(*) FROM pg_stat_activity WHERE usename = '{}' AND state = 'active'",
        s.user
    );
    let mut running: i64 = 1;
    for _ in 0..50 {
        running = fetch_count(&s.db.store, &query).await;
        if running == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(running, 0, "the source query is still running");
    assert_eq!(staged_files(&s.data), 0);
    let (status, _, list) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/catalog"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(!list.to_string().contains("slow"), "{list}");
    s.finish().await;
}
