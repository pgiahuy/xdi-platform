// SPDX-License-Identifier: Apache-2.0
/* service/understanding_tests.rs - test của service/understanding.rs: câu SQL của mock (engine/llm/mock_answers.json)
đi qua guard thật rồi dựng thẻ và vai trò cột, như endpoint hỏi (#33) sẽ làm; cộng các dạng câu khó đọc (phân quyền,
khoảng thời gian, OR, CTE, UNION, ORDER BY 2, SELECT *, không GROUP BY). Mọi test kiểm mỗi đoạn SQL của thẻ là đoạn
nguyên văn của sql_guarded. Hồ sơ cột là hồ sơ viết sẵn của dataco trong mock_answers.json, cột ngày có vai trò time.
Giả định đi qua ask_choices.rs như endpoint hỏi sẽ làm, để thẻ và choices cùng id. */

use std::path::PathBuf;

use uuid::Uuid;

use super::ask_choices::{build_assumption_choices, build_choice_id, TIME_CHOICE_ID};
use super::ask_context::{AskContext, ColumnProfile};
use super::ask_contract::{Choice, ColumnRole, Understanding, UnderstoodItem};
use super::ask_reply::{build_answer_from_reply, Assumption, ChoiceKind, ModelAnswer, SqlAnswer};
use super::understanding::{build_column_roles, build_understanding};
use crate::engine::guard::{build_guarded_sql, AllowedTable, RowFilter};
use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::MetricDefinition;

use ColumnRole::{Dimension, Measure, Other, Time};

const MAX_ROWS: u64 = 100_000;

const MARKET: &str = "Thị trường nơi giao đơn hàng (Market)";
const ORDER_DATE: &str =
    "ngày giờ đặt hàng, dạng tháng/ngày/năm giờ:phút (order date (DateOrders))";
const DATE_SQL: &str = r#"strptime("order date (DateOrders)", '%m/%d/%Y %H:%M')"#;

#[derive(serde::Deserialize)]
struct SampleColumn {
    name: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct SampleProfile {
    columns: Vec<SampleColumn>,
}

fn mock_entries() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("../engine/llm/mock_answers.json")).unwrap()
}

fn profile() -> Vec<ColumnProfile> {
    let answer = mock_entries()
        .into_iter()
        .find(|e| e["task"] == "profile")
        .expect("dataco profile in mock_answers.json")["answer"]
        .clone();
    let sample: SampleProfile = serde_json::from_value(answer).unwrap();
    sample
        .columns
        .into_iter()
        .map(|c| ColumnProfile {
            id: Uuid::nil(),
            role: if c.name.contains("(DateOrders)") {
                "time"
            } else {
                "dimension"
            }
            .to_string(),
            name: c.name,
            data_type: "VARCHAR".to_string(),
            time_format: None,
            description: Some(c.description),
            aliases: vec![],
            is_sensitive: false,
            values: vec![],
            values_complete: false,
            matched: false,
        })
        .collect()
}

fn metrics() -> Vec<MetricDefinition> {
    vec![MetricDefinition {
        name: "Doanh thu".to_string(),
        description: "tổng Order Item Total".to_string(),
        expression: r#"sum("Order Item Total")"#.to_string(),
        filter: None,
        time_column: None,
        unit: None,
        confirmed: true,
    }]
}

/// Giả định của câu trả lời thành Choice bằng hàm của ask_choices.rs, trên ngữ cảnh chỉ có hồ sơ cột.
fn choices(answer: &SqlAnswer) -> Vec<Choice> {
    let context = AskContext {
        columns: profile(),
        hints: vec![],
        ambiguous: vec![],
        picks: vec![],
    };
    build_assumption_choices(answer, &context, &metrics())
}

fn tables(row_filters: Vec<RowFilter>) -> Vec<AllowedTable> {
    vec![AllowedTable {
        name: "dataco".to_string(),
        columns: profile().into_iter().map(|c| c.name).collect(),
        parquet_dir: PathBuf::from("/data/dataco"),
        row_filters,
    }]
}

struct Card {
    sql_guarded: String,
    understanding: Understanding,
    roles: Vec<ColumnRole>,
}

/// Như endpoint hỏi: guard, đọc hình dạng sql_guarded, dựng thẻ và vai trò. Kiểm luôn mọi đoạn SQL nằm trong câu.
fn build_card(sql: &str, row_filters: Vec<RowFilter>, assumptions: &[Choice]) -> Card {
    let guarded = build_guarded_sql(sql, &tables(row_filters), MAX_ROWS).unwrap();
    let shape = build_query_shape(&guarded.shown).unwrap();
    let profile = profile();
    let understanding = build_understanding(
        &shape,
        guarded.limit_note.is_none(),
        &profile,
        &metrics(),
        &guarded.row_filters,
        assumptions,
    );
    for item in all_items(&understanding) {
        assert!(
            guarded.shown.contains(&item.sql),
            "{:?} is not part of {}",
            item.sql,
            guarded.shown
        );
    }
    Card {
        roles: build_column_roles(&shape, &profile, shape.outputs.len()),
        sql_guarded: guarded.shown,
        understanding,
    }
}

