// SPDX-License-Identifier: Apache-2.0
/* api/ask_row_filter_error_tests.rs - qua route hỏi thật, với một member mang vai dữ liệu chỉ thấy LATAM: lời báo lỗi
của DuckDB không đưa ô của dòng Europe (dòng bị ẩn) vào prompt sửa gửi mô hình, response, lịch sử queries hay nhật ký
lỗi. Dòng Europe của CSV mang chữ không đổi được sang số hay ngày. Admin hỏi cùng câu thì lời báo có ô đó: dữ liệu
đúng là làm câu lỗi. Các dạng câu và bốn dạng điều kiện lọc ở service/ask/repair_row_filter_tests.rs. Không có
DATABASE_URL thì bỏ qua. */

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::ask_tests::{sql_reply, start_on, Asking};
use crate::api::test_support::call;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::test_support::{fetch_count, fetch_text, give_data_role};
use crate::error::AppError;

const CSV: &str = "Market,Qty,Ship Day,Order Item Total\n\
                   LATAM,5,1/31/2018,200\n\
                   Europe,Nguyen Van A,secret-date,100.5\n\
                   LATAM,7,2/15/2018,50\n";

/// Chữ chỉ có ở dòng Europe.
const HIDDEN: [&str; 2] = ["Nguyen Van A", "secret-date"];

/// Câu SQL đầu chỉ lỗi ở dòng Europe (Qty không phải số).
const BY_QTY: &str = "Tổng số lượng";
/// Câu SQL đầu lỗi ở mọi dòng: đọc ngày/tháng/năm trong khi dữ liệu ghi tháng/ngày/năm.
const BY_DAY: &str = "Số đơn theo ngày giao";

/// Mô hình giả: câu SQL đầu và câu sửa theo câu hỏi; ghi lại mọi request (loại việc, câu hỏi, prompt).
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
        let sql = match (request.task, request.question) {
            (LlmTask::Sql, BY_QTY) => r#"SELECT SUM(CAST("Qty" AS INTEGER)) AS total FROM dataco"#,
            (LlmTask::Sql, _) => {
                r#"SELECT strptime("Ship Day", '%d/%m/%Y') AS day, COUNT(*) AS n FROM dataco GROUP BY 1"#
            }
            (LlmTask::Repair, BY_QTY) => r#"SELECT COUNT(*) AS n FROM dataco"#,
            (LlmTask::Repair, _) => {
                r#"SELECT "Ship Day", COUNT(*) AS n FROM dataco GROUP BY 1 ORDER BY 1"#
            }
            // Phân loại, hồ sơ ngữ nghĩa, viết lời, chấm: lỗi tạm thì luồng hỏi dùng luật từ khóa và câu do code viết.
            _ => return Err(AppError::Upstream("not in this test".to_string())),
        };
        Ok(sql_reply(sql).to_string())
    }
}

/// Thêm minh vào workspace với vai member chỉ thấy LATAM, rồi hỏi bằng phiên của minh.
async fn switch_to_latam_member(asking: &mut Asking) {
    let user = json!({"email": "minh@example.com", "password": "a long password"});
    let (status, _, _) = call(
        &asking.app,
        "POST",
        "/api/admin/users",
        Some(&asking.session),
        Some(user.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let add = json!({"email": "minh@example.com", "role": "member"});
    let members = "/api/workspaces/cua-hang/members";
    let (status, _, _) = call(
        &asking.app,
        "POST",
        members,
        Some(&asking.session),
        Some(add),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, minh, _) = call(&asking.app, "POST", "/api/auth/login", None, Some(user)).await;
    let dataset = fetch_text(&asking.db.store, "SELECT id::text FROM datasets").await;
    give_data_role(
        &asking.db.store,
        "minh@example.com",
        "latam",
        &[(&dataset, "Market", &["LATAM"])],
    )
    .await;
    asking.session = minh.unwrap();
}

fn hidden_value_in(text: &str) -> Option<&'static str> {
    HIDDEN.into_iter().find(|value| text.contains(value))
}

/// Chi tiết các bước của response, nối thành một chuỗi.
fn step_details(response: &Value) -> String {
    let steps = response["steps"].as_array().unwrap();
    steps
        .iter()
        .filter_map(|s| s["detail"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn run_errors_of_a_filtered_member_quote_only_visible_rows() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(RecordingLlm {
        requests: Arc::clone(&requests),
    });
    let Some(mut asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };

    // Admin thấy mọi dòng: lời báo của lần chạy đầu chép ô của dòng Europe.
    let (status, admin) = asking.ask(BY_QTY, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{admin}");
    assert_eq!(admin["status"], "answered", "{admin}");
    assert!(step_details(&admin).contains("Nguyen Van A"), "{admin}");
    let admin_calls = requests.lock().unwrap().len();

    switch_to_latam_member(&mut asking).await;
    // Câu chỉ lỗi ở dòng Europe: với minh câu chạy được ngay, không vào vòng sửa.
    let (status, qty) = asking.ask(BY_QTY, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{qty}");
    assert_eq!(qty["status"], "answered", "{qty}");
    assert_eq!(qty["result"]["rows"], json!([["12"]]), "{qty}");
    // Câu lỗi cả ở dòng LATAM: lời báo chép ô của dòng LATAM, mô hình sửa theo đó.
    let (status, day) = asking.ask(BY_DAY, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{day}");
    assert_eq!(day["status"], "answered", "{day}");
    assert!(step_details(&day).contains("1/31/2018"), "{day}");

    // Chép ra khỏi khóa: phần sau còn chờ Postgres, không giữ khóa qua await.
    let requests = requests.lock().unwrap().clone();
    let member_requests = &requests[admin_calls..];
    let repairs: Vec<&(LlmTask, String, String)> = member_requests
        .iter()
        .filter(|(task, _, _)| *task == LlmTask::Repair)
        .collect();
    assert_eq!(repairs.len(), 1, "{repairs:?}");
    assert_eq!(repairs[0].1, BY_DAY);
    assert!(repairs[0].2.contains("1/31/2018"), "{}", repairs[0].2);
    for (task, _, prompt) in member_requests {
        assert_eq!(hidden_value_in(prompt), None, "{task:?}: {prompt}");
    }
    for response in [&qty, &day] {
        assert_eq!(hidden_value_in(&response.to_string()), None, "{response}");
    }
    let history = fetch_text(
        &asking.db.store,
        "SELECT string_agg(row_to_json(q)::text, ' ') FROM queries q \
         JOIN users u ON u.id = q.user_id WHERE u.email = 'minh@example.com'",
    )
    .await;
    assert_eq!(hidden_value_in(&history), None, "{history}");
    let errors = fetch_text(
        &asking.db.store,
        &format!("SELECT string_agg(error, ' ') FROM error_log WHERE question = '{BY_DAY}'"),
    )
    .await;
    assert!(errors.contains("1/31/2018"), "{errors}");
    assert_eq!(hidden_value_in(&errors), None, "{errors}");
    // Nhật ký lỗi của câu số lượng chỉ có dòng của admin.
    let qty_errors = format!("SELECT count(*) FROM error_log WHERE question = '{BY_QTY}'");
    assert_eq!(fetch_count(&asking.db.store, &qty_errors).await, 1);
}
