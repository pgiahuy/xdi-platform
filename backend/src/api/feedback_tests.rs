// SPDX-License-Identifier: Apache-2.0
/* api/feedback_tests.rs - test đánh giá câu trả lời và câu mẫu qua route thật (#212): người hỏi đánh giá câu của mình,
người khác không đánh giá được; member không duyệt, không xem, không xuất được (403); câu admin duyệt vào prompt của
câu sau có từ chung, câu chưa duyệt hay đã đổi sang "Chưa đạt" thì không; đánh giá hiện lại khi mở cuộc trò chuyện.
Dùng start_on của api/ask_tests.rs và add_member của api/metrics_tests.rs; không có DATABASE_URL thì bỏ qua. */

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::ask_tests::{sql_reply, start_on, Asking};
use crate::api::metrics_tests::add_member;
use crate::api::test_support::call;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;

const CSV: &str = "Market,Order Item Total\nEurope,100.5\nLATAM,200\nEurope,50\n";

const SQL: &str =
    r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market""#;

/// Mô hình giả: câu SQL nào cũng trả cùng một câu, và giữ mọi prompt sinh SQL đã nhận.
struct RecordingLlm {
    prompts: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlm {
    fn name(&self) -> String {
        "recording".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        // Hồ sơ ngữ nghĩa, phân loại và đoạn trả lời lỗi tạm: phân loại dùng luật từ khóa, đoạn trả lời dùng câu khuôn.
        if request.task != LlmTask::Sql {
            return Err(AppError::Upstream("only sql in this test".to_string()));
        }
        self.prompts
            .lock()
            .unwrap()
            .push(request.prompt.to_string());
        Ok(sql_reply(SQL).to_string())
    }
}

struct Rig {
    asking: Asking,
    prompts: Arc<Mutex<Vec<String>>>,
    minh: String,
}

impl Rig {
    async fn start() -> Option<Self> {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let llm = Arc::new(RecordingLlm {
            prompts: Arc::clone(&prompts),
        });
        let asking = start_on(Some(llm), CSV, |_| {}).await?;
        let minh = add_member(&asking).await;
        Some(Rig {
            asking,
            prompts,
            minh,
        })
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        who: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, _, body) = call(&self.asking.app, method, path, Some(who), body).await;
        (status, body)
    }

    /// Minh hỏi một câu; trả response.
    async fn minh_asks(&self, question: &str) -> Value {
        let body = json!({"question": question});
        let (status, response) = self
            .call("POST", &self.asking.ask_url, &self.minh, Some(body))
            .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["status"], "answered", "{response}");
        response
    }

    fn last_prompt(&self) -> String {
        self.prompts.lock().unwrap().last().unwrap().clone()
    }
}

fn feedback_url(query_id: &str) -> String {
    format!("/api/workspaces/cua-hang/queries/{query_id}/feedback")
}

fn approval_url(query_id: &str) -> String {
    format!("/api/workspaces/cua-hang/examples/{query_id}/approval")
}

fn llm_detail(response: &Value) -> Value {
    let steps = response["steps"].as_array().unwrap();
    let llm = steps.iter().find(|s| s["name"] == "llm").unwrap();
    llm["detail"].clone()
}

