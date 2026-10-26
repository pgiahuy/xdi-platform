// SPDX-License-Identifier: Apache-2.0
/* api/ask_tests.rs - test luồng hỏi qua route thật (#33): nạp một CSV nhỏ tên dataco, đúng bảng logic của câu mẫu trong
engine/llm/mock_answers.json, rồi hỏi và kiểm status, các bước, lịch sử queries. Mô hình là mock, hoặc mô hình giả trả
câu viết sẵn cho ca mock không có (câu DELETE, câu sửa vẫn lỗi, câu hỏi lại). Mỗi test dùng database và thư mục dữ
liệu riêng; không có DATABASE_URL thì bỏ qua. */

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::api::test_support::{app_with, app_with_llm, call, multipart, send_raw, CODE};
use crate::config::{Config, Secret};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::test_support::{exec, fetch_count, fresh_database, TestDb};
use crate::error::AppError;

/// Đủ cột cho các câu mẫu của mock dùng ở đây. Ngày giao hàng ghi tháng/ngày/năm như DataCo, nên câu mẫu đọc
/// ngày/tháng/năm lỗi ở dòng đầu (tháng 31) và câu sửa đọc đúng.
const CSV: &str = "Market,Order Region,Order Item Total,shipping date (DateOrders)\n\
                   Europe,Western Europe,100.5,1/31/2018 22:56\n\
                   LATAM,South America,200,2/1/2018 10:00\n\
                   Europe,Northern Europe,50,2/15/2018 09:30\n";

pub(super) struct DataDir(PathBuf);

impl DataDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-ask-api-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        DataDir(dir)
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Mô hình giả: trả lần lượt các câu viết sẵn cho câu hỏi (sinh SQL và sửa).
struct ScriptedLlm {
    replies: Mutex<VecDeque<String>>,
}

pub(super) fn scripted(replies: &[Value]) -> Arc<dyn LlmProvider> {
    let replies = replies.iter().map(Value::to_string).collect();
    Arc::new(ScriptedLlm {
        replies: Mutex::new(replies),
    })
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        // Nạp file xong, lớp ngữ nghĩa (service/semantic.rs) chạy nền và cũng gọi mô hình. Lỗi tạm thì nó không ghi gì
        // vào hồ sơ, nên câu viết sẵn chỉ dành cho câu hỏi và bí danh test tự đặt không bị ghi đè.
        if request.task == LlmTask::Profile {
            return Err(AppError::Upstream(
                "scripted model has no profile".to_string(),
            ));
        }
        let reply = self.replies.lock().unwrap().pop_front();
        Ok(reply.expect("model called more times than the test expects"))
    }
}

pub(super) fn sql_reply(sql: &str) -> Value {
    json!({"terms": [], "assumptions": [], "clarify": null, "sql": sql})
}

/// Một workspace có dataset dataco đã nạp: database, thư mục dữ liệu, app, phiên đăng nhập, đường dẫn hỏi.
pub(super) struct Asking {
    pub db: TestDb,
    _data: DataDir,
    pub app: axum::Router,
    pub session: String,
    pub ask_url: String,
}

/// llm None là mock của cấu hình.
async fn start(llm: Option<Arc<dyn LlmProvider>>) -> Option<Asking> {
    start_with(llm, |_| {}).await
}

async fn start_with(
    llm: Option<Arc<dyn LlmProvider>>,
    configure: impl FnOnce(&mut Config),
) -> Option<Asking> {
    start_on(llm, CSV, configure).await
}

