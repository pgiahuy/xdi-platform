// SPDX-License-Identifier: Apache-2.0
/* service/explain/check_tests.rs - test phần thuần của câu "tại sao" (#38): nhận câu "tại sao" (explain/form.rs), đọc
và kiểm form (explain/check.rs), kỳ (explain/period.rs), dựng SQL (explain/sql.rs). Không cần mô hình, DuckDB hay
Postgres. Hồ sơ là bản thu nhỏ của DataCo; explain/run_tests.rs dùng lại hồ sơ này để chạy trọn chuỗi. */

use serde_json::json;
use uuid::Uuid;

use crate::engine::store::{MetricDefinition, ProfileColumn, ValueRow};
use crate::service::ask::examples::build_why_example;
use crate::service::ask::values::ValueIndex;
use crate::service::explain::check::{build_why_check, PlanSlices, WhyPlan, WhyProfile};
use crate::service::explain::choices::build_base_choice;
use crate::service::explain::form::{
    build_form_from_reply, build_question_kind, build_why_repair_prompt, FieldError, QuestionKind,
    WhyForm,
};
use crate::service::explain::measure::{parse_expr, Additive};
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::{
    build_dimension_sql, build_equals, build_slices_sql, build_why_sql,
};
use crate::service::prompt_fixtures::check_fixture;

pub(super) const TABLE: &str = "dataco";
pub(super) const DATE: &str = "order date (DateOrders)";
pub(super) const DATE_FORMAT: &str = "%m/%d/%Y %H:%M";
pub(super) const MARKETS: [&str; 5] = ["Europe", "LATAM", "Pacific Asia", "USCA", "Africa"];
pub(super) const SEGMENTS: [&str; 3] = ["Consumer", "Corporate", "Home Office"];

pub(crate) fn column(
    id: u128,
    name: &str,
    data_type: &str,
    role: &str,
    distinct: i64,
) -> ProfileColumn {
    ProfileColumn {
        id: Uuid::from_u128(id),
        position: i32::try_from(id).unwrap(),
        name: name.to_string(),
        data_type: data_type.to_string(),
        role: role.to_string(),
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        null_count: Some(0),
        distinct_count: Some(distinct),
        min_value: None,
        max_value: None,
        time_format: None,
        unit: None,
    }
}

/// Hồ sơ kiểu DataCo: hai cột chiều, một cột ngày dạng chữ, hai cột độ đo, Order Id là key lặp lại, một cột chiều
/// nhạy cảm và một cột chiều nhiều giá trị (không phân rã).
pub(super) fn dataco_columns(min_day: &str, max_day: &str) -> Vec<ProfileColumn> {
    let mut date = column(3, DATE, "VARCHAR", "time", 900);
    date.time_format = Some(DATE_FORMAT.to_string());
    date.min_value = Some(format!("{min_day} 00:00:00"));
    date.max_value = Some(format!("{max_day} 23:00:00"));
    let mut email = column(7, "Customer Email", "VARCHAR", "dimension", 3);
    email.is_sensitive = true;
    vec![
        column(1, "Market", "VARCHAR", "dimension", 5),
        column(2, "Customer Segment", "VARCHAR", "dimension", 3),
        date,
        column(4, "Order Item Total", "DOUBLE", "measure", 5000),
        column(5, "Order Id", "INTEGER", "key", 9400),
        column(6, "Order Region", "VARCHAR", "dimension", 64),
        email,
        column(8, "Order Item Quantity", "INTEGER", "measure", 5),
    ]
}

pub(super) fn dataco_values() -> ValueIndex {
    let row = |id: u128, name: &str, value: &str| ValueRow {
        column_id: Uuid::from_u128(id),
        column_name: name.to_string(),
        value: value.to_string(),
        aliases: Vec::new(),
    };
    let mut rows: Vec<ValueRow> = MARKETS.iter().map(|m| row(1, "Market", m)).collect();
    rows.extend(SEGMENTS.iter().map(|s| row(2, "Customer Segment", s)));
    ValueIndex::build(&rows)
}

