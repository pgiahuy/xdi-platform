// SPDX-License-Identifier: Apache-2.0
/* service/explain/frames_tests.rs - test phần thuần của các khuôn câu "tại sao" thêm ở #244: một nhóm so với các nhóm
còn lại (explain/group_base.rs), bộ lọc mở rộng (explain/filter.rs), độ đo mới (explain/measure_column.rs), kỳ tuần và
ngày (explain/period.rs): đọc và kiểm form, SQL dựng ra. Hồ sơ DataCo thu nhỏ của explain/check_tests.rs; trọn đường
trên Parquet tạm ở explain/frames_run_tests.rs. */

use std::collections::HashMap;

use serde_json::json;

use crate::service::explain::check::PlanSlices;
use crate::service::explain::check_tests::{Fixture, DATE};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::{build_dimension_sql, build_slices_sql, build_why_sql};

fn rest_form() -> serde_json::Value {
    json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Pacific Asia"},
        "filters": []
    })
}

/// "Vì sao Pacific Asia khác các thị trường khác": base vắng là mọi nhóm còn lại; dòng trống của Market không thuộc
/// lát nào.
#[test]
fn group_without_base_is_compared_with_the_rest() {
    let fixture = Fixture::new();
    let plan = fixture.plan(rest_form());
    let PlanSlices::Groups { base, compare, .. } = &plan.slices else {
        panic!("expected two groups, got {:?}", plan.slices);
    };
    assert_eq!(base, &GroupBase::Rest);
    assert_eq!(compare, "Pacific Asia");
    let sql = build_why_sql(&plan);
    assert_eq!(sql.base_filter, "\"Market\" <> 'Pacific Asia'");
    assert_eq!(sql.compare_filter, "\"Market\" = 'Pacific Asia'");
    assert!(
        sql.total_sql
            .contains("WHERE (\"Market\" <> 'Pacific Asia' OR \"Market\" = 'Pacific Asia')"),
        "{}",
        sql.total_sql
    );
    // Cột của hai lát không là chiều phân rã, như so hai nhóm.
    assert_eq!(plan.dimensions, ["Customer Segment"]);
    let segment = build_dimension_sql(&plan, "Customer Segment");
    assert!(
        segment.contains("CASE WHEN \"Market\" <> 'Pacific Asia' THEN"),
        "{segment}"
    );
    // Chuỗi rỗng cũng là phần còn lại: Gemini hay ghi "" thay cho bỏ trường.
    let mut empty = rest_form();
    empty["groups"]["base"] = json!("");
    assert_eq!(fixture.plan(empty), plan);
}

#[test]
fn rest_is_named_by_the_column_meaning() {
    let mut labels = HashMap::new();
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels),
        "các nhóm còn lại"
    );
    labels.insert("Market".to_string(), "Thị trường".to_string());
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels),
        "các thị trường còn lại"
    );
    assert_eq!(
        GroupBase::Value("LATAM".to_string()).build_label("Market", &labels),
        "LATAM"
    );
    // Dấu nháy trong giá trị nhóm in thành chuỗi SQL đúng, không cắt được câu.
    let condition = GroupBase::Rest.build_condition("Market", "Europe' OR 1=1 --");
    assert_eq!(condition.to_string(), "\"Market\" <> 'Europe'' OR 1=1 --'");
}

/// Nhóm đang so vẫn phải có trong danh sách giá trị khi so với phần còn lại.
#[test]
fn rest_needs_a_listed_compare_group() {
    let fixture = Fixture::new();
    let mut form = rest_form();
    form["groups"]["compare"] = json!("Asia");
    assert_eq!(fixture.fields(form), ["groups.compare"]);
}

// Bộ lọc mở rộng (explain/filter.rs).

/// Form so quý 3 với quý 2 năm 2017, kèm một bộ lọc.
fn filtered(filter: serde_json::Value) -> serde_json::Value {
    json!({
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": [filter]
    })
}

fn filter_sql(fixture: &Fixture, filter: serde_json::Value) -> Vec<String> {
    let plan = fixture.plan(filtered(filter));
    plan.filters.iter().map(ToString::to_string).collect()
}

