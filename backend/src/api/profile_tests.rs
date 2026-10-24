// SPDX-License-Identifier: Apache-2.0
/* api/profile_tests.rs - test hồ sơ cột qua route nạp thật (issue #22): ghi hồ sơ lỗi không để lại phiên bản hay
dataset, dữ liệu Postgres không lưu được (ký tự NUL, tên cột quá dài), cột ngày đọc được hai cách giữ cách của phiên
bản trước. Mỗi test dùng database và thư mục dữ liệu riêng. */

use std::path::PathBuf;

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::{exec, fetch_count, fresh_database};
use crate::engine::store::Store;

const URL: &str = "/api/workspaces/cua-hang/datasets";

struct DataDir(PathBuf);

impl DataDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-profile-api-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        DataDir(dir)
    }

    /// Còn file nào dưới thư mục dữ liệu không, kể cả thư mục tạm.
    fn has_files(&self) -> bool {
        let mut stack = vec![self.0.clone()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap().flatten() {
                if entry.path().is_dir() {
                    stack.push(entry.path());
                } else {
                    return true;
                }
            }
        }
        false
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn setup(app: &axum::Router) -> String {
    let body = json!({"setup_code": CODE, "email": "lan@example.com", "password": "a long password",
                      "workspace_name": "Cửa hàng"});
    let (status, session, _) = call(app, "POST", "/api/auth/setup", None, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    session.unwrap()
}

async fn upload(
    app: &axum::Router,
    session: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &[u8])],
) -> (StatusCode, Value) {
    let (content_type, body) = multipart(fields, files);
    send_raw(app, URL, session, &content_type, body).await
}

async fn count(store: &Store, table: &str) -> i64 {
    fetch_count(store, &format!("SELECT count(*) FROM {table}")).await
}

/// Ghi hồ sơ lỗi (giả lập bằng trigger từ chối cột tên boom): dataset mới bị xóa, dataset có sẵn giữ phiên bản cũ, không
/// còn file nào; bỏ trigger rồi nạp lại đúng file đó thì thành công, không vấp phiên bản trùng hash.
#[tokio::test]
async fn failed_profile_write_leaves_nothing_and_retry_works() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;
    exec(
        &db.store,
        "CREATE FUNCTION reject_boom() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN IF NEW.name = 'boom' THEN RAISE EXCEPTION 'boom'; END IF; RETURN NEW; END $$",
    )
    .await;
    exec(
        &db.store,
        "CREATE TRIGGER reject_boom BEFORE INSERT ON columns FOR EACH ROW EXECUTE FUNCTION reject_boom()",
    )
    .await;
    let bad: &[u8] = b"region,boom\nSouth,1\nNorth,2\n";

    // Dataset mới: không còn dataset, nguồn, phiên bản hay file nào.
    let (status, body) = upload(&app, &lan, &[], &[("bad.csv", bad)]).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    for table in ["datasets", "sources", "dataset_versions", "columns"] {
        assert_eq!(count(&db.store, table).await, 0, "{table}");
    }
    assert!(!data.has_files());

    // Dataset có sẵn: giữ nguyên phiên bản hiện hành, không thêm phiên bản.
    let (status, v1) = upload(&app, &lan, &[], &[("ok.csv", b"region,amount\nSouth,1\n")]).await;
    assert_eq!(status, StatusCode::CREATED, "{v1}");
    let dataset_id = v1["dataset"]["id"].as_str().unwrap().to_string();
    let (status, _) = upload(
        &app,
        &lan,
        &[("dataset_id", &dataset_id)],
        &[("bad.csv", bad)],
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(count(&db.store, "dataset_versions").await, 1);
    let (_, _, detail) = call(
        &app,
        "GET",
        &format!("{URL}/{dataset_id}"),
        Some(&lan),
        None,
    )
    .await;
    assert_eq!(detail["current_version_id"], v1["version"]["id"]);

    exec(&db.store, "DROP TRIGGER reject_boom ON columns").await;
    let (status, body) = upload(
        &app,
        &lan,
        &[("dataset_id", &dataset_id)],
        &[("bad.csv", bad)],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["version"], 2);
    assert_eq!(body["version"]["profile_status"], "structure");
    let (status, body) = upload(&app, &lan, &[], &[("bad.csv", bad)]).await;
    assert_eq!(
        (status, body["created"].clone()),
        (StatusCode::OK, json!(false))
    );

    db.drop().await;
}