pub(super) fn metric(name: &str, expression: &str, filter: Option<&str>) -> MetricDefinition {
    MetricDefinition {
        name: name.to_string(),
        description: String::new(),
        expression: expression.to_string(),
        filter: filter.map(str::to_string),
        time_column: None,
        unit: None,
        confirmed: true,
    }
}

pub(super) fn revenue() -> MetricDefinition {
    metric("doanh thu", "SUM(\"Order Item Total\")", None)
}

/// Form của câu "Tại sao doanh thu quý 3 năm 2017 giảm", như mock trả.
pub(super) fn quarter_form() -> serde_json::Value {
    json!({
        "terms": [{"text": "doanh thu", "kind": "metric", "value": "doanh thu"}],
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": []
    })
}

struct Fixture {
    columns: Vec<ProfileColumn>,
    values: ValueIndex,
    metrics: Vec<MetricDefinition>,
}

impl Fixture {
    fn new() -> Self {
        Fixture {
            columns: dataco_columns("2015-01-01", "2018-01-31"),
            values: dataco_values(),
            metrics: vec![
                revenue(),
                metric(
                    "doanh thu đã giao",
                    "SUM(\"Order Item Total\")",
                    Some(
                        "\"Delivery Status\" <> 'Shipping canceled' OR \"Delivery Status\" IS NULL",
                    ),
                ),
                metric("giá trị đơn trung bình", "AVG(\"Order Item Total\")", None),
                metric("số đơn", "COUNT(DISTINCT \"Order Id\")", None),
                metric("số dòng", "count(*)", None),
                metric(
                    "gài",
                    "SUM(\"Order Item Total\") FROM dataco; DROP TABLE dataco",
                    None,
                ),
            ],
        }
    }

    fn profile(&self) -> WhyProfile<'_> {
        WhyProfile {
            table: TABLE,
            columns: &self.columns,
            values: &self.values,
            metrics: &self.metrics,
            row_count: Some(20_000),
        }
    }

    fn check(&self, form: serde_json::Value) -> Result<WhyPlan, Vec<FieldError>> {
        self.check_with(form, None)
    }

    fn check_with(
        &self,
        form: serde_json::Value,
        choice: Option<&str>,
    ) -> Result<WhyPlan, Vec<FieldError>> {
        let form: WhyForm = serde_json::from_value(form).unwrap();
        build_why_check(&form, &self.profile(), choice)
    }

    fn plan(&self, form: serde_json::Value) -> WhyPlan {
        self.check(form).unwrap()
    }

    /// Tên các trường sai.
    fn fields(&self, form: serde_json::Value) -> Vec<String> {
        let errors = self.check(form).unwrap_err();
        errors.into_iter().map(|e| e.field).collect()
    }
}

fn with(mut form: serde_json::Value, key: &str, value: serde_json::Value) -> serde_json::Value {
    form[key] = value;
    form
}

fn periods(plan: &WhyPlan) -> (String, String, bool) {
    match &plan.slices {
        PlanSlices::Periods {
            base,
            compare,
            base_by_default,
            ..
        } => (base.text(), compare.text(), *base_by_default),
        other => panic!("expected two periods, got {other:?}"),
    }
}

#[test]
fn why_questions_are_found_with_and_without_accents() {
    for question in [
        "Tại sao doanh thu quý 3 năm 2017 giảm?",
        "Vì sao doanh thu tháng 5 tăng",
        "tai sao doanh thu giam",
        "VÌ SAO lợi nhuận Europe thấp?",
        "Lý do doanh thu giảm",
        "Nguyên nhân số đơn giảm ở LATAM",
        "Doanh thu giảm do đâu?",
        "Why did revenue drop in Q3?",
        "Tại sao doanh thu Europe thấp hơn LATAM",
        "Sao lại giảm doanh thu quý 3?",
        "doanh thu sao lai giam",
        "Vì đâu lợi nhuận Europe thấp?",
        "Tại vì sao số đơn giảm",
        "Do đâu mà doanh thu tháng 5 tăng",
        "Vì lý do gì doanh thu giảm",
        "How come revenue fell?",
    ] {
        assert_eq!(
            build_question_kind(question),
            QuestionKind::Why,
            "{question}"
        );
    }
}

