// SPDX-License-Identifier: Apache-2.0
/* service/ask/checks_tests.rs - test của kiểm hợp lý kết quả (ask/checks.rs, ask/filter_values.rs, ask/magnitude.rs):
mỗi luật một test bắt đúng và một test không bắt nhầm. Hàm thuần nên không cần Postgres: hình dạng câu đọc từ SQL viết
sẵn, danh sách giá trị dựng bằng ValueIndex::build. Test qua route hỏi ở api/ask_tests.rs. */

use uuid::Uuid;

use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::{ProfileColumn, ValueRow};
use crate::service::ask::checks::build_warnings;
use crate::service::ask::contract::{
    CellValue, ColumnRole, ResultColumn, ResultTable, ValueKind, WarningCode,
};
use crate::service::ask::filter_values::{
    build_filter_values, build_missing_values, FilterValue, MissingValue,
};
use crate::service::ask::reply::{Term, TermKind};
use crate::service::ask::values::ValueIndex;
use crate::service::explain::check_tests::column;

fn profile() -> Vec<ProfileColumn> {
    let market = column(1, "Market", "VARCHAR", "dimension", 3);
    let mut sales = column(2, "Sales", "DOUBLE", "measure", 100);
    sales.min_value = Some("10".to_string());
    sales.max_value = Some("500".to_string());
    let mut late = column(3, "Late_delivery_risk", "INTEGER", "dimension", 2);
    late.min_value = Some("0".to_string());
    late.max_value = Some("1".to_string());
    let mut profit = column(4, "Profit", "DOUBLE", "measure", 100);
    profit.min_value = Some("-50".to_string());
    profit.max_value = Some("80".to_string());
    let mut email = column(5, "Customer Email", "VARCHAR", "dimension", 3);
    email.is_sensitive = true;
    vec![market, sales, late, profit, email]
}

fn index() -> ValueIndex {
    let row = |value: &str, aliases: &[&str]| ValueRow {
        column_id: Uuid::from_u128(1),
        column_name: "Market".to_string(),
        value: value.to_string(),
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
    };
    ValueIndex::build(&[
        row("Europe", &["châu Âu"]),
        row("LATAM", &[]),
        row("Pacific Asia", &[]),
    ])
}

fn number(value: f64) -> Option<CellValue> {
    Some(CellValue::Number(value))
}

/// Bảng kết quả: tên cột, rồi các dòng; total_rows bằng số dòng trừ khi truncated.
fn table(names: &[&str], rows: Vec<Vec<Option<CellValue>>>, truncated: bool) -> ResultTable {
    let total_rows = if truncated { 5000 } else { rows.len() as u64 };
    ResultTable {
        columns: names
            .iter()
            .map(|name| ResultColumn {
                name: name.to_string(),
                kind: ValueKind::Number,
                role: ColumnRole::Other,
            })
            .collect(),
        rows,
        total_rows,
        truncated,
        limit_reached: false,
    }
}

fn codes(sql: &str, result: &ResultTable) -> Vec<WarningCode> {
    let shape = build_query_shape(sql).unwrap();
    build_warnings(Some(result), Some(&shape), &profile(), &[])
        .iter()
        .map(|w| w.code)
        .collect()
}

fn missing(sql: &str, terms: &[Term]) -> Vec<MissingValue> {
    let shape = build_query_shape(sql).unwrap();
    let wanted = build_filter_values(Some(&shape), &[], terms, &profile());
    build_missing_values(&wanted, &index())
}

#[test]
fn empty_result_is_flagged_and_rows_are_not() {
    let sql = r#"SELECT "Market" FROM dataco"#;
    assert_eq!(
        codes(sql, &table(&["Market"], vec![], false)),
        [WarningCode::EmptyResult]
    );
    let one = vec![Some(CellValue::String("Europe".to_string()))];
    assert!(codes(sql, &table(&["Market"], vec![one], false)).is_empty());
}

