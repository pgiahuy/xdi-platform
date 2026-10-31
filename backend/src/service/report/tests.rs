// SPDX-License-Identifier: Apache-2.0
/* service/report/tests.rs - test không cần Postgres của báo cáo: kế hoạch báo cáo tự động (auto.rs), kiểm số và quyền
xem lời đã lưu (render.rs), file Excel (xlsx.rs). Test qua router, với Postgres, ở api/report_tests.rs. */

use std::io::Cursor;

use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use chrono::Utc;
use uuid::Uuid;

use crate::engine::store::{DatasetRelation, MetricDefinition, ProfileColumn};
use crate::service::ask::contract::{
    AnswerVerdict, CellValue, ColumnRole, QuestionType, ResultColumn, ResultTable, Understanding,
    UnderstoodItem, ValueKind,
};
use crate::service::conversation::contract::SavedAnswer;
use crate::service::report::auto::{build_auto_plan, AutoTable};
use crate::service::report::contract::{
    NumberCheck, ReportItem, ReportKind, ReportResponse, ReportSummary, ReportTableVersion,
};
use crate::service::report::render::{build_number_check, is_within_filters};
use crate::service::report::xlsx::build_report_xlsx;

fn column(name: &str, role: &str, groups: i64) -> ProfileColumn {
    ProfileColumn {
        id: Uuid::new_v4(),
        position: 0,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        description: None,
        description_source: None,
        aliases: Vec::new(),
        is_sensitive: false,
        null_count: Some(0),
        distinct_count: Some(groups),
        min_value: None,
        max_value: None,
        time_format: None,
        unit: None,
        aggregation: "sum".to_string(),
    }
}

fn date_column(name: &str) -> ProfileColumn {
    ProfileColumn {
        data_type: "TIMESTAMP".to_string(),
        min_value: Some("2015-01-01 00:00:00".to_string()),
        max_value: Some("2017-12-31 00:00:00".to_string()),
        ..column(name, "time", 900)
    }
}

fn revenue() -> MetricDefinition {
    MetricDefinition {
        name: "Doanh thu".to_string(),
        description: String::new(),
        expression: "SUM(item_total)".to_string(),
        filter: None,
        time_column: None,
        unit: None,
        confirmed: true,
    }
}

fn relation(from: Uuid, to: Uuid, column: &str) -> DatasetRelation {
    DatasetRelation {
        from_dataset_id: from,
        from_dataset_name: "order_items".to_string(),
        from_columns: vec![column.to_string()],
        to_dataset_id: to,
        to_dataset_name: "orders".to_string(),
        to_columns: vec![column.to_string()],
        constraint_name: "order_items_order_id_fkey".to_string(),
        cardinality: "many_to_one".to_string(),
    }
}