#[test]
fn other_questions_are_plain() {
    for question in [
        "Doanh thu theo thị trường",
        "Top 10 danh mục có doanh thu cao nhất",
        "Số đơn hàng theo trạng thái giao hàng",
        "Tài sản theo năm",
        "doanh thu whyte",
        // "sao" đứng một mình, "đâu", "lại" không đủ để là câu "tại sao".
        "Sao chép bảng doanh thu",
        "Doanh số theo vùng",
        "Doanh thu theo sao lưu",
        "Đơn hàng giao tới đâu nhiều nhất",
        "Doanh thu lại theo quý",
        "Số sao đánh giá trung bình",
        // Không có từ khóa: đi đường câu thường.
        "thế nào mà giảm",
    ] {
        assert_eq!(
            build_question_kind(question),
            QuestionKind::Plain,
            "{question}"
        );
    }
}

/// Khoảng ngày [đầu, cuối) của một kỳ viết như trong form; Err khi kỳ viết sai dạng.
fn build_period_range(
    grain: Grain,
    text: &str,
) -> Result<(chrono::NaiveDate, chrono::NaiveDate), String> {
    Period::parse(grain, text)?
        .range()
        .ok_or_else(|| format!("{text} is outside the supported dates"))
}

#[test]
fn periods_become_date_ranges() {
    let day = |text: &str| chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap();
    assert_eq!(
        build_period_range(Grain::Quarter, "2017-Q3"),
        Ok((day("2017-07-01"), day("2017-10-01")))
    );
    assert_eq!(
        build_period_range(Grain::Month, "2017-12"),
        Ok((day("2017-12-01"), day("2018-01-01")))
    );
    assert_eq!(
        build_period_range(Grain::Year, "2016"),
        Ok((day("2016-01-01"), day("2017-01-01")))
    );
    for (grain, text) in [
        (Grain::Quarter, "Q3 2017"),
        (Grain::Quarter, "2017-Q5"),
        (Grain::Quarter, "2017-07"),
        (Grain::Month, "2017-7"),
        (Grain::Month, "2017-13"),
        (Grain::Year, "17"),
        (Grain::Year, "2017-Q1"),
    ] {
        assert!(build_period_range(grain, text).is_err(), "{text}");
    }
    let q1 = Period::parse(Grain::Quarter, "2018-Q1").unwrap();
    assert_eq!(q1.previous().text(), "2017-Q4");
    assert_eq!(q1.same_last_year().label(), "Quý 1/2017");
}

/// "Tại sao doanh thu quý 3 năm 2017 giảm?": base trống là quý 2/2017, điều kiện đúng như mẫu why.
#[test]
fn quarter_compares_with_the_quarter_before() {
    let fixture = Fixture::new();
    let plan = fixture.plan(quarter_form());
    assert_eq!(
        periods(&plan),
        ("2017-Q2".to_string(), "2017-Q3".to_string(), true)
    );
    let sql = build_why_sql(&plan);
    let example = build_why_example().explanation.unwrap();
    assert_eq!(sql.base_filter, example.base.filter);
    assert_eq!(sql.compare_filter, example.compare.filter);
    assert_eq!(
        sql.total_sql,
        format!(
            "SELECT date_trunc('quarter', strptime(\"{DATE}\", '{DATE_FORMAT}')) AS \"quarter\", \
             SUM(CAST(\"Order Item Total\" AS DECIMAL(18,2))) AS \"Doanh thu\" FROM \"dataco\" WHERE ({} OR {}) GROUP BY 1 ORDER BY 1",
            example.base.filter, example.compare.filter
        )
    );
    // Cùng form thì cùng SQL.
    assert_eq!(build_why_sql(&fixture.plan(quarter_form())), sql);
    assert_eq!(plan.dimensions, ["Market", "Customer Segment"]);
    assert_eq!(plan.volume.as_deref(), Some("Order Id"));
}

