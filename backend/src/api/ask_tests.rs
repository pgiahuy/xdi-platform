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
use crate::service::ask::run::REFUSED_MESSAGE;

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

/// Mô hình giả: trả lần lượt các câu viết sẵn cho câu hỏi (sinh SQL và sửa). classify là câu trả lời cho bước phân
/// loại; None thì bước đó lỗi và luồng hỏi dùng luật từ khóa.
struct ScriptedLlm {
    replies: Mutex<VecDeque<String>>,
    classify: Option<String>,
}

pub(super) fn scripted(replies: &[Value]) -> Arc<dyn LlmProvider> {
    let replies = replies.iter().map(Value::to_string).collect();
    Arc::new(ScriptedLlm {
        replies: Mutex::new(replies),
        classify: None,
    })
}

/// Như scripted, và bước phân loại nhận kind ("plain" hay "cause").
pub(super) fn scripted_with_kind(kind: &str, replies: &[Value]) -> Arc<dyn LlmProvider> {
    let replies = replies.iter().map(Value::to_string).collect();
    let classify = json!({"kind": kind, "reason": "Câu trả lời viết sẵn của test."});
    Arc::new(ScriptedLlm {
        replies: Mutex::new(replies),
        classify: Some(classify.to_string()),
    })
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        if let (LlmTask::Classify, Some(reply)) = (request.task, &self.classify) {
            return Ok(reply.clone());
        }
        // Nạp file xong, lớp ngữ nghĩa (service/semantic/mod.rs) chạy nền và cũng gọi mô hình. Lỗi tạm thì nó không ghi gì
        // vào hồ sơ, nên câu viết sẵn chỉ dành cho câu hỏi và bí danh test tự đặt không bị ghi đè.
        // Đoạn trả lời của câu thường (task summary) cũng gọi mô hình; lỗi thì luồng hỏi dùng câu khuôn. Bước phân
        // loại (task classify) lỗi thì dùng luật từ khóa, nên câu viết sẵn chỉ dành cho SQL và form như trước.
        // Lời của phần giải thích (task insight, #227) cũng vậy: lỗi thì dùng câu do code viết.
        if matches!(
            request.task,
            LlmTask::Profile | LlmTask::Summary | LlmTask::Classify | LlmTask::Insight
        ) {
            return Err(AppError::Upstream(
                "scripted model has no profile or summary".to_string(),
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
        ["context", "classify", "llm", "guard", "run", "narrate", "explain", "narrate", "record"]
    );
    // Mock có câu phân loại viết sẵn cho câu mẫu: mô hình chọn, không phải luật từ khóa.
    let classify = &response["steps"][1];
    assert_eq!(classify["ok"], true, "{classify}");
    assert!(classify["detail"]
        .as_str()
        .unwrap()
        .starts_with("Câu hỏi thường: "));
    // Mock không có câu trả lời viết sẵn cho task summary nên đoạn trả lời là câu khuôn, không gọi mạng.
    assert_eq!(
        response["answer_text"],
        "Có 2 dòng; cao nhất là LATAM với 200."
    );
    assert_eq!(response["answer_text_by_model"], false);
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
    // So sánh nhóm: thanh ngang (ask/chart.rs, luật 1).
    assert_eq!(
        response["chart"],
        json!({"kind": "bar", "x": "market", "y": ["revenue"], "layout": "single", "series": null,
               "ratio": [], "top": null, "other_bucket": false})
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
        [
            "context", "classify", "llm", "guard", "run", "repair", "guard", "run", "narrate",
            "explain", "narrate", "record"
        ]
    );
    assert!(response["sql_generated"]
        .as_str()
        .unwrap()
        .contains("%m/%d/%Y"));
    // Trục là ngày, chỉ 3 kỳ nên cột đứng (ask/chart.rs, luật 2: tới 10 kỳ là cột).
    assert_eq!(response["chart"]["kind"], "column");
    assert_eq!(response["chart"]["x"], "ship_day");
    // Lần chạy lỗi vào nhật ký lỗi (#228), kèm câu SQL đầu của mô hình và query_id của câu đã trả lời.
    let logged = format!(
        "SELECT count(*) FROM error_log WHERE step = 'run' AND strpos(model_output, '%d/%m/%Y') > 0
         AND query_id = '{}'",
        response["query_id"].as_str().unwrap()
    );
    assert_eq!(fetch_count(&asking.db.store, &logged).await, 1);
}

/// Mô hình báo dữ liệu không có thông tin (#164): rejected, không chạy SQL, lời nêu lý do và chỉ các cột có thật, ghi
/// lịch sử như câu bị từ chối.
#[tokio::test]
async fn unanswerable_question_is_rejected_without_sql() {
    let reply = json!({"terms": [], "assumptions": [], "sql": "",
        "unanswerable": {"reason": "dữ liệu chỉ có các ngày đã qua, không có dự báo.",
                         "related_columns": ["Order Item Total", "Forecast"]}});
    let Some(asking) = start(Some(scripted(&[reply]))).await else {
        return;
    };
    let (status, response) = asking
        .ask("Doanh thu năm tới là bao nhiêu?", json!([]))
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "rejected", "{response}");
    assert_eq!(
        response["message"],
        "Dữ liệu không có thông tin để trả lời câu này: dữ liệu chỉ có các ngày đã qua, không có dự báo. Có thể hỏi \
         về các cột gần nhất: Order Item Total."
    );
    assert!(response["sql_generated"].is_null());
    assert!(response["query_id"].is_string());
    assert_eq!(
        step_names(&response),
        ["context", "classify", "llm", "record"]
    );
    assert_eq!(asking.queries_with("rejected").await, 1);
    assert!(response["understanding"].is_null());
}

/// Câu mẫu của mock: câu "tại sao" hỏi mức hài lòng mà dữ liệu không có cột đánh giá. Form báo unanswerable nên không
/// kiểm form, không chạy SQL, không lưu form.
#[tokio::test]
async fn mock_why_question_without_data_is_rejected() {
    let csv = "Market,Delivery Status,Order Status,Order Item Total\n\
               Europe,Late delivery,COMPLETE,100.5\n\
               LATAM,Shipping on time,PENDING,200\n";
    let Some(asking) = start_on(None, csv, |_| {}).await else {
        return;
    };
    let (status, response) = asking
        .ask("Tại sao khách hàng không hài lòng?", json!([]))
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "rejected", "{response}");
    assert_eq!(
        response["message"],
        "Dữ liệu không có thông tin để trả lời câu này: dataset không có cột đánh giá hay mức hài lòng của khách \
         hàng. Có thể hỏi về các cột gần nhất: Delivery Status, Order Status."
    );
    assert!(response["explanation"].is_null());
    assert_eq!(
        step_names(&response),
        ["context", "classify", "llm", "record"]
    );
    assert_eq!(asking.queries_with("rejected").await, 1);
    let sql = "SELECT count(*) FROM queries WHERE explanation IS NULL";
    assert_eq!(fetch_count(&asking.db.store, sql).await, 1);
}

