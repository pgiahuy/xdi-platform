// SPDX-License-Identifier: Apache-2.0
/* service/overview/tests.rs - test phần thuần của trang tổng quan, không cần Postgres hay DuckDB: chọn độ đo chính và cột
thời gian, câu SQL dựng ra, chọn theo ngày hay theo tháng, hồ sơ cột với cột nhạy cảm và người bị lọc dòng, biểu đồ số
dòng theo giá trị. Trọn đường qua route (guard, lọc theo vai, cache) ở api/overview_tests.rs. */

use uuid::Uuid;

use crate::engine::store::{ColumnValue, MetricDefinition, ProfileColumn};
use crate::service::ask::context::build_profiles;
use crate::service::ask::contract::{ChartKind, ColumnRole};
use crate::service::explain::sql::build_time_expr;
use crate::service::overview::charts::build_value_chart;
use crate::service::overview::columns::{build_columns, values_of};
use crate::service::overview::contract::{OverviewRole, TimeGrain};
use crate::service::overview::sql::{
    build_grain, build_measure_over_time_sql, build_rows_over_time_sql, build_summary_sql,
    choose_measure, choose_time_column,
};

fn column(name: &str, role: &str) -> ProfileColumn {
    ProfileColumn {
        id: Uuid::new_v4(),
        position: 0,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        null_count: Some(0),
        distinct_count: Some(3),
        min_value: None,
        max_value: None,
        time_format: None,
        unit: None,
        origin_table: None,
        aggregation: "sum".to_string(),
    }
}

fn date_column(name: &str) -> ProfileColumn {
    ProfileColumn {
        data_type: "DATE".to_string(),
        ..column(name, "time")
    }
}

fn metric(name: &str, confirmed: bool) -> MetricDefinition {
    MetricDefinition {
        name: name.to_string(),
        description: String::new(),
        expression: "SUM(\"amount\")".to_string(),
        filter: Some("\"status\" <> 'CANCELED'".to_string()),
        time_column: Some("shipped".to_string()),
        unit: None,
        confirmed,
    }
}

fn value(column: &ProfileColumn, value: &str, frequency: i64) -> ColumnValue {
    ColumnValue {
        column_id: column.id,
        value: value.to_string(),
        aliases: Vec::new(),
        frequency: Some(frequency),
    }
}

#[test]
fn a_confirmed_metric_is_the_main_measure_before_any_column() {
    let money = ProfileColumn {
        unit: Some("money".to_string()),
        ..column("amount", "measure")
    };
    let metrics = [metric("Bản nháp", false), metric("Doanh thu", true)];
    let plan = choose_measure(&metrics, std::slice::from_ref(&money)).unwrap();
    assert_eq!(plan.main.metric.as_deref(), Some("Doanh thu"));
    assert_eq!(plan.filter.as_deref(), Some("\"status\" <> 'CANCELED'"));

    // Chỉ có bản nháp: không chọn chỉ số, lấy cột tiền và nhãn là bí danh đầu của cột.
    let labelled = ProfileColumn {
        aliases: vec!["Số tiền".to_string()],
        ..money
    };
    let plan = choose_measure(&metrics[..1], &[labelled]).unwrap();
    assert_eq!(plan.main.label, "Số tiền");
    assert_eq!(plan.main.column.as_deref(), Some("amount"));
    assert_eq!(plan.expression, "SUM(\"amount\")");
}

#[test]
fn no_main_measure_without_a_metric_or_a_money_or_quantity_column() {
    let unknown = column("score", "measure");
    let secret = ProfileColumn {
        unit: Some("money".to_string()),
        is_sensitive: true,
        ..column("salary", "measure")
    };
    assert!(choose_measure(&[], &[unknown, secret]).is_none());
}

#[test]
fn the_metric_time_column_wins_over_the_first_time_column() {
    let columns = [date_column("ordered"), date_column("shipped")];
    let plan = choose_measure(&[metric("Doanh thu", true)], &[]).unwrap();
    let chosen = choose_time_column(&columns, Some(&plan)).unwrap();
    assert_eq!(chosen.name, "shipped");
    assert_eq!(choose_time_column(&columns, None).unwrap().name, "ordered");
    // Cột chữ không có time_format không đọc được thành ngày nên không được chọn.
    assert!(choose_time_column(&[column("note", "time")], None).is_none());
}