fn all_items(u: &Understanding) -> Vec<&UnderstoodItem> {
    u.measures
        .iter()
        .chain(&u.filters)
        .chain(&u.group_by)
        .chain(&u.order_by)
        .chain(&u.limit)
        .chain(&u.time_range)
        .collect()
}

fn pairs(items: &[UnderstoodItem]) -> Vec<(&str, &str)> {
    items
        .iter()
        .map(|i| (i.label.as_str(), i.sql.as_str()))
        .collect()
}

fn pair(item: &Option<UnderstoodItem>) -> Option<(&str, &str)> {
    item.as_ref().map(|i| (i.label.as_str(), i.sql.as_str()))
}

/// Câu trả lời có SQL của mock (bước sql và bước sửa), kèm câu hỏi.
fn mock_answers() -> Vec<(String, SqlAnswer)> {
    mock_entries()
        .into_iter()
        .filter(|e| e["task"] == "sql" || e["task"] == "repair")
        .map(|e| {
            let question = e["question"].as_str().unwrap().to_string();
            match build_answer_from_reply(&e["answer"].to_string()) {
                Ok(ModelAnswer::Sql(answer)) => (question, answer),
                other => panic!("mock answer for {question} has no sql: {other:?}"),
            }
        })
        .collect()
}

#[test]
fn mock_answers_get_cards_and_roles() {
    // Theo thứ tự trong mock_answers.json; hai câu cuối là bước sql và bước sửa của cùng một câu hỏi.
    let expected_roles: [&[ColumnRole]; 12] = [
        &[Dimension, Measure],
        &[Time, Measure],
        &[Dimension, Measure],
        &[Dimension, Measure],
        &[Dimension, Measure],
        &[Dimension, Measure],
        &[Dimension, Measure],
        &[Dimension, Measure, Measure],
        &[Time, Measure],
        &[Dimension, Measure],
        &[Time, Measure],
        &[Time, Measure],
    ];
    let answers = mock_answers();
    assert_eq!(answers.len(), expected_roles.len());
    for ((question, answer), roles) in answers.iter().zip(expected_roles) {
        let card = build_card(&answer.sql, vec![], &choices(answer));
        assert_eq!(card.roles, roles, "{question}");
        // Mọi câu mẫu đều nhóm và tổng hợp, và guard không lọc gì.
        assert!(!card.understanding.measures.is_empty(), "{question}");
        assert!(!card.understanding.group_by.is_empty(), "{question}");
        assert_eq!(card.understanding.order_by.len(), 1, "{question}");
    }
}

