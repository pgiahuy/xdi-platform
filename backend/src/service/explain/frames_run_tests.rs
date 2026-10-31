// SPDX-License-Identifier: Apache-2.0
/* service/explain/frames_run_tests.rs - test trọn đường câu "tại sao" (explain/run.rs) cho các khuôn thêm ở #244, trên
file Parquet tạm kiểu DataCo của explain/run_tests.rs: form của mô hình giả, kiểm form, SQL qua guard, chạy trên DuckDB,
phân rã, câu khuôn (mô hình giả hết câu ở bước viết lời) và thẻ "Đã hiểu là".

Số của quý 3/2017 (COMPARE_CELLS của run_tests.rs): Pacific Asia 1.960.120,00; các thị trường còn lại cộng lại
6.158.980,00. */

use serde_json::json;

use crate::service::ask::contract::{AskStatus, CellValue, Comparison, WarningCode};
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
    // Doanh thu cộng được: so với trung bình mỗi thị trường còn lại (6.158.980 trên 4 thị trường), không với tổng của
    // chúng, nên Pacific Asia hóa ra cao hơn, trái tiền đề của câu hỏi.
    assert_eq!(explanation.base.label, "trung bình các thị trường còn lại");
    assert_eq!(explanation.base.value, "1539745.00");
    assert_eq!(explanation.compare.value, "1960120.00");
    assert_eq!(explanation.base.filter, "\"Market\" <> 'Pacific Asia'");
    assert!(
        explanation
            .summary_title
            .contains("cao hơn trung bình các thị trường còn lại"),
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
        ["Market: Pacific Asia so với trung bình các thị trường còn lại"]
    );
}

/// Bộ lọc "in" và ">" đi qua guard, chạy trên DuckDB: chỉ còn dòng Europe, LATAM trên 1.000.000; Market vẫn là chiều
/// phân rã vì còn hai nhóm.
#[tokio::test]
async fn extended_filters_reach_every_query() {
    let data = Dataco::new();
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": [
            {"column": "Market", "op": "in", "values": ["Europe", "LATAM"]},
            {"column": "Order Item Total", "op": ">", "value": "1000000"}
        ]
    });
    let llm = ScriptedLlm::new(vec![form]);
    let question = "Vì sao doanh thu các đơn trên 1 triệu ở Europe và LATAM quý 3/2017 giảm?";
    let outcome = data.ask(&llm, question, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.base.value, "4880200.15");
    assert_eq!(explanation.compare.value, "4150100.50");
    let market = explanation
        .dimensions
        .iter()
        .find(|d| d.column == "Market")
        .unwrap();
    assert_eq!(market.groups.len(), 2);
    assert!(
        market.sql.contains("\"Market\" IN ('Europe', 'LATAM')"),
        "{}",
        market.sql
    );
    let view = view(&outcome, &data.metrics);
    let sql = outcome.sql_guarded.as_deref().unwrap();
    for item in items(&view.understanding) {
        assert!(sql.contains(&item.sql), "{} not in {sql}", item.sql);
    }
}

/// Tỷ lệ dòng Corporate của Pacific Asia so với các thị trường còn lại, quý 3/2017. Bảng chỉ có hai cột chiều, đều đã
/// dùng (Market làm hai lát, Customer Segment là cột của tỷ lệ, không phân rã theo chính nó), nên câu answered với bảng
/// tổng theo thị trường và cảnh báo không phân rã được; bảng ghi tỷ lệ của từng thị trường, Pacific Asia là 1.0000.
#[tokio::test]
async fn share_of_rows_compares_one_group_with_the_rest() {
    let data = Dataco::new();
    let form = json!({
        "terms": [],
        "measure": {"column": "Customer Segment", "agg": "share", "value": "Corporate"},
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "groups": {"column": "Market", "compare": "Pacific Asia"},
        "filters": []
    });
    let llm = ScriptedLlm::new(vec![form]);
    let question =
        "Vì sao Pacific Asia có tỷ lệ khách doanh nghiệp cao hơn các thị trường khác quý 3/2017?";
    let outcome = data.ask(&llm, question, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert!(outcome.explanation.is_none());
    assert_eq!(
        outcome.warnings[0].code,
        WarningCode::ExplanationUnavailable
    );
    let result = outcome.result.as_ref().unwrap();
    let pacific = result
        .rows
        .iter()
        .find(|row| matches!(&row[0], Some(CellValue::String(m)) if m == "Pacific Asia"))
        .expect("a Pacific Asia row");
    assert_eq!(
        format!("{:?}", pacific[1]),
        format!("{:?}", Some(CellValue::Number(1.0)))
    );
    let label = &outcome.plan.as_ref().unwrap().measure.label;
    assert_eq!(label, "Tỷ lệ Customer Segment là Corporate");
}

/// Đếm phân biệt: số đơn quý 3 so với quý 2. Đơn 1 tới 100 của quý 2 có dòng ở hai thị trường (run_tests.rs), nên theo
/// Market các nhóm cộng lại vượt tổng: không chiều nào cộng đúng, câu vẫn answered với bảng tổng kèm cảnh báo. Lọc bỏ các
/// đơn đó (Order Id > 100) thì mỗi đơn thuộc đúng một nhóm và phân rã có lại: 4.900 đơn quý 2, 4.400 đơn quý 3.
#[tokio::test]
async fn distinct_count_needs_groups_that_add_up() {
    let data = Dataco::new();
    let form = |filters: serde_json::Value| {
        json!({
            "terms": [],
            "measure": {"column": "Order Id", "agg": "count_distinct"},
            "time": {"column": DATE, "grain": "quarter"},
            "compare": "2017-Q3",
            "filters": filters
        })
    };
    let question = "Vì sao số đơn quý 3/2017 giảm?";
    let llm = ScriptedLlm::new(vec![form(json!([]))]);
    let outcome = data.ask(&llm, question, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert!(outcome.explanation.is_none());
    assert_eq!(
        outcome.warnings[0].code,
        WarningCode::ExplanationUnavailable
    );

    let later = json!([{"column": "Order Id", "op": ">", "value": "100"}]);
    let llm = ScriptedLlm::new(vec![form(later)]);
    let outcome = data.ask(&llm, question, &[]).await;
    let explanation = outcome.explanation.as_ref().expect("a breakdown");
    assert_eq!(
        (
            explanation.base.value.as_str(),
            explanation.compare.value.as_str()
        ),
        ("4900", "4400")
    );
    assert!(explanation.volume_price.is_none());
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert!(columns.contains(&"Market"), "{columns:?}");
}

/// Tuần trên DuckDB: tuần 28/8 - 3/9/2017 (các dòng 0 đồng ngày 1/9) so với tuần 14/8 - 20/8/2017 (mọi ô của quý 3), kỳ
/// gốc ghi rõ trong form; date_trunc('week') của DuckDB và tuần ISO của Period trùng nhau nên bảng tổng có đúng hai dòng.
#[tokio::test]
async fn weeks_are_compared_on_duckdb() {
    let data = Dataco::new();
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "week"},
        "compare": "2017-W35",
        "base": "2017-W33",
        "filters": []
    });
    let llm = ScriptedLlm::new(vec![form]);
    let outcome = data
        .ask(
            &llm,
            "Vì sao doanh thu tuần này giảm so với hai tuần trước?",
            &[],
        )
        .await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.base.label, "Tuần 14/8/2017 - 20/8/2017");
    assert_eq!(explanation.base.value, "8119100.00");
    assert_eq!(explanation.compare.value, "0.00");
    let result = outcome.result.as_ref().unwrap();
    assert_eq!(result.total_rows, 2);
    assert_eq!(result.columns[0].name, "Tuần");
}