#[test]
fn builds_the_summary_and_period_queries() {
    let time = build_time_expr(&date_column("ordered")).unwrap();
    assert_eq!(
        build_summary_sql("ban_hang", Some(&time)),
        "SELECT COUNT(*) AS row_count, strftime(MIN(\"ordered\"), '%Y-%m-%d') AS time_start, \
         strftime(MAX(\"ordered\"), '%Y-%m-%d') AS time_end FROM \"ban_hang\""
    );
    assert_eq!(
        build_summary_sql("ban_hang", None),
        "SELECT COUNT(*) AS row_count FROM \"ban_hang\""
    );
    assert_eq!(
        build_rows_over_time_sql("ban_hang", &time, TimeGrain::Month),
        "SELECT date_trunc('month', \"ordered\") AS \"Tháng\", COUNT(*) AS \"Số dòng\" FROM \"ban_hang\" \
         WHERE \"ordered\" IS NOT NULL GROUP BY 1 ORDER BY 1"
    );
    let plan = choose_measure(&[metric("Doanh thu", true)], &[]).unwrap();
    assert_eq!(
        build_measure_over_time_sql("ban_hang", &time, TimeGrain::Day, &plan),
        "SELECT date_trunc('day', \"ordered\") AS \"Ngày\", SUM(\"amount\") AS \"Doanh thu\" FROM \"ban_hang\" \
         WHERE \"ordered\" IS NOT NULL AND (\"status\" <> 'CANCELED') GROUP BY 1 ORDER BY 1"
    );
}

#[test]
fn short_spans_are_drawn_by_day_longer_ones_by_month() {
    assert_eq!(build_grain("2024-01-01", "2024-04-02"), TimeGrain::Day);
    assert_eq!(build_grain("2024-01-01", "2024-04-03"), TimeGrain::Month);
    assert_eq!(build_grain("2015-01-01", "2018-01-31"), TimeGrain::Month);
    assert_eq!(build_grain("not a date", "2024-01-01"), TimeGrain::Month);
}

#[test]
fn columns_carry_labels_and_counts_but_sensitive_ones_no_values() {
    let market = ProfileColumn {
        aliases: vec!["Thị trường".to_string()],
        description: Some("Khu vực bán".to_string()),
        null_count: Some(1),
        ..column("Market", "dimension")
    };
    let email = ProfileColumn {
        is_sensitive: true,
        ..column("Email", "dimension")
    };
    let ignored = column("_source_file", "ignore");
    let values = [
        value(&market, "Europe", 3),
        value(&market, "LATAM", 1),
        value(&email, "a@example.com", 4),
    ];
    let columns = [market, email, ignored];
    let built = build_columns(&columns, &values, Some(4), false);
    assert_eq!(built.len(), 2, "ignore columns are left out");
    assert_eq!(built[0].label.as_deref(), Some("Thị trường"));
    assert_eq!(built[0].role, OverviewRole::Dimension);
    assert_eq!(built[0].null_percent, Some(25.0));
    assert_eq!(built[0].values.len(), 2);
    assert_eq!(built[0].values[0].rows, 3);
    assert!(built[1].sensitive);
    assert!(built[1].values.is_empty());

    // Người bị lọc dòng: không số đếm, không giá trị nào của hồ sơ.
    let restricted = build_columns(&columns, &values, Some(4), true);
    assert!(restricted[0].values.is_empty());
    assert_eq!(restricted[0].null_percent, None);
    assert_eq!(restricted[0].distinct_count, None);
}

#[test]
fn value_chart_is_a_bar_with_an_other_bucket_past_fifteen_groups() {
    let market = column("Market", "dimension");
    let many: Vec<ColumnValue> = (0..20)
        .map(|i| value(&market, &format!("m{i}"), 100 - i))
        .collect();
    let profile = build_profiles(vec![market.clone()]);
    let chart =
        build_value_chart("ban_hang", &market, &values_of(&market, &many), &profile).unwrap();
    assert_eq!(chart.chart.kind, ChartKind::Bar);
    assert_eq!(chart.chart.x, "Market");
    assert_eq!(chart.chart.y, vec!["Số dòng".to_string()]);
    assert_eq!(chart.chart.top, Some(15));
    assert!(chart.chart.other_bucket);
    assert_eq!(chart.result.rows.len(), 20);
    assert_eq!(chart.result.columns[0].role, ColumnRole::Dimension);
    assert_eq!(chart.result.columns[1].role, ColumnRole::Measure);
    assert!(chart.sql.is_none());

    // Một giá trị: không có gì để so, không vẽ.
    let one = [value(&market, "Europe", 3)];
    assert!(build_value_chart("ban_hang", &market, &values_of(&market, &one), &profile).is_none());
}