#[test]
fn revenue_by_market_card() {
    let (_, answer) = mock_answers().remove(0);
    let u = build_card(&answer.sql, vec![], &choices(&answer)).understanding;
    assert_eq!(
        pairs(&u.measures),
        [(
            "Doanh thu: tổng Order Item Total",
            r#"SUM("Order Item Total")"#
        )]
    );
    assert_eq!(pairs(&u.group_by), [(MARKET, r#""Market""#)]);
    assert_eq!(
        pairs(&u.order_by),
        [("Doanh thu, giảm dần", "revenue DESC")]
    );
    // LIMIT do guard thêm, không phải của câu hỏi.
    assert!(u.limit.is_none());
    assert!(u.time_range.is_none());
    assert!(u.filters.is_empty());
    assert_eq!(u.assumptions.len(), 1);
    assert_eq!(u.assumptions[0].id, TIME_CHOICE_ID);
    assert_eq!(u.assumptions[0].selected.as_deref(), Some("all"));
}

#[test]
fn group_key_by_alias_shows_the_expression() {
    let sql = mock_answers().remove(1).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    let month = format!("date_trunc('month', {DATE_SQL})");
    let label = format!("Tháng của {ORDER_DATE}");
    assert_eq!(pairs(&u.group_by), [(label.as_str(), month.as_str())]);
    assert_eq!(
        u.order_by[0].label,
        format!("Tháng của {ORDER_DATE}, tăng dần")
    );
}

#[test]
fn limit_of_the_question_is_kept() {
    let sql = mock_answers().remove(2).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    assert_eq!(pair(&u.limit), Some(("10 dòng đầu", "LIMIT 10")));
}

#[test]
fn limit_lowered_by_the_guard_is_not_the_question() {
    let sql = r#"SELECT "Market", SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market" LIMIT 500000"#;
    let card = build_card(sql, vec![], &[]);
    assert!(card.sql_guarded.contains("LIMIT 100000"));
    assert!(card.understanding.limit.is_none());
}

#[test]
fn measures_without_metric_use_aggregate_words() {
    let sql = mock_answers().remove(4).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    assert_eq!(
        u.measures[0].label,
        "Số mã đơn hàng; một đơn có thể có nhiều dòng (Order Id) khác nhau"
    );
    let sql = mock_answers().remove(3).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    // Tỷ lệ không theo khuôn nào: nhãn là tên cột kết quả, đoạn SQL cho thấy cách tính.
    assert_eq!(u.measures[0].label, "late_rate");
}

#[test]
fn year_filter_becomes_time_range() {
    let sql = mock_answers().remove(8).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    let label = format!("Năm của {ORDER_DATE} là 2017");
    let condition = format!("year({DATE_SQL}) = 2017");
    assert_eq!(
        pair(&u.time_range),
        Some((label.as_str(), condition.as_str()))
    );
    assert!(u.filters.is_empty());
}

#[test]
fn value_filter_is_labelled_with_the_column() {
    let sql = mock_answers().remove(9).1.sql;
    let u = build_card(&sql, vec![], &[]).understanding;
    let label = format!("{MARKET} là Europe");
    assert_eq!(
        pairs(&u.filters),
        [(label.as_str(), r#""Market" = 'Europe'"#)]
    );
    assert_eq!(pair(&u.limit), Some(("5 dòng đầu", "LIMIT 5")));
}

#[test]
fn row_filters_of_the_guard_are_labelled_as_permission() {
    let sql = mock_answers().remove(9).1.sql;
    let policy = vec![RowFilter {
        column: "Market".to_string(),
        allowed_values: vec!["Europe".to_string(), "LATAM".to_string()],
    }];
    let card = build_card(&sql, policy, &[]);
    let question = format!("{MARKET} là Europe");
    let permission =
        "Phân quyền: chỉ thấy thị trường nơi giao đơn hàng (Market) là Europe, LATAM".to_string();
    assert_eq!(
        pairs(&card.understanding.filters),
        [
            (question.as_str(), r#""Market" = 'Europe'"#),
            (permission.as_str(), r#""Market" IN ('Europe', 'LATAM')"#),
        ]
    );
    assert_eq!(card.roles, [Dimension, Measure]);
}

#[test]
fn row_filter_hiding_everything_says_so() {
    let sql = mock_answers().remove(0).1.sql;
    let policy = vec![RowFilter {
        column: "Market".to_string(),
        allowed_values: vec![],
    }];
    let u = build_card(&sql, policy, &[]).understanding;
    assert_eq!(
        pairs(&u.filters),
        [("Phân quyền: không được thấy dòng nào", "false")]
    );
}

#[test]
fn time_range_from_bounds_and_between() {
    let sql = format!(
        r#"SELECT SUM("Order Item Total") AS revenue FROM dataco WHERE {DATE_SQL} >= TIMESTAMP '2017-07-01' AND {DATE_SQL} < TIMESTAMP '2017-10-01' AND "Market" = 'Europe'"#
    );
    let u = build_card(&sql, vec![], &[]).understanding;
    let range = pair(&u.time_range).unwrap();
    assert_eq!(
        range.0,
        "Ngày giờ đặt hàng, dạng tháng/ngày/năm giờ:phút (order date (DateOrders)) từ 2017-07-01 đến trước \
         2017-10-01"
    );
    assert_eq!(
        range.1,
        format!("{DATE_SQL} >= TIMESTAMP '2017-07-01' AND {DATE_SQL} < TIMESTAMP '2017-10-01'")
    );
    assert_eq!(u.filters.len(), 1);

    let sql = format!(
        r#"SELECT SUM("Order Item Total") AS revenue FROM dataco WHERE {DATE_SQL} BETWEEN DATE '2017-01-01' AND DATE '2017-03-31'"#
    );
    let u = build_card(&sql, vec![], &[]).understanding;
    assert!(u
        .time_range
        .unwrap()
        .label
        .ends_with("từ 2017-01-01 đến 2017-03-31"));
    assert!(u.filters.is_empty());
}

#[test]
fn only_the_first_run_of_time_bounds_is_the_range() {
    let sql = format!(
        r#"SELECT SUM("Order Item Total") AS revenue FROM dataco WHERE {DATE_SQL} >= TIMESTAMP '2017-07-01' AND "Market" = 'Europe' AND {DATE_SQL} < TIMESTAMP '2017-10-01'"#
    );
    let u = build_card(&sql, vec![], &[]).understanding;
    assert_eq!(
        u.time_range.unwrap().sql,
        format!("{DATE_SQL} >= TIMESTAMP '2017-07-01'")
    );
    let filters: Vec<&str> = u.filters.iter().map(|f| f.sql.as_str()).collect();
    assert_eq!(
        filters,
        [
            r#""Market" = 'Europe'"#.to_string(),
            format!("{DATE_SQL} < TIMESTAMP '2017-10-01'")
        ]
    );
}

#[test]
fn or_filter_is_one_item_labelled_by_its_sql() {
    let sql = r#"SELECT SUM("Order Item Total") AS revenue FROM dataco WHERE "Market" = 'Europe' OR "Market" = 'LATAM'"#;
    let u = build_card(sql, vec![], &[]).understanding;
    let condition = r#""Market" = 'Europe' OR "Market" = 'LATAM'"#;
    assert_eq!(pairs(&u.filters), [(condition, condition)]);
}

#[test]
fn cte_reads_the_outer_query() {
    let sql = r#"WITH t AS (SELECT "Market", "Order Item Total" FROM dataco WHERE "Market" <> 'Africa') SELECT "Market", SUM("Order Item Total") AS revenue FROM t GROUP BY "Market""#;
    let card = build_card(sql, vec![], &[]);
    assert_eq!(card.roles, [Dimension, Measure]);
    assert_eq!(
        card.understanding.measures[0].label,
        "Doanh thu: tổng Order Item Total"
    );
    // Vế lọc trong CTE không phải của câu ngoài cùng.
    assert!(card.understanding.filters.is_empty());
}

#[test]
fn union_gives_order_and_limit_only() {
    let sql = r#"SELECT "Market" AS place FROM dataco UNION SELECT "Order Region" FROM dataco ORDER BY 1 LIMIT 20"#;
    let card = build_card(sql, vec![], &[]);
    assert_eq!(card.roles, [Other]);
    let u = &card.understanding;
    assert!(u.measures.is_empty() && u.group_by.is_empty() && u.filters.is_empty());
    assert_eq!(
        pairs(&u.order_by),
        [("Thị trường nơi giao đơn hàng (Market), tăng dần", "1")]
    );
    assert_eq!(pair(&u.limit), Some(("20 dòng đầu", "LIMIT 20")));
}

#[test]
fn order_by_position_names_the_column() {
    let sql = r#"SELECT "Market", SUM("Order Item Total") FROM dataco GROUP BY 1 ORDER BY 2 DESC"#;
    let card = build_card(sql, vec![], &[]);
    assert_eq!(card.roles, [Dimension, Measure]);
    assert_eq!(
        pairs(&card.understanding.order_by),
        [("Doanh thu, giảm dần", "2 DESC")]
    );
    assert_eq!(
        pairs(&card.understanding.group_by),
        [(MARKET, r#""Market""#)]
    );
}

#[test]
fn select_star_gives_other_roles() {
    let sql = r#"SELECT * FROM dataco WHERE "Market" = 'Europe'"#;
    let guarded = build_guarded_sql(sql, &tables(vec![]), MAX_ROWS).unwrap();
    let shape = build_query_shape(&guarded.shown).unwrap();
    let column_count = profile().len();
    let roles = build_column_roles(&shape, &profile(), column_count);
    assert_eq!(roles, vec![Other; column_count]);
    let u = build_card(sql, vec![], &[]).understanding;
    assert!(u.measures.is_empty());
    assert_eq!(u.filters.len(), 1);
}

#[test]
fn no_group_by_and_no_aggregate_gives_other_roles() {
    let sql = r#"SELECT "Market", "Order Item Total" FROM dataco ORDER BY "Order Item Total" DESC LIMIT 3"#;
    let card = build_card(sql, vec![], &[]);
    assert_eq!(card.roles, [Other, Other]);
    assert!(card.understanding.measures.is_empty());
}

#[test]
fn card_keeps_the_choices_of_ask_choices() {
    let (_, mut answer) = mock_answers().remove(9);
    answer.assumptions = vec![Assumption {
        kind: ChoiceKind::Column,
        phrase: Some("Khu vực".to_string()),
        chosen: "Order Region".to_string(),
        alternatives: vec!["Market".to_string(), "Order Region".to_string()],
        label: "Khu vực hiểu là Order Region".to_string(),
    }];
    let choices = choices(&answer);
    let u = build_card(&answer.sql, vec![], &choices).understanding;
    let ids: Vec<&str> = u.assumptions.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [build_choice_id(ChoiceKind::Column, Some("Khu vực")).as_str()]
    );
    let expected: Vec<&str> = choices.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, expected);
}