/// Lựa chọn same_quarter_last_year của người hỏi đè base của form: quý 3/2016.
#[test]
fn base_choice_picks_the_same_quarter_last_year() {
    let fixture = Fixture::new();
    let form = with(quarter_form(), "base", json!("2017-Q1"));
    let plan = fixture
        .check_with(form.clone(), Some("same_quarter_last_year"))
        .unwrap();
    assert_eq!(
        periods(&plan),
        ("2016-Q3".to_string(), "2017-Q3".to_string(), false)
    );
    let sql = build_why_sql(&plan);
    assert!(
        sql.base_filter.contains("TIMESTAMP '2016-07-01'"),
        "{}",
        sql.base_filter
    );
    assert!(
        sql.base_filter.contains("TIMESTAMP '2016-10-01'"),
        "{}",
        sql.base_filter
    );
    // Lựa chọn không đọc được thì bỏ qua, dùng base của form.
    let plan = fixture.check_with(form, Some("tomorrow")).unwrap();
    assert_eq!(periods(&plan).0, "2017-Q1");
}

#[test]
fn base_choice_lists_the_usual_periods() {
    let q3 = Period::parse(Grain::Quarter, "2017-Q3").unwrap();
    let choice = build_base_choice(q3.previous(), q3);
    assert_eq!(choice.id, "why:base");
    assert_eq!(choice.selected.as_deref(), Some("previous_quarter"));
    let options: Vec<(&str, &str)> = choice
        .options
        .iter()
        .map(|o| (o.id.as_str(), o.label.as_str()))
        .collect();
    assert_eq!(
        options,
        [
            ("previous_quarter", "Quý liền trước (Quý 2/2017)"),
            ("same_quarter_last_year", "Cùng kỳ năm trước (Quý 3/2016)")
        ]
    );
    // Kỳ khác cả hai thành phương án thứ ba, đang chọn.
    let choice = build_base_choice(Period::parse(Grain::Quarter, "2016-Q1").unwrap(), q3);
    assert_eq!(choice.selected.as_deref(), Some("2016-Q1"));
    assert_eq!(choice.options.len(), 3);
    // So theo năm: kỳ liền trước là cùng kỳ năm trước, chỉ một phương án.
    let year = Period::parse(Grain::Year, "2017").unwrap();
    assert_eq!(build_base_choice(year.previous(), year).options.len(), 1);
}

/// "Vì sao doanh thu tháng 5 tăng": so tháng 5 với tháng 4.
#[test]
fn month_compares_with_the_month_before() {
    let fixture = Fixture::new();
    let form = with(
        with(
            quarter_form(),
            "time",
            json!({"column": DATE, "grain": "month"}),
        ),
        "compare",
        json!("2017-05"),
    );
    let plan = fixture.plan(form);
    assert_eq!(periods(&plan).0, "2017-04");
    let sql = build_why_sql(&plan);
    assert!(sql.compare_filter.contains("TIMESTAMP '2017-05-01'"));
    assert!(sql.compare_filter.contains("TIMESTAMP '2017-06-01'"));
    assert!(sql.total_sql.starts_with("SELECT date_trunc('month', "));
}

/// "Tại sao doanh thu Europe thấp hơn LATAM": hai nhóm của Market, Market không còn là chiều phân rã.
#[test]
fn two_groups_of_a_dimension() {
    let fixture = Fixture::new();
    let form = json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Europe", "base": "LATAM"},
        "filters": []
    });
    let plan = fixture.plan(form.clone());
    let sql = build_why_sql(&plan);
    assert_eq!(sql.base_filter, "\"Market\" = 'LATAM'");
    assert_eq!(sql.compare_filter, "\"Market\" = 'Europe'");
    assert_eq!(
        sql.total_sql,
        "SELECT \"Market\", SUM(CAST(\"Order Item Total\" AS DECIMAL(18,2))) AS \"Doanh thu\" FROM \"dataco\" WHERE (\"Market\" = 'LATAM' \
         OR \"Market\" = 'Europe') GROUP BY 1 ORDER BY 1"
    );
    assert_eq!(plan.dimensions, ["Customer Segment"]);
    // compare rỗng (schema bắt buộc trường này) khi so hai nhóm không có kỳ: cùng plan như khi bỏ trường.
    let mut empty = form.clone();
    empty["compare"] = json!("");
    assert_eq!(fixture.plan(empty), plan);
    // Có kỳ thì kỳ là khoảng thời gian của câu hỏi.
    let form = json!({
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "2017-Q3",
        "groups": {"column": "Market", "compare": "Europe", "base": "LATAM"},
    });
    let total = build_why_sql(&fixture.plan(form)).total_sql;
    assert!(total.contains("TIMESTAMP '2017-07-01'"), "{total}");
}

