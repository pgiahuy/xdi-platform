// SPDX-License-Identifier: Apache-2.0
/* api/source_comments_tests.rs - mô tả bảng và cột ghi trong database nguồn (COMMENT ON, #245) qua route thật: chụp
bảng có comment, lớp ngữ nghĩa chạy nền với mô hình giả, rồi đọc tổng quan, màn Chỉ số và prompt của câu hỏi. Database
ngoài là database riêng của test như api/sources_tests.rs. Không có DATABASE_URL thì bỏ qua. */

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::sql_reply;
use super::sources_tests::{start_with_llm, KEY, SHOP};
use crate::api::test_support::call;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::test_support::exec;
use crate::error::AppError;

/// Mô hình giả: phần cột của hồ sơ ngữ nghĩa viết mô tả cho mọi cột, kể cả cột đã có mô tả từ database; câu hỏi nhận
/// một câu SQL. Giữ prompt của từng lần gọi kèm task.
struct CommentsLlm {
    prompts: Arc<Mutex<Vec<(LlmTask, String)>>>,
}

#[async_trait]
impl LlmProvider for CommentsLlm {
    fn name(&self) -> String {
        "comments".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.prompts
            .lock()
            .unwrap()
            .push((request.task, request.prompt.to_string()));
        match request.task {
            LlmTask::Profile => Ok(json!({
                "columns": [
                    {"name": "region", "description": "Model text for region", "aliases": ["khu vực"]},
                    {"name": "amount", "description": "Số tiền của đơn", "aliases": ["số tiền"],
                     "unit": "money", "aggregation": "sum"},
                    {"name": "ordered_at", "description": "Ngày đặt hàng", "aliases": ["ngày đặt"]}
                ],
                "values": [],
                "suggested_questions": ["Doanh thu theo vùng"]
            })
            .to_string()),
            LlmTask::Sql => Ok(sql_reply(
                "SELECT region, SUM(amount) AS total FROM orders GROUP BY region ORDER BY region",
            )
            .to_string()),
            // Phân loại, lời giải thích và mô hình chấm lỗi thì luồng hỏi dùng luật và câu do code viết.
            _ => Err(AppError::Upstream("comments model has no reply for this task".to_string())),
        }
    }
}

/// Comment của bảng và cột thành mô tả nguồn database, lớp ngữ nghĩa không ghi đè chúng, cột không có comment (hay
/// comment rỗng) nhận mô tả do mô hình viết; prompt hồ sơ và prompt câu hỏi có mô tả từ database.
#[tokio::test]
async fn database_comments_become_descriptions() {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(CommentsLlm {
        prompts: prompts.clone(),
    });
    let Some(s) = start_with_llm(Some(KEY), llm, None).await else {
        return;
    };
    exec(
        &s.db.store,
        "COMMENT ON TABLE orders IS E'  Đơn hàng bán ra\\n theo vùng '",
    )
    .await;
    exec(
        &s.db.store,
        "COMMENT ON COLUMN orders.region IS 'Vùng giao hàng'",
    )
    .await;
    exec(&s.db.store, "COMMENT ON COLUMN orders.ordered_at IS '   '").await;

    let url = format!("{SHOP}/sources");
    let (status, _, source) = call(&s.app, "POST", &url, Some(&s.lan), Some(s.form.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let id = source["id"].as_str().unwrap();
    let snap = json!({"table": "public.orders"});
    let snapshot_url = format!("{url}/{id}/snapshot");
    let (status, _, result) = call(&s.app, "POST", &snapshot_url, Some(&s.lan), Some(snap)).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(
        result["datasets"][0]["version"]["description"],
        "Đơn hàng bán ra theo vùng"
    );
    let dataset_id = result["datasets"][0]["dataset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dataset_url = format!("{SHOP}/datasets/{dataset_id}");
    wait_for_semantic(&s.app, &s.lan, &dataset_url).await;

    let (status, _, overview) = call(
        &s.app,
        "GET",
        &format!("{dataset_url}/overview"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["description"], "Đơn hàng bán ra theo vùng");
    let column = |name: &str| -> (Value, Value) {
        let c = overview["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("no column {name}: {overview}"));
        (c["description"].clone(), c["description_source"].clone())
    };
    assert_eq!(
        column("region"),
        (json!("Vùng giao hàng"), json!("database"))
    );
    assert_eq!(column("amount"), (json!("Số tiền của đơn"), json!("model")));
    assert_eq!(
        column("ordered_at"),
        (json!("Ngày đặt hàng"), json!("model"))
    );

    let (status, _, metrics) = call(
        &s.app,
        "GET",
        &format!("{dataset_url}/metrics"),
        Some(&s.lan),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{metrics}");
    let region = metrics["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "region")
        .unwrap();
    assert_eq!(region["description_source"], "database");

    // Prompt phần cột: mô tả từ database đi kèm dòng của cột, mô tả bảng đứng đầu phần dữ liệu.
    let profile_prompt = prompts
        .lock()
        .unwrap()
        .iter()
        .find(|(task, p)| *task == LlmTask::Profile && p.contains("documenting the columns"))
        .map(|(_, p)| p.clone())
        .expect("columns profile prompt");
    assert!(
        profile_prompt.contains("description: \"Vùng giao hàng\""),
        "{profile_prompt}"
    );
    assert!(
        profile_prompt.contains("Table description: \"Đơn hàng bán ra theo vùng\""),
        "{profile_prompt}"
    );

    let ask = json!({"question": "Doanh thu theo vùng", "choices": []});
    let (status, _, answer) = call(
        &s.app,
        "POST",
        &format!("{dataset_url}/ask"),
        Some(&s.lan),
        Some(ask),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let sql_prompt = prompts
        .lock()
        .unwrap()
        .iter()
        .find(|(task, _)| *task == LlmTask::Sql)
        .map(|(_, p)| p.clone())
        .expect("sql prompt");
    assert!(sql_prompt.contains("Vùng giao hàng"), "{sql_prompt}");
    assert!(
        !sql_prompt.contains("Model text for region"),
        "{sql_prompt}"
    );
    s.finish().await;
}

/// Lớp ngữ nghĩa chạy nền sau khi chụp trả 201: chờ tới khi phiên bản hiện hành ở semantic.
async fn wait_for_semantic(app: &axum::Router, session: &str, dataset_url: &str) {
    for _ in 0..200 {
        let (_, _, dataset) = call(app, "GET", dataset_url, Some(session), None).await;
        if dataset["current_profile_status"] == "semantic" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("semantic profile did not finish");
}
