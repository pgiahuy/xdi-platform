// SPDX-License-Identifier: Apache-2.0
/* api/datasets/tests.rs - test endpoint nạp, xem và xóa dataset qua router thật, cần Postgres (bỏ qua khi không có
DATABASE_URL). */

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::{fetch_count, fetch_text, fresh_database};

const URL: &str = "/api/workspaces/cua-hang/datasets";

/// Thư mục dữ liệu riêng cho một test, tự xóa khi ra khỏi phạm vi.
struct DataDir(PathBuf);

impl DataDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-upload-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        DataDir(dir)
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Đếm file .parquet dưới thư mục dữ liệu, bỏ qua thư mục tạm.
fn parquet_files(dir: &Path) -> usize {
    let mut count = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "staging") {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "parquet") {
                count += 1;
            }
        }
    }
    count
}

fn staging_is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir.join("staging")).map_or(true, |mut it| it.next().is_none())
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

const T1: &[u8] = b"region,amount\nSouth,10\nNorth,20\n";
const T2: &[u8] = b"amount,region\n30,South\n";

#[tokio::test]
async fn upload_creates_dataset_versions_and_skips_duplicates() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    // Hai file cùng cột thành một dataset, phiên bản 1.
    let (status, body) = upload(
        &app,
        &lan,
        &[("name", "Bán hàng")],
        &[("thang1.csv", T1), ("thang2.csv", T2)],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["created"], true);
    assert_eq!(body["dataset"]["logical_table"], "ban_hang");
    assert_eq!(body["version"]["version"], 1);
    assert_eq!(body["version"]["row_count"], 3);
    assert_eq!(body["version"]["source_kind"], "csv");
    assert_eq!(body["dataset"]["current_version_id"], body["version"]["id"]);
    let columns: Vec<&str> = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(columns, ["region", "amount", "_source_file"]);
    // Hồ sơ cột có trước khi trả 201: đủ ba cột, cột ít giá trị có tần suất (region 2, amount 3, _source_file 2).
    assert_eq!(body["version"]["profile_status"], "structure");
    let version_id = body["version"]["id"].as_str().unwrap().to_string();
    let count = |table: &str| {
        let sql = match table {
            "columns" => {
                format!("SELECT count(*) FROM columns WHERE version_id = '{version_id}'")
            }
            _ => format!(
                "SELECT count(*) FROM column_values v JOIN columns c ON c.id = v.column_id \
                 WHERE c.version_id = '{version_id}'"
            ),
        };
        let store = db.store.clone();
        async move { fetch_count(&store, &sql).await }
    };
    assert_eq!(count("columns").await, 3);
    assert_eq!(count("column_values").await, 7);
    let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
    assert_eq!(parquet_files(&data.0), 2);
    assert!(staging_is_empty(&data.0));

    // Đúng bộ file đó, thứ tự khác: bỏ qua, trả phiên bản cũ.
    let (status, again) = upload(
        &app,
        &lan,
        &[("dataset_id", &dataset_id)],
        &[("thang2.csv", T2), ("thang1.csv", T1)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (again["created"].clone(), again["version"]["id"].clone()),
        (json!(false), body["version"]["id"].clone())
    );
    // Nạp lại như dataset mới cũng không tạo bản trùng.
    let (status, again) = upload(&app, &lan, &[], &[("thang1.csv", T1), ("thang2.csv", T2)]).await;
    assert_eq!(
        (status, again["dataset"]["id"].as_str()),
        (StatusCode::OK, Some(dataset_id.as_str()))
    );

    // Bộ file khác: phiên bản 2, thành phiên bản hiện hành.
    let t3: &[u8] = b"region,amount\nEast,5\n";
    let (status, v2) = upload(
        &app,
        &lan,
        &[("dataset_id", &dataset_id)],
        &[("thang1.csv", T1), ("thang3.csv", t3)],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v2}");
    assert_eq!(v2["version"]["version"], 2);
    assert_eq!(v2["dataset"]["current_version_id"], v2["version"]["id"]);

    // File khác cùng tên là một dataset khác (#257): tên bảng logic chỉ không trùng trong một dataset, nên giữ ban_hang.
    let (_, other) = upload(&app, &lan, &[("name", "Bán hàng")], &[("x.csv", b"a\n1\n")]).await;
    assert_eq!(other["dataset"]["logical_table"], "ban_hang");
    assert_ne!(other["dataset"]["parent_id"], v2["dataset"]["parent_id"]);

    let (_, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
    assert_eq!(list.as_array().map(Vec::len), Some(2));

    db.drop().await;
}

#[tokio::test]
async fn bad_uploads_leave_nothing_behind() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone();
        c.max_upload_bytes = 64;
    });
    let lan = setup(&app).await;

    let (status, body) = upload(
        &app,
        &lan,
        &[],
        &[("a.csv", T1), ("b.csv", b"region,qty\nSouth,1\n")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"].as_str().unwrap().contains("b.csv"),
        "{body}"
    );

    let (status, _) = upload(&app, &lan, &[], &[("data.xlsx", b"x")]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = upload(&app, &lan, &[], &[("a.csv", T1), ("b.parquet", b"x")]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = upload(&app, &lan, &[("name", "x")], &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let big = vec![b'a'; 100];
    let (status, body) = upload(&app, &lan, &[], &[("big.csv", &big)]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );

    // Giới hạn trong test là 64 byte: trường dataset_id đã 36 byte, nên dùng file nhỏ.
    let missing = uuid::Uuid::new_v4().to_string();
    let (status, _) = upload(
        &app,
        &lan,
        &[("dataset_id", &missing)],
        &[("a.csv", b"a\n1\n")],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = send_raw(&app, URL, &lan, "application/json", b"{}".to_vec()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (_, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
    assert_eq!(list.as_array().map(Vec::len), Some(0));
    assert_eq!(parquet_files(&data.0), 0);
    assert!(staging_is_empty(&data.0));

    db.drop().await;
}

#[tokio::test]
async fn concurrent_duplicate_uploads_do_not_fail() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;
    let (_, first) = upload(&app, &lan, &[("name", "Bán hàng")], &[("thang1.csv", T1)]).await;
    let id = first["dataset"]["id"].as_str().unwrap().to_string();

    // Cùng bộ file mới, gửi hai lần cùng lúc vào cùng dataset: một bên tạo phiên bản, bên kia nhận phiên bản đó.
    let fields = [("dataset_id", id.as_str())];
    let files: [(&str, &[u8]); 1] = [("thang2.csv", T2)];
    let a = upload(&app, &lan, &fields, &files);
    let b = upload(&app, &lan, &fields, &files);
    let ((sa, ba), (sb, bb)) = tokio::join!(a, b);
    let mut statuses = [sa, sb];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CREATED], "{ba} {bb}");
    assert_eq!(ba["version"]["id"], bb["version"]["id"]);
    assert!(staging_is_empty(&data.0));

    db.drop().await;
}

#[tokio::test]
async fn field_flood_and_file_names_are_bounded() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    // Hàng nghìn trường rỗng: nội dung 0 byte nhưng vẫn bị chặn.
    let empty: Vec<(&str, &str)> = (0..1500).map(|_| ("x", "")).collect();
    let (status, _) = upload(&app, &lan, &empty, &[("a.csv", T1)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let long = format!("{}.csv", "a".repeat(300));
    let (status, _) = upload(&app, &lan, &[], &[(long.as_str(), T1)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Phần thư mục trong tên file bị bỏ: chỉ còn tên, cả ở tên dataset lẫn cột _source_file.
    let (status, body) = upload(&app, &lan, &[], &[("../../secret/orders.csv", T1)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["dataset"]["name"], "orders");

    // Tên mặc định dài thì cắt còn 100 ký tự, không báo lỗi.
    let stem = "c".repeat(150);
    let name = format!("{stem}.csv");
    let (status, body) = upload(&app, &lan, &[], &[(name.as_str(), b"z\n9\n")]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body["dataset"]["name"].as_str().map(|n| n.chars().count()),
        Some(100)
    );

    db.drop().await;
}

/// Nhiều lần nạp KHÁC nội dung vào cùng dataset cùng lúc: hai lần có thể tính ra cùng số phiên bản; bên vấp
/// UNIQUE thử lại, không trả 500.
#[tokio::test]
async fn concurrent_different_uploads_all_succeed() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;
    let (_, first) = upload(&app, &lan, &[("name", "Bán hàng")], &[("thang1.csv", T1)]).await;
    let id = first["dataset"]["id"].as_str().unwrap().to_string();
    // Bốn lần nạp chạy cùng lúc, mỗi lần một nội dung khác nhau.
    let mut uploads = tokio::task::JoinSet::new();
    for i in 0..4 {
        let (app, lan, id) = (app.clone(), lan.clone(), id.clone());
        uploads.spawn(async move {
            let content = format!("region,amount\nSouth,{i}\n").into_bytes();
            upload(
                &app,
                &lan,
                &[("dataset_id", id.as_str())],
                &[("t.csv", content.as_slice())],
            )
            .await
        });
    }
    let mut versions: Vec<i64> = uploads
        .join_all()
        .await
        .iter()
        .map(|(status, body)| {
            assert_eq!(*status, StatusCode::CREATED, "{body}");
            body["version"]["version"].as_i64().unwrap()
        })
        .collect();
    versions.sort();
    assert_eq!(versions, [2, 3, 4, 5]);

    db.drop().await;
}

#[tokio::test]
async fn odd_file_selections_are_handled() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    // Cùng một file chọn hai lần: báo, không nhân đôi số dòng.
    let (status, body) = upload(&app, &lan, &[], &[("a.csv", T1), ("b.csv", T1)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // Hai file khác thư mục, trùng tên gốc: báo để cột _source_file phân biệt được.
    let (status, _) = upload(
        &app,
        &lan,
        &[],
        &[("hanoi/thang1.csv", T1), ("hcm/thang1.csv", T2)],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Tên file chỉ có đuôi: tên dataset mặc định, không 500.
    let (status, body) = upload(&app, &lan, &[], &[(".csv", T1)]).await;
    assert_eq!(
        (status, body["dataset"]["name"].as_str()),
        (StatusCode::CREATED, Some("Dataset")),
        "{body}"
    );

    db.drop().await;
}

#[test]
fn leftover_staging_is_cleared() {
    let data = DataDir::new();
    let leftover = data.0.join("staging").join("dead-upload").join("upload");
    std::fs::create_dir_all(&leftover).unwrap();
    std::fs::write(leftover.join("0000"), b"half").unwrap();
    crate::service::ingest::clear_staging(&data.0);
    assert!(!data.0.join("staging").exists());
    // Không có thư mục staging cũng không lỗi.
    crate::service::ingest::clear_staging(&data.0);
}

const DATACO_CSV: &[u8] = b"Type,Market,Order Id,Product Image,Order Item Total\n\
    DEBIT,Europe,1,img/1.jpg,100.5\n\
    CASH,LATAM,1,img/2.jpg,200.0\n\
    DEBIT,Europe,2,img/1.jpg,150.25\n";

/// Nạp xong, hồ sơ ngữ nghĩa chạy nền tới xong: dataset tên DataCo (bảng logic dataco) nhận hồ sơ mock — mô tả,
/// bí danh, vai trò sửa, 5 câu gợi ý, current_profile_status thành semantic. Xóa dòng spawn trong ingest thì test
/// này đỏ.
#[tokio::test]
async fn upload_spawns_the_semantic_profile() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone();
        // Tường minh mock, không theo biến môi trường của máy chạy test.
        c.llm_provider = "mock".to_string();
    });
    let lan = setup(&app).await;

    let (status, body) = upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["profile_status"], "structure");
    let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
    let url = format!("{URL}/{dataset_id}");

    let mut detail = serde_json::Value::Null;
    for _ in 0..100 {
        let (_, _, body) = call(&app, "GET", &url, Some(&lan), None).await;
        if body["current_profile_status"] == "semantic" {
            detail = body;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(detail["current_profile_status"], "semantic", "{detail}");
    assert_eq!(
        detail["suggested_questions"].as_array().map(Vec::len),
        Some(5),
        "{detail}"
    );
    assert_eq!(detail["semantic_unavailable"], false, "{detail}");

    db.drop().await;
}

/// Mock không có hồ sơ cho dataset khác DataCo (#140): lượt ngữ nghĩa lỗi vĩnh viễn, phiên bản giữ structure, và
/// semantic_unavailable thành true (cả trong danh sách lẫn chi tiết) để trang hỏi thôi báo "đang lập". Cùng
/// database mà backend chạy với provider khác thì trường này lại false: việc nền lúc khởi động sẽ thử lại với
/// provider đó.
#[tokio::test]
async fn semantic_unavailable_follows_the_running_provider() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone();
        c.llm_provider = "mock".to_string();
    });
    let lan = setup(&app).await;
    let csv: &[u8] = b"Region,Sales\nNorth,10\nSouth,20\n";
    let (status, body) = upload(&app, &lan, &[("name", "Sales")], &[("s.csv", csv)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
    let url = format!("{URL}/{dataset_id}");

    let mut detail = serde_json::Value::Null;
    for _ in 0..100 {
        let (_, _, body) = call(&app, "GET", &url, Some(&lan), None).await;
        if body["semantic_unavailable"] == true {
            detail = body;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(detail["semantic_unavailable"], true, "{detail}");
    assert_eq!(detail["current_profile_status"], "structure", "{detail}");
    // Tên provider không ra API.
    assert!(detail.get("semantic_attempted_with").is_none(), "{detail}");
    let (_, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
    assert_eq!(list[0]["semantic_unavailable"], true, "{list}");

    // Cùng database, backend khởi động lại với provider khác: phiên bản chưa thử với provider này.
    let other = app_with(db.store.clone(), None, |c| {
        c.data_dir = data.0.clone();
        c.llm_provider = "ollama".to_string();
        c.ollama_url = "http://127.0.0.1:9".to_string();
    });
    let (status, _, detail) = call(&other, "GET", &url, Some(&lan), None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["semantic_unavailable"], false, "{detail}");
    assert_eq!(detail["current_profile_status"], "structure", "{detail}");

    db.drop().await;
}

/// Cột nhạy cảm được đánh dấu ngay trong lượt lập hồ sơ cấu trúc: sau khi nạp (dù lớp ngữ nghĩa chưa chạy hay
/// lỗi mãi, ở đây provider luôn lỗi), giá trị của Customer Email không vào ngữ cảnh hỏi của #28
/// (build_ask_context), cũng không có trong danh sách giá trị đọc ra (list_value_rows).
#[tokio::test]
async fn sensitive_columns_keep_values_out_of_the_ask_context() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone();
        c.llm_provider = "ollama".to_string();
        c.ollama_url = "http://127.0.0.1:9".to_string();
    });
    let lan = setup(&app).await;
    let csv: &[u8] = b"Customer Email,Market,Order Id\n\
        lan@example.com,Europe,1\n\
        huy@example.com,LATAM,2\n\
        lan@example.com,Europe,3\n";
    let (status, body) = upload(&app, &lan, &[("name", "Khách")], &[("k.csv", csv)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["profile_status"], "structure");
    let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();

    // Dựng lại bối cảnh gọi của tầng service từ dữ liệu vừa nạp.
    let ws_text = fetch_text(
        &db.store,
        "SELECT workspace_id::text FROM memberships LIMIT 1",
    )
    .await;
    let ws = crate::auth::WorkspaceId::for_test(uuid::Uuid::parse_str(&ws_text).unwrap());
    let version = db
        .store
        .get_current_version(&ws, uuid::Uuid::parse_str(&dataset_id).unwrap())
        .await
        .unwrap()
        .unwrap();
    let state = crate::state::AppState::new(crate::config::Config::from_env(), db.store.clone());
    let context = crate::service::ask::context::build_ask_context(
        &state,
        &ws,
        &version,
        "doanh thu theo thị trường",
        &[],
    )
    .await
    .unwrap();
    let context = format!("{context:?}");
    assert!(!context.contains("lan@example.com"), "{context}");
    assert!(!context.contains("huy@example.com"), "{context}");
    // Cột không nhạy cảm vẫn có giá trị trong ngữ cảnh (Europe có trong câu hỏi qua "thị trường"? không; kiểm
    // trực tiếp danh sách đọc ra thay vì suy từ câu hỏi).
    let rows = db
        .store
        .list_value_rows(&ws, version.id, std::time::Duration::from_secs(2))
        .await
        .unwrap()
        .unwrap();
    assert!(rows.iter().any(|r| r.value == "Europe"));
    assert!(!rows.iter().any(|r| r.value.contains('@')));
    let email_col = db.store.list_columns(&ws, version.id).await.unwrap();
    let email_col = email_col
        .iter()
        .find(|c| c.name == "Customer Email")
        .unwrap();
    assert!(email_col.is_sensitive);

    db.drop().await;
}

/// Mô hình lỗi tạm (provider ollama trỏ vào cổng không nghe: kết nối bị từ chối, mã Upstream): lần nạp vẫn 201,
/// phiên bản giữ structure và không bị đánh dấu đã thử (lần khởi động sau thử lại); nạp lại đúng bộ file đó trả
/// 200 created = false và gọi lại mô hình lần nữa (vẫn lỗi, vẫn 200).
#[tokio::test]
async fn llm_failure_does_not_block_the_upload() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone();
        c.llm_provider = "ollama".to_string();
        // Cổng 9 (discard) hầu như luôn không có gì nghe: kết nối bị từ chối ngay, không phải chờ timeout.
        c.ollama_url = "http://127.0.0.1:9".to_string();
    });
    let lan = setup(&app).await;

    let (status, body) = upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let version_id = body["version"]["id"].as_str().unwrap().to_string();
    // Chờ lượt nền chạy xong (lỗi tạm: không đánh dấu), rồi kiểm: structure, chưa đánh dấu.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let attempted = format!(
        "SELECT count(*) FROM dataset_versions WHERE id = '{version_id}' AND semantic_attempted_with IS NOT NULL"
    );
    assert_eq!(fetch_count(&db.store, &attempted).await, 0);
    let status_of = format!(
        "SELECT count(*) FROM dataset_versions WHERE id = '{version_id}' AND profile_status = 'structure'"
    );
    assert_eq!(fetch_count(&db.store, &status_of).await, 1);

    // Nạp lại đúng bộ file: không tạo gì mới, nhưng là đường thử lại tường minh của lớp ngữ nghĩa.
    let (status, body) = upload(&app, &lan, &[("name", "DataCo")], &[("d.csv", DATACO_CSV)]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["created"], false);

    db.drop().await;
}

#[tokio::test]
async fn admin_deletes_a_dataset_with_its_files_member_cannot() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = DataDir::new();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    let (status, body) = upload(&app, &lan, &[("name", "Bán hàng")], &[("thang1.csv", T1)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["dataset"]["id"].as_str().unwrap().to_string();
    assert!(parquet_files(&data.0) > 0);

    // Member chỉ xem và hỏi, không xóa được.
    let new_user = json!({"email": "minh@example.com", "password": "a long password"});
    let (status, _, _) = call(&app, "POST", "/api/admin/users", Some(&lan), Some(new_user)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": "a long password"});
    let (_, minh, _) = call(&app, "POST", "/api/auth/login", None, Some(login)).await;
    let minh = minh.unwrap();
    let add = json!({"email": "minh@example.com", "role": "member"});
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/workspaces/cua-hang/members",
        Some(&lan),
        Some(add),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _, res) = call(&app, "DELETE", &format!("{URL}/{id}"), Some(&minh), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{res}");

    // Admin xóa: 204, danh sách trống, file Parquet đi theo; xóa lại thì 404.
    let (status, _, res) = call(&app, "DELETE", &format!("{URL}/{id}"), Some(&lan), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{res}");
    let (status, _, list) = call(&app, "GET", URL, Some(&lan), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 0, "{list}");
    assert_eq!(parquet_files(&data.0), 0);
    let (status, _, _) = call(&app, "DELETE", &format!("{URL}/{id}"), Some(&lan), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    db.drop().await;
}