#[test]
fn unknown_columns_and_wrong_roles_are_field_errors() {
    let fixture = Fixture::new();
    let measure = |column: &str, agg: &str| {
        json!({
            "measure": {"column": column, "agg": agg},
            "time": {"column": DATE, "grain": "quarter"},
            "compare": "2017-Q3",
        })
    };
    assert_eq!(
        fixture.fields(measure("Revenue", "sum")),
        ["measure.column"]
    );
    assert_eq!(fixture.fields(measure("Market", "sum")), ["measure.column"]);
    // Đếm nhận cột bất kỳ.
    assert!(fixture.check(measure("Market", "count")).is_ok());
    let form = with(
        quarter_form(),
        "time",
        json!({"column": "Market", "grain": "quarter"}),
    );
    assert_eq!(fixture.fields(form), ["time.column"]);
    let form = json!({
        "metric": "doanh thu",
        "groups": {"column": "Order Item Total", "compare": "1", "base": "2"},
    });
    assert_eq!(fixture.fields(form), ["groups.column"]);
    // Nhiều chỗ sai thì báo hết trong một lần.
    let form = json!({
        "metric": "lợi nhuận",
        "time": {"column": "Ngày", "grain": "quarter"},
        "compare": "2017-Q3",
        "filters": [{"column": "Order Item Total", "op": "=", "value": "1"}]
    });
    assert_eq!(
        fixture.fields(form),
        ["metric", "time.column", "filters[0].column"]
    );
}

#[test]
fn periods_must_be_written_right_and_have_data() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.fields(with(quarter_form(), "compare", json!("Q3 2017"))),
        ["compare"]
    );
    let errors = fixture
        .check(with(quarter_form(), "compare", json!("2019-Q1")))
        .unwrap_err();
    assert_eq!(errors[0].field, "compare");
    assert!(
        errors[0].message.contains("no rows"),
        "{}",
        errors[0].message
    );
    // Quý đầu của dữ liệu không có quý trước để so.
    assert_eq!(
        fixture.fields(with(quarter_form(), "compare", json!("2015-Q1"))),
        ["base"]
    );
    assert_eq!(
        fixture.fields(with(quarter_form(), "base", json!("2017-Q3"))),
        ["base"]
    );
    let form = with(quarter_form(), "compare", serde_json::Value::Null);
    assert_eq!(fixture.fields(form), ["compare"]);
    // Schema bắt buộc compare nên mô hình có thể ghi chuỗi rỗng thay vì bỏ trường: so hai kỳ vẫn là lỗi compare.
    let form = with(quarter_form(), "compare", json!(""));
    assert_eq!(fixture.fields(form), ["compare"]);
    let form = json!({"metric": "doanh thu", "compare": "2017-Q3"});
    assert_eq!(fixture.fields(form), ["time"]);
}

