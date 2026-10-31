// SPDX-License-Identifier: Apache-2.0
/* service/ask/data_range_tests.rs - test khoảng dữ liệu của câu trả lời (ask/data_range.rs): chọn cột thời gian có tên
trong câu SQL, không có thì cột đầu; bỏ cột nhạy cảm; báo kỳ hỏi chưa đủ dữ liệu khi câu lọc tới quá ngày cuối, không báo
khi mốc trên loại trừ đúng ngày sau ngày cuối. */

use uuid::Uuid;

use crate::engine::store::ProfileColumn;
use crate::service::ask::data_range::build_data_range;

fn time_column(name: &str, from: &str, to: &str, sensitive: bool) -> ProfileColumn {
    ProfileColumn {
        id: Uuid::nil(),
        position: 0,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: "time".to_string(),
        description: None,
        description_source: None,
        aliases: vec!["Ngày đặt hàng".to_string()],
        is_sensitive: sensitive,
        null_count: None,
        distinct_count: None,
        min_value: Some(format!("{from} 00:00:00")),
        max_value: Some(format!("{to} 23:38:00")),
        time_format: Some("%m/%d/%Y %H:%M".to_string()),
        unit: None,
        aggregation: "sum".to_string(),
    }
}

const ORDER: &str = "order date (DateOrders)";
const SHIP: &str = "shipping date (DateOrders)";

#[test]
fn column_named_in_the_sql_is_used() {
    let columns = [
        time_column(SHIP, "2015-01-03", "2018-02-06", false),
        time_column(ORDER, "2015-01-01", "2018-01-31", false),
    ];
    let sql = format!(r#"SELECT SUM("Sales") FROM t WHERE "{ORDER}" IS NOT NULL"#);
    let range = build_data_range(&columns, Some(&sql)).unwrap();
    assert_eq!(range.column, ORDER);
    assert_eq!(
        (range.from.as_str(), range.to.as_str()),
        ("2015-01-01", "2018-01-31")
    );
    assert_eq!(range.label.as_deref(), Some("Ngày đặt hàng"));
    assert!(!range.partial);
    // Câu không nêu cột thời gian nào: cột thời gian đầu tiên.
    let range = build_data_range(&columns, Some(r#"SELECT SUM("Sales") FROM t"#)).unwrap();
    assert_eq!(range.column, SHIP);
}

#[test]
fn sensitive_or_missing_time_columns_give_nothing() {
    let columns = [time_column(ORDER, "2015-01-01", "2018-01-31", true)];
    assert!(build_data_range(&columns, None).is_none());
    assert!(build_data_range(&[], None).is_none());
}

/// Mốc trên loại trừ ngày ngay sau ngày cuối (cả tháng 1/2018) là đủ dữ liệu; mốc xa hơn (tháng 6/2027) là chưa đủ.
#[test]
fn period_after_the_last_day_is_partial() {
    let columns = [time_column(ORDER, "2015-01-01", "2018-01-31", false)];
    let january = format!(
        "SELECT 1 FROM t WHERE strptime(\"{ORDER}\", '%m/%d/%Y %H:%M') >= TIMESTAMP '2018-01-01' AND \
         strptime(\"{ORDER}\", '%m/%d/%Y %H:%M') < TIMESTAMP '2018-02-01'"
    );
    assert!(!build_data_range(&columns, Some(&january)).unwrap().partial);
    let later = january
        .replace("2018-01-01", "2027-06-01")
        .replace("2018-02-01", "2027-07-01");
    assert!(build_data_range(&columns, Some(&later)).unwrap().partial);
}