/// Ô có ký tự NUL: nạp được; cột đó không có danh sách giá trị (Postgres không lưu NUL) nên là text, min bỏ NUL.
/// Tên cột dài quá 1000 byte: 400 kèm lời rõ, không ghi gì.
#[tokio::test]
async fn values_and_names_postgres_cannot_store() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    let (status, body) = upload(
        &app,
        &lan,
        &[],
        &[("nul.csv", b"code,amount\na\x00b,1\nc,2\nc,3\n")],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let code = "FROM columns WHERE name = 'code'";
    assert_eq!(
        fetch_count(
            &db.store,
            &format!("SELECT count(*) {code} AND role = 'text'")
        )
        .await,
        1
    );
    assert_eq!(
        fetch_count(
            &db.store,
            &format!("SELECT count(*) {code} AND min_value = 'ab'")
        )
        .await,
        1
    );
    let values =
        format!("SELECT count(*) FROM column_values WHERE column_id IN (SELECT id {code})");
    assert_eq!(fetch_count(&db.store, &values).await, 0);

    // Tiêu đề dài như câu hỏi Google Forms (300 byte) vẫn nạp được; quá 1000 byte thì không.
    let forms = format!("{},amount\n1,2\n", "x".repeat(300));
    let (status, body) = upload(&app, &lan, &[], &[("forms.csv", forms.as_bytes())]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let header = format!("{},amount\n1,2\n", "x".repeat(1001));
    let (status, body) = upload(&app, &lan, &[], &[("long.csv", header.as_bytes())]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("1000 byte"),
        "{body}"
    );
    assert_eq!(count(&db.store, "datasets").await, 2);

    db.drop().await;
}

/// Phiên bản 1 chỉ đọc được theo tháng/ngày; phiên bản 2 đọc được cả hai cách thì giữ tháng/ngày. Cùng dữ liệu đó nạp
/// thành dataset mới (không có phiên bản trước) thì theo ngày/tháng.
#[tokio::test]
async fn ambiguous_dates_keep_the_previous_version_format() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    let (status, v1) = upload(
        &app,
        &lan,
        &[],
        &[(
            "t1.csv",
            b"day,amount\n1/31/2018 22:56,1\n2/3/2018 1:00,2\n",
        )],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v1}");
    let dataset_id = v1["dataset"]["id"].as_str().unwrap().to_string();
    let ambiguous: &[u8] = b"day,amount\n3/4/2018 7:05,1\n5/6/2018 8:00,2\n";
    let (status, v2) = upload(
        &app,
        &lan,
        &[("dataset_id", &dataset_id)],
        &[("t2.csv", ambiguous)],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v2}");
    let (status, other) = upload(
        &app,
        &lan,
        &[("name", "Khác")],
        &[("t3.csv", b"day,amount\n3/4/2018 7:05,9\n5/6/2018 8:00,9\n")],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{other}");

    let format_of = |version: &Value| {
        let sql = format!(
            "SELECT count(*) FROM columns WHERE version_id = '{}' AND name = 'day' AND time_format = '%m/%d/%Y %H:%M'",
            version["version"]["id"].as_str().unwrap()
        );
        let store = db.store.clone();
        async move { fetch_count(&store, &sql).await }
    };
    assert_eq!(format_of(&v1).await, 1);
    assert_eq!(format_of(&v2).await, 1);
    assert_eq!(format_of(&other).await, 0);

    db.drop().await;
}
