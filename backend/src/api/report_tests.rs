// SPDX-License-Identifier: Apache-2.0
/* api/report_tests.rs - test báo cáo qua router thật, với Postgres: chỉ người tạo đọc được báo cáo và chỉ thêm được câu
của chính mình, mục dựng lại bằng cách chạy lại câu SQL đã lưu với quyền của người xem (lọc dòng theo vai dữ liệu áp
cả khi vai đổi sau lúc hỏi, lời đã lưu khi đó không hiện), đổi thứ tự, bỏ mục, xóa dataset thì mục mất theo, file
Excel, và báo cáo tự động chạy với bộ lọc dòng của người bấm. Mỗi test dùng database riêng.

Như api/drill_tests.rs, câu hỏi được ghi thẳng vào queries bằng SQL (đúng các cột luồng hỏi ghi), không hỏi qua mock:
chọn được đúng câu cần thử. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::{exec, fetch_text, fresh_database, give_data_role};
use crate::engine::store::Store;

const PASSWORD: &str = "a long password";
const SHOP: &str = "/api/workspaces/cua-hang";
const SALES: &[u8] = b"Market,amount,ordered\n\
Europe,10,2024-01-05\n\
LATAM,5,2024-01-20\n\
Europe,7,2024-02-03\n\
USCA,1,2024-03-09\n\
LATAM,2,2024-03-15\n";
const BY_MARKET: &str =
    "SELECT \"Market\", SUM(\"amount\") AS revenue FROM ban_hang GROUP BY \"Market\"";

fn temp_data_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xdi-report-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Lan: admin của "Cửa hàng". Minh: member của cùng workspace. Trả hai phiên đăng nhập.
async fn lan_and_minh(app: &axum::Router) -> (String, String) {
    let setup = json!({"setup_code": CODE, "email": "lan@example.com", "password": PASSWORD,
                       "workspace_name": "Cửa hàng"});
    let (status, lan, _) = call(app, "POST", "/api/auth/setup", None, Some(setup)).await;
    assert_eq!(status, StatusCode::OK);
    let lan = lan.unwrap();
    let user = json!({"email": "minh@example.com", "password": PASSWORD});
    let (status, _, _) = call(app, "POST", "/api/admin/users", Some(&lan), Some(user)).await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "minh@example.com", "role": "member"});
    let (status, _, _) = call(
        app,
        "POST",
        &format!("{SHOP}/members"),
        Some(&lan),
        Some(add),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": PASSWORD});
    let (_, minh, _) = call(app, "POST", "/api/auth/login", None, Some(login)).await;
    (lan, minh.unwrap())
}

async fn upload(app: &axum::Router, session: &str) -> String {
    let (content_type, body) = multipart(&[("name", "Bán hàng")], &[("sales.csv", SALES)]);
    let (status, body) = send_raw(
        app,
        &format!("{SHOP}/datasets"),
        session,
        &content_type,
        body,
    )
    .await;
    assert!(status.is_success(), "{status} {body}");
    body["dataset"]["id"].as_str().unwrap().to_string()
}

/// Một câu Minh đã hỏi và đã trả lời, như luồng hỏi ghi: SQL mô hình viết, SQL sau guard, số dòng, lời trả lời.
async fn record_minh_answer(store: &Store, dataset_id: &str) -> String {
    let guarded = format!("{BY_MARKET} LIMIT 100000");
    let answer = r#"{"text": "Europe dẫn đầu với 17.", "by_model": true, "verdict": "match", "warnings": []}"#;
    fetch_text(
        store,
        &format!(
            "INSERT INTO queries (workspace_id, dataset_id, version_id, user_id, question, sql_generated, sql_guarded,
                                  status, row_count, answer)
             SELECT d.workspace_id, d.id, d.current_version_id, u.id, 'Doanh thu theo thị trường',
                    '{generated}', '{guarded}', 'ok', 3, '{answer}'::jsonb
             FROM datasets d, users u WHERE d.id = '{dataset_id}' AND u.email = 'minh@example.com'
             RETURNING id::text",
            generated = BY_MARKET.replace('\'', "''"),
            guarded = guarded.replace('\'', "''"),
        ),
    )
    .await
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    session: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, _, body) = call(app, method, path, Some(session), body).await;
    (status, body)
}

async fn create_report(app: &axum::Router, session: &str, title: &str) -> String {
    let (status, body) = send(
        app,
        "POST",
        &format!("{SHOP}/reports"),
        session,
        Some(json!({"title": title})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["id"].as_str().unwrap().to_string()
}

fn markets(item: &Value) -> Vec<String> {
    item["result"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[0].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn owner_builds_a_report_and_others_cannot_see_it() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    let query_id = record_minh_answer(&db.store, &dataset_id).await;
    let report = create_report(&app, &minh, "Báo cáo tháng").await;
    let items = format!("{SHOP}/reports/{report}/items");

    let (status, body) = send(
        &app,
        "POST",
        &items,
        &minh,
        Some(json!({"query_id": query_id})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["item_count"], 1);
    let (status, body) = send(
        &app,
        "POST",
        &items,
        &minh,
        Some(json!({"query_id": query_id})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item = &body["items"][0];
    assert_eq!(item["question"], "Doanh thu theo thị trường");
    assert_eq!(item["question_type"], "quantitative");
    assert_eq!(item["answer"]["text"], "Europe dẫn đầu với 17.");
    assert_eq!(item["number_check"], "passed");
    assert_eq!(item["meaning_check"], "match");
    assert_eq!(item["error"], Value::Null, "{item}");
    assert_eq!(item["result"]["total_rows"], 3);
    assert_eq!(item["tables"][0]["version"], 1);
    assert_eq!(item["tables"][0]["is_current"], true);
    assert_eq!(item["notes"], json!([]));
    assert!(
        item["understanding"]["measures"].as_array().unwrap().len() == 1,
        "{item}"
    );
    assert!(item["sql"].as_str().unwrap().contains("LIMIT"), "{item}");

    // Lan (admin) không đọc được báo cáo của Minh, và không thêm được câu của Minh vào báo cáo của mình.
    let (status, _) = send(&app, "GET", &format!("{SHOP}/reports/{report}"), &lan, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = send(&app, "GET", &format!("{SHOP}/reports"), &lan, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reports"], json!([]));
    let lans = create_report(&app, &lan, "Của Lan").await;
    let path = format!("{SHOP}/reports/{lans}/items");
    let (status, _) = send(
        &app,
        "POST",
        &path,
        &lan,
        Some(json!({"query_id": query_id})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // File Excel: dựng từ cùng bản dựng lại; nội dung các sheet kiểm ở service/report/tests.rs.
    let path = format!("{SHOP}/reports/{report}/xlsx");
    let (status, _, _) = call(&app, "GET", &path, Some(&minh), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&app, "GET", &path, Some(&lan), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn row_filters_of_the_viewer_apply_when_the_report_is_opened() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    let query_id = record_minh_answer(&db.store, &dataset_id).await;
    let report = create_report(&app, &minh, "Báo cáo").await;
    let items = format!("{SHOP}/reports/{report}/items");
    let (status, _) = send(
        &app,
        "POST",
        &items,
        &minh,
        Some(json!({"query_id": query_id})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Sau lúc hỏi, admin gán cho Minh vai dữ liệu chỉ thấy LATAM.
    give_data_role(
        &db.store,
        "minh@example.com",
        "latam",
        &[(&dataset_id, "Market", &["LATAM"])],
    )
    .await;
    let (status, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let item = &body["items"][0];
    assert_eq!(markets(item), vec!["LATAM"], "{item}");
    assert!(
        item["sql"]
            .as_str()
            .unwrap()
            .contains("\"Market\" IN ('LATAM')"),
        "{item}"
    );
    // Lời đã lưu nói về Europe, nhóm Minh không còn được thấy: không hiện.
    assert_eq!(item["answer"], Value::Null, "{item}");
    let notes = item["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2, "{item}");
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn items_reorder_remove_and_go_with_the_dataset() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    let first = record_minh_answer(&db.store, &dataset_id).await;
    let second = record_minh_answer(&db.store, &dataset_id).await;
    let report = create_report(&app, &minh, "Báo cáo").await;
    let items = format!("{SHOP}/reports/{report}/items");
    for id in [&first, &second] {
        let (status, _) = send(&app, "POST", &items, &minh, Some(json!({"query_id": id}))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    let ids: Vec<String> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(body["items"][0]["query_id"], first.as_str());

    let order = format!("{SHOP}/reports/{report}/order");
    let (status, _) = send(
        &app,
        "PUT",
        &order,
        &minh,
        Some(json!({"item_ids": [ids[0]]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &app,
        "PUT",
        &order,
        &minh,
        Some(json!({"item_ids": [ids[1], ids[0]]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(body["items"][0]["query_id"], second.as_str());

    let path = format!("{items}/{}", ids[1]);
    let (status, _) = send(&app, "DELETE", &path, &minh, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // Xóa dataset thì câu hỏi mất theo (0011), mục của báo cáo cũng vậy; báo cáo tự gom còn lại, rỗng.
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("{SHOP}/datasets/{dataset_id}"),
        &lan,
        None,
    )
    .await;
    assert!(status.is_success(), "{status}");
    let (status, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"], json!([]));

    let (status, _) = send(
        &app,
        "DELETE",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn automatic_report_uses_the_row_filters_of_the_member() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    exec(
        &db.store,
        &format!(
            "INSERT INTO metrics (workspace_id, dataset_id, source_id, name, expression, status, origin, version)
             SELECT workspace_id, id, source_id, 'Doanh thu', 'SUM(\"amount\")', 'confirmed', 'sample', 1
             FROM datasets WHERE id = '{dataset_id}'"
        ),
    )
    .await;
    give_data_role(
        &db.store,
        "minh@example.com",
        "latam",
        &[(&dataset_id, "Market", &["LATAM"])],
    )
    .await;
    let source = fetch_text(
        &db.store,
        &format!("SELECT source_id::text FROM datasets WHERE id = '{dataset_id}'"),
    )
    .await;

    let path = format!("{SHOP}/reports/automatic");
    let (status, body) = send(
        &app,
        "POST",
        &path,
        &minh,
        Some(json!({"dataset_id": source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["report"]["kind"], "automatic");
    assert_eq!(body["report"]["dataset_id"], source.as_str());
    // Theo tháng và theo Market (cột chiều duy nhất); mock không viết lời nên không có lần gọi nào được dùng.
    assert_eq!(body["report"]["item_count"], 2, "{body}");
    let report = body["report"]["id"].as_str().unwrap().to_string();

    let (status, body) = send(
        &app,
        "GET",
        &format!("{SHOP}/reports/{report}"),
        &minh,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let by_market = &body["items"][1];
    assert_eq!(by_market["question"], "Doanh thu theo Market");
    assert_eq!(markets(by_market), vec!["LATAM"], "{by_market}");
    // Câu do code dựng, đã ghi với bộ lọc của Minh: lời đã lưu hiện được, và là câu của hệ thống.
    assert_eq!(by_market["number_check"], "not_needed", "{by_market}");
    assert!(
        by_market["answer"]["text"]
            .as_str()
            .unwrap()
            .contains("LATAM"),
        "{by_market}"
    );
    let by_day = &body["items"][0];
    // Dữ liệu chỉ dài ba tháng nên chia theo ngày (overview/sql.rs, build_grain); hai ngày có dòng LATAM.
    assert_eq!(by_day["question"], "Doanh thu theo ngày");
    assert_eq!(by_day["result"]["total_rows"], 2, "{by_day}");

    // Lan không thấy báo cáo của Minh.
    let (status, _) = send(&app, "GET", &format!("{SHOP}/reports/{report}"), &lan, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&data);
}
