// SPDX-License-Identifier: Apache-2.0
/* api/overview_tests.rs - test tổng quan dataset (GET .../datasets/{id}/overview, #198) qua router thật, với Postgres và
DuckDB: tóm tắt và biểu đồ tính bằng SQL qua guard, cột nhạy cảm không lộ giá trị, người bị lọc dòng theo vai dữ liệu
chỉ thấy phần của mình và không nhận số đếm của hồ sơ, chỉ số đã xác nhận thành độ đo chính, 404 cho dataset lạ. Phần
thuần ở service/overview/tests.rs. Mỗi test dùng database riêng. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::{exec, fresh_database, give_data_role};

const PASSWORD: &str = "a long password";
const SHOP: &str = "/api/workspaces/cua-hang";
const SALES: &[u8] = b"Market,amount,ordered,Email\n\
Europe,10,2024-01-05,a@example.com\n\
LATAM,5,2024-01-20,b@example.com\n\
Europe,7,2024-02-03,c@example.com\n\
USCA,1,2024-03-10,d@example.com\n\
LATAM,2,2024-04-15,e@example.com\n";

fn temp_data_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xdi-overview-api-{}", uuid::Uuid::new_v4()));
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

async fn overview(app: &axum::Router, session: &str, dataset_id: &str) -> (StatusCode, Value) {
    let path = format!("{SHOP}/datasets/{dataset_id}/overview");
    let (status, _, body) = call(app, "GET", &path, Some(session), None).await;
    (status, body)
}

fn chart<'a>(body: &'a Value, kind: &str) -> Option<&'a Value> {
    body["charts"]
        .as_array()?
        .iter()
        .find(|c| c["kind"] == kind)
}

fn column<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()
}

fn codes(body: &Value) -> Vec<String> {
    body["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["code"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn members_see_the_profile_and_the_charts_without_sensitive_values() {
    let Some(db) = fresh_database().await else {
        return;
    };
    let dir = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = dir.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;

    let (status, body) = overview(&app, &minh, &dataset_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"]["row_count"], 5);
    assert_eq!(body["summary"]["time_column"], "ordered");
    assert_eq!(body["summary"]["time_start"], "2024-01-05");
    assert_eq!(body["summary"]["time_end"], "2024-04-15");
    assert_eq!(body["summary"]["restricted"], false);
    assert_eq!(body["version"]["number"], 1);

    let market = column(&body, "Market");
    assert_eq!(market["role"], "dimension");
    assert_eq!(market["values"][0]["value"], "Europe");
    assert_eq!(market["values"][0]["rows"], 2);
    let email = column(&body, "Email");
    assert_eq!(email["sensitive"], true);
    assert_eq!(email["values"], json!([]));
    assert!(!body.to_string().contains("a@example.com"), "{body}");

    // Mock không viết đơn vị cột và chưa có chỉ số: không có độ đo chính, vẫn có số dòng theo kỳ; dữ liệu dài 101 ngày
    // nên theo tháng.
    assert!(body["main_measure"].is_null());
    assert!(codes(&body).contains(&"no_measure".to_string()));
    let rows = chart(&body, "rows_over_time").expect("rows over time");
    assert_eq!(rows["grain"], "month");
    assert_eq!(rows["result"]["rows"].as_array().unwrap().len(), 4);
    assert!(rows["sql"].as_str().unwrap().contains("date_trunc"));
    let by_market = body["charts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["kind"] == "rows_by_value" && c["column"] == "Market")
        .expect("rows by market");
    assert_eq!(by_market["chart"]["kind"], "bar");
    assert!(!body["charts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["column"] == "Email"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_confirmed_metric_is_drawn_over_time() {
    let Some(db) = fresh_database().await else {
        return;
    };
    let dir = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = dir.clone());
    let (lan, _) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    exec(
        &db.store,
        &format!(
            "INSERT INTO metrics (workspace_id, dataset_id, source_id, name, expression, filter, time_column, status,
                                  origin, version)
             SELECT workspace_id, id, source_id, 'Doanh thu', 'SUM(\"amount\")', '\"Market\" <> ''USCA''', 'ordered',
                    'confirmed', 'sample', 1
             FROM datasets WHERE id = '{dataset_id}'"
        ),
    )
    .await;

    let (status, body) = overview(&app, &lan, &dataset_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["main_measure"]["metric"], "Doanh thu");
    assert_eq!(body["metrics"][0]["name"], "Doanh thu");
    let measure = chart(&body, "measure_over_time").expect("measure over time");
    assert_eq!(measure["chart"]["y"], json!(["Doanh thu"]));
    // Tháng 1: 10 + 5; tháng 3 chỉ có USCA, bị điều kiện của chỉ số loại.
    let rows = measure["result"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{measure}");
    assert_eq!(rows[0][1], "15");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn row_filters_of_the_viewer_apply_and_profile_counts_are_left_out() {
    let Some(db) = fresh_database().await else {
        return;
    };
    let dir = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = dir.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    give_data_role(
        &db.store,
        "minh@example.com",
        "latam",
        &[(&dataset_id, "Market", &["LATAM"])],
    )
    .await;

    let (status, body) = overview(&app, &minh, &dataset_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"]["restricted"], true);
    assert_eq!(body["summary"]["row_count"], 2);
    assert_eq!(body["summary"]["time_start"], "2024-01-20");
    assert!(codes(&body).contains(&"restricted".to_string()));
    let market = column(&body, "Market");
    assert_eq!(market["values"], json!([]));
    assert!(market["distinct_count"].is_null());
    assert!(!body.to_string().contains("Europe"), "{body}");
    let rows = chart(&body, "rows_over_time").expect("rows over time");
    assert!(rows["sql"].as_str().unwrap().contains("LATAM"));
    assert!(chart(&body, "rows_by_value").is_none());

    // Admin không bị lọc: cùng câu, kết quả riêng trong cache.
    let (_, body) = overview(&app, &lan, &dataset_id).await;
    assert_eq!(body["summary"]["row_count"], 5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn unknown_datasets_and_strangers_get_errors() {
    let Some(db) = fresh_database().await else {
        return;
    };
    let dir = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = dir.clone());
    let (lan, _) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan).await;
    let (status, _) = overview(&app, &lan, &uuid::Uuid::new_v4().to_string()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let path = format!("{SHOP}/datasets/{dataset_id}/overview");
    let (status, _, _) = call(&app, "GET", &path, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // An chỉ là member của workspace thứ hai: hỏi dataset của "Cửa hàng" qua slug của mình hay slug "Cửa hàng" đều 404.
    let (status, _, ws) = call(
        &app,
        "POST",
        "/api/workspaces",
        Some(&lan),
        Some(json!({"name": "Kho hai"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let second = format!("/api/workspaces/{}", ws["slug"].as_str().unwrap());
    let user = json!({"email": "an@example.com", "password": PASSWORD});
    let (status, _, _) = call(&app, "POST", "/api/admin/users", Some(&lan), Some(user)).await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "an@example.com", "role": "member"});
    let members = format!("{second}/members");
    let (status, _, _) = call(&app, "POST", &members, Some(&lan), Some(add)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "an@example.com", "password": PASSWORD});
    let (_, an, _) = call(&app, "POST", "/api/auth/login", None, Some(login)).await;
    let an = an.unwrap();
    for ws in [second.as_str(), SHOP] {
        let path = format!("{ws}/datasets/{dataset_id}/overview");
        let (status, _, body) = call(&app, "GET", &path, Some(&an), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{ws}: {body}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
