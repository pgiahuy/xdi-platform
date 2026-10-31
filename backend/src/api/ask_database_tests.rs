// SPDX-License-Identifier: Apache-2.0
/* api/ask_database_tests.rs - hỏi trên cả database (#249) qua route thật, trên Postgres thật, schema shop của
api/source_relations_tests.rs: bảng ba bước khóa ngoại (order_items -> orders -> customers -> regions) đọc được, bộ lọc
dòng vẫn áp trên mọi bảng gửi đi; database vượt trần metadata thì bảng chỉ có trong danh sách gọn được gửi lại một lần
kèm chi tiết; hai bảng không có quan hệ thì lời báo nói rõ. Phần thuần ở service/ask/table_pick_tests.rs. Không có
DATABASE_URL thì bỏ qua. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_related_tests::{ask, give_north_role, tables_read};
use super::ask_tests::sql_reply;
use super::source_relations_tests::{add_source, dataset_id, snapshot};
use super::sources_tests::{start, Setup, KEY};
use crate::engine::store::test_support::exec;

const QTY_BY_REGION_NAME: &str = "SELECT r.name, SUM(oi.qty) AS qty FROM order_items oi \
     JOIN orders o ON oi.order_id = o.id JOIN customers c ON o.customer_id = c.id \
     JOIN regions r ON c.region_code = r.code GROUP BY r.name ORDER BY r.name";

/// Chụp order_items cùng các bảng liên quan (và các bảng thêm), trả kết quả chụp.
async fn snapshot_shop(s: &Setup, extra: &[&str]) -> Value {
    let id = add_source(s).await;
    let mut tables = vec![
        "shop.orders",
        "shop.customers",
        "shop.products",
        "shop.regions",
    ];
    tables.extend_from_slice(extra);
    let body = json!({"table": "shop.order_items", "tables": tables});
    let (status, result) = snapshot(s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    result
}

/// Chi tiết của bước tên name đầu tiên có chữ text, hay panic kèm mọi bước.
fn step_detail<'a>(answer: &'a Value, name: &str, text: &str) -> &'a str {
    answer["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["name"] == name)
        .filter_map(|s| s["detail"].as_str())
        .find(|d| d.contains(text))
        .unwrap_or_else(|| panic!("no {name} step with {text:?}: {}", answer["steps"]))
}

/// regions cách order_items ba bước khóa ngoại: trước #249 nhóm chỉ đi hai bước nên guard từ chối bảng này. Database
/// nhỏ nên mọi bảng gửi chi tiết; Minh bị lọc trên customers và regions chỉ thấy dòng North, qua điều kiện lan theo
/// khóa ngoại của order_items và orders.
#[tokio::test]
async fn three_hops_away_is_readable_with_row_filters() {
    let replies = [sql_reply(QTY_BY_REGION_NAME), sql_reply(QTY_BY_REGION_NAME)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let result = snapshot_shop(&s, &[]).await;
    let items = dataset_id(&result, "order_items");

    let answer = ask(&s, &s.lan, &items, "Số lượng theo tên vùng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(
        tables_read(&answer),
        ["order_items", "orders", "customers", "regions"]
    );
    assert_eq!(
        answer["result"]["rows"],
        json!([["North", "5"], ["South", "2"]]),
        "{answer}"
    );
    step_detail(
        &answer,
        "context",
        "Gửi chi tiết mọi bảng của database (5 bảng)",
    );

    let customers = dataset_id(&result, "customers");
    let regions = dataset_id(&result, "regions");
    give_north_role(
        &s,
        &[(&customers, "region_code", "N"), (&regions, "code", "N")],
    )
    .await;
    let answer = ask(&s, &s.minh, &items, "Số lượng theo tên vùng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    // Dòng (10, acme, 2) có gift_customer_id 3 (Chi, không vùng): đường thứ hai tới customers ẩn dòng đó
    // (ask/row_filters.rs), nên North còn 4 chứ không 5.
    assert_eq!(
        answer["result"]["rows"],
        json!([["North", "4"]]),
        "{answer}"
    );
    s.finish().await;
}

/// Metadata vượt trần: chỉ order_items gửi chi tiết, các bảng khác chỉ có tên. Mô hình xin bảng qua needed_tables thì
/// luồng hỏi gửi lại một lần kèm chi tiết các bảng đó, rồi câu SQL mới chạy.
#[tokio::test]
async fn over_the_limit_resends_once_with_the_needed_tables() {
    let ask_tables = json!({"terms": [], "assumptions": [],
                            "needed_tables": ["orders", "customers", "regions"], "sql": ""});
    let replies = [ask_tables, sql_reply(QTY_BY_REGION_NAME)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let result = snapshot_shop(&s, &[]).await;
    let items = dataset_id(&result, "order_items");
    // Mô tả dài ở mọi cột đẩy tổng metadata qua trần (WHOLE_DATASET_MAX_CHARS), như một database hàng chục bảng.
    exec(
        &s.db.store,
        "UPDATE columns SET description = repeat('d', 2000)",
    )
    .await;

    let answer = ask(&s, &s.lan, &items, "Số lượng theo tên vùng").await;
    assert_eq!(answer["status"], "answered", "{answer}");
    assert_eq!(
        tables_read(&answer),
        ["order_items", "orders", "customers", "regions"]
    );
    let context = step_detail(
        &answer,
        "context",
        "Bảng gửi chi tiết: order_items (bảng bắt đầu)",
    );
    assert!(context.contains("Chỉ gửi tên:"), "{context}");
    assert!(context.contains("regions"), "{context}");
    let resent = step_detail(&answer, "llm", "Gửi lại kèm chi tiết bảng");
    assert!(resent.contains("regions"), "{resent}");
    s.finish().await;
}

/// notes không có khóa ngoại nào: câu nối order_items với notes bị guard từ chối, và lời báo nói hai bảng không có
/// quan hệ trong database.
#[tokio::test]
async fn unrelated_tables_get_a_clear_message() {
    let join_notes = "SELECT n.body, SUM(oi.qty) AS qty FROM order_items oi JOIN notes n ON oi.qty = n.id GROUP BY n.body";
    let replies = [sql_reply(join_notes), sql_reply(join_notes)];
    let Some(s) = start(Some(KEY), &replies).await else {
        return;
    };
    let id = add_source(&s).await;
    for sql in [
        "CREATE TABLE shop.notes (id int PRIMARY KEY, body text)".to_string(),
        "INSERT INTO shop.notes VALUES (1, 'a'), (4, 'b')".to_string(),
        format!("GRANT SELECT ON shop.notes TO {}", s.user),
    ] {
        exec(&s.db.store, &sql).await;
    }
    let body = json!({"table": "shop.order_items", "tables": ["shop.orders", "shop.notes"]});
    let (status, result) = snapshot(&s, &id, body).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    let items = dataset_id(&result, "order_items");

    let answer = ask(&s, &s.lan, &items, "Số lượng theo ghi chú").await;
    assert_ne!(answer["status"], "answered", "{answer}");
    let message = answer["message"].as_str().unwrap();
    assert!(
        message.contains("Bảng order_items và bảng notes không có quan hệ khóa ngoại"),
        "{message}"
    );
    s.finish().await;
}
