// SPDX-License-Identifier: Apache-2.0
/* api/drill_tests.rs - test truy dòng nguồn (POST .../datasets/{id}/drill) qua router thật, với Postgres: đọc câu hỏi
theo workspace và dataset, quyền của người đang bấm, phiên bản của câu hỏi, các lỗi 400 và 404. Phần dựng SQL và các
dạng câu ở service/drill/tests.rs. Mỗi test dùng database riêng.

Test ghi thẳng dòng queries bằng SQL, với sql_guarded như guard in ra, thay vì hỏi qua route ask: chọn được đúng câu
cần thử (UNION, câu bị từ chối) mà không phụ thuộc câu trả lời mẫu của mock. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::{fetch_text, fresh_database, give_data_role};
use crate::engine::store::Store;

const PASSWORD: &str = "a long password";
const SHOP: &str = "/api/workspaces/cua-hang";
const SALES: &[u8] = b"Market,amount\nEurope,10\nLATAM,5\nEurope,7\nUSCA,1\nLATAM,2\n";
const BY_MARKET: &str =
    "SELECT \"Market\", SUM(\"amount\") AS revenue FROM ban_hang GROUP BY \"Market\" \
                         LIMIT 100000";

/// Thư mục dữ liệu riêng cho một test; test tự xóa khi xong.
fn temp_data_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xdi-drill-api-{}", uuid::Uuid::new_v4()));
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
    let members = format!("{SHOP}/members");
    let (status, _, _) = call(app, "POST", &members, Some(&lan), Some(add)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": PASSWORD});
    let (_, minh, _) = call(app, "POST", "/api/auth/login", None, Some(login)).await;
    (lan, minh.unwrap())
}

/// Nạp một file CSV vào workspace; fields thêm dataset_id để nạp phiên bản mới. Trả id của dataset.
async fn upload(
    app: &axum::Router,
    session: &str,
    ws: &str,
    fields: &[(&str, &str)],
    csv: &[u8],
) -> String {
    let (content_type, body) = multipart(fields, &[("sales.csv", csv)]);
    let (status, body) =
        send_raw(app, &format!("{ws}/datasets"), session, &content_type, body).await;
    assert!(status.is_success(), "{status} {body}");
    body["dataset"]["id"].as_str().unwrap().to_string()
}

/// Ghi một câu hỏi trên phiên bản hiện hành của dataset như luồng hỏi sẽ ghi; trả query_id.
async fn record(
    store: &Store,
    dataset_id: &str,
    status: &str,
    sql_guarded: Option<&str>,
) -> String {
    let sql = sql_guarded.map_or("NULL".to_string(), |s| {
        format!("'{}'", s.replace('\'', "''"))
    });
    fetch_text(
        store,
        &format!(
            "INSERT INTO queries (workspace_id, dataset_id, version_id, question, sql_guarded, status)
             SELECT workspace_id, id, current_version_id, 'doanh thu theo thị trường', {sql}, '{status}'
             FROM datasets WHERE id = '{dataset_id}' RETURNING id::text"
        ),
    )
    .await
}

fn cell(query_id: &str, market: &str) -> Value {
    json!({"query_id": query_id, "cell": [{"column": "Market", "value": market}]})
}

async fn drill(
    app: &axum::Router,
    session: &str,
    ws: &str,
    dataset_id: &str,
    body: Value,
) -> (StatusCode, Value) {
    let path = format!("{ws}/datasets/{dataset_id}/drill");
    let (status, _, body) = call(app, "POST", &path, Some(session), Some(body)).await;
    (status, body)
}

#[tokio::test]
async fn member_drills_a_cell_with_their_own_row_filters() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, minh) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan, SHOP, &[("name", "Bán hàng")], SALES).await;
    // Câu do admin hỏi.
    let query_id = record(&db.store, &dataset_id, "ok", Some(BY_MARKET)).await;

    let (status, body) = drill(&app, &lan, SHOP, &dataset_id, cell(&query_id, "Europe")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total_rows"], 2);
    let columns: Vec<&str> = body["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(columns, ["Market", "amount", "_source_file"]);
    assert_eq!(
        body["result"]["rows"],
        // Trang sắp theo mọi cột (ORDER BY ALL): amount 7 trước 10.
        json!([["Europe", "7", "sales.csv"], ["Europe", "10", "sales.csv"]])
    );

    // Minh có vai dữ liệu chỉ thấy LATAM: bấm ô Europe của câu admin hỏi thì không thấy dòng nào.
    give_data_role(
        &db.store,
        "minh@example.com",
        "latam",
        &[(&dataset_id, "Market", &["LATAM"])],
    )
    .await;
    let (status, body) = drill(&app, &minh, SHOP, &dataset_id, cell(&query_id, "Europe")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["total_rows"].clone(), body["result"]["rows"].clone()),
        (json!(0), json!([]))
    );
    assert!(
        body["sql"].as_str().unwrap().contains("IN ('LATAM')"),
        "{body}"
    );
    let (_, body) = drill(&app, &minh, SHOP, &dataset_id, cell(&query_id, "LATAM")).await;
    assert_eq!(body["total_rows"], 2);
    // Admin không bị lọc dù có chính sách.
    let (_, body) = drill(&app, &lan, SHOP, &dataset_id, cell(&query_id, "Europe")).await;
    assert_eq!(body["total_rows"], 2);

    // Nạp phiên bản mới: câu hỏi cũ vẫn truy trên phiên bản của nó.
    let more = b"Market,amount\nEurope,100\nEurope,200\nEurope,300\n";
    upload(&app, &lan, SHOP, &[("dataset_id", &dataset_id)], more).await;
    let (_, body) = drill(&app, &lan, SHOP, &dataset_id, cell(&query_id, "Europe")).await;
    assert_eq!(body["total_rows"], 2, "{body}");

    db.drop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn questions_that_cannot_be_drilled_are_refused() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, _) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan, SHOP, &[("name", "Bán hàng")], SALES).await;
    let answered = record(&db.store, &dataset_id, "ok", Some(BY_MARKET)).await;
    let rejected = record(&db.store, &dataset_id, "rejected", None).await;
    let failed = record(&db.store, &dataset_id, "error", Some(BY_MARKET)).await;
    let union = "SELECT \"Market\", SUM(\"amount\") AS revenue FROM ban_hang GROUP BY \"Market\" \
                 UNION ALL SELECT 'all', SUM(\"amount\") FROM ban_hang LIMIT 100000";
    let union = record(&db.store, &dataset_id, "ok", Some(union)).await;
    let breakdown = json!({"slice": "compare", "column": "Market", "value": "Europe"});

    let cases = [
        (cell(&rejected, "Europe"), "bị guard từ chối"),
        (cell(&failed, "Europe"), "chạy lỗi"),
        (
            cell(&union, "Europe"),
            "Câu hỏi này chưa truy được dòng nguồn: câu ghép nhiều câu SELECT",
        ),
        (
            // Câu thường không có bảng phân rã.
            json!({"query_id": answered, "breakdown": breakdown}),
            "không có bảng phân rã",
        ),
        (
            json!({"query_id": answered, "cell": [], "breakdown": breakdown}),
            "Gửi đúng một trong hai",
        ),
        (json!({"query_id": answered}), "Gửi đúng một trong hai"),
        (
            json!({"query_id": answered, "cell": [], "limit": 1001}),
            "limit phải từ 1 tới 1000",
        ),
    ];
    for (body, expected) in cases {
        let (status, res) = drill(&app, &lan, SHOP, &dataset_id, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {res}");
        let message = res["message"].as_str().unwrap();
        assert!(message.contains(expected), "{body}: {message}");
    }

    db.drop().await;
    let _ = std::fs::remove_dir_all(&data);
}

#[tokio::test]
async fn question_of_another_dataset_or_workspace_is_not_found() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = temp_data_dir();
    let app = app_with(db.store.clone(), Some(CODE), |c| c.data_dir = data.clone());
    let (lan, _) = lan_and_minh(&app).await;
    let dataset_id = upload(&app, &lan, SHOP, &[("name", "Bán hàng")], SALES).await;
    let other_dataset = upload(
        &app,
        &lan,
        SHOP,
        &[("name", "Kho")],
        b"Market,amount\nEurope,1\n",
    )
    .await;
    let query_id = record(&db.store, &dataset_id, "ok", Some(BY_MARKET)).await;
    // Workspace thứ hai của Lan, có dataset riêng.
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
    let second_dataset = upload(&app, &lan, &second, &[("name", "Bán hàng")], SALES).await;

    let cases = [
        (SHOP, other_dataset.as_str(), query_id.clone()),
        (second.as_str(), second_dataset.as_str(), query_id.clone()),
        (SHOP, dataset_id.as_str(), uuid::Uuid::new_v4().to_string()),
        (SHOP, "not-a-uuid", query_id.clone()),
    ];
    for (ws, dataset, query) in cases {
        let (status, body) = drill(&app, &lan, ws, dataset, cell(&query, "Europe")).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{ws} {dataset} {query}: {body}"
        );
    }

    db.drop().await;
    let _ = std::fs::remove_dir_all(&data);
}