/// order_items có chỉ số Doanh thu mà không có cột ngày; orders (bảng cha) có ngày đặt và các cột chiều.
#[test]
fn plan_reads_time_and_dimensions_from_the_parent_table() {
    let items_id = Uuid::new_v4();
    let orders_id = Uuid::new_v4();
    let items_columns = vec![
        column("order_id", "key", 1000),
        column("item_total", "measure", 500),
    ];
    let mut email = column("email", "dimension", 5);
    email.is_sensitive = true;
    let orders_columns = vec![
        column("order_id", "key", 1000),
        date_column("order_date"),
        column("payment_type", "dimension", 4),
        column("order_status", "dimension", 9),
        column("order_region", "dimension", 23),
        column("order_country", "dimension", 160),
        email,
    ];
    let metrics = vec![revenue()];
    let tables = vec![
        AutoTable {
            id: items_id,
            logical_table: "order_items",
            columns: &items_columns,
            metrics: &metrics,
        },
        AutoTable {
            id: orders_id,
            logical_table: "orders",
            columns: &orders_columns,
            metrics: &[],
        },
    ];
    let plan = build_auto_plan(&tables, &[relation(items_id, orders_id, "order_id")]);
    assert_eq!(plan.table_id, Some(items_id));
    assert_eq!(plan.skipped, Vec::<String>::new());
    let questions: Vec<&str> = plan.sections.iter().map(|s| s.question.as_str()).collect();
    // Chiều A: nhiều nhóm nhất mà không quá 12 (order_status, 9); chiều B: nhiều nhóm nhất còn lại, không quá 30
    // (order_region, 23). order_country quá 30 nhóm, email nhạy cảm.
    assert_eq!(
        questions,
        vec![
            "Doanh thu theo tháng",
            "Doanh thu theo order_status",
            "Doanh thu theo order_region"
        ]
    );
    let join = "JOIN \"orders\" ON \"order_items\".\"order_id\" = \"orders\".\"order_id\"";
    assert_eq!(
        plan.sections[0].sql,
        format!(
            "SELECT date_trunc('month', \"orders\".\"order_date\") AS \"Tháng\", SUM(item_total) AS \"Doanh thu\" \
             FROM \"order_items\" {join} WHERE \"orders\".\"order_date\" IS NOT NULL GROUP BY \"Tháng\" ORDER BY \"Tháng\""
        )
    );
    assert_eq!(
        plan.sections[2].sql,
        format!(
            "SELECT \"orders\".\"order_region\" AS \"order_region\", SUM(item_total) AS \"Doanh thu\" FROM \"order_items\" \
             {join} GROUP BY \"orders\".\"order_region\" ORDER BY \"Doanh thu\" DESC"
        )
    );
}

#[test]
fn plan_without_a_measure_has_no_section() {
    let columns = vec![column("region", "dimension", 4)];
    let tables = vec![AutoTable {
        id: Uuid::new_v4(),
        logical_table: "regions",
        columns: &columns,
        metrics: &[],
    }];
    let plan = build_auto_plan(&tables, &[]);
    assert_eq!(plan.table_id, None);
    assert!(plan.sections.is_empty());
    assert_eq!(plan.skipped.len(), 1);
}

#[test]
fn plan_without_time_says_so_and_keeps_the_group_sections() {
    let mut amount = column("amount", "measure", 50);
    amount.unit = Some("money".to_string());
    let columns = vec![amount, column("market", "dimension", 5)];
    let tables = vec![AutoTable {
        id: Uuid::new_v4(),
        logical_table: "sales",
        columns: &columns,
        metrics: &[],
    }];
    let plan = build_auto_plan(&tables, &[]);
    assert_eq!(plan.sections.len(), 1);
    assert_eq!(plan.sections[0].question, "amount theo market");
    assert!(plan.skipped[0].contains("thời gian"), "{:?}", plan.skipped);
}

#[test]
fn stored_answer_is_shown_only_when_current_row_filters_were_already_applied() {
    let stored = "SELECT x FROM (SELECT * FROM t WHERE \"region\" IN ('South')) t LIMIT 100000";
    assert!(is_within_filters(Some(stored), &[]));
    assert!(is_within_filters(
        Some(stored),
        &["\"region\" IN ('South')".to_string()]
    ));
    assert!(!is_within_filters(
        Some("SELECT x FROM t"),
        &["\"region\" IN ('South')".to_string()]
    ));
    assert!(!is_within_filters(
        None,
        &["\"region\" IN ('South')".to_string()]
    ));
}

fn answer(by_model: bool, verdict: Option<AnswerVerdict>) -> SavedAnswer {
    SavedAnswer {
        text: "Europe dẫn đầu".to_string(),
        by_model,
        verdict,
        warnings: Vec::new(),
    }
}

#[test]
fn number_check_follows_who_wrote_the_text() {
    assert_eq!(
        build_number_check(&answer(true, Some(AnswerVerdict::Match))),
        NumberCheck::Passed
    );
    assert_eq!(
        build_number_check(&answer(false, Some(AnswerVerdict::Mismatch))),
        NumberCheck::Replaced
    );
    assert_eq!(
        build_number_check(&answer(false, None)),
        NumberCheck::NotNeeded
    );
}

