// SPDX-License-Identifier: Apache-2.0
/* service/explain/frames_run_tests.rs - test trọn đường câu "tại sao" (explain/run.rs) cho các khuôn thêm ở #244, trên
file Parquet tạm kiểu DataCo của explain/run_tests.rs: form của mô hình giả, kiểm form, SQL qua guard, chạy trên DuckDB,
phân rã, câu khuôn (mô hình giả hết câu ở bước viết lời) và thẻ "Đã hiểu là".

Số của quý 3/2017 (COMPARE_CELLS của run_tests.rs): Pacific Asia 1.960.120,00; các thị trường còn lại cộng lại
6.158.980,00. */

use serde_json::json;

use crate::service::ask::contract::{AskStatus, Comparison};
use crate::service::explain::run_tests::{items, view, Dataco, ScriptedLlm};
use crate::service::explain::{build_number_check, check_tests::DATE};

/// "Vì sao doanh thu Pacific Asia quý 3/2017 thấp hơn các thị trường khác?"
fn rest_form() -> serde_json::Value {
    json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "groups": {"column": "Market", "compare": "Pacific Asia"},
        "filters": []
    })
}

/// Một nhóm so với các nhóm còn lại: hai lát đúng số, câu khuôn và thẻ gọi lát gốc là "các thị trường còn lại", mỗi
/// đoạn SQL của thẻ có trong câu đã chạy.
#[tokio::test]
async fn one_group_is_compared_with_the_rest() {
    let mut data = Dataco::new();
    data.columns[0].aliases = vec!["Thị trường".to_string()];
    let llm = ScriptedLlm::new(vec![rest_form()]);
    let question = "Vì sao doanh thu Pacific Asia quý 3/2017 thấp hơn các thị trường khác?";
    let outcome = data.ask(&llm, question, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.comparison, Comparison::Groups);
    assert_eq!(explanation.base.label, "các thị trường còn lại");
    assert_eq!(explanation.base.value, "6158980.00");
    assert_eq!(explanation.compare.value, "1960120.00");
    assert_eq!(explanation.base.filter, "\"Market\" <> 'Pacific Asia'");
    assert!(
        explanation
            .summary_title
            .contains("thấp hơn các thị trường còn lại"),
        "{}",
        explanation.summary_title
    );
    build_number_check(&explanation.summary, explanation).unwrap();
    // Bảng tổng có mọi thị trường trong quý, để người hỏi thấy từng nhóm của phần còn lại.
    assert_eq!(outcome.result.as_ref().unwrap().total_rows, 5);

    let view = view(&outcome, &data.metrics);
    let sql = outcome.sql_guarded.as_deref().unwrap();
    for item in items(&view.understanding) {
        assert!(sql.contains(&item.sql), "{} not in {sql}", item.sql);
    }
    let labels: Vec<&str> = view
        .understanding
        .filters
        .iter()
        .map(|f| f.label.as_str())
        .collect();
    assert_eq!(
        labels,
        ["Market: Pacific Asia so với các thị trường còn lại"]
    );
}
