// SPDX-License-Identifier: Apache-2.0
/* service/explain/frames_tests.rs - test phần thuần của các khuôn câu "tại sao" thêm ở #244: một nhóm so với các nhóm
còn lại (explain/group_base.rs), bộ lọc mở rộng (explain/filter.rs), độ đo mới (explain/measure_column.rs), kỳ tuần và
ngày (explain/period.rs): đọc và kiểm form, SQL dựng ra. Hồ sơ DataCo thu nhỏ của explain/check_tests.rs; trọn đường
trên Parquet tạm ở explain/frames_run_tests.rs. */

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde_json::json;

use crate::service::explain::check::PlanSlices;
use crate::service::explain::check_tests::{Fixture, DATE};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::premise::AskedDirection;
use crate::service::explain::sql::{build_dimension_sql, build_slices_sql, build_why_sql};
use crate::service::explain::unusual::{build_group_parts, pick_step};

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
        GroupBase::Rest.build_label("Market", &labels, false),
        "các nhóm còn lại"
    );
    labels.insert("Market".to_string(), "Thị trường".to_string());
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels, false),
        "các thị trường còn lại"
    );
    // So với trung bình các nhóm còn lại (độ đo cộng được, explain/rest_average.rs) thì nhãn nói rõ là trung bình.
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels, true),
        "trung bình các thị trường còn lại"
    );
    assert_eq!(
        GroupBase::Value("LATAM".to_string()).build_label("Market", &labels, true),
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
    let key = measured(json!({"column": "Order Id", "agg": "avg"}));
    assert_eq!(fixture.fields(key), ["measure.column"]);
}

#[test]
fn share_counts_rows_with_one_listed_value() {
    let fixture = Fixture::new();
    let form = measured(json!({"column": "Market", "agg": "share", "value": "Europe"}));
    let plan = fixture.plan(form);
    assert_eq!(plan.measure.label, "Tỷ lệ Market là Europe");
    assert!(!plan.dimensions.contains(&"Market".to_string()));
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

// Kỳ mở rộng: độ chi tiết tuần, ngày và kiểu so (compare_with).

fn compared_periods(form: serde_json::Value) -> (String, String) {
    let fixture = Fixture::new();
    match fixture.plan(form).slices {
        PlanSlices::Periods { base, compare, .. } => (base.text(), compare.text()),
        other => panic!("expected two periods, got {other:?}"),
    }
}

#[test]
fn week_and_day_grains_compare_with_the_period_before() {
    let week = json!({
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "week"},
        "compare": "2017-W33",
        "filters": []
    });
    assert_eq!(
        compared_periods(week.clone()),
        ("2017-W32".to_string(), "2017-W33".to_string())
    );
    let plan = Fixture::new().plan(week);
    let sql = build_why_sql(&plan);
    assert!(
        sql.total_sql.starts_with("SELECT date_trunc('week', "),
        "{}",
        sql.total_sql
    );
    assert!(
        sql.compare_filter.contains("TIMESTAMP '2017-08-14'"),
        "{}",
        sql.compare_filter
    );
    assert!(
        sql.compare_filter.contains("TIMESTAMP '2017-08-21'"),
        "{}",
        sql.compare_filter
    );
    let day = json!({
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "day"},
        "compare": "2017-08-15",
        "filters": []
    });
    assert_eq!(
        compared_periods(day),
        ("2017-08-14".to_string(), "2017-08-15".to_string())
    );
}

#[test]
fn compare_with_picks_the_same_period_last_year() {
    let form = |compare_with: &str| {
        json!({
            "metric": "doanh thu",
            "time": {"column": DATE, "grain": "quarter"},
            "compare": "2017-Q3",
            "compare_with": compare_with,
            "filters": []
        })
    };
    assert_eq!(
        compared_periods(form("same_period_last_year")),
        ("2016-Q3".to_string(), "2017-Q3".to_string())
    );
    assert_eq!(
        compared_periods(form("previous")),
        ("2017-Q2".to_string(), "2017-Q3".to_string())
    );
    // base ghi rõ thì thắng compare_with.
    let mut explicit = form("same_period_last_year");
    explicit["base"] = json!("2017-Q1");
    assert_eq!(
        compared_periods(explicit),
        ("2017-Q1".to_string(), "2017-Q3".to_string())
    );
    // Hồ sơ có dữ liệu từ 2015-01: cùng kỳ năm trước của quý 1/2015 không có dòng nào.
    let mut early = form("same_period_last_year");
    early["compare"] = json!("2015-Q1");
    assert_eq!(Fixture::new().fields(early), ["compare_with"]);
}

// Tìm kỳ hay nhóm bất thường (explain/unusual.rs).

