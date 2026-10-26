// SPDX-License-Identifier: Apache-2.0
/* api/ask_why_tests.rs - test câu "tại sao" qua route thật (#38): hỏi, kiểm phân rã, thẻ, biểu đồ, form và Explanation
được ghi vào queries.explanation, rồi truy dòng nguồn của một dòng phân rã và một ô của bảng tổng trên chính câu vừa
ghi. Đường dự phòng khi form sai hai lần. Mô hình giả trả form viết sẵn (ScriptedLlm của ask_tests.rs), độ đo là cột
nên không cần chỉ số. Mỗi test dùng database và thư mục dữ liệu riêng; không có DATABASE_URL thì bỏ qua.

Dữ liệu: quý 2/2017 Europe 140.5 (hai dòng), LATAM 200; quý 3/2017 Europe 80 (hai dòng), LATAM 120.25. */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{scripted, sql_reply, start_on, step_names, Asking};
use crate::api::test_support::call;
use crate::engine::store::test_support::{exec, fetch_count, fetch_text};

const CSV: &str = "Market,Customer Segment,order date (DateOrders),Order Item Total\n\
                   Europe,Consumer,5/15/2017 10:00,100.5\n\
                   LATAM,Consumer,5/20/2017 10:00,200\n\
                   Europe,Corporate,6/2/2017 10:00,40\n\
                   Europe,Corporate,8/15/2017 10:00,50\n\
                   LATAM,Corporate,8/16/2017 10:00,120.25\n\
                   Europe,Consumer,8/17/2017 10:00,30\n";
const QUESTION: &str = "Tại sao doanh thu quý 3 năm 2017 giảm?";

/// Form của QUESTION với độ đo là cột Order Item Total.
fn quarter_form() -> Value {
    json!({
        "terms": [{"text": "doanh thu", "kind": "column", "column": "Order Item Total"}],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": "order date (DateOrders)", "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": []
    })
}

async fn drill(asking: &Asking, body: Value) -> (StatusCode, Value) {
    let url = asking.ask_url.replace("/ask", "/drill");
    let (status, _, response) =
        call(&asking.app, "POST", &url, Some(&asking.session), Some(body)).await;
    (status, response)
}

/// Tổng cột Order Item Total của các dòng nguồn một trang drill trả về.
fn source_total(page: &Value) -> f64 {
    let columns = page["result"]["columns"].as_array().unwrap();
    let index = columns
        .iter()
        .position(|c| c["name"] == "Order Item Total")
        .unwrap();
    page["result"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[index].as_f64().unwrap())
        .sum()
}