#[test]
fn filter_and_group_values_must_be_listed() {
    let fixture = Fixture::new();
    let filter = |column: &str, value: &str| {
        with(
            quarter_form(),
            "filters",
            json!([{"column": column, "op": "=", "value": value}]),
        )
    };
    let errors = fixture
        .check(filter("Market", "Europe' OR 1=1 --"))
        .unwrap_err();
    assert_eq!(errors[0].field, "filters[0].value");
    assert!(
        errors[0].message.contains("\"Europe\""),
        "{}",
        errors[0].message
    );
    // Khác hoa thường cũng là giá trị khác.
    assert_eq!(
        fixture.fields(filter("Market", "europe")),
        ["filters[0].value"]
    );
    // Cột nhạy cảm không có danh sách giá trị.
    assert_eq!(
        fixture.fields(filter("Customer Email", "a@b.c")),
        ["filters[0].value"]
    );
    let form = with(
        filter("Market", "Europe"),
        "filters",
        json!([{"column": "Market", "op": "<>", "value": "Europe"}]),
    );
    assert_eq!(fixture.fields(form), ["filters[0].op"]);
    let groups = json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Europe", "base": "Europe' OR 1=1 --"},
    });
    assert_eq!(fixture.fields(groups), ["groups.base"]);
    let same = json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Europe", "base": "Europe"},
    });
    assert_eq!(fixture.fields(same), ["groups.base"]);

    // Bộ lọc hợp lệ vào mọi câu, và cột đã lọc không còn là chiều phân rã.
    let plan = fixture.plan(filter("Market", "Europe"));
    assert_eq!(plan.dimensions, ["Customer Segment"]);
    let sql = build_why_sql(&plan);
    assert!(
        sql.total_sql.contains("WHERE \"Market\" = 'Europe' AND ("),
        "{}",
        sql.total_sql
    );
}

/// Giá trị có dấu nháy, nếu tới được chỗ dựng SQL, vẫn chỉ là một chuỗi: dấu nháy được nhân đôi khi in.
#[test]
fn quotes_in_values_stay_inside_the_string() {
    let condition = build_equals("Market", "Europe' OR 1=1 --");
    assert_eq!(condition.to_string(), "\"Market\" = 'Europe'' OR 1=1 --'");
    let condition = build_equals("Cột \"lạ\"", "a");
    assert_eq!(condition.to_string(), "\"Cột \"\"lạ\"\"\" = 'a'");
}

#[test]
fn metrics_insert_their_expression_and_filter() {
    let fixture = Fixture::new();
    let plan = fixture.plan(with(quarter_form(), "metric", json!("Doanh Thu Đã Giao")));
    assert_eq!(plan.measure.label, "Doanh thu đã giao");
    let sql = build_why_sql(&plan);
    assert!(sql
        .total_sql
        .contains("SUM(CAST(\"Order Item Total\" AS DECIMAL(18,2))) AS \"Doanh thu đã giao\""));
    // Điều kiện có OR được bọc ngoặc trước khi ghép bằng AND.
    assert!(
        sql.total_sql.contains(
            "WHERE (\"Delivery Status\" <> 'Shipping canceled' OR \"Delivery Status\" IS NULL) AND ("
        ),
        "{}",
        sql.total_sql
    );
    let dimension = build_dimension_sql(&plan, "Market");
    assert!(
        dimension.contains("\"Delivery Status\" <> 'Shipping canceled'"),
        "{dimension}"
    );

    // Có cả chỉ số lẫn measure thì dùng chỉ số.
    let form = with(
        quarter_form(),
        "measure",
        json!({"column": "Order Item Quantity", "agg": "sum"}),
    );
    assert_eq!(
        fixture.plan(form).measure.metric.as_deref(),
        Some("doanh thu")
    );
    // Chỉ số không cộng được, không phải tỷ lệ của hai dạng cộng được, hay có chữ thừa sau công thức, thì không đi
    // đường form.
    for name in ["số đơn", "gài"] {
        assert_eq!(
            fixture.fields(with(quarter_form(), "metric", json!(name))),
            ["metric"],
            "{name}"
        );
    }
    let plan = fixture.plan(with(quarter_form(), "metric", json!("số dòng")));
    assert_eq!(plan.measure.additive, Additive::Count { value: None });
    assert_eq!(plan.measure.denominator, None);
}