#[test]
fn in_filter_keeps_the_column_as_a_dimension() {
    let fixture = Fixture::new();
    let filter = json!({"column": "Market", "op": "in", "values": ["Europe", "LATAM"]});
    assert_eq!(
        filter_sql(&fixture, filter.clone()),
        ["\"Market\" IN ('Europe', 'LATAM')"]
    );
    // Hai thị trường vẫn là hai nhóm để phân rã; lọc "=" thì không.
    let plan = fixture.plan(filtered(filter));
    assert!(plan.dimensions.contains(&"Market".to_string()));
    let total = build_why_sql(&plan).total_sql;
    assert!(
        total.contains("WHERE \"Market\" IN ('Europe', 'LATAM') AND ("),
        "{total}"
    );
    let one = json!({"column": "Market", "op": "in", "values": ["Europe"]});
    assert_eq!(fixture.fields(filtered(one)), ["filters[0].values"]);
    let unknown = json!({"column": "Market", "op": "in", "values": ["Europe", "Asia"]});
    assert_eq!(fixture.fields(filtered(unknown)), ["filters[0].values[1]"]);
}

#[test]
fn number_filters_take_numbers_only() {
    let fixture = Fixture::new();
    let column = "Order Item Total";
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": column, "op": ">", "value": "100"})
        ),
        ["\"Order Item Total\" > 100"]
    );
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": column, "op": "between", "values": ["100.50", "500"]})
        ),
        [
            "\"Order Item Total\" >= 100.5",
            "\"Order Item Total\" <= 500"
        ]
    );
    // Chữ không phải số không vào được SQL.
    let words = json!({"column": column, "op": "<=", "value": "100 OR 1=1"});
    assert_eq!(fixture.fields(filtered(words)), ["filters[0].value"]);
    let reversed = json!({"column": column, "op": "between", "values": ["500", "100"]});
    assert_eq!(fixture.fields(filtered(reversed)), ["filters[0].values"]);
    let three = json!({"column": column, "op": "between", "values": ["1", "2", "3"]});
    assert_eq!(fixture.fields(filtered(three)), ["filters[0].values"]);
    // "=" vẫn chỉ nhận giá trị trong danh sách của cột chiều.
    let equals = json!({"column": column, "op": "=", "value": "100"});
    assert_eq!(fixture.fields(filtered(equals)), ["filters[0].column"]);
}

/// Kỳ trong bộ lọc ngày tính trọn kỳ, viết thành hai vế >= và <.
#[test]
fn date_filters_cover_whole_periods() {
    let fixture = Fixture::new();
    let date = format!("strptime(\"{DATE}\", '%m/%d/%Y %H:%M')");
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": DATE, "op": "between", "values": ["2017-03", "2017-06"]})
        ),
        [
            format!("{date} >= TIMESTAMP '2017-03-01'"),
            format!("{date} < TIMESTAMP '2017-07-01'")
        ]
    );
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": DATE, "op": ">", "value": "2017-05"})
        ),
        [format!("{date} >= TIMESTAMP '2017-06-01'")]
    );
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": DATE, "op": "<=", "value": "2017-05-31"})
        ),
        [format!("{date} < TIMESTAMP '2017-06-01'")]
    );
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": DATE, "op": "<", "value": "2017"})
        ),
        [format!("{date} < TIMESTAMP '2017-01-01'")]
    );
    let wrong = json!({"column": DATE, "op": ">=", "value": "tháng 3"});
    assert_eq!(fixture.fields(filtered(wrong)), ["filters[0].value"]);
}

#[test]
fn text_ranges_and_unknown_ops() {
    let fixture = Fixture::new();
    assert_eq!(
        filter_sql(
            &fixture,
            json!({"column": "Market", "op": "<", "value": "LATAM"})
        ),
        ["\"Market\" < 'LATAM'"]
    );
    let like = json!({"column": "Market", "op": "like", "value": "E%"});
    assert_eq!(fixture.fields(filtered(like)), ["filters[0].op"]);
    // Cột nhạy cảm không có danh sách giá trị để đối chiếu.
    let email = json!({"column": "Customer Email", "op": ">", "value": "a"});
    assert_eq!(fixture.fields(filtered(email)), ["filters[0].value"]);
}

// Kỳ tuần và ngày (explain/period.rs).

