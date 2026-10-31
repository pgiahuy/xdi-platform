// SPDX-License-Identifier: Apache-2.0
/* api/ask_privacy_tests.rs - qua route hỏi thật: không prompt nào gửi mô hình chứa dòng của bảng kết quả hay giá trị
của cột nhạy cảm (docs/PRIVACY.md). Dataset có cột "Customer Email" (luật của service/profile.rs đánh dấu nhạy cảm theo
tên). Ba câu: theo nhóm của cột phân loại (mô hình viết lời từ câu do code tính, lời đó thành đoạn trả lời), theo nhóm
của cột nhạy cảm và danh sách dòng không gộp (cả hai không gọi mô hình viết lời, đoạn trả lời là câu khuôn). Mô hình
giả ghi lại mọi prompt. Không có DATABASE_URL thì bỏ qua. */

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use crate::api::ask_tests::{sql_reply, start_on};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;

const CSV: &str = "Market,Customer Email,Order Item Total\n\
                   Europe,an.nguyen@shop.vn,100.5\n\
                   LATAM,binh.tran@shop.vn,200\n\
                   Europe,chi.le@shop.vn,50\n";

const BY_MARKET: &str = "Doanh thu theo thị trường";
const BY_EMAIL: &str = "Doanh thu theo email khách hàng";
const LIST: &str = "Danh sách đơn ở Europe";

/// Lời mô hình viết cho phần giải thích theo thị trường: chỉ chép số trong các câu do code tính.
const MARKET_TEXT: &str = "LATAM đứng đầu với 200, chiếm 57,1% tổng doanh thu.";

/// Mô hình giả: SQL theo câu hỏi, lời viết sẵn cho task insight; ghi mọi request.
struct RecordingLlm {
    requests: Arc<Mutex<Vec<(LlmTask, String, String)>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlm {
    fn name(&self) -> String {
        "recording".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.requests.lock().unwrap().push((
            request.task,
            request.question.to_string(),
            request.prompt.to_string(),
        ));
        let sql = match request.question {
            BY_MARKET => {
                r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market""#
            }
            BY_EMAIL => {
                r#"SELECT "Customer Email", SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1"#
            }
            _ => {
                r#"SELECT "Customer Email", "Order Item Total" FROM dataco WHERE "Market" = 'Europe'"#
            }
        };
        match request.task {
            LlmTask::Sql => Ok(sql_reply(sql).to_string()),
            LlmTask::Insight => Ok(MARKET_TEXT.to_string()),
            // Mô hình chấm (cổng 2, #201) thấy lời khớp; prompt của nó cũng được ghi và kiểm như mọi prompt bên dưới.
            LlmTask::Judge => Ok(r#"{"verdict": "match", "problems": []}"#.to_string()),
            // Hồ sơ ngữ nghĩa và phân loại lỗi tạm: phân loại dùng luật từ khóa.
            _ => Err(AppError::Upstream("not in this test".to_string())),
        }
    }
}

#[tokio::test]
async fn prompts_never_carry_result_rows_or_sensitive_values() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(RecordingLlm {
        requests: Arc::clone(&requests),
    });
    let Some(asking) = start_on(Some(llm), CSV, |c| c.answer_text = true).await else {
        return;
    };

    let (_, market) = asking.ask(BY_MARKET, json!([])).await;
    assert_eq!(market["status"], "answered", "{market}");
    assert_eq!(market["answer_text"], MARKET_TEXT, "{market}");
    assert_eq!(market["answer_text_by_model"], true);

    let (_, email) = asking.ask(BY_EMAIL, json!([])).await;
    assert_eq!(email["status"], "answered", "{email}");
    assert_eq!(email["answer_text_by_model"], false, "{email}");
    assert_eq!(email["insight"]["summary_by_model"], false, "{email}");

    let (_, list) = asking.ask(LIST, json!([])).await;
    assert_eq!(list["status"], "answered", "{list}");
    assert_eq!(list["insight"]["kind"], "rows", "{list}");
    let answer = list["answer_text"].as_str().unwrap();
    assert!(answer.starts_with("Có 2 dòng"), "{answer}");
    assert_eq!(list["answer_text_by_model"], false);

    let requests = requests.lock().unwrap();
    let insight_questions: Vec<&str> = requests
        .iter()
        .filter(|(task, _, _)| *task == LlmTask::Insight)
        .map(|(_, question, _)| question.as_str())
        .collect();
    assert_eq!(insight_questions, [BY_MARKET]);
    for (task, _, prompt) in requests.iter() {
        for email in ["an.nguyen@shop.vn", "binh.tran@shop.vn", "chi.le@shop.vn"] {
            assert!(!prompt.contains(email), "{task:?}: {prompt}");
        }
        // Ô của từng dòng (100.5, 50) không vào prompt nào; tổng do code tính (150,5) thì được.
        for cell in ["100.5", "100,5"] {
            assert!(!prompt.contains(cell), "{task:?}: {prompt}");
        }
    }
}
