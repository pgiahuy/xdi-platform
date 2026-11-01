// SPDX-License-Identifier: Apache-2.0
/* api/ask_why_database_tests.rs - câu hỏi nguyên nhân trên cả một database qua route thật, trên Postgres thật, schema shop
của api/source_relations_tests.rs. Hai lỗi đã gặp khi chạy thật với Gemini trên sales_demo, edu_demo:
- bảng bắt đầu chọn theo chữ của câu hỏi không giữ độ đo ("Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM"
  bắt đầu ở customers và bị từ chối): lượt phân loại nêu cột order_items.qty cùng products.title, nên câu chạy lại từ
  order_items (ask/why_table.rs), độ đo ở bảng con, nhóm ở bảng cha (explain/joined.rs);
- câu nêu hai nhóm mà form chỉ điền một ("giữa môn AAA và GGG" thành AAA so với các môn còn lại): kiểm form bắt và mô
  hình sửa một lần (explain/question_match.rs).
Mô hình giả trả câu phân loại và form viết sẵn, đúng các form sai đã gặp. Không có DATABASE_URL thì bỏ qua.

Dữ liệu: Pen (acme, 1) có qty 4 + 1 = 5, Book (acme, 2) có qty 1 + 3 = 4. Cột orders.status không dùng được: order_items
có sẵn một cột tên "orders.status", nên cột cùng tên của bảng liên quan bị bỏ (explain/joined.rs). */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_related_tests::tables_read;
use super::ask_tests::scripted_with_classify;
use super::source_relations_tests::{add_source, snapshot};
use super::sources_tests::{start_with_llm, Setup, KEY, SHOP};
use crate::api::test_support::call;

/// Câu hỏi nhắc "customers" nên bảng bắt đầu theo chữ là customers, không giữ qty.
const QUESTION: &str = "Vì sao số lượng của customers khác nhau giữa Pen và Book?";

fn classify_reply() -> Value {
    json!({
        "kind": "cause",
        "reason": "Câu trả lời viết sẵn của test.",
        "columns": ["order_items.qty", "products.title"]
    })
}

fn groups_form(base: Option<&str>) -> Value {
    let mut groups = json!({"column": "products.title", "compare": "Pen"});
    if let Some(base) = base {
        groups["base"] = json!(base);
    }
    json!({
        "terms": [{"text": "số lượng", "kind": "column", "column": "qty"}],
        "measure": {"column": "qty", "agg": "sum"},
        "groups": groups,
        "compare": "",
        "filters": []
    })
}

/// Chụp mọi bảng của shop mà tài khoản nguồn đọc được; trả id của dataset database (catalog).
async fn snapshot_shop(s: &Setup) -> String {
    let id = add_source(s).await;
    let tables = [
        "shop.orders",
        "shop.customers",
        "shop.products",
        "shop.regions",
    ];
    let body = json!({"table": "shop.order_items", "tables": tables});
    let (status, result) = snapshot(s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    id
}

async fn ask_database(s: &Setup, id: &str, question: &str) -> Value {
    let url = format!("{SHOP}/catalog/{id}/ask");
    let body = json!({"question": question});
    let (status, _, answer) = call(&s.app, "POST", &url, Some(&s.lan), Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

fn step_names(answer: &Value) -> Vec<&str> {
    answer["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect()
}

/// Bảng bắt đầu đổi sang order_items theo các cột lượt phân loại nêu; nhóm là cột products.title của bảng cha.
#[tokio::test]
async fn the_measure_table_is_the_start_and_the_parent_holds_the_groups() {
    let llm = scripted_with_classify(classify_reply(), &[groups_form(Some("Book"))]);
    let Some(s) = start_with_llm(Some(KEY), llm, None).await else {
        return;
    };
    let id = snapshot_shop(&s).await;
    let answer = ask_database(&s, &id, QUESTION).await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(answer["question_type"], "causal", "{answer}");
    let classify = answer["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "classify")
        .unwrap();
    assert!(
        classify["detail"]
            .as_str()
            .unwrap()
            .contains("Bắt đầu từ bảng order_items"),
        "{classify}"
    );
    let explanation = &answer["explanation"];
    assert_eq!(
        explanation["slice_column"], "products.title",
        "{explanation}"
    );
    assert_eq!(explanation["base"]["label"], "Book");
    assert_eq!(explanation["compare"]["label"], "Pen");
    assert_eq!(explanation["base"]["value"], "4");
    assert_eq!(explanation["compare"]["value"], "5");
    let read = tables_read(&answer);
    assert_eq!(read[0], "order_items", "{read:?}");
    assert!(read.contains(&"products"), "{read:?}");
    s.finish().await;
}

/// Form đầu bỏ nhóm Book (so Pen với các nhóm còn lại): kiểm form gửi lỗi chỉ đúng hai nhóm, form sửa so đúng Pen với
/// Book.
#[tokio::test]
async fn a_form_that_drops_a_named_group_is_repaired() {
    let replies = [groups_form(None), groups_form(Some("Book"))];
    let llm = scripted_with_classify(classify_reply(), &replies);
    let Some(s) = start_with_llm(Some(KEY), llm, None).await else {
        return;
    };
    let id = snapshot_shop(&s).await;
    let answer = ask_database(&s, &id, QUESTION).await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert!(step_names(&answer).contains(&"repair"), "{answer}");
    let explanation = &answer["explanation"];
    assert_eq!(explanation["base"]["label"], "Book", "{explanation}");
    assert_eq!(explanation["compare"]["label"], "Pen");
    s.finish().await;
}