fn label(text: &str) -> UnderstoodItem {
    UnderstoodItem {
        label: text.to_string(),
        sql: String::new(),
        from_previous: false,
    }
}

fn item() -> ReportItem {
    ReportItem {
        id: Uuid::new_v4(),
        query_id: Uuid::new_v4(),
        question: "Doanh thu theo thị trường".to_string(),
        asked_at: Utc::now(),
        question_type: QuestionType::Quantitative,
        tables: vec![ReportTableVersion {
            table_id: Uuid::new_v4(),
            name: "orders".to_string(),
            version: 2,
            captured_at: Utc::now(),
            is_current: true,
        }],
        answer: Some(answer(true, Some(AnswerVerdict::Match))),
        number_check: Some(NumberCheck::Passed),
        meaning_check: Some(AnswerVerdict::Match),
        facts: None,
        understanding: Some(Understanding {
            measures: vec![label("Tổng doanh thu")],
            filters: vec![label("Phân quyền: region = South")],
            group_by: vec![label("Thị trường")],
            order_by: Vec::new(),
            limit: None,
            time_range: None,
            assumptions: Vec::new(),
        }),
        sql: Some("SELECT market, SUM(amount) FROM orders GROUP BY market".to_string()),
        result: Some(ResultTable {
            columns: vec![
                ResultColumn {
                    name: "market".to_string(),
                    kind: ValueKind::Text,
                    role: ColumnRole::Dimension,
                },
                ResultColumn {
                    name: "revenue".to_string(),
                    kind: ValueKind::Decimal,
                    role: ColumnRole::Measure,
                },
            ],
            rows: vec![
                vec![
                    Some(CellValue::String("Europe".to_string())),
                    Some(CellValue::String("17.5".to_string())),
                ],
                vec![Some(CellValue::String("LATAM".to_string())), None],
            ],
            total_rows: 2,
            truncated: false,
            limit_reached: false,
        }),
        chart: None,
        breakdown: None,
        notes: Vec::new(),
        error: None,
    }
}

#[test]
fn workbook_has_a_summary_one_sheet_per_item_and_the_method_sheet() {
    let report = ReportResponse {
        report: ReportSummary {
            id: Uuid::new_v4(),
            title: "Báo cáo quý".to_string(),
            intro: "Mở đầu".to_string(),
            kind: ReportKind::Manual,
            dataset_id: None,
            model_calls: None,
            item_count: 2,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        items: vec![item(), item()],
        rendered_at: Utc::now(),
    };
    let bytes = build_report_xlsx(&report).expect("workbook");
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("open workbook");
    assert_eq!(
        book.sheet_names(),
        vec!["Tóm tắt", "Mục 1", "Mục 2", "Cách tính"]
    );
    let first = book.worksheet_range("Mục 1").expect("item sheet");
    // Dòng tiêu đề, dòng số dòng, rồi bảng ở dòng 3: ô số đo decimal thành số của Excel.
    assert_eq!(
        first.get_value((2, 1)),
        Some(&Data::String("revenue".to_string()))
    );
    assert_eq!(first.get_value((3, 1)), Some(&Data::Float(17.5)));
    let method = book.worksheet_range("Cách tính").expect("method sheet");
    assert_eq!(
        method.get_value((1, 2)),
        Some(&Data::String("Tổng doanh thu".to_string()))
    );
    assert_eq!(
        method.get_value((1, 3)),
        Some(&Data::String("Phân quyền: region = South".to_string()))
    );
    let summary = book.worksheet_range("Tóm tắt").expect("summary sheet");
    assert_eq!(
        summary.get_value((5, 4)),
        Some(&Data::String("Đạt".to_string()))
    );
    assert_eq!(
        summary.get_value((5, 7)),
        Some(&Data::String("Mục 1".to_string()))
    );
}