/// Như start_with, trên một CSV khác (api/ask_why_tests.rs cần cột ngày và nhiều kỳ).
pub(super) async fn start_on(
    llm: Option<Arc<dyn LlmProvider>>,
    csv: &str,
    configure: impl FnOnce(&mut Config),
) -> Option<Asking> {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return None;
    };
    let data = DataDir::new();
    let dir = data.0.clone();
    let configure = move |c: &mut Config| {
        c.data_dir = dir;
        c.llm_provider = "mock".to_string();
        configure(c);
    };
    let app = match llm {
        Some(llm) => app_with_llm(db.store.clone(), Some(CODE), configure, llm),
        None => app_with(db.store.clone(), Some(CODE), configure),
    };
    let body = json!({"setup_code": CODE, "email": "lan@example.com", "password": "a long password",
                      "workspace_name": "Cửa hàng"});
    let (status, session, _) = call(&app, "POST", "/api/auth/setup", None, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    let session = session.unwrap();
    let (content_type, upload) =
        multipart(&[("name", "dataco")], &[("dataco.csv", csv.as_bytes())]);
    let (status, uploaded) = send_raw(
        &app,
        "/api/workspaces/cua-hang/datasets",
        &session,
        &content_type,
        upload,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{uploaded}");
    let dataset_id = uploaded["dataset"]["id"].as_str().unwrap();
    let ask_url = format!("/api/workspaces/cua-hang/datasets/{dataset_id}/ask");
    Some(Asking {
        db,
        _data: data,
        app,
        session,
        ask_url,
    })
}

impl Asking {
    pub(super) async fn ask(&self, question: &str, choices: Value) -> (StatusCode, Value) {
        let body = json!({"question": question, "choices": choices});
        let (status, _, response) = call(
            &self.app,
            "POST",
            &self.ask_url,
            Some(&self.session),
            Some(body),
        )
        .await;
        (status, response)
    }

    async fn queries_with(&self, status: &str) -> i64 {
        let sql = format!("SELECT count(*) FROM queries WHERE status = '{status}'");
        fetch_count(&self.db.store, &sql).await
    }
}

pub(super) fn step_names(response: &Value) -> Vec<&str> {
    response["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn answers_a_question_and_records_it() {
    let Some(asking) = start(None).await else {
        return;
    };
    let question = "  Doanh thu theo thị trường?  ";
    let (status, response) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(response["question"], question);
    assert_eq!(response["model"], "mock");
    assert!(response["query_id"].is_string(), "{response}");
    assert!(response["sql_guarded"]
        .as_str()
        .unwrap()
        .ends_with("LIMIT 100000"));
    assert_eq!(
        response["result"]["rows"],
        json!([["LATAM", 200.0], ["Europe", 150.5]])
    );
    assert_eq!(
        step_names(&response),
        ["context", "llm", "guard", "run", "record"]
    );
    // Giả định "toàn bộ thời gian" của mô hình thành một lựa chọn với id do code dựng.
    assert_eq!(
        response["choices"],
        json!([{"choice_id": "time:range", "option_id": "all"}])
    );
    assert_eq!(asking.queries_with("ok").await, 1);

    // Thẻ, vai trò cột và biểu đồ dựng từ sql_guarded. LIMIT 100000 là trần guard thêm, không phải của câu hỏi, nên
    // thẻ không có mục limit; giả định trên thẻ cùng id với choices.
    let roles: Vec<&str> = response["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["dimension", "measure"]);
    let understanding = &response["understanding"];
    assert_eq!(understanding["group_by"].as_array().unwrap().len(), 1);
    assert_eq!(understanding["measures"].as_array().unwrap().len(), 1);
    assert!(understanding["limit"].is_null(), "{understanding}");
    assert_eq!(understanding["assumptions"][0]["id"], "time:range");
    assert_eq!(
        response["chart"],
        json!({"kind": "bar", "x": "market", "y": ["revenue"]})
    );
}

/// Câu mẫu đọc ngày sai thứ tự, DuckDB báo lỗi, câu sửa của mock chạy được.
#[tokio::test]
async fn repairs_a_failed_run_once() {
    let Some(asking) = start(None).await else {
        return;
    };
    let (status, response) = asking.ask("Doanh thu theo ngày giao hàng", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(
        step_names(&response),
        ["context", "llm", "guard", "run", "repair", "guard", "run", "record"]
    );
    assert!(response["sql_generated"]
        .as_str()
        .unwrap()
        .contains("%m/%d/%Y"));
    // Trục x là ngày nên vẽ đường.
    assert_eq!(
        response["chart"],
        json!({"kind": "line", "x": "ship_day", "y": ["revenue"]})
    );
}

#[tokio::test]
async fn guard_refusal_is_recorded_as_rejected() {
    let llm = scripted(&[sql_reply("DELETE FROM dataco")]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let (status, response) = asking.ask("Xóa hết dữ liệu", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "rejected", "{response}");
    assert_eq!(response["sql_generated"], "DELETE FROM dataco");
    assert!(response["sql_guarded"].is_null());
    assert!(response["query_id"].is_string());
    assert_eq!(step_names(&response), ["context", "llm", "guard", "record"]);
    assert_eq!(asking.queries_with("rejected").await, 1);
    // Không có câu nào chạy nên không có thẻ hay biểu đồ.
    assert!(response["understanding"].is_null());
    assert!(response["chart"].is_null());
}

/// Câu đầu và câu sửa cùng lỗi lúc chạy: failed, vẫn ghi lịch sử kèm câu đã sửa.
#[tokio::test]
async fn failed_run_after_repair_is_recorded_as_error() {
    let broken =
        r#"SELECT strptime("shipping date (DateOrders)", '%d/%m/%Y %H:%M') AS d FROM dataco"#;
    let llm = scripted(&[sql_reply(broken), sql_reply(broken)]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let (status, response) = asking.ask("Ngày giao hàng", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "failed", "{response}");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("sửa một lần"));
    assert_eq!(asking.queries_with("error").await, 1);
}

/// "khu vực" là bí danh của hai cột: hỏi lại trước khi gọi mô hình, không ghi lịch sử. Gửi lại kèm lựa chọn thì trả lời;
/// lựa chọn lạ thì báo choice_not_applied.
#[tokio::test]
async fn ambiguous_word_asks_back_until_the_user_picks() {
    let market =
        r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market""#;
    let llm = scripted(&[sql_reply(market)]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    exec(
        &asking.db.store,
        "UPDATE columns SET aliases = ARRAY['khu vực'] WHERE name IN ('Market', 'Order Region')",
    )
    .await;

    let (status, response) = asking.ask("Doanh thu theo khu vực", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "needs_choice", "{response}");
    assert!(response["query_id"].is_null());
    assert_eq!(step_names(&response), ["context"]);
    let choice = &response["clarifications"][0];
    assert_eq!(choice["id"], "column:khu vuc");
    assert!(choice["selected"].is_null());
    assert_eq!(choice["options"].as_array().unwrap().len(), 2);
    // Người dùng nhận ra cột qua giá trị của nó: nhãn kèm giá trị nhiều dòng nhất, id vẫn là tên cột.
    let market = choice["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == "Market")
        .unwrap();
    assert_eq!(market["label"], "gồm Europe, LATAM (Market)");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("theo cách nào"));
    assert_eq!(asking.queries_with("ok").await, 0);

    let picked = json!([
        {"choice_id": "column:khu vuc", "option_id": "Market"},
        {"choice_id": "column:san pham", "option_id": "Product"}
    ]);
    let (status, response) = asking.ask("Doanh thu theo khu vực", picked).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(
        response["choices"],
        json!([{"choice_id": "column:khu vuc", "option_id": "Market"}])
    );
    let warnings = response["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{response}");
    assert_eq!(warnings[0]["code"], "choice_not_applied");
}

/// Mô hình hỏi lại: phương án không phải cột có thật bị bỏ; không còn phương án nào thì câu là failed và vẫn ghi lịch
/// sử, như đường dự phòng của câu "tại sao" (explain_plain.rs).
#[tokio::test]
async fn model_clarify_keeps_only_real_columns() {
    let clarify = |options: Value| {
        json!({"terms": [], "assumptions": [], "sql": "",
               "clarify": {"kind": "column", "phrase": "khu vực", "question": "Khu vực là cột nào?",
                           "options": options}})
    };
    let llm = scripted(&[
        clarify(json!(["Market", "Order Region", "Country"])),
        clarify(json!(["Country"])),
    ]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let (status, response) = asking.ask("Doanh thu theo khu vực", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "needs_choice", "{response}");
    assert_eq!(response["message"], "Khu vực là cột nào?");
    let options: Vec<&str> = response["clarifications"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert_eq!(options, ["Market", "Order Region"]);

    let (status, response) = asking.ask("Doanh thu theo khu vực", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "failed", "{response}");
    assert!(response["clarifications"].as_array().unwrap().is_empty());
    assert_eq!(asking.queries_with("error").await, 1);
}

/// Mô hình giả như ScriptedLlm, giữ lại prompt của từng câu hỏi để test đọc.
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
        if request.task == LlmTask::Profile {
            return Err(AppError::Upstream(
                "recording model has no profile".to_string(),
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

/// Câu Gemini thật trả cho "Doanh thu từng tháng năm 2017" trên dataset chưa có chỉ số: hỏi lại loại metric với
/// phương án là tên cột. Phản hồi là câu hỏi lại về cột (chỉ cột độ đo có thật); gửi lại kèm lựa chọn thì prompt có
/// cột đã chọn, câu được trả lời, không có choice_not_applied.
#[tokio::test]
async fn metric_clarify_with_columns_asks_for_a_column_and_applies_it() {
    let clarify = json!({"terms": [], "assumptions": [], "sql": "",
        "clarify": {"kind": "metric", "phrase": "doanh thu", "question": "Bạn muốn tính doanh thu bằng cột nào?",
                    "options": ["Sales", "Order Item Total", "Sales per customer"]}});
    let total = r#"SELECT SUM("Order Item Total") AS revenue FROM dataco"#;
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(RecordingLlm {
        replies: Mutex::new([clarify.to_string(), sql_reply(total).to_string()].into()),
        prompts: prompts.clone(),
    });
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let question = "Doanh thu từng tháng năm 2017";
    let (status, response) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "needs_choice", "{response}");
    let choice = &response["clarifications"][0];
    assert_eq!(choice["id"], "column:doanh thu");
    assert_eq!(
        choice["options"],
        json!([{"id": "Order Item Total", "label": "Order Item Total"}])
    );

    let picked = json!([{"choice_id": "column:doanh thu", "option_id": "Order Item Total"}]);
    let (status, response) = asking.ask(question, picked.clone()).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(response["choices"], picked);
    assert!(
        response["warnings"].as_array().unwrap().is_empty(),
        "{response}"
    );
    let prompts = prompts.lock().unwrap();
    assert!(
        prompts[1]
            .contains(r#""doanh thu" means the column "Order Item Total" (chosen by the user)"#),
        "{}",
        prompts[1]
    );
}

/// Tên cột trùng trong kết quả được đổi cho duy nhất; giá trị mô hình nêu mà cột không có thì báo cạnh kết quả.
#[tokio::test]
async fn result_names_are_unique_and_unknown_values_are_flagged() {
    let reply = json!({
        "terms": [{"text": "châu Á", "kind": "value", "column": "Market", "value": "Asia"}],
        "assumptions": [],
        "clarify": null,
        "sql": r#"SELECT "Market", "Market" FROM dataco WHERE "Market" = 'Asia'"#
    });
    let llm = scripted(&[reply]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let (status, response) = asking.ask("Thị trường châu Á", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    let names: Vec<&str> = response["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Market", "Market_2"]);
    assert_eq!(response["warnings"][0]["code"], "filter_value_not_found");
}

#[tokio::test]
async fn bad_requests_get_error_codes() {
    let Some(asking) = start(None).await else {
        return;
    };
    let (status, _) = asking.ask("   ", json!([])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let body = json!({"question": "Doanh thu theo thị trường"});
    let (status, _, _) = call(
        &asking.app,
        "POST",
        &asking.ask_url,
        None,
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    for id in ["not-a-uuid", "00000000-0000-0000-0000-000000000001"] {
        let url = format!("/api/workspaces/cua-hang/datasets/{id}/ask");
        let (status, _, _) = call(
            &asking.app,
            "POST",
            &url,
            Some(&asking.session),
            Some(body.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }
}

/// Mô hình chưa cấu hình (Gemini thiếu khóa): 503 trước khi có SQL, không ghi lịch sử.
#[tokio::test]
async fn unconfigured_model_is_503() {
    let Some(asking) = start_with(None, |c| {
        c.llm_provider = "gemini".to_string();
        c.gemini_api_key = Secret::new("");
    })
    .await
    else {
        return;
    };
    let (status, response) = asking.ask("Doanh thu theo thị trường", json!([])).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{response}");
    assert_eq!(
        asking.queries_with("ok").await + asking.queries_with("error").await,
        0
    );
}
