// SPDX-License-Identifier: Apache-2.0
/* api/source_snapshot_all_tests.rs - test chụp mọi bảng đọc được (#247) qua route thật, trên Postgres thật (schema shop
của api/source_relations_tests.rs, cộng bảng public.orders của api/sources_tests.rs): danh sách bảng kèm khóa ngoại và
cờ bảng danh mục, chụp mặc định mọi bảng với mọi quan hệ, chụp lại không đổi thì không tạo phiên bản, bỏ tick rồi chụp
riêng bảng đó sau, trần dung lượng giữ metadata, câu hỏi cần bảng chỉ có metadata được báo đúng tên bảng. Không có
DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{scripted, sql_reply};
use super::source_relations_tests::{add_source, snapshot};
use super::sources_tests::{start, start_configured, Setup, KEY, SHOP};
use crate::api::test_support::call;

/// Mọi bảng, view tài khoản nguồn đọc được trong database của test.
const READABLE: [&str; 9] = [
    "public.orders",
    "shop.big_items",
    "shop.channels",
    "shop.customers",
    "shop.order_items",
    "shop.orders",
    "shop.products",
    "shop.regions",
    "shop.returns",
];

async fn list_tables(s: &Setup, id: &str) -> Vec<Value> {
    let url = format!("{SHOP}/sources/{id}/tables");
    let (status, _, body) = call(&s.app, "GET", &url, Some(&s.lan), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().unwrap().clone()
}

fn find<'a>(tables: &'a [Value], name: &str) -> &'a Value {
    tables
        .iter()
        .find(|t| t["table"] == name)
        .unwrap_or_else(|| panic!("no table {name}"))
}

fn table_names(result: &Value) -> Vec<String> {
    let mut names: Vec<String> = result["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["dataset"]["source_table"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/// (bảng phía nhiều, bảng phía một) của mọi quan hệ trong response, đã sắp.
fn relation_pairs(result: &Value) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = result["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["from_dataset_name"].as_str().unwrap().to_string(),
                r["to_dataset_name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    pairs.sort();
    pairs
}

/// Danh sách bảng có khóa ngoại và cờ bảng danh mục; thân rỗng chụp mọi bảng một lần với mọi quan hệ khai báo; chụp lại
/// cả nguồn khi không đổi gì không tạo phiên bản nào.
#[tokio::test]
async fn every_readable_table_is_snapshotted_by_default() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let tables = list_tables(&s, &id).await;
    let names: Vec<&str> = tables
        .iter()
        .map(|t| t["table"].as_str().unwrap())
        .collect();
    assert_eq!(names, READABLE);
    let customers = find(&tables, "shop.customers");
    assert_eq!(customers["is_referenced"], true);
    assert_eq!(customers["comment"], Value::Null);
    assert_eq!(customers["state"], Value::Null);
    let items = find(&tables, "shop.order_items");
    let mut refs: Vec<&str> = items["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["to_table"].as_str().unwrap())
        .collect();
    refs.sort();
    // shop.secrets không đọc được nên không có.
    assert_eq!(
        refs,
        [
            "shop.channels",
            "shop.customers",
            "shop.orders",
            "shop.products"
        ]
    );
    assert_eq!(items["is_referenced"], false);
    assert_eq!(find(&tables, "shop.big_items")["kind"], "view");

    let (status, result) = snapshot(&s, &id, json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(table_names(&result), READABLE);
    assert_eq!(result["metadata_only"], json!([]));
    // Mọi khóa ngoại khai báo giữa các bảng đã chụp, kể cả xa hai bước (order_items -> orders -> customers -> regions) và
    // khóa tới cột chỉ có chỉ mục UNIQUE (channels).
    let pairs = relation_pairs(&result);
    let expected = [
        ("customers", "regions"),
        ("order_items", "channels"),
        ("order_items", "customers"),
        ("order_items", "orders"),
        ("order_items", "products"),
        ("orders", "customers"),
        ("returns", "orders"),
    ];
    assert_eq!(pairs, expected.map(|(a, b)| (a.to_string(), b.to_string())));
    let tables = list_tables(&s, &id).await;
    assert!(
        tables.iter().all(|t| t["state"] == "snapshotted"),
        "{tables:?}"
    );

    let url = format!("{SHOP}/sources/{id}/resnapshot");
    let (status, _, again) = call(&s.app, "POST", &url, Some(&s.lan), None).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(table_names(&again), READABLE);
    assert!(again["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["created"] == false && d["version"]["version"] == 1));
    assert_eq!(relation_pairs(&again).len(), expected.len());
    s.finish().await;
}

/// Bỏ tick regions: không có dataset, không có quan hệ tới nó. Chụp riêng nó sau thì quan hệ customers -> regions (từ
/// dataset chụp trước) được ghi.
#[tokio::test]
async fn unticked_table_can_be_added_later() {
    let Some(s) = start(Some(KEY), &[]).await else {
        return;
    };
    let id = add_source(&s).await;
    let ticked: Vec<&str> = READABLE
        .iter()
        .copied()
        .filter(|t| *t != "shop.regions")
        .collect();
    let (status, result) = snapshot(&s, &id, json!({ "tables": ticked })).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(table_names(&result), ticked);
    assert!(!relation_pairs(&result)
        .iter()
        .any(|(_, to)| to == "regions"));
    assert_eq!(
        find(&list_tables(&s, &id).await, "shop.regions")["state"],
        Value::Null
    );

    let (status, later) = snapshot(&s, &id, json!({"tables": ["shop.regions"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{later}");
    assert_eq!(table_names(&later), ["shop.regions"]);
    assert_eq!(
        relation_pairs(&later),
        [("customers".to_string(), "regions".to_string())]
    );
    s.finish().await;
}

/// Trần dung lượng 1 byte: mọi bảng có dung lượng chỉ lưu metadata, view (không dung lượng) vẫn chụp. Câu hỏi trên view
/// mà SQL cần order_items thì lời báo nêu đúng bảng đó. Chụp lại cả nguồn giữ metadata, không tạo gì.
#[tokio::test]
async fn size_limit_keeps_metadata_and_questions_name_the_table() {
    let sql = "SELECT SUM(qty) AS total FROM order_items";
    let llm = scripted(&[sql_reply(sql), sql_reply(sql)]);
    let Some(s) = start_configured(Some(KEY), llm, |c| c.source_snapshot_max_bytes = 1).await
    else {
        return;
    };
    let id = add_source(&s).await;
    let (status, result) = snapshot(&s, &id, json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(table_names(&result), ["shop.big_items"]);
    let skipped = result["metadata_only"].as_array().unwrap();
    assert_eq!(skipped.len(), READABLE.len() - 1);
    assert!(
        skipped.iter().all(|t| t["cause"] == "size_limit"),
        "{skipped:?}"
    );
    let items = skipped
        .iter()
        .find(|t| t["table"] == "shop.order_items")
        .unwrap();
    let column = &items["columns"][0];
    assert_eq!(
        (
            &column["name"],
            &column["data_type"],
            &column["primary_key"]
        ),
        (&json!("order_id"), &json!("integer"), &json!(false))
    );
    let tables = list_tables(&s, &id).await;
    let listed = find(&tables, "shop.order_items");
    assert_eq!(listed["state"], "metadata_only");
    assert!(listed["reason"]
        .as_str()
        .unwrap()
        .contains("SOURCE_SNAPSHOT_MAX_MB"));

    let view = result["datasets"][0]["dataset"]["id"].as_str().unwrap();
    let ask = json!({"question": "Tổng số lượng hàng", "choices": []});
    let url = format!("{SHOP}/datasets/{view}/ask");
    let (status, _, answer) = call(&s.app, "POST", &url, Some(&s.lan), Some(ask)).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_ne!(answer["status"], "answered", "{answer}");
    let message = answer["message"].as_str().unwrap();
    assert!(message.contains("bảng shop.order_items"), "{message}");
    assert!(message.contains("chưa được chụp"), "{message}");

    let url = format!("{SHOP}/sources/{id}/resnapshot");
    let (status, _, again) = call(&s.app, "POST", &url, Some(&s.lan), None).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(table_names(&again), ["shop.big_items"]);
    assert_eq!(
        again["metadata_only"].as_array().unwrap().len(),
        READABLE.len() - 1
    );
    s.finish().await;
}