#[test]
fn weeks_and_days_are_periods() {
    let week = Period::parse(Grain::Week, "2017-W22").unwrap();
    let (start, end) = week.range().unwrap();
    assert_eq!(
        (start.to_string(), end.to_string()),
        ("2017-05-29".to_string(), "2017-06-05".to_string())
    );
    assert_eq!(week.label(), "Tuần 29/5/2017 - 4/6/2017");
    assert_eq!(week.previous().text(), "2017-W21");
    assert_eq!(week.same_last_year().text(), "2016-W22");
    // Tuần đầu năm 2020 bắt đầu 30/12/2019 theo ISO 8601.
    assert_eq!(
        Period::parse(Grain::Week, "2020-W01")
            .unwrap()
            .range()
            .unwrap()
            .0
            .to_string(),
        "2019-12-30"
    );
    let day = Period::parse(Grain::Day, "2016-02-29").unwrap();
    assert_eq!(day.label(), "Ngày 29/2/2016");
    assert_eq!(day.previous().text(), "2016-02-28");
    assert_eq!(day.same_last_year().text(), "2015-02-28");
    let date = chrono::NaiveDate::from_ymd_opt(2017, 6, 1).unwrap();
    assert_eq!(Period::containing(Grain::Week, date).text(), "2017-W22");
    for (grain, text) in [
        (Grain::Week, "2017-22"),
        (Grain::Week, "2017-W54"),
        (Grain::Day, "2017-02-30"),
        (Grain::Day, "2017-2-3"),
    ] {
        assert!(Period::parse(grain, text).is_err(), "{text}");
    }
}

// Độ đo trung bình, tỷ lệ dòng có một giá trị, đếm phân biệt (explain/measure_column.rs).

fn measured(measure: serde_json::Value) -> serde_json::Value {
    json!({
        "measure": measure,
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": []
    })
}

#[test]
fn average_is_a_ratio_of_sum_and_count() {
    let fixture = Fixture::new();
    let plan = fixture.plan(measured(
        json!({"column": "Order Item Total", "agg": "avg"}),
    ));
    assert_eq!(plan.measure.label, "Trung bình Order Item Total");
    assert!(plan.measure.denominator.is_some());
    let slices = build_slices_sql(&plan);
    assert!(
        slices.contains("COUNT(CASE WHEN") && slices.contains("AS compare_denominator"),
        "{slices}"
    );
    let wrong = measured(json!({"column": "Market", "agg": "avg"}));
    assert_eq!(fixture.fields(wrong), ["measure.column"]);
}

#[test]
fn share_counts_rows_with_one_listed_value() {
    let fixture = Fixture::new();
    let form = measured(json!({"column": "Market", "agg": "share", "value": "Europe"}));
    let plan = fixture.plan(form);
    assert_eq!(plan.measure.label, "Tỷ lệ Market là Europe");
    assert_eq!(
        plan.measure.aggregate.to_string(),
        "ROUND(COUNT(CASE WHEN \"Market\" = 'Europe' THEN 1 END) / NULLIF(COUNT(*), 0), 4)"
    );
    let missing = measured(json!({"column": "Market", "agg": "share"}));
    assert_eq!(fixture.fields(missing), ["measure.value"]);
    let unknown = measured(json!({"column": "Market", "agg": "share", "value": "Asia"}));
    assert_eq!(fixture.fields(unknown), ["measure.value"]);
    let number = measured(json!({"column": "Order Item Total", "agg": "share", "value": "1"}));
    assert_eq!(fixture.fields(number), ["measure.column"]);
}

#[test]
fn distinct_count_needs_a_key_and_has_no_volume_split() {
    let fixture = Fixture::new();
    let plan = fixture.plan(measured(
        json!({"column": "Order Id", "agg": "count_distinct"}),
    ));
    assert_eq!(plan.measure.label, "Số Order Id");
    assert_eq!(
        plan.measure.aggregate.to_string(),
        "COUNT(DISTINCT \"Order Id\")"
    );
    assert_eq!(plan.volume, None);
    let segment = build_dimension_sql(&plan, "Customer Segment");
    assert!(segment.contains("COUNT(DISTINCT CASE WHEN"), "{segment}");
    let wrong = measured(json!({"column": "Market", "agg": "count_distinct"}));
    assert_eq!(fixture.fields(wrong), ["measure.column"]);
}