#[test]
fn a_column_of_nulls_is_flagged_and_some_nulls_are_not() {
    let sql = r#"SELECT "Market", SUM("Sales") AS s FROM dataco GROUP BY 1"#;
    let text = |t: &str| Some(CellValue::String(t.to_string()));
    let all_null = table(
        &["Market", "s"],
        vec![vec![text("Europe"), None], vec![text("LATAM"), None]],
        false,
    );
    let shape = build_query_shape(sql).unwrap();
    let warnings = build_warnings(Some(&all_null), Some(&shape), &profile(), &[]);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, WarningCode::AllNull);
    assert!(
        warnings[0].message.starts_with("Cột s chỉ có ô trống"),
        "{}",
        warnings[0].message
    );

    let some_null = table(
        &["Market", "s"],
        vec![
            vec![text("Europe"), None],
            vec![text("LATAM"), number(20.0)],
        ],
        false,
    );
    assert!(codes(sql, &some_null).is_empty());
}

/// Câu lọc theo cột thời gian mà ra ô trống hay không dòng nào: lời kèm khoảng thời gian dữ liệu có, lấy từ min, max của
/// hồ sơ cột. Câu không lọc theo thời gian thì không kèm.
#[test]
fn empty_answer_names_the_time_range_of_the_data() {
    let mut date = column(6, "order date (DateOrders)", "VARCHAR", "time", 1000);
    date.min_value = Some("2015-01-01 00:00:00".to_string());
    date.max_value = Some("2018-01-31 23:38:00".to_string());
    let mut profile = profile();
    profile.push(date);
    let note =
        "Trong dữ liệu, cột order date (DateOrders) có dữ liệu từ 01/01/2015 tới 31/01/2018.";

    let year = r#"SELECT sum("Sales") FROM dataco WHERE strptime("order date (DateOrders)", '%m/%d/%Y %H:%M') >= TIMESTAMP '2019-01-01'"#;
    let shape = build_query_shape(year).unwrap();
    let null_sum = table(&["sum(\"Sales\")"], vec![vec![None]], false);
    let warnings = build_warnings(Some(&null_sum), Some(&shape), &profile, &[]);
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].message.ends_with(note),
        "{}",
        warnings[0].message
    );
    let empty = table(&["Market"], vec![], false);
    let warnings = build_warnings(Some(&empty), Some(&shape), &profile, &[]);
    assert_eq!(warnings[0].code, WarningCode::EmptyResult);
    assert!(
        warnings[0].message.ends_with(note),
        "{}",
        warnings[0].message
    );

    let market = r#"SELECT sum("Sales") FROM dataco WHERE "Market" = 'Asia'"#;
    let shape = build_query_shape(market).unwrap();
    let warnings = build_warnings(Some(&null_sum), Some(&shape), &profile, &[]);
    assert!(
        !warnings[0].message.contains("Trong dữ liệu"),
        "{}",
        warnings[0].message
    );
}

/// Lời kèm khoảng thời gian chỉ khi câu lọc đúng một cột thời gian bằng tên đầy đủ, ngoài hộp đọc bảng: cột ship_date
/// chứa chữ date nhưng không phải cột date; cột chiều có min, max vẫn không phải cột thời gian; vế trong hộp đọc bảng
/// có thể là phân quyền guard thêm.
#[test]
fn the_range_note_needs_a_time_column_the_question_filters() {
    let mut date = column(6, "date", "DATE", "time", 1000);
    date.min_value = Some("2015-01-01".to_string());
    date.max_value = Some("2018-01-31".to_string());
    let mut region = column(7, "region", "VARCHAR", "dimension", 4);
    region.min_value = Some("East".to_string());
    region.max_value = Some("West".to_string());
    let mut profile = profile();
    profile.extend([date, region]);
    let null_sum = table(&["s"], vec![vec![None]], false);
    for sql in [
        "SELECT sum(\"Sales\") AS s FROM dataco WHERE ship_date >= DATE '2019-01-01'",
        "SELECT sum(\"Sales\") AS s FROM dataco WHERE region = 'North'",
        "SELECT sum(\"Sales\") AS s FROM (SELECT * FROM dataco WHERE \"date\" >= DATE '2019-01-01') AS dataco",
    ] {
        let shape = build_query_shape(sql).unwrap();
        let warnings = build_warnings(Some(&null_sum), Some(&shape), &profile, &[]);
        assert!(
            !warnings[0].message.contains("Trong dữ liệu"),
            "{sql}: {}",
            warnings[0].message
        );
    }
    // Cùng cột date, lọc ngoài hộp đọc bảng: có lời.
    let shape = build_query_shape(
        "SELECT sum(\"Sales\") AS s FROM dataco WHERE \"date\" >= DATE '2019-01-01'",
    )
    .unwrap();
    let warnings = build_warnings(Some(&null_sum), Some(&shape), &profile, &[]);
    assert!(
        warnings[0]
            .message
            .ends_with("Trong dữ liệu, cột date có dữ liệu từ 01/01/2015 tới 31/01/2018."),
        "{}",
        warnings[0].message
    );
}

