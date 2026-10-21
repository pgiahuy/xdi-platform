// SPDX-License-Identifier: Apache-2.0
/* api/roles_tests.rs - test bảng quyền trong docs/ARCHITECTURE.md mục 4b, gọi qua router thật: admin và member của
workspace làm được gì, luật luôn còn ít nhất một admin, quản trị hệ thống thấy gì. Mỗi test dùng database riêng.
Luật admin cuối khi hai lần đổi vai chạy cùng lúc được test ở engine/store/members.rs. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app, app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::fresh_database;

const PASSWORD: &str = "a long password";
const SHOP: &str = "/api/workspaces/cua-hang";

/// Lan: quản trị hệ thống, admin của workspace "Cửa hàng". Minh: tài khoản thường, chưa thuộc workspace nào.
async fn lan_and_minh(app: &axum::Router) -> (String, String) {
    let setup = json!({"setup_code": CODE, "email": "lan@example.com", "password": PASSWORD,
                       "workspace_name": "Cửa hàng"});
    let (status, lan, _) = call(app, "POST", "/api/auth/setup", None, Some(setup)).await;
    assert_eq!(status, StatusCode::OK);
    let lan = lan.unwrap();
    let minh = new_account(app, &lan, "minh@example.com").await;
    (lan, minh)
}

/// Quản trị hệ thống (Lan) tạo thêm một tài khoản; trả phiên đăng nhập của người đó.
async fn new_account(app: &axum::Router, lan: &str, email: &str) -> String {
    let new_user = json!({"email": email, "password": PASSWORD});
    let (status, _, _) = call(app, "POST", "/api/admin/users", Some(lan), Some(new_user)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": email, "password": PASSWORD});
    let (_, session, _) = call(app, "POST", "/api/auth/login", None, Some(login)).await;
    session.unwrap()
}

/// Thư mục dữ liệu riêng cho một test; test tự xóa khi xong.
fn temp_data_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xdi-roles-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Vai của từng người trong workspace, theo email.
async fn roles(app: &axum::Router, session: &str, ws: &str) -> Vec<(String, String)> {
    let (status, _, body) = call(app, "GET", &format!("{ws}/members"), Some(session), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["email"].as_str().unwrap().to_string(),
                m["role"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

async fn user_id(app: &axum::Router, session: &str, ws: &str, email: &str) -> String {
    let (_, _, body) = call(app, "GET", &format!("{ws}/members"), Some(session), None).await;
    body.as_array()
        .unwrap()
        .iter()
        .find(|m| m["email"] == email)
        .map(|m| m["user_id"].as_str().unwrap().to_string())
        .unwrap()
}

async fn upload_csv(app: &axum::Router, session: &str) -> (StatusCode, Value) {
    let (content_type, body) = multipart(
        &[("name", "Bán hàng")],
        &[("t.csv", b"region,amount\nSouth,10\n".as_slice())],
    );
    send_raw(
        app,
        &format!("{SHOP}/datasets"),
        session,
        &content_type,
        body,
    )
    .await
}

#[tokio::test]
async fn member_reads_and_admin_manages() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let members = format!("{SHOP}/members");

    // Thêm thành viên: email phải có tài khoản, vai chỉ admin hoặc member, không thêm trùng.
    let add = |email: &str, role: &str| Some(json!({"email": email, "role": role}));
    let cases = [
        (add("minh@example.com", "member"), StatusCode::CREATED),
        (add("MINH@example.com", "member"), StatusCode::BAD_REQUEST),
        (add("nobody@example.com", "member"), StatusCode::NOT_FOUND),
        (add("minh@example.com", "owner"), StatusCode::BAD_REQUEST),
    ];
    for (body, expected) in cases {
        let (status, _, res) = call(&app, "POST", &members, Some(&lan), body).await;
        assert_eq!(status, expected, "{res}");
    }
    let lan_id = user_id(&app, &lan, SHOP, "lan@example.com").await;
    let minh_id = user_id(&app, &lan, SHOP, "minh@example.com").await;

    // Member xem được dữ liệu, danh sách thành viên, và hỏi được.
    let (status, body) = upload_csv(&app, &lan).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let dataset_id = body["dataset"]["id"].as_str().unwrap().to_string();
    let (status, _, _) = call(&app, "GET", &format!("{SHOP}/datasets"), Some(&minh), None).await;
    assert_eq!(status, StatusCode::OK);
    let path = format!("{SHOP}/datasets/{dataset_id}");
    let (status, _, _) = call(&app, "GET", &path, Some(&minh), None).await;
    assert_eq!(status, StatusCode::OK);
    let ask = Some(json!({"question": "doanh thu"}));
    let (status, _, _) = call(&app, "POST", &format!("{path}/ask"), Some(&minh), ask).await;
    // Route hỏi chưa làm xong nên trả 501; làm xong thì đổi thành mã thành công. Quan trọng là không 403, không 404.
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        roles(&app, &minh, SHOP).await,
        [
            ("lan@example.com".to_string(), "admin".to_string()),
            ("minh@example.com".to_string(), "member".to_string())
        ]
    );

    // Member không nạp file, không quản lý workspace hay thành viên.
    let (status, body) = upload_csv(&app, &minh).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let lan_path = format!("{members}/{lan_id}");
    let forbidden = [
        ("PATCH", SHOP.to_string(), Some(json!({"name": "Đổi tên"}))),
        ("DELETE", SHOP.to_string(), None),
        ("POST", members.clone(), add("lan@example.com", "member")),
        ("PATCH", lan_path.clone(), Some(json!({"role": "member"}))),
        ("DELETE", lan_path.clone(), None),
    ];
    for (method, path, body) in forbidden {
        let (status, _, res) = call(&app, method, &path, Some(&minh), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {res}");
    }

    // Admin đổi tên: slug giữ nguyên.
    let rename = Some(json!({"name": "Cửa hàng mới"}));
    let (status, _, _) = call(&app, "PATCH", SHOP, Some(&lan), rename).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, _, list) = call(&app, "GET", "/api/workspaces", Some(&minh), None).await;
    assert_eq!(
        (list[0]["name"].as_str(), list[0]["slug"].as_str()),
        (Some("Cửa hàng mới"), Some("cua-hang"))
    );

    // Admin nâng Minh lên admin; Minh làm được việc của admin.
    let minh_path = format!("{members}/{minh_id}");
    let promote = Some(json!({"role": "admin"}));
    let (status, _, _) = call(&app, "PATCH", &minh_path, Some(&lan), promote).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(&app, "PATCH", SHOP, Some(&minh), Some(json!({"name": "X"}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Admin xóa một thành viên khác.
    let nga = new_account(&app, &lan, "nga@example.com").await;
    let (status, _, _) = call(
        &app,
        "POST",
        &members,
        Some(&lan),
        add("nga@example.com", "member"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let nga_path = format!(
        "{members}/{}",
        user_id(&app, &lan, SHOP, "nga@example.com").await
    );
    let (status, _, _) = call(&app, "DELETE", &nga_path, Some(&minh), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(&app, "GET", &format!("{SHOP}/datasets"), Some(&nga), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Admin tự hạ vai khi còn admin khác; được nâng lại; rồi tự rời khi đang là admin.
    let demote = Some(json!({"role": "member"}));
    let (status, _, _) = call(&app, "PATCH", &lan_path, Some(&lan), demote.clone()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let promote = Some(json!({"role": "admin"}));
    let (status, _, _) = call(&app, "PATCH", &lan_path, Some(&minh), promote).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(&app, "DELETE", &lan_path, Some(&lan), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Rời rồi thì như chưa từng thuộc workspace: mọi route trả 404, không phải 403.
    let outsider = [
        ("GET", format!("{SHOP}/datasets"), None),
        ("GET", members.clone(), None),
        ("PATCH", SHOP.to_string(), Some(json!({"name": "Y"}))),
        ("DELETE", SHOP.to_string(), None),
        ("POST", members.clone(), add("lan@example.com", "admin")),
        ("PATCH", minh_path.clone(), Some(json!({"role": "member"}))),
        ("DELETE", minh_path.clone(), None),
    ];
    for (method, path, body) in outsider {
        let (status, _, res) = call(&app, method, &path, Some(&lan), body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {res}");
    }

    // Minh là admin cuối cùng: không tự hạ vai, không tự rời được.
    let (status, _, _) = call(&app, "PATCH", &minh_path, Some(&minh), demote).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(&app, "DELETE", &minh_path, Some(&minh), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        roles(&app, &minh, SHOP).await,
        [("minh@example.com".to_string(), "admin".to_string())]
    );

    // Người không còn trong workspace thì không đổi được gì nữa.
    let (status, _, _) = call(&app, "DELETE", &lan_path, Some(&minh), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(&data);
    db.drop().await;
}

/// Quản trị hệ thống thấy mọi workspace và thành viên, không vào được dữ liệu của workspace người khác, trừ khi tự
/// chỉ định mình làm admin.
#[tokio::test]
async fn system_admin_oversees_but_does_not_read_data() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let app = app(db.store.clone(), Some(CODE));
    let (lan, minh) = lan_and_minh(&app).await;
    let (status, _, ws) = call(
        &app,
        "POST",
        "/api/workspaces",
        Some(&minh),
        Some(json!({"name": "Của Minh"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let minh_ws = "/api/workspaces/cua-minh";
    let workspace_id = ws["workspace_id"].as_str().unwrap();

    let (status, _, list) = call(&app, "GET", "/api/admin/workspaces", Some(&lan), None).await;
    assert_eq!(status, StatusCode::OK);
    let found = list
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["slug"] == "cua-minh")
        .unwrap();
    assert_eq!(found["members"][0]["email"], "minh@example.com");
    assert_eq!(found["members"][0]["role"], "admin");
    assert!(found.get("datasets").is_none());

    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{minh_ws}/datasets"),
        Some(&lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(&app, "GET", &format!("{minh_ws}/members"), Some(&lan), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Người thường không dùng được phần quản trị hệ thống.
    let assign = |email: &str| Some(json!({"email": email}));
    let assign_path = format!("/api/admin/workspaces/{workspace_id}/admins");
    let (status, _, _) = call(&app, "GET", "/api/admin/workspaces", Some(&minh), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = call(
        &app,
        "POST",
        &assign_path,
        Some(&minh),
        assign("minh@example.com"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Tài khoản đã khóa không được chỉ định làm admin, cũng không được thêm làm thành viên.
    let old = json!({"email": "old@example.com", "password": PASSWORD});
    let (_, _, old) = call(&app, "POST", "/api/admin/users", Some(&lan), Some(old)).await;
    let deactivate = format!(
        "/api/admin/users/{}/deactivate",
        old["id"].as_str().unwrap()
    );
    let (status, _, _) = call(&app, "POST", &deactivate, Some(&lan), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let add_old = Some(json!({"email": "old@example.com", "role": "member"}));
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("{minh_ws}/members"),
        Some(&minh),
        add_old,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let other_workspace = format!("/api/admin/workspaces/{}/admins", uuid::Uuid::new_v4());
    let missing = [
        (
            other_workspace.as_str(),
            "lan@example.com",
            StatusCode::NOT_FOUND,
        ),
        (
            assign_path.as_str(),
            "nobody@example.com",
            StatusCode::NOT_FOUND,
        ),
        (
            assign_path.as_str(),
            "old@example.com",
            StatusCode::NOT_FOUND,
        ),
        (
            "/api/admin/workspaces/not-a-uuid/admins",
            "lan@example.com",
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (path, email, expected) in missing {
        let (status, _, _) = call(&app, "POST", path, Some(&lan), assign(email)).await;
        assert_eq!(status, expected, "{path} {email}");
    }

    // Chỉ định chính mình: từ đây Lan là admin của workspace của Minh.
    let (status, _, _) = call(
        &app,
        "POST",
        &assign_path,
        Some(&lan),
        assign("lan@example.com"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("{minh_ws}/datasets"),
        Some(&lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        roles(&app, &lan, minh_ws).await,
        [
            ("lan@example.com".to_string(), "admin".to_string()),
            ("minh@example.com".to_string(), "admin".to_string())
        ]
    );

    db.drop().await;
}

#[tokio::test]
async fn deleting_a_workspace_removes_its_files() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, _) = lan_and_minh(&app).await;

    let (status, body) = upload_csv(&app, &lan).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, _, list) = call(&app, "GET", "/api/workspaces", Some(&lan), None).await;
    let ws_dir = data.join(list[0]["workspace_id"].as_str().unwrap());
    assert!(ws_dir.is_dir());

    let (status, _, _) = call(&app, "DELETE", SHOP, Some(&lan), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!ws_dir.exists());
    let (_, _, list) = call(&app, "GET", "/api/workspaces", Some(&lan), None).await;
    assert_eq!(list, json!([]));
    let (status, _, _) = call(&app, "GET", &format!("{SHOP}/datasets"), Some(&lan), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(&data);
    db.drop().await;
}