/// AVG(x) và thương của hai dạng cộng được là độ đo tỷ lệ: tử và mẫu tính riêng theo từng lát cắt.
#[test]
fn ratio_metrics_split_into_numerator_and_denominator() {
    let mut fixture = Fixture::new();
    let plan = fixture.plan(with(
        quarter_form(),
        "metric",
        json!("giá trị đơn trung bình"),
    ));
    let total = parse_expr("\"Order Item Total\"").unwrap();
    assert_eq!(
        plan.measure.additive,
        Additive::Sum {
            value: total.clone(),
            decimal: true
        }
    );
    assert_eq!(
        plan.measure.denominator,
        Some(Additive::Count { value: Some(total) })
    );
    let sql = build_why_sql(&plan);
    assert!(
        sql.total_sql.contains(
            "ROUND(SUM(CAST(\"Order Item Total\" AS DECIMAL(18,2))) / NULLIF(COUNT(\"Order Item Total\"), 0), 4)"
        ),
        "{}",
        sql.total_sql
    );
    let dimension = build_dimension_sql(&plan, "Market");
    assert!(
        dimension.contains("COUNT(CASE WHEN ") && dimension.contains("AS compare_denominator"),
        "{dimension}"
    );
    assert!(build_slices_sql(&plan).contains("AS base_denominator"));

    fixture.metrics.push(metric(
        "tỷ lệ giao trễ",
        "COUNT(CASE WHEN \"Late_delivery_risk\" = 1 THEN 1 END) / (COUNT(*))",
        None,
    ));
    fixture.metrics.push(metric(
        "lãi trên doanh thu",
        "SUM(\"Order Item Total\") / SUM(\"Order Item Quantity\")",
        None,
    ));
    let late = fixture.plan(with(quarter_form(), "metric", json!("tỷ lệ giao trễ")));
    assert_eq!(
        late.measure.denominator,
        Some(Additive::Count { value: None })
    );
    let margin = fixture.plan(with(quarter_form(), "metric", json!("lãi trên doanh thu")));
    assert!(matches!(
        margin.measure.denominator,
        Some(Additive::Sum { .. })
    ));
    // Thương có tử không cộng được (đếm phân biệt) thì vẫn không phân rã được.
    fixture.metrics.push(metric(
        "doanh thu mỗi đơn",
        "SUM(\"Order Item Total\") / COUNT(DISTINCT \"Order Id\")",
        None,
    ));
    assert_eq!(
        fixture.fields(with(quarter_form(), "metric", json!("doanh thu mỗi đơn"))),
        ["metric"]
    );
}

/// Số thực đổi sang DECIMAL(18, 2) ở từng dòng; số nguyên cộng thẳng; đếm không cần COALESCE.
#[test]
fn sliced_measures_are_exact() {
    let fixture = Fixture::new();
    let plan = fixture.plan(quarter_form());
    let example = build_why_example().explanation.unwrap();
    assert_eq!(
        build_dimension_sql(&plan, "Market"),
        format!(
            "SELECT \"Market\", COALESCE(SUM(CASE WHEN {base} THEN CAST(\"Order Item Total\" AS DECIMAL(18, 2)) END), \
             0) AS base_value, COALESCE(SUM(CASE WHEN {compare} THEN CAST(\"Order Item Total\" AS DECIMAL(18, 2)) \
             END), 0) AS compare_value FROM \"dataco\" WHERE ({base} OR {compare}) GROUP BY \"Market\"",
            base = example.base.filter,
            compare = example.compare.filter
        )
    );
    let slices = build_slices_sql(&plan);
    assert!(
        slices.contains(&format!(
            "COUNT(DISTINCT CASE WHEN {} THEN \"Order Id\" END) AS base_volume",
            example.base.filter
        )),
        "{slices}"
    );
    let form = with(quarter_form(), "metric", serde_json::Value::Null);
    let quantity = with(
        form.clone(),
        "measure",
        json!({"column": "Order Item Quantity", "agg": "sum"}),
    );
    let sql = build_dimension_sql(&fixture.plan(quantity), "Market");
    assert!(sql.contains("THEN \"Order Item Quantity\" END"), "{sql}");
    let count = with(
        form,
        "measure",
        json!({"column": "Order Id", "agg": "count"}),
    );
    let sql = build_dimension_sql(&fixture.plan(count), "Market");
    assert!(sql.contains("COUNT(CASE WHEN "), "{sql}");
}

