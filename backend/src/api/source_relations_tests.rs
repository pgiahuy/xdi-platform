// SPDX-License-Identifier: Apache-2.0
/* api/source_relations_tests.rs - test chụp một nhóm bảng liên quan (mỗi bảng một dataset, kèm quan hệ khóa ngoại) qua
route thật, trên Postgres thật (database riêng của test, như api/sources_tests.rs). Schema shop dựng tay đủ các ca: khóa
ngoại nhiều cột, chuỗi orders -> customers -> regions, hai bảng sự kiện (order_items, returns) cùng trỏ vào orders,
khóa ngoại trỏ vào bảng (một-nhiều), bảng đích chỉ có chỉ mục UNIQUE không kèm ràng buộc, bảng không có quyền đọc,
view. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::sources_tests::{start, start_with_max_rows, Setup, KEY, SHOP};
use crate::api::test_support::call;
use crate::engine::store::test_support::exec;

/// Các câu dựng schema shop; cấp quyền đọc cho tài khoản nguồn, trừ shop.secrets.
fn shop_schema(user: &str) -> Vec<String> {
    let statements = [
        "CREATE SCHEMA shop",
        "CREATE TABLE shop.regions (code text PRIMARY KEY, name text)",
        "CREATE TABLE shop.customers (id int PRIMARY KEY, name text, region_code text REFERENCES shop.regions (code))",
        "CREATE TABLE shop.products (maker text, sku int, title text, price numeric(10,2), PRIMARY KEY (maker, sku))",
        "CREATE TABLE shop.orders (id int PRIMARY KEY, customer_id int REFERENCES shop.customers (id), status text)",
        "CREATE TABLE shop.channels (code text, label text)",
        "CREATE UNIQUE INDEX channels_code ON shop.channels (code)",
        "CREATE TABLE shop.secrets (id int PRIMARY KEY, note text)",
        "CREATE TABLE shop.order_items (order_id int REFERENCES shop.orders (id), maker text, sku int, qty int,
             \"orders.status\" text, gift_customer_id int REFERENCES shop.customers (id),
             channel text REFERENCES shop.channels (code), secret_id int REFERENCES shop.secrets (id),
             FOREIGN KEY (maker, sku) REFERENCES shop.products (maker, sku))",
        "CREATE TABLE shop.returns (id int PRIMARY KEY, order_id int REFERENCES shop.orders (id))",
        "CREATE VIEW shop.big_items AS SELECT * FROM shop.order_items WHERE qty > 1",
        "INSERT INTO shop.regions VALUES ('N', 'North'), ('S', 'South')",
        "INSERT INTO shop.customers VALUES (1, 'An', 'N'), (2, 'Binh', 'S'), (3, 'Chi', NULL)",
        "INSERT INTO shop.products VALUES ('acme', 1, 'Pen', 2.50), ('acme', 2, 'Book', 10), ('zeta', 1, 'Lamp', 30)",
        "INSERT INTO shop.orders VALUES (10, 1, 'done'), (11, 2, 'open'), (12, 3, 'done')",
        "INSERT INTO shop.channels VALUES ('web', 'Web'), ('shop', 'Store')",
        "INSERT INTO shop.secrets VALUES (1, 'x')",
        "INSERT INTO shop.returns VALUES (1, 10), (2, 12)",
        "INSERT INTO shop.order_items VALUES
             (10, 'acme', 1, 4, 'raw', NULL, 'web', 1), (10, 'acme', 2, 1, 'raw', 3, 'web', NULL),
             (11, 'zeta', 1, 2, 'raw', NULL, 'shop', NULL), (12, 'acme', 1, 1, 'raw', 1, NULL, NULL),
             (12, 'acme', 2, 3, 'raw', NULL, 'web', NULL)",
    ];
    let mut all: Vec<String> = statements.iter().map(|s| s.to_string()).collect();
    all.push(format!("GRANT USAGE ON SCHEMA shop TO {user}"));
    for table in [
        "regions",
        "customers",
        "products",
        "orders",
        "channels",
        "order_items",
        "returns",
        "big_items",
    ] {
        all.push(format!("GRANT SELECT ON shop.{table} TO {user}"));
    }
    all
}

async fn add_source(s: &Setup) -> String {
    for sql in shop_schema(&s.user) {
        exec(&s.db.store, &sql).await;
    }
    let (status, _, source) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/sources"),
        Some(&s.lan),
        Some(s.form.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    source["id"].as_str().unwrap().to_string()
}

async fn relations(s: &Setup, id: &str, table: &str) -> Value {
    let (status, _, body) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/sources/{id}/relations"),
        Some(&s.lan),
        Some(json!({ "table": table })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn snapshot(s: &Setup, id: &str, body: Value) -> (StatusCode, Value) {
    let url = format!("{SHOP}/sources/{id}/snapshot");
    let (status, _, result) = call(&s.app, "POST", &url, Some(&s.lan), Some(body)).await;
    (status, result)
}

fn find<'a>(list: &'a Value, table: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|r| r["table"] == table)
        .unwrap_or_else(|| panic!("no table {table} in {list}"))
}

/// (tên dataset, phiên bản, created) của từng bảng trong kết quả chụp.
fn summary(result: &Value) -> Vec<(String, i64, bool)> {
    result["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["dataset"]["name"].as_str().unwrap().to_string(),
                d["version"]["version"].as_i64().unwrap(),
                d["created"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn dataset_id(result: &Value, name: &str) -> String {
    result["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["dataset"]["name"] == name)
        .unwrap_or_else(|| panic!("no dataset {name} in {result}"))["dataset"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Bảng liên quan theo cả hai chiều, khóa nhiều cột, lý do khi không chụp kèm được; view không có quan hệ; member 403.
#[tokio::test]
async fn relations_list_both_directions_with_reasons() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let body = relations(&s, &id, "shop.order_items").await;
    assert_eq!(body["column_count"], 8, "{body}");
    let many = &body["many_to_one"];

    let products = find(many, "shop.products");
    assert_eq!(products["from_columns"], json!(["maker", "sku"]));
    assert_eq!(products["to_columns"], json!(["maker", "sku"]));
    assert_eq!(products["addable"], true);
    assert_eq!(products["column_count"], 4);

    let channel = find(many, "shop.channels");
    assert_eq!(channel["addable"], false);
    assert!(
        channel["reason"].as_str().unwrap().contains("UNIQUE"),
        "{channel}"
    );
    let secret = find(many, "shop.secrets");
    assert_eq!(secret["addable"], false);
    assert!(
        secret["reason"].as_str().unwrap().contains("quyền đọc"),
        "{secret}"
    );
    assert!(body["one_to_many"].as_array().unwrap().is_empty());

    // orders: trỏ tới customers, và có hai bảng trỏ vào (order_items, returns), cả hai chụp kèm được.
    let orders = relations(&s, &id, "shop.orders").await;
    assert_eq!(
        find(&orders["many_to_one"], "shop.customers")["addable"],
        true
    );
    let incoming: Vec<&str> = orders["one_to_many"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["table"].as_str().unwrap())
        .collect();
    assert_eq!(incoming, ["shop.order_items", "shop.returns"]);
    assert_eq!(orders["one_to_many"][0]["addable"], true);
    assert_eq!(orders["one_to_many"][0]["to_table"], "shop.orders");

    let view = relations(&s, &id, "shop.big_items").await;
    assert!(view["many_to_one"].as_array().unwrap().is_empty(), "{view}");

    let (status, _, _) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/sources/{id}/relations"),
        Some(&s.minh),
        Some(json!({"table": "shop.order_items"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    s.finish().await;
}

/// Ba bảng nối nhau thành ba dataset và hai quan hệ; member thấy quan hệ trong danh sách dataset; chụp lại từ một bảng
/// của nhóm chụp lại cả nhóm.
#[tokio::test]
async fn related_tables_become_datasets_with_relations() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let body = json!({"table": "shop.orders", "tables": ["shop.customers", "shop.regions"]});
    let (status, result) = snapshot(&s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(
        summary(&result),
        [
            ("orders".to_string(), 1, true),
            ("customers".to_string(), 1, true),
            ("regions".to_string(), 1, true)
        ]
    );
    assert_eq!(result["datasets"][0]["version"]["row_count"], 3);
    assert_eq!(
        result["datasets"][0]["dataset"]["source_table"],
        "shop.orders"
    );
    let pairs: Vec<(&str, &str)> = result["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["from_dataset_name"].as_str().unwrap(),
                r["to_dataset_name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(pairs, [("customers", "regions"), ("orders", "customers")]);
    assert_eq!(
        result["relations"][1]["from_columns"],
        json!(["customer_id"])
    );
    assert_eq!(result["relations"][1]["cardinality"], "many_to_one");

    // Member đọc được quan hệ qua danh sách dataset, nhìn từ phía từng dataset.
    let (status, _, list) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/datasets"),
        Some(&s.minh),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let customers = list
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "customers")
        .unwrap();
    let related: Vec<(&str, &str)> = customers["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap(),
                r["direction"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        related,
        [("regions", "many_to_one"), ("orders", "one_to_many")]
    );

    // Chụp lại từ regions, không đổi gì: cả nhóm được đọc, không tạo phiên bản nào, trả 200.
    let regions = dataset_id(&result, "regions");
    let (status, same) = snapshot(&s, &id, json!({"dataset_id": regions})).await;
    assert_eq!(status, StatusCode::OK, "{same}");
    assert_eq!(same["datasets"].as_array().unwrap().len(), 3);
    assert!(summary(&same)
        .iter()
        .all(|(_, v, created)| *v == 1 && !created));

    // Đổi cả ba bảng rồi chụp lại: mỗi dataset một phiên bản 2, quan hệ giữ nguyên.
    for sql in [
        "INSERT INTO shop.regions VALUES ('E', 'East')",
        "INSERT INTO shop.customers VALUES (4, 'Dung', 'E')",
        "INSERT INTO shop.orders VALUES (13, 4, 'open')",
    ] {
        exec(&s.db.store, sql).await;
    }
    let (status, next) = snapshot(&s, &id, json!({"dataset_id": regions})).await;
    assert_eq!(status, StatusCode::CREATED, "{next}");
    let mut versions = summary(&next);
    versions.sort();
    assert_eq!(
        versions,
        [
            ("customers".to_string(), 2, true),
            ("orders".to_string(), 2, true),
            ("regions".to_string(), 2, true)
        ]
    );
    assert_eq!(next["datasets"][0]["dataset"]["id"], regions.as_str());
    assert_eq!(next["relations"].as_array().unwrap().len(), 2);

    // Chụp lại kèm bảng khác: từ chối, nhóm lấy từ quan hệ đã ghi.
    let (status, _) = snapshot(
        &s,
        &id,
        json!({"dataset_id": regions, "tables": ["shop.orders"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    s.finish().await;
}

/// orders là bảng chiều chung của order_items và returns: chụp hai nhóm thì orders vẫn là một dataset, mang quan hệ
/// với cả hai; quan hệ cũ của orders với customers (nhóm khác) giữ nguyên.
#[tokio::test]
async fn shared_dimension_is_reused() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let first = json!({"table": "shop.orders", "tables": ["shop.customers"]});
    let (status, result) = snapshot(&s, &id, first).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    let orders = dataset_id(&result, "orders");

    let items = json!({"table": "shop.order_items", "tables": ["shop.orders", "shop.products"]});
    let (status, result) = snapshot(&s, &id, items).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    // orders không đổi: cùng dataset, không tạo phiên bản mới.
    assert_eq!(dataset_id(&result, "orders"), orders);
    assert_eq!(summary(&result)[1], ("orders".to_string(), 1, false));
    let names: Vec<&str> = result["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["to_dataset_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["products", "orders"]);

    let returns = json!({"table": "shop.returns", "tables": ["shop.orders"]});
    let (status, result) = snapshot(&s, &id, returns).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(dataset_id(&result, "orders"), orders);

    let (_, _, list) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/datasets"),
        Some(&s.lan),
        None,
    )
    .await;
    let list = list.as_array().unwrap();
    assert_eq!(list.iter().filter(|d| d["name"] == "orders").count(), 1);
    assert_eq!(list.len(), 5);
    let orders_row = list.iter().find(|d| d["id"] == orders.as_str()).unwrap();
    let mut related: Vec<&str> = orders_row["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    related.sort();
    assert_eq!(related, ["customers", "order_items", "returns"]);
    s.finish().await;
}

/// Quá 10 bảng thì từ chối trước khi kết nối; giới hạn dòng tính riêng từng bảng, báo đúng bảng vượt; member 403.
#[tokio::test]
async fn snapshot_limits_and_roles() {
    let Some(s) = start_with_max_rows(Some(KEY), &[], Some(4)).await else {
        return;
    };
    let id = add_source(&s).await;
    let many: Vec<String> = (0..10).map(|i| format!("shop.t{i}")).collect();
    let (status, body) = snapshot(&s, &id, json!({"table": "shop.orders", "tables": many})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("tối đa 10"),
        "{body}"
    );

    // orders 3 dòng, customers 3 dòng: mỗi bảng dưới 4 thì chụp được, dù cộng lại quá 4.
    let ok = json!({"table": "shop.orders", "tables": ["shop.customers"]});
    let (status, body) = snapshot(&s, &id, ok).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // order_items 5 dòng: cả lần chụp bị từ chối, lời báo nêu bảng đó.
    let big = json!({"table": "shop.orders", "tables": ["shop.order_items"]});
    let (status, body) = snapshot(&s, &id, big).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("shop.order_items"),
        "{body}"
    );

    let url = format!("{SHOP}/sources/{id}/snapshot");
    let body = json!({"table": "shop.orders", "tables": ["shop.customers"]});
    let (status, _, _) = call(&s.app, "POST", &url, Some(&s.minh), Some(body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    s.finish().await;
}
