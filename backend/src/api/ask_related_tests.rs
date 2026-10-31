// SPDX-License-Identifier: Apache-2.0
/* api/ask_related_tests.rs - hỏi trên dataset chụp từ database cùng các bảng liên quan (#213), qua route thật, trên
Postgres thật: chụp schema shop của api/source_relations_tests.rs (mỗi bảng một dataset, kèm quan hệ), rồi mô hình giả
trả câu SQL có JOIN. Kiểm bảng đã đọc trong response và queries.read_versions, bộ lọc dòng trên từng bảng, truy dòng
nguồn của câu nối bảng qua route (dòng của order_items, tổng khớp ô). Bộ lọc lan theo khóa ngoại (#215) ở api/row_filter_fk_tests.rs. Không có
DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::sql_reply;
use super::source_relations_tests::{add_source, dataset_id, snapshot};
use super::sources_tests::{start, Setup, KEY, SHOP};
use crate::api::test_support::call;
use crate::engine::store::test_support::{exec, fetch_count};

/// Hỏi một câu trên dataset, bằng phiên của session.
pub(super) async fn ask(s: &Setup, session: &str, dataset: &str, question: &str) -> Value {
    let body = json!({"question": question, "choices": []});
    let url = format!("{SHOP}/datasets/{dataset}/ask");
    let (status, _, answer) = call(&s.app, "POST", &url, Some(session), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

/// Tên bảng logic trong AskResponse.tables_read.
pub(super) fn tables_read(answer: &Value) -> Vec<&str> {
    answer["tables_read"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["logical_table"].as_str().unwrap())
        .collect()
}

/// Minh có vai dữ liệu north; mỗi cặp (dataset, cột, giá trị) là một chính sách của vai đó.
pub(super) async fn give_north_role(s: &Setup, policies: &[(&str, &str, &str)]) {
    exec(
        &s.db.store,
        "UPDATE memberships SET data_role = 'north' WHERE user_id = (SELECT id FROM users WHERE email = 'minh@example.com')",
    )
    .await;
    for (dataset, column, value) in policies {
        exec(
            &s.db.store,
            &format!(
                "INSERT INTO rls_policies (workspace_id, dataset_id, data_role, column_name, allowed_values)
                 SELECT workspace_id, id, 'north', '{column}', ARRAY['{value}'] FROM datasets WHERE id = '{dataset}'"
            ),
        )
        .await;
    }
}

const QTY_BY_REGION: &str = "SELECT c.region_code, SUM(oi.qty) AS qty FROM order_items oi \
     JOIN orders o ON oi.order_id = o.id JOIN customers c ON o.customer_id = c.id \
     GROUP BY c.region_code ORDER BY c.region_code";

const CUSTOMERS_BY_REGION: &str = "SELECT r.name, COUNT(*) AS customers FROM customers c \
     JOIN regions r ON c.region_code = r.code GROUP BY r.name ORDER BY r.name";

/// Hỏi trên order_items: câu JOIN tới orders và customers chạy, response và lịch sử ghi đủ ba bảng; truy dòng nguồn trả
/// dòng của order_items kèm cột region_code của khách, tổng qty của các dòng bằng ô. Minh bị lọc trên customers và regions: câu JOIN hai bảng đó chỉ thấy dòng North.
#[tokio::test]
async fn join_question_reads_related_tables_with_their_row_filters() {
    let replies = [sql_reply(QTY_BY_REGION), sql_reply(CUSTOMERS_BY_REGION)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let id = add_source(&s).await;
    let body = json!({"table": "shop.order_items",
                      "tables": ["shop.orders", "shop.customers", "shop.products", "shop.regions"]});
    let (status, result) = snapshot(&s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    let items = dataset_id(&result, "order_items");

    let answer = ask(&s, &s.lan, &items, "Số lượng theo vùng của khách hàng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(tables_read(&answer), ["order_items", "orders", "customers"]);
    assert_eq!(
        answer["result"]["rows"],
        json!([["N", "5"], ["S", "2"], [null, "4"]]),
        "{answer}"
    );
    let query_id = answer["query_id"].as_str().unwrap();
    let read = fetch_count(
        &s.db.store,
        &format!(
            "SELECT jsonb_array_length(read_versions)::bigint FROM queries WHERE id = '{query_id}'"
        ),
    )
    .await;
    assert_eq!(read, 3);

    let drill = json!({"query_id": query_id, "cell": [{"column": "region_code", "value": "N"}]});
    let url = format!("{SHOP}/datasets/{items}/drill");
    let (status, _, body) = call(&s.app, "POST", &url, Some(&s.lan), Some(drill)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["source_table"], "order_items", "{body}");
    assert_eq!(body["joined_columns"], json!(["region_code"]), "{body}");
    assert_eq!(body["measures"][0]["rule"], "sum", "{body}");
    assert_eq!(body["measures"][0]["value"], "5", "{body}");
    assert_eq!(body["measures"][0]["row_column"], "qty", "{body}");

    let customers = dataset_id(&result, "customers");
    let regions = dataset_id(&result, "regions");
    give_north_role(
        &s,
        &[(&customers, "region_code", "N"), (&regions, "code", "N")],
    )
    .await;
    let answer = ask(&s, &s.minh, &customers, "Số khách hàng theo vùng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(
        answer["result"]["rows"],
        json!([["North", "1"]]),
        "{answer}"
    );
    assert_eq!(tables_read(&answer), ["customers", "regions"]);
    let guarded = answer["sql_guarded"].as_str().unwrap();
    assert!(
        guarded.contains("\"region_code\" IN ('N')") && guarded.contains("\"code\" IN ('N')"),
        "{guarded}"
    );
    s.finish().await;
}