#[test]
fn nulls_in_a_cut_result_name_the_rows_checked() {
    let result = table(&["s"], vec![vec![None]; 1000], true);
    let warnings = build_warnings(Some(&result), None, &profile(), &[]);
    assert_eq!(warnings[0].code, WarningCode::AllNull);
    assert!(
        warnings[0]
            .message
            .starts_with("Trong 1.000 dòng đầu, cột s"),
        "{}",
        warnings[0].message
    );
}

#[test]
fn a_value_in_another_case_suggests_the_right_one() {
    let found = missing(r#"SELECT * FROM dataco WHERE "Market" = 'europe'"#, &[]);
    assert_eq!(
        found,
        [MissingValue {
            column: "Market".to_string(),
            value: "europe".to_string(),
            suggestions: vec!["Europe".to_string()],
        }]
    );
    let warnings = build_warnings(None, None, &[], &found);
    assert_eq!(warnings[0].code, WarningCode::FilterValueNotFound);
    assert_eq!(
        warnings[0].message,
        "Cột Market không có giá trị \"europe\". Giá trị gần nhất: Europe. Kiểm lại giá trị lọc: kết quả có thể \
         rỗng hoặc thiếu dòng."
    );
}

#[test]
fn a_misspelt_value_suggests_the_nearest_and_an_unknown_one_suggests_nothing() {
    let found = missing(
        r#"SELECT * FROM dataco WHERE "Market" IN ('Europa', 'Mars', 'LATAM')"#,
        &[],
    );
    let pairs: Vec<(&str, Vec<String>)> = found
        .iter()
        .map(|m| (m.value.as_str(), m.suggestions.clone()))
        .collect();
    assert_eq!(
        pairs,
        [("Europa", vec!["Europe".to_string()]), ("Mars", vec![])]
    );
}

#[test]
fn values_that_exist_or_columns_without_a_list_are_not_flagged() {
    // Giá trị có thật, vế phủ định, cột số đo, cột nhạy cảm (không có danh sách), câu đọc bảng tạm.
    assert!(missing(r#"SELECT * FROM dataco WHERE "Market" = 'Europe'"#, &[]).is_empty());
    assert!(missing(r#"SELECT * FROM dataco WHERE "Market" <> 'Mars'"#, &[]).is_empty());
    assert!(missing(r#"SELECT * FROM dataco WHERE "Sales" = '12'"#, &[]).is_empty());
    assert!(missing(
        r#"SELECT * FROM dataco WHERE "Customer Email" = 'x@y'"#,
        &[]
    )
    .is_empty());
    let cte = r#"WITH t AS (SELECT 'Mars' AS "Market") SELECT * FROM t WHERE "Market" = 'Mars'"#;
    assert!(missing(cte, &[]).is_empty());
}

#[test]
fn row_filters_of_the_data_role_are_not_checked() {
    let sql =
        r#"SELECT * FROM (SELECT * FROM dataco WHERE "Market" = 'Mars') WHERE "Market" = 'Europe'"#;
    let shape = build_query_shape(sql).unwrap();
    let row_filters = [r#""Market" = 'Mars'"#.to_string()];
    let wanted = build_filter_values(Some(&shape), &row_filters, &[], &profile());
    assert_eq!(
        wanted,
        [FilterValue {
            column: "Market".to_string(),
            value: "Europe".to_string(),
        }]
    );
}

#[test]
fn values_named_by_the_model_are_checked_once() {
    let term = Term {
        text: "châu Á".to_string(),
        kind: TermKind::Value,
        column: Some("Market".to_string()),
        value: Some("Asia".to_string()),
    };
    let found = missing(r#"SELECT * FROM dataco WHERE "Market" = 'Asia'"#, &[term]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].value, "Asia");
}

#[test]
fn an_empty_result_with_a_wrong_value_gives_two_warnings_in_rule_order() {
    let sql = r#"SELECT "Market" FROM dataco WHERE "Market" = 'Europa'"#;
    let shape = build_query_shape(sql).unwrap();
    let found = missing(sql, &[]);
    let result = table(&["Market"], vec![], false);
    let codes: Vec<WarningCode> = build_warnings(Some(&result), Some(&shape), &profile(), &found)
        .iter()
        .map(|w| w.code)
        .collect();
    assert_eq!(
        codes,
        [WarningCode::EmptyResult, WarningCode::FilterValueNotFound]
    );
}

#[test]
fn a_ratio_of_two_counts_above_one_is_flagged_and_below_one_is_not() {
    let sql = r#"SELECT "Market", ROUND(COUNT_IF("Late_delivery_risk" = 1) * 1.0 / COUNT(*), 2) AS late_rate
                 FROM dataco GROUP BY "Market""#;
    let rows = |rate: f64| {
        vec![vec![
            Some(CellValue::String("Europe".to_string())),
            number(rate),
        ]]
    };
    let flagged = table(&["Market", "late_rate"], rows(1.5), false);
    let shape = build_query_shape(sql).unwrap();
    let warnings = build_warnings(Some(&flagged), Some(&shape), &profile(), &[]);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, WarningCode::UnusualMagnitude);
    assert!(
        warnings[0].message.contains("1,5"),
        "{}",
        warnings[0].message
    );
    assert!(codes(sql, &table(&["Market", "late_rate"], rows(0.4), false)).is_empty());
    // Phần trăm (nhân 100) không phải tỷ lệ trong [0, 1].
    let percent =
        r#"SELECT 100.0 * COUNT_IF("Late_delivery_risk" = 1) / COUNT(*) AS p FROM dataco"#;
    assert!(codes(percent, &table(&["p"], vec![vec![number(55.0)]], false)).is_empty());
}

#[test]
fn the_average_of_a_zero_one_column_is_a_ratio() {
    let sql = r#"SELECT AVG("Late_delivery_risk") AS r FROM dataco"#;
    assert_eq!(
        codes(sql, &table(&["r"], vec![vec![number(1.2)]], false)),
        [WarningCode::UnusualMagnitude]
    );
    assert!(codes(sql, &table(&["r"], vec![vec![number(0.55)]], false)).is_empty());
}

#[test]
fn avg_min_max_outside_the_profile_range_are_flagged() {
    let sql = r#"SELECT AVG("Sales") AS a, MAX("Sales") AS m FROM dataco"#;
    let result = table(&["a", "m"], vec![vec![number(600.0), number(500.0)]], false);
    let shape = build_query_shape(sql).unwrap();
    let warnings = build_warnings(Some(&result), Some(&shape), &profile(), &[]);
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0]
            .message
            .starts_with("Cột a có giá trị 600, ngoài khoảng 10 đến 500 của cột Sales"),
        "{}",
        warnings[0].message
    );
    let inside = table(&["a", "m"], vec![vec![number(120.5), number(500.0)]], false);
    assert!(codes(sql, &inside).is_empty());
}

#[test]
fn a_negative_sum_of_a_column_without_negatives_is_flagged() {
    let sql = r#"SELECT SUM("Sales") AS s, SUM("Profit") AS p FROM dataco"#;
    let result = table(&["s", "p"], vec![vec![number(-3.0), number(-40.0)]], false);
    let shape = build_query_shape(sql).unwrap();
    let warnings = build_warnings(Some(&result), Some(&shape), &profile(), &[]);
    // Profit có giá trị âm trong hồ sơ, tổng âm là bình thường.
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0]
            .message
            .starts_with("Cột s là tổng của cột Sales"),
        "{}",
        warnings[0].message
    );
    let positive = table(&["s", "p"], vec![vec![number(3.0), number(-40.0)]], false);
    assert!(codes(sql, &positive).is_empty());
}

#[test]
fn magnitude_is_not_checked_when_columns_cannot_be_matched() {
    // `*` và bảng tạm: không biết chắc cột kết quả hay cột trong biểu thức là cột nào của hồ sơ.
    let star = r#"SELECT *, SUM("Sales") AS s FROM dataco GROUP BY ALL"#;
    let cte =
        r#"WITH t AS (SELECT -"Sales" AS "Sales" FROM dataco) SELECT SUM("Sales") AS s FROM t"#;
    let result = table(&["s"], vec![vec![number(-3.0)]], false);
    assert!(codes(star, &result).is_empty());
    assert!(codes(cte, &result).is_empty());
}