#[tokio::test]
async fn approved_good_answer_becomes_an_example_and_member_cannot_approve() {
    let Some(rig) = Rig::start().await else {
        return;
    };
    let lan = rig.asking.session.clone();
    let minh = rig.minh.clone();
    let first = rig.minh_asks("Doanh thu theo thị trường").await;
    let query_id = first["query_id"].as_str().unwrap().to_string();
    let conversation = first["conversation_id"].as_str().unwrap().to_string();

    // Câu của Minh: Lan (kể cả admin) không đánh giá thay được.
    let good = json!({"rating": "good"});
    let (status, _) = rig
        .call("PUT", &feedback_url(&query_id), &lan, Some(good.clone()))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let with_reason = json!({"rating": "good", "reason": "other"});
    let (status, _) = rig
        .call("PUT", &feedback_url(&query_id), &minh, Some(with_reason))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, rated) = rig
        .call("PUT", &feedback_url(&query_id), &minh, Some(good))
        .await;
    assert_eq!(status, StatusCode::OK, "{rated}");
    assert_eq!(
        (rated["rating"].as_str(), rated["approved"].as_bool()),
        (Some("good"), Some(false))
    );

    // Câu "Tốt" chưa duyệt không vào prompt.
    rig.minh_asks("Doanh thu theo tháng").await;
    assert!(
        !rig.last_prompt().contains("Earlier questions"),
        "{}",
        rig.last_prompt()
    );

    // Member không xem, không duyệt, không xuất được.
    let examples = "/api/workspaces/cua-hang/examples";
    let export = rig.asking.ask_url.replace("/ask", "/examples/export");
    for (method, path) in [
        ("GET", examples.to_string()),
        ("PUT", approval_url(&query_id)),
        ("DELETE", approval_url(&query_id)),
        ("GET", export.clone()),
    ] {
        let (status, body) = rig.call(method, &path, &minh, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
    }

    let (status, list) = rig.call("GET", examples, &lan, None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let item = &list["examples"][0];
    assert_eq!(list["examples"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(item["question"], "Doanh thu theo thị trường");
    assert_eq!(item["sql"], SQL);
    assert_eq!(item["rated_by"], "minh@example.com");
    assert_eq!(item["approved_at"], Value::Null);

    let (status, _) = rig.call("PUT", &approval_url(&query_id), &lan, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, list) = rig.call("GET", examples, &lan, None).await;
    assert_eq!(list["examples"][0]["approved_by"], "lan@example.com");
    assert!(list["examples"][0]["approved_at"].is_string(), "{list}");

    // Câu sau có từ chung ("doanh", "thu") nhận câu mẫu; bước llm nêu câu mẫu đã dùng.
    let next = rig.minh_asks("Doanh thu theo tháng").await;
    let prompt = rig.last_prompt();
    assert!(
        prompt.contains(&format!(
            "Earlier questions answered correctly:\nExample question: Doanh thu theo thị trường\nExample SQL: {SQL}"
        )),
        "{prompt}"
    );
    assert_eq!(
        llm_detail(&next),
        "Prompt có 1 câu mẫu đã duyệt: \"Doanh thu theo thị trường\""
    );
    // Câu không có từ chung thì không.
    rig.minh_asks("Số đơn hàng").await;
    assert!(!rig.last_prompt().contains("Earlier questions"));

    let (status, cases) = rig.call("GET", &export, &lan, None).await;
    assert_eq!(status, StatusCode::OK, "{cases}");
    assert_eq!(cases[0]["question"], "Doanh thu theo thị trường");
    assert_eq!(cases[0]["expected_sql"], json!([SQL]));
    assert_eq!(cases[0]["ordered"], false);
    assert!(cases[0]["id"].as_str().unwrap().starts_with("approved-"));

    // Mở lại cuộc trò chuyện: đánh giá và nhãn đã duyệt hiện lại.
    let conversation_url = rig
        .asking
        .ask_url
        .replace("/ask", &format!("/conversations/{conversation}"));
    let (_, opened) = rig.call("GET", &conversation_url, &minh, None).await;
    let turn = &opened["turns"][0];
    assert_eq!(turn["feedback"]["rating"], "good", "{opened}");
    assert_eq!(turn["feedback"]["approved"], true);

    // Đổi sang "Chưa đạt": lần duyệt mất, câu không còn trong prompt, cũng không còn trong bản xuất.
    let bad =
        json!({"rating": "bad", "reason": "wrong_numbers", "note": "  Thiếu thị trường Africa  "});
    let (status, rated) = rig
        .call("PUT", &feedback_url(&query_id), &minh, Some(bad))
        .await;
    assert_eq!(status, StatusCode::OK, "{rated}");
    assert_eq!(rated["reason"], "wrong_numbers");
    assert_eq!(rated["note"], "Thiếu thị trường Africa");
    assert_eq!(rated["approved"], false);
    rig.minh_asks("Doanh thu theo tháng").await;
    assert!(!rig.last_prompt().contains("Earlier questions"));
    let (_, cases) = rig.call("GET", &export, &lan, None).await;
    assert_eq!(cases, json!([]));
    let (_, list) = rig.call("GET", examples, &lan, None).await;
    assert_eq!(list["examples"], json!([]));

    // Bấm lại nút đang chọn: bỏ đánh giá.
    let (status, _) = rig
        .call("DELETE", &feedback_url(&query_id), &minh, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, opened) = rig.call("GET", &conversation_url, &minh, None).await;
    assert_eq!(opened["turns"][0]["feedback"], Value::Null, "{opened}");
}

#[tokio::test]
async fn approval_needs_a_good_rating() {
    let Some(rig) = Rig::start().await else {
        return;
    };
    let lan = rig.asking.session.clone();
    let response = rig.minh_asks("Doanh thu theo thị trường").await;
    let query_id = response["query_id"].as_str().unwrap();
    // Chưa ai đánh giá "Tốt": không có gì để duyệt.
    let (status, _) = rig.call("PUT", &approval_url(query_id), &lan, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = rig
        .call("PUT", &approval_url("not-a-uuid"), &lan, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let long = "x".repeat(501);
    let bad = json!({"rating": "bad", "note": long});
    let (status, _) = rig
        .call("PUT", &feedback_url(query_id), &rig.minh, Some(bad))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
