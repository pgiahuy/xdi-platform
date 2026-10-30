// SPDX-License-Identifier: Apache-2.0
/* api/row_filter_fk_tests.rs - bộ lọc dòng lan theo khóa ngoại (#215) qua route thật, trên ảnh chụp Postgres thật của
schema shop (api/source_relations_tests.rs), với mô hình giả trả câu SQL viết sẵn. Minh có vai north, chỉ một chính
sách: customers.region_code = 'N' (khách 1). Theo dữ liệu shop, Minh chỉ thấy đơn 10, phiếu trả 1; không thấy dòng hàng
nào, vì mỗi dòng hàng phải qua cả đường đơn hàng lẫn đường khách nhận quà (gift_customer_id), mà dòng hàng của đơn 10
có khách nhận quà rỗng hay là khách 3 (không có vùng). Lan bảng chỉ đọc một mình, truy dòng nguồn và tổng quan dataset
đều được kiểm, cùng câu với admin để so. Không có DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_related_tests::{ask, give_north_role, tables_read};
use super::ask_tests::sql_reply;
use super::source_relations_tests::{add_source, dataset_id, snapshot};
use super::sources_tests::{start, Setup, KEY, SHOP};
use crate::api::test_support::call;

/// Điều kiện guard thêm vào chỗ đọc orders của Minh.
const ORDERS_OF_NORTH: &str =
    "\"customer_id\" IN (SELECT \"id\" FROM \"customers\" WHERE \"region_code\" IN ('N'))";

const COUNT_ORDERS: &str = "SELECT COUNT(*) AS orders FROM orders";
const COUNT_RETURNS: &str = "SELECT COUNT(*) AS returns FROM returns";
const COUNT_ITEMS: &str = "SELECT COUNT(*) AS items FROM order_items";
const ORDERS_BY_REGION: &str = "SELECT c.region_code, COUNT(*) AS orders FROM orders o \
     JOIN customers c ON o.customer_id = c.id GROUP BY c.region_code ORDER BY c.region_code";
const ORDERS_BY_STATUS: &str =
    "SELECT status, COUNT(*) AS orders FROM orders GROUP BY status ORDER BY status";

/// Chụp order_items cùng orders, customers, products, returns; Minh có vai north. Trả (kết quả chụp, id customers).
async fn snapshot_shop(s: &Setup) -> Value {
    let id = add_source(s).await;
    let body = json!({"table": "shop.order_items",
                      "tables": ["shop.orders", "shop.customers", "shop.products", "shop.returns"]});
    let (status, result) = snapshot(s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    let customers = dataset_id(&result, "customers");
    give_north_role(s, &[(&customers, "region_code", "N")]).await;
    result
}

fn rows(answer: &Value) -> &Value {
    assert_eq!(answer["status"], "answered", "{answer}");
    &answer["result"]["rows"]
}

/// Đọc orders một mình (lỗ chính của #215), returns qua hai bước, order_items qua hai đường; câu JOIN vẫn đúng; admin
/// thấy đủ với cùng câu.
#[tokio::test]
async fn member_filtered_on_customers_sees_only_rows_reaching_their_customers() {
    let replies = [
        sql_reply(COUNT_ORDERS),
        sql_reply(COUNT_ORDERS),
        sql_reply(COUNT_RETURNS),
        sql_reply(COUNT_ITEMS),
        sql_reply(COUNT_ITEMS),
        sql_reply(ORDERS_BY_REGION),
        sql_reply(ORDERS_BY_REGION),
    ];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let result = snapshot_shop(&s).await;
    let orders = dataset_id(&result, "orders");
    let returns = dataset_id(&result, "returns");
    let items = dataset_id(&result, "order_items");

    let answer = ask(&s, &s.minh, &orders, "Có bao nhiêu đơn hàng").await;
    assert_eq!(rows(&answer), &json!([["1"]]));
    assert_eq!(tables_read(&answer), ["orders"]);
    let guarded = answer["sql_guarded"].as_str().unwrap();
    assert!(guarded.contains(ORDERS_OF_NORTH), "{guarded}");
    let answer = ask(&s, &s.lan, &orders, "Có bao nhiêu đơn hàng").await;
    assert_eq!(rows(&answer), &json!([["3"]]));
    assert!(!answer["sql_guarded"]
        .as_str()
        .unwrap()
        .contains("IN (SELECT"));

    // returns -> orders -> customers: chỉ phiếu trả của đơn 10.
    let answer = ask(&s, &s.minh, &returns, "Có bao nhiêu phiếu trả hàng").await;
    assert_eq!(rows(&answer), &json!([["1"]]));

    // order_items: dòng hàng phải qua cả hai đường tới customers.
    let answer = ask(&s, &s.minh, &items, "Có bao nhiêu dòng hàng").await;
    assert_eq!(rows(&answer), &json!([["0"]]));
    let guarded = answer["sql_guarded"].as_str().unwrap();
    assert!(
        guarded.contains("\"order_id\" IN (SELECT \"id\" FROM \"orders\" WHERE \"customer_id\" IN")
            && guarded.contains("\"gift_customer_id\" IN (SELECT \"id\" FROM \"customers\""),
        "{guarded}"
    );
    let answer = ask(&s, &s.lan, &items, "Có bao nhiêu dòng hàng").await;
    assert_eq!(rows(&answer), &json!([["5"]]));

    // Câu JOIN tới customers: orders mang điều kiện lan, customers mang chính sách của nó; kết quả như khi chỉ có dòng
    // được phép.
    let answer = ask(&s, &s.minh, &orders, "Số đơn theo vùng").await;
    assert_eq!(rows(&answer), &json!([["N", "1"]]));
    assert_eq!(tables_read(&answer), ["orders", "customers"]);
    let answer = ask(&s, &s.lan, &orders, "Số đơn theo vùng").await;
    assert_eq!(rows(&answer), &json!([["N", "1"], ["S", "1"], [null, "1"]]));
    s.finish().await;
}

async fn drill(s: &Setup, session: &str, dataset: &str, query_id: &str) -> (StatusCode, Value) {
    let body = json!({"query_id": query_id, "cell": [{"column": "status", "value": "done"}]});
    let url = format!("{SHOP}/datasets/{dataset}/drill");
    let (status, _, body) = call(&s.app, "POST", &url, Some(session), Some(body)).await;
    (status, body)
}

async fn overview(s: &Setup, session: &str, dataset: &str) -> Value {
    let url = format!("{SHOP}/datasets/{dataset}/overview");
    let (status, _, body) = call(&s.app, "GET", &url, Some(session), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Truy dòng nguồn và tổng quan của orders lọc theo người đang xem: Minh bấm vào câu của admin chỉ thấy đơn 10; admin
/// bấm vào câu của Minh vẫn đọc được câu đã lưu (hộp lọc của Minh nhắc tới customers) và chỉ thấy dòng sau con số Minh
/// đã nhận.
#[tokio::test]
async fn drill_and_overview_on_orders_respect_the_filter() {
    let replies = [sql_reply(ORDERS_BY_STATUS), sql_reply(ORDERS_BY_STATUS)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let result = snapshot_shop(&s).await;
    let orders = dataset_id(&result, "orders");

    let minh_answer = ask(&s, &s.minh, &orders, "Số đơn theo trạng thái").await;
    assert_eq!(rows(&minh_answer), &json!([["done", "1"]]));
    let lan_answer = ask(&s, &s.lan, &orders, "Số đơn theo trạng thái").await;
    assert_eq!(rows(&lan_answer), &json!([["done", "2"], ["open", "1"]]));
    let minh_query = minh_answer["query_id"].as_str().unwrap();
    let lan_query = lan_answer["query_id"].as_str().unwrap();

    let cases = [
        (&s.minh, minh_query, 1),
        (&s.lan, minh_query, 1),
        (&s.minh, lan_query, 1),
        (&s.lan, lan_query, 2),
    ];
    for (session, query_id, expected) in cases {
        let (status, body) = drill(&s, session, &orders, query_id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["total_rows"], expected, "{body}");
    }
    let (_, body) = drill(&s, &s.minh, &orders, lan_query).await;
    assert!(
        body["sql"].as_str().unwrap().contains(ORDERS_OF_NORTH),
        "{body}"
    );

    // Tổng quan: số dòng tính bằng SQL qua guard; Minh bị lọc nên không nhận số đếm của hồ sơ.
    let body = overview(&s, &s.minh, &orders).await;
    assert_eq!(body["summary"]["restricted"], true, "{body}");
    assert_eq!(body["summary"]["row_count"], 1, "{body}");
    let body = overview(&s, &s.lan, &orders).await;
    assert_eq!(body["summary"]["restricted"], false, "{body}");
    assert_eq!(body["summary"]["row_count"], 3, "{body}");
    s.finish().await;
}
