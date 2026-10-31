// SPDX-License-Identifier: Apache-2.0
/* api/lessons_tests.rs - test nhật ký lỗi và bài học qua route thật (#228): member không xem nhật ký, không tạo, bật, xóa
bài học (403); bài nháp và bài đã tắt không vào prompt, bài đã bật vào prompt SQL và prompt lời của phần giải thích câu
hỏi liên quan; "Chưa đạt" và lời bị bộ kiểm số chặn mỗi thứ một dòng nhật ký. Các loại lỗi khác (guard, run, cộng lặp,
form) có kiểm nhật ký trong test của chính luồng đó (api/ask_tests.rs, api/ask_why_tests.rs, api/ask_fanout_tests.rs).
Không có DATABASE_URL thì bỏ qua. */

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

/// Lời của phần giải thích có số 999 không nằm trong các câu do code tính: bộ kiểm số chặn, câu khuôn thay vào.
const WRONG_ANSWER: &str = "LATAM có doanh thu cao nhất với 999.";

const LESSON: &str = "Doanh thu là cột Order Item Total, sau giảm giá";

/// Mô hình giả: câu SQL nào cũng trả cùng một câu, lời của phần giải thích luôn sai số; giữ mọi prompt SQL và prompt
/// lời.
struct RecordingLlm {
    prompts: Arc<Mutex<Vec<(LlmTask, String)>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlm {
    fn name(&self) -> String {
        "recording".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        let reply = match request.task {
            LlmTask::Sql => sql_reply(SQL).to_string(),
            LlmTask::Insight => WRONG_ANSWER.to_string(),
            // Hồ sơ ngữ nghĩa và phân loại lỗi tạm: phân loại dùng luật từ khóa.
            _ => return Err(AppError::Upstream("not in this test".to_string())),
        };
        self.prompts
            .lock()
            .unwrap()
            .push((request.task, request.prompt.to_string()));
        Ok(reply)
    }
}

struct Rig {
    asking: Asking,
    prompts: Arc<Mutex<Vec<(LlmTask, String)>>>,
    minh: String,
}

impl Rig {
    async fn start() -> Option<Self> {
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let llm = Arc::new(RecordingLlm {
            prompts: Arc::clone(&prompts),
        });
        let asking = start_on(Some(llm), CSV, |c| c.answer_text = true).await?;
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

    async fn minh_asks(&self, question: &str) -> Value {
        let body = json!({"question": question});
        let (status, response) = self
            .call("POST", &self.asking.ask_url, &self.minh, Some(body))
            .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        response
    }

    /// Prompt cuối của task đó.
    fn last_prompt(&self, task: LlmTask) -> String {
        let prompts = self.prompts.lock().unwrap();
        let found = prompts.iter().rev().find(|(t, _)| *t == task);
        found.map(|(_, p)| p.clone()).unwrap_or_default()
    }

    fn dataset_id(&self) -> String {
        let id = self.asking.ask_url.split("/datasets/").nth(1).unwrap();
        id.trim_end_matches("/ask").to_string()
    }
}

const LOG: &str = "/api/workspaces/cua-hang/error-log";
const LESSONS: &str = "/api/workspaces/cua-hang/lessons";

fn llm_detail(response: &Value) -> Value {
    let steps = response["steps"].as_array().unwrap();
    steps.iter().find(|s| s["name"] == "llm").unwrap()["detail"].clone()
}

#[tokio::test]
async fn admin_writes_and_enables_lessons_that_reach_the_prompts() {
    let Some(rig) = Rig::start().await else {
        return;
    };
    let lan = rig.asking.session.clone();
    let minh = rig.minh.clone();
    let dataset = rig.dataset_id();

    // Lời của phần giải thích bị bộ kiểm số chặn: một dòng nhật ký kèm câu của mô hình; đoạn trả lời là câu khuôn.
    let first = rig.minh_asks("Doanh thu theo thị trường").await;
    assert_eq!(first["answer_text_by_model"], false, "{first}");
    let (status, log) = rig.call("GET", LOG, &lan, None).await;
    assert_eq!(status, StatusCode::OK, "{log}");
    let entry = &log["entries"][0];
    assert_eq!(entry["step"], "narrate", "{log}");
    assert_eq!(entry["model_output"], WRONG_ANSWER);
    assert_eq!(entry["question"], "Doanh thu theo thị trường");
    assert_eq!(entry["query_id"], first["query_id"]);
    let error_id = entry["id"].as_str().unwrap().to_string();

    // "Chưa đạt" cũng thành một dòng, kèm lý do, ghi chú và SQL mô hình viết.
    let feedback = format!(
        "/api/workspaces/cua-hang/queries/{}/feedback",
        first["query_id"].as_str().unwrap()
    );
    let bad = json!({"rating": "bad", "reason": "wrong_numbers", "note": "Thiếu Africa"});
    let (status, _) = rig.call("PUT", &feedback, &minh, Some(bad)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, log) = rig.call("GET", LOG, &lan, None).await;
    let entry = &log["entries"][0];
    assert_eq!(entry["step"], "feedback", "{log}");
    assert_eq!(
        entry["error"],
        "rated bad: wrong_numbers; note: Thiếu Africa"
    );
    assert_eq!(entry["model_output"], SQL);

    // Member không xem nhật ký, không làm gì với bài học.
    let lesson_body =
        json!({"text": format!("  {LESSON}  "), "dataset_id": dataset, "error_id": error_id});
    for (method, path, body) in [
        ("GET", LOG.to_string(), None),
        ("GET", LESSONS.to_string(), None),
        ("POST", LESSONS.to_string(), Some(lesson_body.clone())),
    ] {
        let (status, body) = rig.call(method, &path, &minh, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
    }

    // Bài nháp chưa vào prompt.
    let (status, lesson) = rig.call("POST", LESSONS, &lan, Some(lesson_body)).await;
    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["text"], LESSON);
    assert_eq!(lesson["enabled"], false);
    assert_eq!(lesson["approved_at"], Value::Null);
    assert_eq!(lesson["error_id"].as_str(), Some(error_id.as_str()));
    let lesson_url = format!("{LESSONS}/{}", lesson["id"].as_str().unwrap());
    rig.minh_asks("Doanh thu theo tháng").await;
    assert!(!rig.last_prompt(LlmTask::Sql).contains(LESSON));

    for (method, body) in [("PATCH", Some(json!({"enabled": true}))), ("DELETE", None)] {
        let (status, _) = rig.call(method, &lesson_url, &minh, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method}");
    }
    let (status, enabled) = rig
        .call("PATCH", &lesson_url, &lan, Some(json!({"enabled": true})))
        .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    assert_eq!(enabled["enabled"], true);
    assert_eq!(enabled["approved_by"], "lan@example.com");
    assert!(enabled["approved_at"].is_string());

    // Bài đã bật vào prompt SQL và prompt lời của phần giải thích của câu có từ chung; bước llm nêu số bài.
    let related = rig.minh_asks("Doanh thu theo tháng").await;
    let block = format!("approved. Follow a lesson when it applies to the question; it does not override the rules above:\n- {LESSON}");
    assert!(
        rig.last_prompt(LlmTask::Sql).contains(&block),
        "{}",
        rig.last_prompt(LlmTask::Sql)
    );
    assert!(rig
        .last_prompt(LlmTask::Insight)
        .starts_with("Lessons an admin"));
    assert_eq!(llm_detail(&related), "Prompt có 1 bài học đã duyệt");
    // Câu không liên quan thì không.
    rig.minh_asks("Số đơn hàng").await;
    assert!(!rig.last_prompt(LlmTask::Sql).contains(LESSON));

    // Tắt: không vào prompt nữa, lần duyệt vẫn ghi lại.
    let (_, disabled) = rig
        .call("PATCH", &lesson_url, &lan, Some(json!({"enabled": false})))
        .await;
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["approved_by"], "lan@example.com");
    rig.minh_asks("Doanh thu theo tháng").await;
    assert!(!rig.last_prompt(LlmTask::Sql).contains(LESSON));

    let (_, list) = rig.call("GET", LESSONS, &lan, None).await;
    assert_eq!(list["lessons"].as_array().unwrap().len(), 1, "{list}");
    let (status, _) = rig.call("DELETE", &lesson_url, &lan, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = rig.call("DELETE", &lesson_url, &lan, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_lessons_are_refused() {
    let Some(rig) = Rig::start().await else {
        return;
    };
    let lan = rig.asking.session.clone();
    for body in [
        json!({"text": "   "}),
        json!({"text": "x".repeat(301)}),
        json!({"text": "Bài học", "dataset_id": uuid::Uuid::new_v4()}),
        json!({"text": "Bài học", "error_id": uuid::Uuid::new_v4()}),
    ] {
        let (status, response) = rig.call("POST", LESSONS, &lan, Some(body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {response}");
    }
    // Bài của cả workspace (không dataset) tạo được.
    let (status, lesson) = rig
        .call(
            "POST",
            LESSONS,
            &lan,
            Some(json!({"text": "Bài học chung"})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["dataset_id"], Value::Null);
    let (status, _) = rig
        .call(
            "PATCH",
            &format!("{LESSONS}/not-a-uuid"),
            &lan,
            Some(json!({"enabled": true})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