#[test]
fn dimensions_and_volume_follow_the_profile() {
    let mut fixture = Fixture::new();
    // Chín cột chiều ít giá trị: tối đa tám chiều, theo thứ tự cột.
    for i in 0..9 {
        fixture
            .columns
            .push(crate::service::explain::check_tests::column(
                20 + i,
                &format!("d{i}"),
                "VARCHAR",
                "dimension",
                4,
            ));
    }
    let plan = fixture.plan(quarter_form());
    assert_eq!(plan.dimensions.len(), 8);
    assert_eq!(&plan.dimensions[..3], ["Market", "Customer Segment", "d0"]);

    let mut fixture = Fixture::new();
    fixture
        .columns
        .push(column(30, "Order Item Id", "INTEGER", "key", 20_000));
    // Hai cột key: không đoán cột nào là đơn.
    assert_eq!(fixture.plan(quarter_form()).volume, None);
    fixture.columns.retain(|c| c.name != "Order Id");
    // Cột key không lặp lại: đếm nó chỉ là đếm dòng.
    assert_eq!(fixture.plan(quarter_form()).volume, None);
}

#[test]
fn replies_are_read_like_sql_replies() {
    let object = quarter_form().to_string();
    for reply in [object.clone(), format!("```json\n{object}\n```\nXong.")] {
        let read = build_form_from_reply(&reply).unwrap();
        assert_eq!(read.form.compare.as_deref(), Some("2017-Q3"));
        assert_eq!(read.json, quarter_form());
    }
    for reply in [
        "SELECT 1",
        "{\"metric\": ",
        "{\"time\": {\"column\": \"x\", \"grain\": \"week\"}}",
    ] {
        let error = build_form_from_reply(reply).unwrap_err();
        assert_eq!(error.field, "form", "{reply}");
    }
}

#[test]
fn repair_prompt_lists_each_field_error() {
    let errors = vec![
        FieldError::new("measure.column", "\"Revenue\" is not a column of dataco"),
        FieldError::new("compare", "\"Q3 2017\" is not a quarter such as 2017-Q3"),
    ];
    let prompt = build_why_repair_prompt("PROMPT", " {\"compare\": \"Q3 2017\"} ", &errors);
    assert!(prompt.starts_with("PROMPT\n\nYour previous reply:\n{\"compare\": \"Q3 2017\"}\n"));
    assert!(prompt.ends_with(
        "- measure.column: \"Revenue\" is not a column of dataco\n- compare: \"Q3 2017\" is not a quarter such as 2017-Q3"
    ));
}

/// Prompt sửa form đầy đủ so với file mẫu (service/prompt_fixtures.rs). Prompt gốc thay bằng một dòng ngắn: prompt form
/// đã có file mẫu riêng (why_prompt.txt).
#[test]
fn why_repair_prompt_matches_the_fixture() {
    let errors = vec![
        FieldError::new("measure.column", "\"Revenue\" is not a column of dataco"),
        FieldError::new("compare", "\"Q3 2017\" is not a quarter such as 2017-Q3"),
    ];
    let prompt = build_why_repair_prompt(
        "Question: Vì sao doanh thu quý 3 năm 2017 giảm?",
        " {\"measure\": {\"column\": \"Revenue\", \"agg\": \"sum\"}, \"compare\": \"Q3 2017\"} ",
        &errors,
    );
    check_fixture("why_repair_prompt.txt", &prompt);
}

// Cột chiều đặt vào "time" (như cột kỳ học "2013J", "2014J"): lời báo chỉ đường sang so hai nhóm, để vòng sửa của mô
// hình không bỏ "time" rồi quên "groups".
#[test]
fn dimension_as_time_points_to_groups() {
    let fixture = Fixture::new();
    let errors = fixture
        .check(with(
            quarter_form(),
            "time",
            json!({"column": "Market", "grain": "quarter"}),
        ))
        .unwrap_err();
    assert_eq!(errors[0].field, "time.column");
    assert!(
        errors[0].message.contains("set \"groups\" to this column"),
        "{}",
        errors[0].message
    );
}