#[tokio::test]
async fn why_question_is_answered_recorded_and_drilled() {
    let narrate = json!("Doanh thu giảm vì Europe và LATAM cùng giảm.");
    let llm = scripted(&[quarter_form(), narrate]);
    let Some(asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
    let (status, response) = asking.ask(QUESTION, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(
        step_names(&response),
        ["context", "llm", "guard", "run", "explain", "narrate", "record"]
    );
    assert_eq!(response["warnings"], json!([]));
    assert_eq!(
        response["choices"],
        json!([{"choice_id": "why:base", "option_id": "previous_quarter"}])
    );
    let explanation = &response["explanation"];
    assert_eq!(explanation["base"]["label"], "Quý 2/2017");
    assert_eq!(explanation["base"]["value"], "340.50");
    assert_eq!(explanation["compare"]["value"], "200.25");
    let dimensions: Vec<&str> = explanation["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["column"].as_str().unwrap())
        .collect();
    assert!(dimensions.contains(&"Market"), "{dimensions:?}");

    // Bảng tổng theo quý: vai trò, biểu đồ cột đứng (hai kỳ, ask_chart.rs luật 2), thẻ dựng từ form.
    let roles: Vec<&str> = response["result"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["time", "measure"]);
    assert_eq!(response["chart"]["kind"], "column");
    let period = &response["understanding"]["time_range"];
    assert!(
        period["label"]
            .as_str()
            .unwrap()
            .starts_with("Quý 3/2017 so với quý 2/2017"),
        "{period}"
    );
    let sql_guarded = response["sql_guarded"].as_str().unwrap();
    assert!(sql_guarded.contains(period["sql"].as_str().unwrap()));

    // Form và Explanation lưu cùng câu hỏi.
    let stored = |path: &str| {
        format!("SELECT explanation #>> '{{{path}}}' FROM queries WHERE status = 'ok'")
    };
    assert_eq!(
        fetch_text(&asking.db.store, &stored("form,compare")).await,
        "2017-Q3"
    );
    assert_eq!(
        fetch_text(&asking.db.store, &stored("explanation,compare,filter")).await,
        explanation["compare"]["filter"].as_str().unwrap()
    );

    // Dòng phân rã Europe của quý 3: hai dòng nguồn, tổng bằng số của nhóm.
    let query_id = response["query_id"].clone();
    let europe = |slice: &str| json!({"query_id": query_id, "breakdown": {"slice": slice, "column": "Market", "value": "Europe"}});
    let (status, page) = drill(&asking, europe("compare")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["total_rows"], 2);
    assert_eq!(source_total(&page), 80.0);
    let (status, page) = drill(&asking, europe("base")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(source_total(&page), 140.5);

    // Ô quý 3 của bảng tổng truy như ô của câu thường.
    let quarter = response["result"]["rows"][1][0].clone();
    let cell = json!({"query_id": query_id, "cell": [{"column": "quarter", "value": quarter}]});
    let (status, page) = drill(&asking, cell).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["total_rows"], 3);

    // Chiều không có trong bảng phân rã.
    let unknown = json!({"query_id": query_id,
                         "breakdown": {"slice": "base", "column": "Order Region", "value": "x"}});
    let (status, page) = drill(&asking, unknown).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{page}");
}

/// Form sai cả hai lần: chạy như câu thường, vẫn answered, cảnh báo explanation_unavailable, không lưu form.
#[tokio::test]
async fn wrong_form_twice_answers_as_a_plain_question() {
    let mut wrong = quarter_form();
    wrong["measure"]["column"] = json!("Profit");
    let sql =
        "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco GROUP BY \"Market\"";
    let llm = scripted(&[wrong.clone(), wrong, sql_reply(sql)]);
    let Some(asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
    let (status, response) = asking.ask("tai sao doanh thu giam", json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    assert_eq!(response["warnings"][0]["code"], "explanation_unavailable");
    assert!(response["explanation"].is_null());
    assert_eq!(response["sql_generated"], sql);
    // Thẻ và biểu đồ như câu thường.
    assert!(response["understanding"].is_object(), "{response}");
    assert_eq!(response["chart"]["kind"], "bar");
    let recorded = fetch_count(
        &asking.db.store,
        "SELECT count(*) FROM queries WHERE status = 'ok' AND explanation IS NULL",
    )
    .await;
    assert_eq!(recorded, 1);

    let body = json!({"query_id": response["query_id"],
                      "breakdown": {"slice": "base", "column": "Market", "value": "Europe"}});
    let (status, page) = drill(&asking, body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{page}");
    assert!(page["message"]
        .as_str()
        .unwrap()
        .contains("không có bảng phân rã"));
}

/// Minh, member của workspace, có vai dữ liệu chỉ thấy LATAM: mọi câu của đường "tại sao" (bảng tổng, phân rã) lọc theo
/// vai đó, và thẻ có mục phân quyền. Lan (admin) không bị lọc.
#[tokio::test]
async fn member_data_role_filters_every_why_query() {
    let llm = scripted(&[quarter_form(), json!("Doanh thu giảm.")]);
    let Some(mut asking) = start_on(Some(llm), CSV, |_| {}).await else {
        return;
    };
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
    let (_, minh, _) = call(&asking.app, "POST", "/api/auth/login", None, Some(login)).await;
    exec(
        &asking.db.store,
        "UPDATE memberships SET data_role = 'latam' WHERE user_id = (SELECT id FROM users WHERE email = 'minh@example.com')",
    )
    .await;
    exec(
        &asking.db.store,
        "INSERT INTO rls_policies (workspace_id, dataset_id, data_role, column_name, allowed_values)
         SELECT workspace_id, id, 'latam', 'Market', ARRAY['LATAM'] FROM datasets",
    )
    .await;
    asking.session = minh.unwrap();

    let (status, response) = asking.ask(QUESTION, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    let only_latam = "\"Market\" IN ('LATAM')";
    assert!(response["sql_guarded"]
        .as_str()
        .unwrap()
        .contains(only_latam));
    let explanation = &response["explanation"];
    assert_eq!(explanation["base"]["value"], "200.00");
    assert_eq!(explanation["compare"]["value"], "120.25");
    for dimension in explanation["dimensions"].as_array().unwrap() {
        assert!(
            dimension["sql"].as_str().unwrap().contains(only_latam),
            "{dimension}"
        );
    }
    let filters = response["understanding"]["filters"].as_array().unwrap();
    assert!(
        filters
            .iter()
            .any(|f| f["label"].as_str().unwrap().starts_with("Phân quyền")),
        "{filters:?}"
    );
}
