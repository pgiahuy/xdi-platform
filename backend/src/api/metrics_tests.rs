// SPDX-License-Identifier: Apache-2.0
/* api/metrics_tests.rs - test API chỉ số và cách gộp mặc định qua route thật (#185): member xem mà không sửa được,
admin tạo, sửa thành phiên bản mới, xóa; chỉ số sai báo đúng trường; chỉ số đã lưu và cách gộp avg tới prompt sinh SQL
ở câu hỏi kế tiếp. Dùng start_on của api/ask_tests.rs; không có DATABASE_URL thì bỏ qua. */

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::ask_tests::{sql_reply, start_on, Asking};
use crate::api::test_support::call;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;

const CSV: &str = "Market,Order Region,Order Item Total,Days\n\
                   Europe,Western Europe,100.5,3.5\n\
                   LATAM,South America,200,5\n\
                   Europe,Northern Europe,50,4\n";

/// Mô hình giả trả lần lượt các câu viết sẵn và giữ lại mọi prompt nhận được, như ở api/conversation_tests.rs. Test
/// của khái niệm ngành (api/domains_tests.rs) dùng lại.
pub(super) struct RecordingLlm {
    pub(super) replies: Mutex<VecDeque<String>>,
    pub(super) prompts: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlm {
    fn name(&self) -> String {
        "recording".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        // Lớp ngữ nghĩa chạy nền, bước phân loại câu hỏi và đoạn trả lời cũng gọi mô hình; lỗi tạm thì phân loại dùng
        // luật từ khóa, hai bước kia bỏ qua.
        if matches!(
            request.task,
            LlmTask::Profile | LlmTask::Classify | LlmTask::Insight
        ) {
            return Err(AppError::Upstream(
                "no profile, classify or summary".to_string(),
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

fn metrics_url(asking: &Asking) -> String {
    asking.ask_url.replace("/ask", "/metrics")
}

fn aggregation_url(asking: &Asking) -> String {
    asking.ask_url.replace("/ask", "/columns/aggregation")
}

fn revenue(expression: &str) -> Value {
    json!({"name": "Doanh thu", "description": "Tổng tiền sau giảm giá", "expression": expression,
           "time_column": null, "unit": "USD"})
}

/// Lan (admin) tạo một tài khoản và thêm vào workspace làm member; trả phiên của người đó.
pub(super) async fn add_member(asking: &Asking) -> String {
    let user = json!({"email": "minh@example.com", "password": "a long password"});
    let (status, _, _) = call(
        &asking.app,
        "POST",
        "/api/admin/users",
        Some(&asking.session),
        Some(user),
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
    let login = json!({"email": "minh@example.com", "password": "a long password"});
    let (_, session, _) = call(&asking.app, "POST", "/api/auth/login", None, Some(login)).await;
    session.unwrap()
}

#[tokio::test]
async fn admin_manages_metrics_and_member_only_reads() {
    let Some(asking) = start_on(None, CSV, |_| {}).await else {
        return;
    };
    let (app, lan) = (&asking.app, Some(asking.session.as_str()));
    let minh = add_member(&asking).await;
    let url = metrics_url(&asking);

    let (status, _, body) = call(app, "GET", &url, Some(&minh), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["metrics"], json!([]));
    let days = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Days")
        .unwrap();
    assert_eq!(
        (days["role"].as_str(), days["aggregation"].as_str()),
        (Some("measure"), Some("sum"))
    );
    let market = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Market")
        .unwrap();
    assert!(market["aggregation"].is_null());

    // Member không ghi được gì.
    let (status, _, _) = call(
        app,
        "POST",
        &url,
        Some(&minh),
        Some(revenue("SUM(\"Order Item Total\")")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let set_avg = json!({"column": "Days", "aggregation": "avg"});
    let (status, _, _) = call(
        app,
        "PUT",
        &aggregation_url(&asking),
        Some(&minh),
        Some(set_avg.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, created) = call(
        app,
        "POST",
        &url,
        lan,
        Some(revenue("SUM(\"Order Item Total\")")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(
        (created["version"].as_i64(), created["origin"].as_str()),
        (Some(1), Some("ui"))
    );
    assert_eq!(created["confirmed"], true);
    let id = created["id"].as_str().unwrap().to_string();
    // Trùng tên, kể cả khác hoa thường và dấu, thì báo trường name.
    let mut same = revenue("SUM(\"Order Item Total\")");
    same["name"] = json!("doanh THU");
    let (status, _, body) = call(app, "POST", &url, lan, Some(same)).await;
    assert_eq!(
        (status, body["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("name")),
        "{body}"
    );

    let one = format!("{url}/{id}");
    let (status, _, _) = call(
        app,
        "PUT",
        &one,
        Some(&minh),
        Some(revenue("SUM(\"Days\")")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, edited) = call(
        app,
        "PUT",
        &one,
        lan,
        Some(revenue("SUM(\"Order Item Total\") - 1")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["version"], 2);
    let mut renamed = revenue("SUM(\"Days\")");
    renamed["name"] = json!("Lợi nhuận");
    let (status, _, body) = call(app, "PUT", &one, lan, Some(renamed)).await;
    assert_eq!(
        (status, body["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("name")),
        "{body}"
    );

    let (_, _, body) = call(app, "GET", &url, Some(&minh), None).await;
    assert_eq!(body["metrics"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["metrics"][0]["expression"],
        "SUM(\"Order Item Total\") - 1"
    );

    let (status, _, _) = call(app, "DELETE", &one, Some(&minh), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = call(app, "DELETE", &one, lan, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(app, "DELETE", &one, lan, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, body) = call(app, "GET", &url, lan, None).await;
    assert_eq!(body["metrics"], json!([]));

    // Cách gộp chỉ đổi được ở cột measure.
    let (status, _, _) = call(app, "PUT", &aggregation_url(&asking), lan, Some(set_avg)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let on_market = json!({"column": "Market", "aggregation": "avg"});
    let (status, _, body) = call(app, "PUT", &aggregation_url(&asking), lan, Some(on_market)).await;
    assert_eq!(
        (status, body["field"].as_str()),
        (StatusCode::BAD_REQUEST, Some("column")),
        "{body}"
    );
    let (_, _, body) = call(app, "GET", &url, lan, None).await;
    let days = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Days")
        .unwrap();
    assert_eq!(days["aggregation"], "avg");
}

#[tokio::test]
async fn invalid_metric_names_the_wrong_field() {
    let Some(asking) = start_on(None, CSV, |_| {}).await else {
        return;
    };
    let url = metrics_url(&asking);
    let cases = [
        (revenue("SUM(\"Revenue\")"), "expression", "Revenue"),
        (revenue("\"Order Item Total\""), "expression", "gộp"),
        (revenue("SUM(\"Market\")"), "expression", "không chạy được"),
        (
            revenue("SUM(\"Order Item Total\") FROM dataco"),
            "expression",
            "không đọc được",
        ),
        (
            revenue("SUM(read_csv('x.csv'))"),
            "expression",
            "bộ kiểm SQL",
        ),
        (revenue(""), "expression", "trống"),
        (
            json!({"name": " ", "expression": "COUNT(*)"}),
            "name",
            "trống",
        ),
        (
            json!({"name": "a", "expression": "COUNT(*)", "time_column": "Ngày"}),
            "time_column",
            "Ngày",
        ),
        (
            json!({"name": "a", "expression": "COUNT(*)", "filter": "\"Market\" = 1"}),
            "filter",
            "không chạy được",
        ),
        (
            json!({"name": "a", "expression": "COUNT(*)", "filter": "COUNT(*) > 1"}),
            "filter",
            "gộp",
        ),
        (
            json!({"name": "a", "expression": "COUNT(*)", "filter": "\"Order Item Total\" > 0 -- ghi chú"}),
            "filter",
            "chú thích",
        ),
        (
            json!({"name": "Doanh thu \"net\"", "expression": "COUNT(*)"}),
            "name",
            "ngoặc kép",
        ),
    ];
    for (body, field, words) in cases {
        let (status, _, reply) = call(
            &asking.app,
            "POST",
            &url,
            Some(&asking.session),
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} {reply}");
        assert_eq!(reply["field"], field, "{body} {reply}");
        let message = reply["message"].as_str().unwrap();
        assert!(message.contains(words), "{body} {message}");
    }
    let (_, _, body) = call(&asking.app, "GET", &url, Some(&asking.session), None).await;
    assert_eq!(body["metrics"], json!([]));
}

/// Tiêu chí của #185: chỉ số admin vừa định nghĩa và cách gộp avg vào prompt của câu hỏi kế tiếp.
#[tokio::test]
async fn saved_metric_and_avg_column_reach_the_sql_prompt() {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let reply = sql_reply(
        "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco GROUP BY \"Market\"",
    );
    let llm = Arc::new(RecordingLlm {
        replies: Mutex::new(VecDeque::from([reply.to_string()])),
        prompts: prompts.clone(),
    });
    let Some(asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
    let lan = Some(asking.session.as_str());
    let (status, _, body) = call(
        &asking.app,
        "POST",
        &metrics_url(&asking),
        lan,
        Some(revenue("SUM(\"Order Item Total\")")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let set_avg = json!({"column": "Days", "aggregation": "avg"});
    let (status, _, _) = call(
        &asking.app,
        "PUT",
        &aggregation_url(&asking),
        lan,
        Some(set_avg),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, answer) = asking.ask("Doanh thu theo thị trường", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let prompts = prompts.lock().unwrap().clone();
    let prompt = &prompts[0];
    assert!(
        prompt.contains("- \"Doanh thu\": SUM(\"Order Item Total\"), unit USD"),
        "{prompt}"
    );
    assert!(
        prompt.contains(
            "\"Days\" (DOUBLE, measure, one value per row: aggregate it with AVG, never SUM)"
        ),
        "{prompt}"
    );
    // Cột sum ghi cách gộp mặc định (#230).
    assert!(
        prompt.contains("- \"Order Item Total\" (DOUBLE, measure, aggregate with SUM)"),
        "{prompt}"
    );
}
