// SPDX-License-Identifier: Apache-2.0
/* api/conversation_tests.rs - test cuộc trò chuyện qua route thật (#164): câu tiếp nối gửi mô hình khối lịch sử mà
không có dòng dữ liệu hay giá trị kết quả, các lượt lưu và đọc lại được, người khác không đọc hay hỏi tiếp được cuộc
của mình. Dùng dataset dataco nhỏ của api/ask_tests.rs; không có DATABASE_URL thì bỏ qua. */

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::ask_tests::{sql_reply, start_on, Asking};
use crate::api::test_support::call;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::test_support::fetch_count;
use crate::error::AppError;

const CSV: &str = "Market,Order Region,Order Item Total\n\
                   Europe,Western Europe,100.5\n\
                   LATAM,South America,200\n\
                   Europe,Northern Europe,50\n";

/// Mô hình giả trả lần lượt các câu viết sẵn và giữ lại mọi prompt nhận được.
struct RecordingLlm {
    replies: Mutex<VecDeque<String>>,
    prompts: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlm {
    fn name(&self) -> String {
        "recording".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        // Lớp ngữ nghĩa chạy nền sau khi nạp file cũng gọi mô hình; lỗi tạm thì nó bỏ qua (như ScriptedLlm).
        // Đoạn trả lời của câu thường (task summary) cũng gọi mô hình; lỗi thì luồng hỏi dùng câu khuôn. Bước phân
        // loại (task classify) lỗi thì dùng luật từ khóa, nên câu viết sẵn chỉ dành cho SQL và form như trước.
        if matches!(
            request.task,
            LlmTask::Profile | LlmTask::Classify | LlmTask::Insight
        ) {
            return Err(AppError::Upstream(
                "recording model has no profile or summary".to_string(),
            ));
        }
        self.prompts
            .lock()
            .unwrap()
            .push(request.prompt.to_string());
        let reply = self.replies.lock().unwrap().pop_front();
        Ok(reply.expect("model called more times than the test expects"))
    }
}

async fn ask_in(
    asking: &Asking,
    session: &str,
    question: &str,
    conversation: Value,
) -> (StatusCode, Value) {
    let body = json!({"question": question, "conversation_id": conversation});
    let (status, _, response) = call(
        &asking.app,
        "POST",
        &asking.ask_url,
        Some(session),
        Some(body),
    )
    .await;
    (status, response)
}

fn conversations_url(asking: &Asking) -> String {
    asking.ask_url.replace("/ask", "/conversations")
}

#[tokio::test]
async fn follow_up_gets_earlier_turns_without_data() {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let replies = [
        sql_reply("SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco GROUP BY \"Market\""),
        sql_reply(
            "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco WHERE \"Market\" = 'Europe' \
             GROUP BY \"Market\"",
        ),
    ];
    let llm = Arc::new(RecordingLlm {
        replies: Mutex::new(replies.iter().map(Value::to_string).collect()),
        prompts: prompts.clone(),
    });
    let Some(asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
    let session = asking.session.clone();

    let (status, first) = ask_in(&asking, &session, "Doanh thu theo thị trường", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "answered", "{first}");
    // Tổng của Europe chỉ có trong kết quả, không có trong file nạp: thấy nó trong prompt là kết quả đã bị gửi đi.
    assert!(first["result"].to_string().contains("150.5"), "{first}");
    let conversation = first["conversation_id"].clone();
    assert!(conversation.is_string(), "{first}");

    let (status, second) = ask_in(
        &asking,
        &session,
        "Còn châu Âu thì sao?",
        conversation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["conversation_id"], conversation);

    let prompts = prompts.lock().unwrap().clone();
    assert_eq!(prompts.len(), 2);
    assert!(!prompts[0].contains("Earlier turns"), "{}", prompts[0]);
    let follow_up = &prompts[1];
    assert!(
        follow_up.contains("Earlier turns of this conversation"),
        "{follow_up}"
    );
    assert!(
        follow_up.contains("Turn 1: Doanh thu theo thị trường"),
        "{follow_up}"
    );
    assert!(follow_up.contains("GROUP BY \"Market\""), "{follow_up}");
    assert!(follow_up.contains("Understood as: "), "{follow_up}");
    // Khối lịch sử đứng trước câu hỏi cuối.
    assert!(
        follow_up
            .trim_end()
            .ends_with("Question: Còn châu Âu thì sao?"),
        "{follow_up}"
    );
    for data in ["150.5", "100.5"] {
        assert!(
            !follow_up.contains(data),
            "{data} must not reach the model:\n{follow_up}"
        );
    }

    // Hai lượt lưu cùng cuộc, đọc lại theo thứ tự hỏi.
    let linked = fetch_count(
        &asking.db.store,
        &format!(
            "SELECT count(*) FROM queries WHERE conversation_id = '{}'",
            conversation.as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(linked, 2);
    let url = format!(
        "{}/{}",
        conversations_url(&asking),
        conversation.as_str().unwrap()
    );
    let (status, _, opened) = call(&asking.app, "GET", &url, Some(&session), None).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let turns = opened["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["question"], "Doanh thu theo thị trường");
    assert_eq!(turns[1]["status"], "answered");
    assert_eq!(turns[0]["query_id"], first["query_id"]);
    assert!(turns[0]["understood"].is_string(), "{opened}");
    let (status, _, listed) = call(
        &asking.app,
        "GET",
        &conversations_url(&asking),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["conversations"][0]["id"], conversation);
    assert_eq!(
        listed["conversations"][0]["title"],
        "Doanh thu theo thị trường"
    );
    asking.db.drop().await;
}

#[tokio::test]
async fn other_users_cannot_read_or_continue_a_conversation() {
    // Mô hình là mock: câu này có trong mock_answers.json.
    let Some(asking) = start_on(None, CSV, |_| {}).await else {
        return;
    };
    let lan = asking.session.clone();
    let (status, first) = ask_in(&asking, &lan, "Doanh thu theo thị trường", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let conversation = first["conversation_id"].as_str().unwrap().to_string();

    // Minh là admin cùng workspace: vẫn không đọc được cuộc của Lan.
    let new_user = json!({"email": "minh@example.com", "password": "a long password"});
    let (status, _, _) = call(
        &asking.app,
        "POST",
        "/api/admin/users",
        Some(&lan),
        Some(new_user),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "minh@example.com", "role": "admin"});
    let members = "/api/workspaces/cua-hang/members";
    let (status, _, _) = call(&asking.app, "POST", members, Some(&lan), Some(add)).await;
    assert_eq!(status, StatusCode::CREATED);
    let login = json!({"email": "minh@example.com", "password": "a long password"});
    let (_, minh, _) = call(&asking.app, "POST", "/api/auth/login", None, Some(login)).await;
    let minh = minh.unwrap();

    let url = format!("{}/{conversation}", conversations_url(&asking));
    let (status, _, _) = call(&asking.app, "GET", &url, Some(&minh), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, response) = ask_in(
        &asking,
        &minh,
        "Doanh thu theo thị trường",
        json!(conversation),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{response}");
    let (_, _, listed) = call(
        &asking.app,
        "GET",
        &conversations_url(&asking),
        Some(&minh),
        None,
    )
    .await;
    assert_eq!(listed["conversations"], json!([]));

    let (status, _, _) = call(&asking.app, "GET", &url, Some(&lan), None).await;
    assert_eq!(status, StatusCode::OK);
    asking.db.drop().await;
}
