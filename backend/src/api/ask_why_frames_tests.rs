// SPDX-License-Identifier: Apache-2.0
/* api/ask_why_frames_tests.rs - test qua route thật cho các khuôn câu "tại sao" thêm ở #244: hỏi, kiểm hai lát cắt,
thẻ "Đã hiểu là", rồi truy dòng nguồn của một dòng phân rã trên chính câu vừa ghi. Mô hình giả trả form viết sẵn
(ScriptedLlm của ask_tests.rs). Mỗi test dùng database và thư mục dữ liệu riêng; không có DATABASE_URL thì bỏ qua.

Dữ liệu: quý 3/2017 Europe 80 (Corporate 50, Consumer 30), LATAM 120.25 (Corporate), USCA 10 (Consumer). */

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::ask_tests::{scripted, start_on, Asking};
use crate::api::test_support::call;

const CSV: &str = "Market,Customer Segment,order date (DateOrders),Order Item Total\n\
                   Europe,Consumer,5/15/2017 10:00,100.5\n\
                   LATAM,Consumer,5/20/2017 10:00,200\n\
                   Europe,Corporate,8/15/2017 10:00,50\n\
                   LATAM,Corporate,8/16/2017 10:00,120.25\n\
                   Europe,Consumer,8/17/2017 10:00,30\n\
                   USCA,Consumer,8/18/2017 10:00,10\n";

async fn drill(asking: &Asking, body: Value) -> (StatusCode, Value) {
    let url = asking.ask_url.replace("/ask", "/drill");
    let (status, _, response) =
        call(&asking.app, "POST", &url, Some(&asking.session), Some(body)).await;
    (status, response)
}

/// Tổng cột Order Item Total và các thị trường của các dòng nguồn một trang drill trả về.
fn source_rows(page: &Value) -> (f64, Vec<String>) {
    let columns = page["result"]["columns"].as_array().unwrap();
    let find = |name: &str| columns.iter().position(|c| c["name"] == name).unwrap();
    let (total, market) = (find("Order Item Total"), find("Market"));
    let rows = page["result"]["rows"].as_array().unwrap();
    let sum = rows.iter().map(|row| row[total].as_f64().unwrap()).sum();
    let markets = rows
        .iter()
        .map(|row| row[market].as_str().unwrap().to_string())
        .collect();
    (sum, markets)
}

/// "Vì sao doanh thu Europe quý 3/2017 khác các thị trường khác?": lát gốc là mọi thị trường trừ Europe; truy dòng
/// nguồn của nhóm Corporate ở lát gốc chỉ ra dòng của thị trường khác.
#[tokio::test]
async fn one_group_against_the_rest_is_answered_and_drilled() {
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": "order date (DateOrders)", "grain": "quarter"},
        "compare": "2017-Q3",
        "groups": {"column": "Market", "compare": "Europe"},
        "filters": []
    });
    let Some(asking) = start_on(Some(scripted(&[form])), CSV, |_| {}).await else {
        return;
    };
    let question = "Vì sao doanh thu Europe quý 3/2017 khác các thị trường khác?";
    let (status, response) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    let explanation = &response["explanation"];
    assert_eq!(explanation["comparison"], "groups");
    assert!(
        explanation["base"]["label"]
            .as_str()
            .unwrap()
            .ends_with("còn lại"),
        "{explanation}"
    );
    assert_eq!(explanation["base"]["value"], "130.25");
    assert_eq!(explanation["compare"]["value"], "80.00");
    let card = response["understanding"]["filters"][0]["label"]
        .as_str()
        .unwrap();
    assert!(card.contains("Europe so với các"), "{card}");

    let query_id = response["query_id"].clone();
    let corporate = |slice: &str| {
        json!({"query_id": query_id,
               "breakdown": {"slice": slice, "column": "Customer Segment", "value": "Corporate"}})
    };
    let (status, page) = drill(&asking, corporate("base")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(source_rows(&page), (120.25, vec!["LATAM".to_string()]));
    let (status, page) = drill(&asking, corporate("compare")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(source_rows(&page), (50.0, vec!["Europe".to_string()]));
}

/// Bộ lọc `in` qua route: USCA bị loại khỏi mọi câu, Market vẫn là chiều phân rã, và truy dòng nguồn của nhóm LATAM
/// chỉ ra dòng LATAM của quý 3.
#[tokio::test]
async fn in_filter_is_answered_and_drilled() {
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": "order date (DateOrders)", "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": [{"column": "Market", "op": "in", "values": ["Europe", "LATAM"]}]
    });
    let Some(asking) = start_on(Some(scripted(&[form])), CSV, |_| {}).await else {
        return;
    };
    let question = "Vì sao doanh thu quý 3/2017 ở Europe và LATAM giảm?";
    let (status, response) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    let explanation = &response["explanation"];
    assert_eq!(explanation["base"]["value"], "300.50");
    assert_eq!(explanation["compare"]["value"], "200.25");
    let query_id = response["query_id"].clone();
    let latam = json!({"query_id": query_id,
                       "breakdown": {"slice": "compare", "column": "Market", "value": "LATAM"}});
    let (status, page) = drill(&asking, latam).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(source_rows(&page), (120.25, vec!["LATAM".to_string()]));
}

/// Tỷ lệ dòng có một giá trị qua route: tỷ lệ Corporate quý 2 là 0, quý 3 là 2 trên 4 dòng; truy dòng nguồn của
/// nhóm Europe ở quý 3 ra hai dòng Europe (mọi dòng của nhóm, vì mẫu số đếm mọi dòng).
#[tokio::test]
async fn share_measure_is_answered_and_drilled() {
    let form = json!({
        "terms": [],
        "measure": {"column": "Customer Segment", "agg": "share", "value": "Corporate"},
        "time": {"column": "order date (DateOrders)", "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": []
    });
    let Some(asking) = start_on(Some(scripted(&[form])), CSV, |_| {}).await else {
        return;
    };
    let question = "Vì sao tỷ lệ khách doanh nghiệp quý 3/2017 tăng?";
    let (status, response) = asking.ask(question, json!([])).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "answered", "{response}");
    let explanation = &response["explanation"];
    assert_eq!(explanation["measure_kind"], "ratio");
    assert_eq!(explanation["base"]["value"], "0.0000");
    assert_eq!(explanation["compare"]["value"], "0.5000");
    let query_id = response["query_id"].clone();
    let europe = json!({"query_id": query_id,
                        "breakdown": {"slice": "compare", "column": "Market", "value": "Europe"}});
    let (status, page) = drill(&asking, europe).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        source_rows(&page),
        (80.0, vec!["Europe".to_string(), "Europe".to_string()])
    );
}
