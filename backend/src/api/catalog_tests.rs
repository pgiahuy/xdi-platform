// SPDX-License-Identifier: Apache-2.0
/* api/catalog_tests.rs - test dataset là một database hay một file, bảng nằm bên trong (#257), qua route thật trên
Postgres thật: liệt kê dataset kèm bảng, cột, quan hệ; tên bảng logic chỉ không trùng trong một dataset; Parquet ghi
theo <workspace>/<dataset>/<bảng>/<phiên bản>; chụp lại cả dataset; hỏi trên dataset (bảng do backend chọn hay ghim
bằng table_id) và cuộc trò chuyện trên dataset; member chỉ xem và hỏi. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::start_on;
use super::source_relations_tests::{add_source, snapshot};
use super::sources_tests::{start, KEY, SHOP};
use crate::api::test_support::{call, multipart, send_raw};
use crate::engine::store::test_support::fetch_text;

const CSV: &str = "Market,Order Region,Order Item Total\n\
                   Europe,Western Europe,100.5\n\
                   LATAM,South America,200\n\
                   Europe,Northern Europe,50\n";

async fn get(app: &axum::Router, session: &str, path: &str) -> (StatusCode, Value) {
    let (status, _, body) = call(app, "GET", &format!("{SHOP}{path}"), Some(session), None).await;
    (status, body)
}

async fn post(app: &axum::Router, session: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let url = format!("{SHOP}{path}");
    let (status, _, body) = call(app, "POST", &url, Some(session), Some(body)).await;
    (status, body)
}

fn logical_tables(dataset: &Value) -> Vec<&str> {
    dataset["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["logical_table"].as_str().unwrap())
        .collect()
}

/// Một file là một dataset có một bảng; hỏi trên dataset không cần chọn bảng.
#[tokio::test]
async fn file_dataset_has_one_table_and_answers_questions() {
    let Some(asking) = start_on(None, CSV, |_| {}).await else {
        return;
    };
    let (app, lan) = (&asking.app, asking.session.as_str());
    let (status, list) = get(app, lan, "/catalog").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    let dataset = &list[0];
    assert_eq!(dataset["kind"], "file");
    assert_eq!(dataset["name"], "dataco");
    assert_eq!(logical_tables(dataset), ["dataco"]);
    let table = &dataset["tables"][0];
    assert_eq!(table["row_count"], 3);
    assert!(table["schema"].is_null());
    let columns: Vec<&str> = table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        columns,
        ["Market", "Order Region", "Order Item Total", "_source_file"]
    );
    let dataset_id = dataset["id"].as_str().unwrap();
    let table_id = table["id"].as_str().unwrap();

    // Parquet của phiên bản mới nằm dưới thư mục của dataset rồi của bảng.
    let path = fetch_text(
        &asking.db.store,
        "SELECT parquet_path FROM dataset_versions LIMIT 1",
    )
    .await;
    let parts: Vec<&str> = path.split('/').collect();
    assert_eq!(parts.len(), 4, "{path}");
    assert_eq!((parts[1], parts[2]), (dataset_id, table_id), "{path}");

    // Hỏi trên dataset: backend chọn bảng duy nhất, trả table_id để giao diện mở truy dòng nguồn.
    let ask = format!("/catalog/{dataset_id}/ask");
    let question = json!({"question": "Doanh thu theo thị trường"});
    let (status, first) = post(app, lan, &ask, question).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "answered", "{first}");
    assert_eq!(first["table_id"], table_id);
    let conversation = first["conversation_id"].clone();
    let follow = json!({"question": "Doanh thu theo thị trường", "conversation_id": conversation,
                        "table_id": table_id});
    let (status, second) = post(app, lan, &ask, follow).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["conversation_id"], conversation);
    let (status, list) = get(app, lan, &format!("/catalog/{dataset_id}/conversations")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["conversations"][0]["id"], conversation);
    assert_eq!(list["conversations"][0]["table_id"], table_id);

    // Bảng không thuộc dataset, dataset không có: 404. File không chụp lại được.
    let stray = json!({"question": "x", "table_id": dataset_id});
    assert_eq!(post(app, lan, &ask, stray).await.0, StatusCode::NOT_FOUND);
    let missing = format!("/catalog/{}", uuid::Uuid::new_v4());
    assert_eq!(get(app, lan, &missing).await.0, StatusCode::NOT_FOUND);
    let (status, body) = post(
        app,
        lan,
        &format!("/catalog/{dataset_id}/snapshot"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // File thứ hai cùng tên là dataset khác, giữ đúng tên bảng logic dataco (trước đây thành dataco_2).
    let other = CSV.replace("200", "201");
    let (content_type, form) = multipart(&[("name", "dataco")], &[("b.csv", other.as_bytes())]);
    let url = format!("{SHOP}/datasets");
    let (status, uploaded) = send_raw(app, &url, lan, &content_type, form).await;
    assert_eq!(status, StatusCode::CREATED, "{uploaded}");
    assert_eq!(uploaded["dataset"]["logical_table"], "dataco");
    let (_, list) = get(app, lan, "/catalog").await;
    assert_eq!(list.as_array().unwrap().len(), 2, "{list}");
    asking.db.drop().await;
}

/// Một database là một dataset: mọi bảng đã chụp nằm trong nó, kèm quan hệ; chụp lại cả dataset trong một lần.
#[tokio::test]
async fn database_dataset_groups_tables_and_snapshots_again() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let group = json!({"table": "shop.order_items", "tables": ["shop.orders", "shop.products"]});
    let (status, body) = snapshot(&s, &id, group).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = snapshot(&s, &id, json!({"table": "shop.returns"})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // Member xem được; bảng xếp theo schema rồi tên; quan hệ chỉ có giữa các bảng chụp cùng lần.
    let (status, list) = get(&s.app, &s.minh, "/catalog").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let dataset = &list[0];
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    assert_eq!(dataset["kind"], "database");
    assert_eq!(dataset["id"], id.as_str());
    assert_eq!(dataset["name"], s.db.name());
    assert_eq!(
        logical_tables(dataset),
        ["order_items", "orders", "products", "returns"]
    );
    assert_eq!(dataset["tables"][1]["schema"], "shop");
    assert_eq!(dataset["tables"][1]["source_table"], "shop.orders");
    assert_eq!(
        dataset["relations"].as_array().unwrap().len(),
        2,
        "{dataset}"
    );

    // Chụp lại cả dataset: member 403; admin nhận mọi bảng, không bảng nào đổi, và quan hệ returns -> orders có thêm
    // vì lần này hai bảng cùng một lần chụp.
    let again = format!("/catalog/{id}/snapshot");
    assert_eq!(
        post(&s.app, &s.minh, &again, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, result) = post(&s.app, &s.lan, &again, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let created: Vec<bool> = result["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["created"].as_bool().unwrap())
        .collect();
    assert_eq!(created, [false, false, false, false], "{result}");
    let (_, detail) = get(&s.app, &s.minh, &format!("/catalog/{id}")).await;
    assert_eq!(detail["relations"].as_array().unwrap().len(), 3, "{detail}");

    // Database thứ hai cùng có bảng orders: giữ tên orders, không thành orders_2.
    let (status, source) = post(&s.app, &s.lan, "/sources", s.form.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let second = source["id"].as_str().unwrap().to_string();
    let (status, body) = snapshot(&s, &second, json!({"table": "shop.orders"})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["datasets"][0]["dataset"]["logical_table"], "orders");
    s.db.drop().await;
}