/// Guard từ chối câu đầu thì mô hình được sửa một lần; câu sửa vẫn bị từ chối thì câu hỏi là rejected, người hỏi nhận
/// lời của hệ thống còn lý do của guard ở bước guard.
#[tokio::test]
async fn guard_refusal_is_recorded_as_rejected() {
    let llm = scripted(&[
        sql_reply("DELETE FROM dataco"),
        sql_reply("DELETE FROM dataco"),
    ]);
    let Some(asking) = start(Some(llm)).await else {
        return;
    };
    let (status, response) = asking.ask("Xóa hết dữ liệu", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "rejected", "{response}");
    assert_eq!(response["sql_generated"], "DELETE FROM dataco");
    assert!(response["sql_guarded"].is_null());
    assert!(response["query_id"].is_string());
    assert_eq!(
        step_names(&response),
        ["context", "classify", "llm", "guard", "repair", "guard", "record"]
    );
    assert_eq!(response["message"], REFUSED_MESSAGE);
    assert!(
        response["steps"][5]["detail"]
            .as_str()
            .unwrap()
            .contains("SELECT"),
        "{response}"
    );
    assert_eq!(asking.queries_with("rejected").await, 1);
    // Hai lần guard từ chối, hai dòng nhật ký lỗi kèm câu của mô hình (#228).
    let logged = "SELECT count(*) FROM error_log WHERE step = 'guard' AND model_output = 'DELETE FROM dataco'
                  AND question = 'Xóa hết dữ liệu' AND query_id IS NOT NULL";
    assert_eq!(fetch_count(&asking.db.store, logged).await, 2);
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
/// sử, như đường dự phòng của câu "tại sao" (explain/plain.rs).
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
        // Đoạn trả lời của câu thường (task summary) cũng gọi mô hình; lỗi thì luồng hỏi dùng câu khuôn. Bước phân
        // loại (task classify) lỗi thì dùng luật từ khóa, nên câu viết sẵn chỉ dành cho SQL và form như trước.
        // Lời của phần giải thích (task insight, #227) cũng vậy: lỗi thì dùng câu do code viết.
        if matches!(
            request.task,
            LlmTask::Profile | LlmTask::Summary | LlmTask::Classify | LlmTask::Insight
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
    assert_eq!(
        warning_codes(&response),
        ["empty_result", "filter_value_not_found"]
    );
}

/// Kiểm hợp lý (#36) qua route với mock: câu mẫu lọc Market = 'Asia' mà CSV không có, nên ra rỗng. Cảnh báo theo thứ
/// tự luật, câu vẫn answered và vẫn ghi lịch sử như câu đúng.
#[tokio::test]
async fn suspicious_results_come_with_warnings_and_stay_answered() {
    let Some(asking) = start(None).await else {
        return;
    };
    let (status, response) = asking.ask("Doanh thu ở thị trường châu Á", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(response["result"]["total_rows"], 0);
    assert_eq!(
        warning_codes(&response),
        ["empty_result", "filter_value_not_found"]
    );
    assert_eq!(
        response["warnings"][1]["message"],
        "Cột Market không có giá trị \"Asia\". Kiểm lại giá trị lọc: kết quả có thể rỗng hoặc thiếu dòng."
    );
    assert_eq!(asking.queries_with("ok").await, 1);
}

fn warning_codes(response: &Value) -> Vec<&str> {
    response["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["code"].as_str().unwrap())
        .collect()
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
