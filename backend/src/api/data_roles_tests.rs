// SPDX-License-Identifier: Apache-2.0
/* api/data_roles_tests.rs - API vai dữ liệu (api/data_roles.rs) qua route thật trên một file nạp vào, với mô hình mock:
admin tạo vai, đặt luật, gán vai; member nhận 403; người mang vai hỏi "Doanh thu theo thị trường" chỉ thấy dòng được
phép, AND trong một vai, OR giữa các vai, admin không bị lọc, đổi luật có hiệu lực từ request kế tiếp. Lọc lan theo khóa
ngoại với vai đặt bằng store ở api/row_filter_fk_tests.rs. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{start_on, Asking};
use crate::api::test_support::call;

const CSV: &str = "Market,Order Region,Order Item Total\n\
                   Europe,Western Europe,100\n\
                   Europe,Northern Europe,50\n\
                   LATAM,South America,200\n";

const WS: &str = "/api/workspaces/cua-hang";
const QUESTION: &str = "Doanh thu theo thị trường";

struct Roles {
    asking: Asking,
    /// Phiên của Minh, member của workspace.
    minh: String,
    minh_id: String,
    lan_id: String,
    table: String,
}

impl Roles {
    async fn send(
        &self,
        session: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let url = format!("{WS}{path}");
        let (status, _, body) = call(&self.asking.app, method, &url, Some(session), body).await;
        (status, body)
    }

    async fn admin(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.send(&self.asking.session, method, path, body).await
    }

    async fn create_role(&self, name: &str) -> String {
        let (status, role) = self
            .admin("POST", "/data-roles", Some(json!({"name": name})))
            .await;
        assert_eq!(status, StatusCode::CREATED, "{role}");
        role["id"].as_str().unwrap().to_string()
    }

    /// Đặt luật của vai: mỗi cặp (cột, giá trị) là một luật trên bảng dataco.
    async fn set_rules(&self, role: &str, rules: &[(&str, &[&str])]) -> Value {
        let policies: Vec<Value> = rules
            .iter()
            .map(|(column, values)| json!({"table_id": self.table, "column": column, "allowed_values": values}))
            .collect();
        let path = format!("/data-roles/{role}/policies");
        let (status, body) = self
            .admin("PUT", &path, Some(json!({"policies": policies})))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn give(&self, user_id: &str, roles: &[&str]) {
        let path = format!("/members/{user_id}/data-roles");
        let (status, body) = self
            .admin("PUT", &path, Some(json!({"role_ids": roles})))
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    }

    /// Các dòng (thị trường, doanh thu) của câu hỏi mẫu, hỏi bằng phiên session.
    async fn ask(&self, session: &str) -> Value {
        let body = json!({"question": QUESTION, "choices": []});
        let (status, _, answer) = call(
            &self.asking.app,
            "POST",
            &self.asking.ask_url,
            Some(session),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["status"], "answered", "{answer}");
        answer
    }
}

fn rows(answer: &Value) -> &Value {
    &answer["result"]["rows"]
}

fn filter_labels(answer: &Value) -> Vec<String> {
    answer["understanding"]["filters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["label"].as_str().unwrap().to_string())
        .collect()
}

/// Lan (admin) và Minh (member) trong workspace có bảng dataco nạp từ CSV.
async fn start() -> Option<Roles> {
    let asking = start_on(None, CSV, |_| {}).await?;
    let user = json!({"email": "minh@example.com", "password": "a long password"});
    let (status, _, _) = call(
        &asking.app,
        "POST",
        "/api/admin/users",
        Some(&asking.session),
        Some(user),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "minh@example.com", "role": "member"});
    let url = format!("{WS}/members");
    let (status, _, _) = call(&asking.app, "POST", &url, Some(&asking.session), Some(add)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": "a long password"});
    let (_, minh, _) = call(&asking.app, "POST", "/api/auth/login", None, Some(login)).await;
    let (_, _, members) = call(&asking.app, "GET", &url, Some(&asking.session), None).await;
    let id_of = |email: &str| {
        members
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["email"] == email)
            .map(|m| m["user_id"].as_str().unwrap().to_string())
            .unwrap()
    };
    let (minh_id, lan_id) = (id_of("minh@example.com"), id_of("lan@example.com"));
    let table = asking
        .ask_url
        .trim_end_matches("/ask")
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    Some(Roles {
        minh: minh.unwrap(),
        minh_id,
        lan_id,
        table,
        asking,
    })
}

/// Member nhận 403 ở mọi route quản lý vai nhưng xem được vai của chính mình; admin đặt luật với kiểm bảng, cột, giá trị.
#[tokio::test]
async fn admin_manages_roles_and_member_is_refused() {
    let Some(r) = start().await else {
        return;
    };
    let role = r.create_role("Miền Nam").await;
    let table_path = format!("/datasets/{}/filter-columns", r.table);
    let refused = [
        ("GET", "/data-roles".to_string(), None),
        (
            "POST",
            "/data-roles".to_string(),
            Some(json!({"name": "Khác"})),
        ),
        (
            "PATCH",
            format!("/data-roles/{role}"),
            Some(json!({"name": "Khác"})),
        ),
        ("DELETE", format!("/data-roles/{role}"), None),
        (
            "PUT",
            format!("/data-roles/{role}/policies"),
            Some(json!({"policies": []})),
        ),
        (
            "PUT",
            format!("/members/{}/data-roles", r.minh_id),
            Some(json!({"role_ids": []})),
        ),
        ("GET", table_path.clone(), None),
    ];
    for (method, path, body) in refused {
        let (status, body) = r.send(&r.minh, method, &path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
    }

    // Cột dùng được là cột chiều, kèm giá trị của hồ sơ; cột số đo không có.
    let (status, columns) = r.admin("GET", &table_path, None).await;
    assert_eq!(status, StatusCode::OK, "{columns}");
    let market = columns
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Market")
        .unwrap();
    assert_eq!(market["values"], json!(["Europe", "LATAM"]));
    assert!(!columns.to_string().contains("Order Item Total"));

    let (status, taken) = r
        .admin("POST", "/data-roles", Some(json!({"name": " miền nam "})))
        .await;
    assert_eq!(
        (status, taken["field"].clone()),
        (StatusCode::BAD_REQUEST, json!("name")),
        "{taken}"
    );
    let path = format!("/data-roles/{role}/policies");
    let wrong = [
        json!({"table_id": r.table, "column": "Order Item Total", "allowed_values": ["1"]}),
        json!({"table_id": r.table, "column": "Missing", "allowed_values": ["1"]}),
        json!({"table_id": uuid::Uuid::new_v4(), "column": "Market", "allowed_values": ["Europe"]}),
    ];
    for policy in wrong {
        let (status, body) = r
            .admin("PUT", &path, Some(json!({"policies": [policy]})))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let twice = json!({"policies": [
        {"table_id": r.table, "column": "Market", "allowed_values": ["Europe"]},
        {"table_id": r.table, "column": "Market", "allowed_values": ["LATAM"]}]});
    assert_eq!(
        r.admin("PUT", &path, Some(twice)).await.0,
        StatusCode::BAD_REQUEST
    );

    // Vai chưa có luật không gán được.
    let path_minh = format!("/members/{}/data-roles", r.minh_id);
    let (status, body) = r
        .admin("PUT", &path_minh, Some(json!({"role_ids": [role]})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Giá trị lạ vẫn lưu, kèm lời báo; giá trị được bỏ khoảng trắng, bỏ lặp, sắp xếp.
    let saved = r
        .set_rules(&role, &[("Market", &[" Europe", "Asia", "Europe"])])
        .await;
    assert_eq!(
        saved["policies"][0]["allowed_values"],
        json!(["Asia", "Europe"])
    );
    let warnings = saved["warnings"].as_array().unwrap();
    assert!(
        warnings.len() == 1 && warnings[0].as_str().unwrap().contains("Asia"),
        "{saved}"
    );

    // Vai thứ hai trùng bộ luật bị từ chối.
    let copy = r.create_role("Bản sao").await;
    let body = json!({"policies": [{"table_id": r.table, "column": "Market", "allowed_values": ["Europe", "Asia"]}]});
    let (status, body) = r
        .admin("PUT", &format!("/data-roles/{copy}/policies"), Some(body))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    r.give(&r.minh_id, &[&role]).await;
    let (status, mine) = r.send(&r.minh, "GET", "/data-roles/mine", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine.as_array().unwrap().len(), 1);
    assert_eq!(mine[0]["name"], "Miền Nam");
    let (_, list) = r.admin("GET", "/data-roles", None).await;
    let listed = list
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == role.as_str())
        .unwrap();
    assert_eq!(listed["member_ids"], json!([r.minh_id]));
    assert_eq!(listed["policies"][0]["column"], "Market");

    // Vai đang gán không xóa được và không được mất hết luật.
    let (status, _) = r
        .admin("DELETE", &format!("/data-roles/{role}"), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = r.admin("PUT", &path, Some(json!({"policies": []}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, renamed) = r
        .admin(
            "PATCH",
            &format!("/data-roles/{role}"),
            Some(json!({"name": "Châu Âu", "description": "EU"})),
        )
        .await;
    assert_eq!(
        (status, renamed["name"].clone()),
        (StatusCode::OK, json!("Châu Âu"))
    );
    let (status, _) = r
        .admin("DELETE", &format!("/data-roles/{copy}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    r.asking.db.drop().await;
}

/// Cùng câu hỏi: admin thấy mọi thị trường; Minh chỉ thấy phần vai cho phép, thẻ "Đã hiểu là" có mục lọc theo quyền
/// xem; đổi luật hay vai có hiệu lực từ request kế tiếp; AND trong một vai, OR giữa hai vai; admin mang vai vẫn thấy hết.
#[tokio::test]
async fn member_with_roles_sees_only_allowed_rows() {
    let Some(r) = start().await else {
        return;
    };
    let all = json!([["LATAM", "200"], ["Europe", "150"]]);
    assert_eq!(rows(&r.ask(&r.asking.session).await), &all);
    assert_eq!(rows(&r.ask(&r.minh).await), &all);

    let europe = r.create_role("Châu Âu").await;
    r.set_rules(&europe, &[("Market", &["Europe"])]).await;
    r.give(&r.minh_id, &[&europe]).await;
    let answer = r.ask(&r.minh).await;
    assert_eq!(rows(&answer), &json!([["Europe", "150"]]));
    let labels = filter_labels(&answer);
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("Lọc theo quyền xem: ") && l.ends_with("(Market) là Europe")),
        "{labels:?}"
    );

    // Đổi luật: request kế tiếp đã theo luật mới.
    r.set_rules(&europe, &[("Market", &["LATAM"])]).await;
    assert_eq!(rows(&r.ask(&r.minh).await), &json!([["LATAM", "200"]]));

    // AND trong một vai: Europe và vùng Western Europe.
    r.set_rules(
        &europe,
        &[
            ("Market", &["Europe"]),
            ("Order Region", &["Western Europe"]),
        ],
    )
    .await;
    assert_eq!(rows(&r.ask(&r.minh).await), &json!([["Europe", "100"]]));

    // OR giữa hai vai: thêm vai LATAM thì thấy cả dòng LATAM.
    let latam = r.create_role("Mỹ Latinh").await;
    r.set_rules(&latam, &[("Market", &["LATAM"])]).await;
    r.give(&r.minh_id, &[&europe, &latam]).await;
    let answer = r.ask(&r.minh).await;
    assert_eq!(rows(&answer), &json!([["LATAM", "200"], ["Europe", "100"]]));
    let guarded = answer["sql_guarded"].as_str().unwrap();
    assert!(guarded.contains(" OR "), "{guarded}");
    let labels = filter_labels(&answer);
    assert!(labels.iter().any(|l| l.contains(" hoặc ")), "{labels:?}");

    // Admin mang vai vẫn không bị lọc; gỡ hết vai thì Minh thấy hết.
    r.give(&r.lan_id, &[&europe]).await;
    assert_eq!(rows(&r.ask(&r.asking.session).await), &all);
    r.give(&r.minh_id, &[]).await;
    assert_eq!(rows(&r.ask(&r.minh).await), &all);
    r.asking.db.drop().await;
}
