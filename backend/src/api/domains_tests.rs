// SPDX-License-Identifier: Apache-2.0
/* api/domains_tests.rs - test API khái niệm ngành qua route thật (#229): workspace mới có sẵn các gói của
data/domains/, member xem mà không sửa được (403), admin thêm, sửa, tắt khái niệm, khái niệm sai báo đúng trường; khái
niệm vừa sửa tới prompt sinh SQL của câu hỏi kế tiếp, khái niệm tắt thì không. Dùng start_on của api/ask_tests.rs và
mô hình giả, add_member của api/metrics_tests.rs; không có DATABASE_URL thì bỏ qua. */

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::ask_tests::{sql_reply, start_on, Asking};
use crate::api::metrics_tests::{add_member, RecordingLlm};
use crate::api::test_support::call;
use crate::engine::store::test_support::exec;

const CSV: &str = "Market,Order Region,Order Item Total\n\
                   Europe,Western Europe,100.5\n\
                   LATAM,South America,200\n";

const LIST_URL: &str = "/api/workspaces/cua-hang/domains";

fn concepts_url(pack: &str) -> String {
    format!("{LIST_URL}/{pack}/concepts")
}

fn concept(name: &str, synonyms: &[&str]) -> Value {
    json!({"name": name, "description": "Số khách mua lại trong kỳ", "synonyms": synonyms})
}

/// Khái niệm có khóa này trong gói, như GET trả về.
async fn find_concept(asking: &Asking, pack: &str, key: &str) -> Value {
    let (status, _, body) = call(&asking.app, "GET", LIST_URL, Some(&asking.session), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pack = body["packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == pack)
        .unwrap();
    pack["concepts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == key)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn admin_manages_concepts_and_member_only_reads() {
    let Some(asking) = start_on(None, CSV, |_| {}).await else {
        return;
    };
    let (app, lan) = (&asking.app, Some(asking.session.as_str()));
    let minh = add_member(&asking).await;

    // Workspace mới có sẵn mọi gói của data/domains/, member đọc được.
    let (status, _, body) = call(app, "GET", LIST_URL, Some(&minh), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let keys: Vec<&str> = body["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["education", "retail"]);
    let revenue = find_concept(&asking, "retail", "revenue").await;
    assert!(revenue["updated_at"].is_null());

    let url = concepts_url("retail");
    let new = concept("Khách quay lại", &["khách quay lại", "returning customers"]);
    let (status, _, _) = call(app, "POST", &url, Some(&minh), Some(new.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let one = format!("{url}/{}", revenue["id"].as_str().unwrap());
    let (status, _, _) = call(app, "PUT", &one, Some(&minh), Some(new.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, created) = call(app, "POST", &url, lan, Some(new.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert!(created["key"].is_null());
    assert_eq!(created["enabled"], true);
    // Trùng tên trong gói, kể cả khác hoa thường và dấu; từ đồng nghĩa đã thuộc khái niệm khác.
    let (status, _, body) = call(
        app,
        "POST",
        &url,
        lan,
        Some(concept("khách QUAY lại", &["x"])),
    )
    .await;
    assert_eq!(
        (status, body["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("name")),
        "{body}"
    );
    let (status, _, body) = call(
        app,
        "POST",
        &url,
        lan,
        Some(concept("Doanh thu thuần", &["Doanh số"])),
    )
    .await;
    assert_eq!(
        (status, body["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("synonyms")),
        "{body}"
    );
    let (status, _, _) = call(app, "POST", &concepts_url("khong-co"), lan, Some(new)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let mut off = revenue.clone();
    off["enabled"] = json!(false);
    let (status, _, saved) = call(app, "PUT", &one, lan, Some(off)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["enabled"], false);
    assert!(saved["updated_at"].is_string());
    let missing = format!("{url}/{}", uuid::Uuid::new_v4());
    let (status, _, _) = call(app, "PUT", &missing, lan, Some(revenue.clone())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, _, body) = call(app, "GET", LIST_URL, Some(&minh), None).await;
    let retail = body["packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == "retail")
        .unwrap();
    let last = retail["concepts"].as_array().unwrap().last().unwrap();
    assert_eq!(last["name"], "Khách quay lại");
}

/// Tiêu chí của #229: sửa khái niệm thì câu hỏi kế tiếp dùng bản mới, tắt thì khái niệm rời prompt.
#[tokio::test]
async fn edited_concept_reaches_the_next_sql_prompt() {
    let reply =
        sql_reply("SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco GROUP BY 1");
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(RecordingLlm {
        replies: Mutex::new(VecDeque::from(vec![reply.to_string(); 3])),
        prompts: prompts.clone(),
    });
    let Some(asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
    // Chưa có API gắn gói cho dataset; đặt thẳng cột sources.domain (gói của dataset, migration 0017).
    exec(&asking.db.store, "UPDATE sources SET domain = 'retail'").await;
    let question = "Doanh thu theo thị trường";

    let (status, answer) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let revenue = find_concept(&asking, "retail", "revenue").await;
    let original = revenue["description"].as_str().unwrap().to_string();
    assert!(prompts.lock().unwrap()[0].contains("- Doanh thu (doanh thu, doanh số"));
    assert!(prompts.lock().unwrap()[0].contains(&original));

    let one = format!(
        "{}/{}",
        concepts_url("retail"),
        revenue["id"].as_str().unwrap()
    );
    let mut edited = revenue.clone();
    edited["description"] = json!("Tổng tiền bán hàng, tính theo ngày giao");
    let (status, _, body) = call(
        &asking.app,
        "PUT",
        &one,
        Some(&asking.session),
        Some(edited.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, answer) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let second = prompts.lock().unwrap()[1].clone();
    assert!(
        second.contains("- Doanh thu (doanh thu, doanh số, tiền bán hàng, revenue, sales): Tổng tiền bán hàng, tính theo ngày giao"),
        "{second}"
    );
    assert!(!second.contains(&original));

    edited["enabled"] = json!(false);
    let (status, _, _) = call(
        &asking.app,
        "PUT",
        &one,
        Some(&asking.session),
        Some(edited),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    asking.ask(question, json!([])).await;
    let third = prompts.lock().unwrap()[2].clone();
    assert!(!third.contains("- Doanh thu ("), "{third}");
    assert!(third.contains("- Số đơn hàng ("), "{third}");
}