/// Cùng kỳ năm trước theo tháng: tháng 8/2017 so với tháng 8/2016 (một dòng 1.000).
#[tokio::test]
async fn same_month_last_year_is_compared() {
    let data = Dataco::new();
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "month"},
        "compare": "2017-08",
        "compare_with": "same_period_last_year",
        "filters": []
    });
    let llm = ScriptedLlm::new(vec![form]);
    let question = "Vì sao doanh thu tháng 8/2017 tăng so với cùng kỳ năm trước?";
    let outcome = data.ask(&llm, question, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.base.label, "Tháng 8/2016");
    assert_eq!(explanation.base.value, "1000.00");
    assert_eq!(explanation.compare.label, "Tháng 8/2017");
}

/// "Tháng nào doanh thu năm 2017 giảm bất thường?": chuỗi tháng 4, 5, 6, 8, 9 của năm 2017; bước giảm lớn nhất là tháng
/// 6 (chỉ có dòng 0 đồng) so với tháng 5, rồi phân rã như hai kỳ.
#[tokio::test]
async fn unusual_month_is_found_then_explained() {
    let data = Dataco::new();
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "month"},
        "compare": "",
        "find": {"kind": "period"},
        "filters": [{"column": DATE, "op": "between", "values": ["2017", "2017"]}]
    });
    let llm = ScriptedLlm::new(vec![form]);
    let outcome = data
        .ask(&llm, "Tháng nào doanh thu năm 2017 giảm bất thường?", &[])
        .await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.base.label, "Tháng 5/2017");
    assert_eq!(explanation.compare.label, "Tháng 6/2017");
    assert_eq!(explanation.base.value, "8806350.75");
    assert_eq!(outcome.form.as_ref().unwrap()["compare"], "2017-06");
    // Kỳ do code tìm, không phải kỳ gần nhất: không có lựa chọn why:compare.
    assert!(outcome.assumptions.iter().all(|c| c.id != "why:compare"));
    let found = outcome
        .steps
        .iter()
        .find(|s| {
            s.detail
                .as_deref()
                .is_some_and(|d| d.starts_with("Kỳ đổi nhiều nhất"))
        })
        .expect("a step names the period found");
    assert!(found.ok);
}

/// "Thị trường nào kéo doanh thu quý 3/2017 xuống?": thêm một dòng Europe, Corporate 100.000 ở quý 3 để Customer Segment
/// của Europe có hai nhóm. Europe vẫn giảm nhiều nhất (539.780,25, LATAM 339.049,50); câu lọc Europe rồi phân rã hai quý
/// theo Customer Segment.
#[tokio::test]
async fn group_pulling_down_is_found_then_explained() {
    let row = "SELECT CAST(40000 AS INTEGER), 'Europe', 'Corporate', '8/20/2017 10:00', CAST(100000 AS DOUBLE)";
    let data = Dataco::with_rows(&[row.to_string()], 1);
    let form = json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "find": {"kind": "group", "column": "Market"},
        "filters": []
    });
    let llm = ScriptedLlm::new(vec![form]);
    let outcome = data
        .ask(&llm, "Thị trường nào kéo doanh thu quý 3/2017 xuống?", &[])
        .await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(explanation.base.value, "2850120.40");
    assert_eq!(explanation.compare.value, "2310340.15");
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert_eq!(columns, ["Customer Segment"]);
    let sql = outcome.sql_guarded.as_deref().unwrap();
    assert!(sql.contains("\"Market\" = 'Europe'"), "{sql}");
}
