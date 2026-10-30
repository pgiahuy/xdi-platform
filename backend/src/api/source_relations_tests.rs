// SPDX-License-Identifier: Apache-2.0
/* api/source_relations_tests.rs - test chụp bảng chính kèm bảng nối nhiều-một qua route thật, trên Postgres thật (database
riêng của test, như api/sources_tests.rs). Schema shop dựng tay đủ các ca: khóa ngoại nhiều cột, chuỗi hai bước
(order_items -> orders -> customers), hai đường cùng tới customers (tiền tố customers_2), cột của bảng chính trùng tên
cột nối ("orders.status"), khóa ngoại trỏ vào bảng (một-nhiều, không nối), bảng đích chỉ có chỉ mục UNIQUE không kèm ràng
buộc, bảng đích không có quyền đọc, view làm bảng chính. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::sql_reply;
use super::sources_tests::{start, Setup, KEY, SHOP};
use crate::api::test_support::call;
use crate::engine::store::test_support::{exec, fetch_text};

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

fn find<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no relation {id} in {list}"))
}

const ORDERS: &str = "shop.order_items/order_items_order_id_fkey";
const PRODUCTS: &str = "shop.order_items/order_items_maker_sku_fkey";
const GIFT: &str = "shop.order_items/order_items_gift_customer_id_fkey";
const CHANNEL: &str = "shop.order_items/order_items_channel_fkey";
const SECRET: &str = "shop.order_items/order_items_secret_id_fkey";
const ORDER_CUSTOMER: &str =
    "shop.order_items/order_items_order_id_fkey>shop.orders/orders_customer_id_fkey";

/// Danh sách quan hệ: nhiều-một bước 1 và 2, khóa nhiều cột, lý do không nối; một-nhiều liệt kê riêng; view không có
/// quan hệ nào.
#[tokio::test]
async fn relations_list_many_to_one_and_explain_the_rest() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let body = relations(&s, &id, "shop.order_items").await;
    let many = &body["many_to_one"];
    assert_eq!(body["column_count"], 8, "{body}");

    let products = find(many, PRODUCTS);
    assert_eq!(products["from_columns"], json!(["maker", "sku"]));
    assert_eq!(products["to_columns"], json!(["maker", "sku"]));
    assert_eq!(products["joinable"], true);
    // title, price: cột khóa đích không chép lại.
    assert_eq!(products["column_count"], 2);

    let chain = find(many, ORDER_CUSTOMER);
    assert_eq!(chain["depth"], 2);
    assert_eq!(chain["parent"], ORDERS);
    assert_eq!(chain["to_table"], "shop.customers");

    let channel = find(many, CHANNEL);
    assert_eq!(channel["joinable"], false);
    assert!(
        channel["reason"].as_str().unwrap().contains("UNIQUE"),
        "{channel}"
    );
    let secret = find(many, SECRET);
    assert_eq!(secret["joinable"], false);
    assert!(
        secret["reason"].as_str().unwrap().contains("quyền đọc"),
        "{secret}"
    );
    assert!(body["one_to_many"].as_array().unwrap().is_empty());

    // orders có hai bảng trỏ vào: order_items và returns, đều một-nhiều.
    let orders = relations(&s, &id, "shop.orders").await;
    let incoming: Vec<&str> = orders["one_to_many"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["from_table"].as_str().unwrap())
        .collect();
    assert_eq!(incoming, ["shop.order_items", "shop.returns"]);
    assert!(orders["one_to_many"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("nhân dòng"));

    let view = relations(&s, &id, "shop.big_items").await;
    assert!(view["many_to_one"].as_array().unwrap().is_empty(), "{view}");

    // Member không xem được quan hệ, như mọi route của nguồn.
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

/// Chụp kèm bảng nối: số dòng giữ nguyên, tên cột theo tiền tố, mô hình ghi lại và member đọc được, cột từ bảng nối
/// không còn là measure, hỏi được, chụp lại nối đúng như lần trước.
#[tokio::test]
async fn snapshot_with_relations_keeps_rows_and_records_the_model() {
    let sql =
        r#"SELECT "customers_2.name", SUM(qty) AS qty FROM order_items GROUP BY 1 ORDER BY 1"#;
    // Câu hai cộng giá sản phẩm ngay trên bảng dòng hàng (cộng lặp): không chạy mà đi vòng sửa; câu sửa giữ một dòng
    // mỗi sản phẩm rồi mới cộng.
    let summed = r#"SELECT SUM("products.price") AS total FROM order_items"#;
    let fixed = r#"WITH one_row AS (SELECT DISTINCT "maker", "sku", "products.price" FROM order_items)
                   SELECT SUM("products.price") AS total FROM one_row"#;
    let replies = [sql_reply(sql), sql_reply(summed), sql_reply(fixed)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let id = add_source(&s).await;
    let url = format!("{SHOP}/sources/{id}/snapshot");

    // Id lạ hay quan hệ không nối được: từ chối cả lần chụp.
    for bad in ["shop.order_items/nope", CHANNEL] {
        let body = json!({"table": "shop.order_items", "relations": [bad]});
        let (status, _, _) = call(&s.app, "POST", &url, Some(&s.lan), Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }

    // Chọn bước 2 mà không chọn bước 1: bước 1 được nối theo.
    let snap = json!({"table": "shop.order_items", "relations": [PRODUCTS, GIFT, ORDER_CUSTOMER]});
    let (status, _, result) = call(&s.app, "POST", &url, Some(&s.lan), Some(snap)).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(result["version"]["row_count"], 5);
    let names: Vec<&str> = result["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for name in [
        "orders.status",
        "orders.status_2",
        "orders.customer_id",
        "products.title",
        "products.price",
        "customers.name",
        "customers.region_code",
        "customers_2.name",
    ] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
    assert!(!names.contains(&"orders.id") && !names.contains(&"products.maker"));

    let model = &result["dataset"]["current_source_model"];
    assert_eq!(model["fact_table"], "shop.order_items");
    assert_eq!(
        (model["fact_rows"].as_i64(), model["joined_rows"].as_i64()),
        (Some(5), Some(5))
    );
    let order: Vec<&str> = model["joins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["relation"].as_str().unwrap())
        .collect();
    // Thứ tự tìm thấy (theo tên ràng buộc, bước 2 ngay sau bước 1 của nó), không theo thứ tự trong request.
    assert_eq!(order, [GIFT, PRODUCTS, ORDERS, ORDER_CUSTOMER]);
    assert_eq!(model["joins"][0]["prefix"], "customers");
    assert_eq!(model["joins"][3]["prefix"], "customers_2");
    let version = result["version"]["id"].as_str().unwrap().to_string();
    let dataset_id = result["dataset"]["id"].as_str().unwrap().to_string();

    let column = |name: &str| {
        format!(
            "SELECT role || '|' || COALESCE(origin_table, '') FROM columns
             WHERE version_id = '{version}' AND name = '{name}'"
        )
    };
    assert_eq!(
        fetch_text(&s.db.store, &column("products.price")).await,
        "dimension|shop.products"
    );
    assert_eq!(fetch_text(&s.db.store, &column("qty")).await, "measure|");

    // Member đọc được mô hình qua danh sách dataset.
    let (status, _, list) = call(
        &s.app,
        "GET",
        &format!("{SHOP}/datasets"),
        Some(&s.minh),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let listed = list
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == dataset_id.as_str())
        .unwrap();
    assert_eq!(
        listed["current_source_model"]["joins"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let ask = json!({"question": "Số lượng theo khách", "choices": []});
    let (status, _, answer) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/datasets/{dataset_id}/ask"),
        Some(&s.lan),
        Some(ask),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "answered", "{answer}");
    let text = answer.to_string();
    assert!(text.contains("Binh") && text.contains("Chi"), "{text}");

    let ask = json!({"question": "Tổng giá sản phẩm", "choices": []});
    let (status, _, answer) = call(
        &s.app,
        "POST",
        &format!("{SHOP}/datasets/{dataset_id}/ask"),
        Some(&s.lan),
        Some(ask),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "answered", "{answer}");
    // Ba sản phẩm 2.50 + 10 + 30, không phải tổng theo năm dòng hàng.
    assert!(answer["result"].to_string().contains("42.5"), "{answer}");
    assert!(answer["sql_guarded"].as_str().unwrap().contains("one_row"));

    // Chụp lại: không gửi quan hệ, nối đúng như lần trước; gửi quan hệ thì từ chối.
    let with_relations = json!({"dataset_id": dataset_id, "relations": [ORDERS]});
    let (status, _, _) = call(&s.app, "POST", &url, Some(&s.lan), Some(with_relations)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    exec(
        &s.db.store,
        "INSERT INTO shop.order_items VALUES (11, 'acme', 2, 7, 'raw', 2, NULL, NULL)",
    )
    .await;
    let again = json!({"dataset_id": dataset_id});
    let (status, _, next) = call(&s.app, "POST", &url, Some(&s.lan), Some(again)).await;
    assert_eq!(status, StatusCode::CREATED, "{next}");
    assert_eq!(next["version"]["version"], 2);
    assert_eq!(next["version"]["row_count"], 6);
    assert_eq!(
        next["dataset"]["current_source_model"]["joins"],
        model["joins"]
    );
    s.finish().await;
}