#[test]
fn steps_are_picked_by_the_asked_direction() {
    let values: Vec<Decimal> = [10, 30, 5, 6, -40].into_iter().map(Decimal::from).collect();
    // Bước: +20, -25, +1, -46.
    assert_eq!(pick_step(&values, None), Some(4));
    assert_eq!(pick_step(&values, Some(AskedDirection::Up)), Some(1));
    assert_eq!(pick_step(&values, Some(AskedDirection::Down)), Some(4));
    let rising: Vec<Decimal> = [1, 2, 3].into_iter().map(Decimal::from).collect();
    assert_eq!(pick_step(&rising, Some(AskedDirection::Down)), None);
    assert_eq!(pick_step(&rising[..1], None), None);
}

/// Phần của từng nhóm với tỷ lệ cộng lại đúng R1 - R0: nhóm A 1/4 lên 3/4, nhóm B 1/4 lên 1/4 (mẫu 4 và 4 mỗi bên),
/// R0 = 2/8, R1 = 4/8.
#[test]
fn ratio_group_parts_add_up_to_the_change() {
    let d = |n: i64| Decimal::from(n);
    let groups = vec![
        (None, vec![d(1), d(3), d(4), d(4)]),
        (None, vec![d(1), d(1), d(4), d(4)]),
    ];
    let parts = build_group_parts(&groups);
    assert_eq!(parts.iter().sum::<Decimal>(), Decimal::new(25, 2));
    assert_eq!(parts[0], Decimal::new(25, 2));
    let totals = vec![(None, vec![d(5), d(2)])];
    assert_eq!(build_group_parts(&totals), [d(-3)]);
}

#[test]
fn find_needs_two_periods_and_a_dimension() {
    let fixture = Fixture::new();
    let mut form = json!({
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "month"},
        "compare": "2017-08",
        "find": {"kind": "group", "column": "Order Item Total"},
        "filters": []
    });
    assert_eq!(fixture.fields(form.clone()), ["find.column"]);
    form["find"] = json!({"kind": "group"});
    assert_eq!(fixture.fields(form.clone()), ["find.column"]);
    form["find"] = json!({"kind": "period"});
    assert!(fixture.check(form.clone()).is_ok());
    let groups = json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Europe"},
        "find": {"kind": "period"},
    });
    assert_eq!(fixture.fields(groups), ["find"]);
}

/// Đếm dòng đã lọc của một nhóm so với mọi nhóm còn lại: lỗi chỉ sang tỷ lệ (share) với đúng cột và giá trị của bộ lọc.
#[test]
fn rest_with_a_filtered_count_points_to_share() {
    let fixture = Fixture::new();
    let form = json!({
        "measure": {"column": "Order Id", "agg": "count"},
        "groups": {"column": "Market", "compare": "Pacific Asia"},
        "filters": [{"column": "Customer Segment", "op": "=", "value": "Corporate"}]
    });
    let errors = fixture.check(form.clone()).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "measure.agg");
    assert!(
        errors[0].message.contains(
            "{\"column\": \"Customer Segment\", \"agg\": \"share\", \"value\": \"Corporate\"}"
        ),
        "{}",
        errors[0].message
    );
    // So với một nhóm khác thì số đếm vẫn nhận.
    let mut two = form;
    two["groups"]["base"] = json!("Europe");
    assert!(fixture.check(two).is_ok());
}

/// "Vì sao Scotland có tỷ lệ rút học cao ở đợt 2014J?": kỳ học là giá trị của cột chiều. Mô hình đặt nó vào time thì lỗi
/// chỉ sang bộ lọc "=" đúng cột và giá trị, giữ groups.
#[test]
fn dimension_period_with_groups_points_to_a_filter() {
    let fixture = Fixture::new();
    let form = json!({
        "metric": "doanh thu",
        "time": {"column": "Customer Segment", "grain": "year"},
        "compare": "Corporate",
        "groups": {"column": "Market", "compare": "Europe"},
        "filters": []
    });
    let errors = fixture.check(form).unwrap_err();
    assert_eq!(errors[0].field, "time.column");
    assert!(
        errors[0].message.contains(
            "add the filter {\"column\": \"Customer Segment\", \"op\": \"=\", \"value\": \"Corporate\"}"
        ),
        "{}",
        errors[0].message
    );
}

/// Cột có bí danh "mỗi đơn" (#260): sum là tổng nên gọi "Tổng lợi nhuận"; avg và count giữ nghĩa "mỗi đơn" của cột.
#[test]
fn sum_of_a_per_order_column_is_called_a_total() {
    let mut fixture = Fixture::new();
    let total = fixture
        .columns
        .iter_mut()
        .find(|c| c.name == "Order Item Total")
        .unwrap();
    total.aliases = vec!["lợi nhuận mỗi đơn".to_string()];
    let label = |agg: &str| {
        let plan = fixture.plan(measured(json!({"column": "Order Item Total", "agg": agg})));
        plan.measure.label
    };
    assert_eq!(label("sum"), "Tổng lợi nhuận");
    assert_eq!(label("avg"), "Trung bình lợi nhuận mỗi đơn");
    assert_eq!(label("count"), "Số dòng có lợi nhuận mỗi đơn");
}
