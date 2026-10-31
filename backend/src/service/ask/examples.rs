// SPDX-License-Identifier: Apache-2.0
/* service/ask/examples.rs - response mẫu của ask, đúng kiểu AskResponse (ask/contract.rs), trên bảng logic dataco (dữ
liệu bán lẻ mẫu mà mock LLM đã biết, engine/llm/mock_answers.json). Dùng ở hai chỗ:
- example của response 200 trong đặc tả OpenAPI (api/query.rs);
- frontend/src/api/examples/ask.json, sinh bằng `xdi-backend ask-examples` (make openapi), để giao diện dựng màn hỏi
  và pane bằng chứng trước khi luồng hỏi chạy thật (#33).
Mẫu dựng bằng code từ chính các struct; ask/examples_tests.rs báo khi file lệch code hay mẫu lệch đặc tả.

Số liệu là số minh họa, không tính từ dữ liệu thật; thời gian các bước và số token của các lần gọi mô hình cũng vậy,
không phải số đo. Các số trong phân
rã được chọn để cộng khớp đúng như #39 và #40 đòi hỏi (test kiểm). */

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::llm::meter::LlmCall;
use crate::engine::llm::LlmTask;
use crate::service::ask::contract::*;
use crate::service::ask::run::REFUSED_MESSAGE;
use crate::service::explain::annotate::{
    annotate_dimensions, apply_group_notes, build_percent_change, build_percent_text,
    build_value_presences, Totals,
};
use crate::service::explain::summary::apply_summary_parts;
use crate::service::insight::facts::{build_group_parts, Point};
use crate::service::insight::narrate::build_code_text;

const MODEL: &str = "gemini-2.5-flash";
const CONVERSATION_ID: Uuid = Uuid::from_u128(0x7d4a2e91_3b6c_4f10_a5d8_6e2c1b0f9a01);
const VERSION_ID: Uuid = Uuid::from_u128(0x3c8e1d2a_9f4b_4a6c_8d7e_2b1f0a9c8d01);
const DATASET_ID: Uuid = Uuid::from_u128(0x1a6f3b2c_4d5e_4f60_8a7b_9c0d1e2f3a01);
// Trần số dòng guard thêm vào SQL trong mẫu; con số thật do #33 chọn khi gọi guard.
const GUARD_LIMIT: &str = " LIMIT 100000";
const ORDER_DATE: &str = r#"strptime("order date (DateOrders)", '%m/%d/%Y %H:%M')"#;

/// Mọi mẫu, theo tên: nội dung file frontend/src/api/examples/ask.json (JSON thụt lề, kết thúc bằng xuống dòng).
pub fn build_examples_json() -> String {
    let examples = serde_json::json!({
        "why": build_why_example(),
        "simple": build_simple_example(),
        "needs_choice": build_needs_choice_example(),
        "rejected": build_rejected_example(),
        "failed": build_failed_example(),
    });
    let mut json = serde_json::to_string_pretty(&examples).expect("serialize ask examples");
    json.push('\n');
    json
}

/// "Doanh thu theo khu vực": "khu vực" khớp hai cột, nên trả lời bằng Market và để Order Region thành giả định đổi được;
/// cột đang dùng nằm trong choices và assumptions. Một chiều, một độ đo, nên gợi ý biểu đồ cột.
pub fn build_simple_example() -> AskResponse {
    let sql = r#"SELECT "Market" AS market, SUM("Order Item Total") AS revenue FROM dataco GROUP BY "Market" ORDER BY revenue DESC"#;
    let rows = [
        ("Europe", 10845230.42),
        ("LATAM", 10276850.17),
        ("Pacific Asia", 8124900.8),
        ("USCA", 5070310.55),
        ("Africa", 2340560.12),
    ];
    AskResponse {
        choices: vec![
            build_chosen("region_column", "market"),
            build_chosen("revenue_measure", "order_item_total"),
        ],
        sql_generated: Some(sql.to_string()),
        sql_guarded: Some(format!("{sql}{GUARD_LIMIT}")),
        tables_read: build_dataco_read(),
        understanding: Some(Understanding {
            measures: vec![build_revenue_item()],
            filters: vec![],
            group_by: vec![build_item("Thị trường (Market)", r#""Market""#)],
            order_by: vec![build_item("Doanh thu, giảm dần", "revenue DESC")],
            limit: None,
            time_range: None,
            assumptions: vec![build_region_choice(), build_revenue_choice()],
        }),
        result: Some(ResultTable {
            columns: vec![
                build_column("market", ValueKind::Text, ColumnRole::Dimension),
                build_column("revenue", ValueKind::Number, ColumnRole::Measure),
            ],
            rows: rows
                .iter()
                .map(|(market, revenue)| {
                    vec![string_cell(market), Some(CellValue::Number(*revenue))]
                })
                .collect(),
            total_rows: 5,
            truncated: false,
            limit_reached: false,
        }),
        chart: Some(ChartSuggestion {
            kind: ChartKind::Bar,
            x: "market".to_string(),
            y: vec!["revenue".to_string()],
            layout: ChartLayout::Single,
            series: None,
            ratio: vec![],
            top: None,
            other_bucket: false,
        }),
        // Phần giải thích của mẫu là câu do code viết (INSIGHT_TEXT=false), nên đoạn trả lời là câu khuôn
        // (ask/summary.rs).
        answer_text: Some("Có 5 dòng; cao nhất là Europe với 10.845.230,42.".to_string()),
        answer_text_by_model: Some(false),
        insight: Some(build_group_insight(&rows)),
        steps: vec![
            build_step(StepName::Context, 15, true, None),
            build_step(
                StepName::Classify,
                640,
                true,
                Some(
                    "Câu hỏi thường: câu hỏi hỏi doanh thu theo thị trường, không hỏi nguyên nhân.",
                ),
            ),
            build_step(StepName::Llm, 1120, true, None),
            build_step(StepName::Guard, 3, true, Some("Thêm LIMIT 100000")),
            build_step(StepName::Run, 38, true, None),
            build_step(StepName::Explain, 0, true, None),
            build_step(
                StepName::Narrate,
                0,
                true,
                Some("INSIGHT_TEXT=false nên phần giải thích dùng câu do code viết"),
            ),
            build_step(StepName::Record, 6, true, None),
        ],
        // Bước classify và llm ở trên mỗi bước một lần gọi; bước narrate dùng câu do code viết nên không gọi. Số token
        // là số minh họa, không phải số đo.
        model_calls: vec![
            build_call(LlmTask::Classify, 630, 410, 25),
            build_call(LlmTask::Sql, 1100, 2900, 120),
        ],
        question_type: Some(QuestionType::Quantitative),
        ..build_base(AskStatus::Answered, "Doanh thu theo khu vực", Some(1))
    }
}

/// "Tại sao doanh thu quý 3 năm 2017 giảm?": câu "tại sao" có phân rã theo hai chiều và tách lượng, giá.
pub fn build_why_example() -> AskResponse {
    let quarter = format!("date_trunc('quarter', {ORDER_DATE})");
    let period = build_period_filter("2017-04-01", "2017-10-01");
    let sql = format!(
        r#"SELECT {quarter} AS quarter, SUM("Order Item Total") AS revenue, COUNT(DISTINCT "Order Id") AS orders FROM dataco WHERE {period} GROUP BY quarter ORDER BY quarter"#
    );
    AskResponse {
        choices: vec![
            build_chosen("revenue_measure", "order_item_total"),
            build_chosen("base_period", "previous_quarter"),
        ],
        sql_generated: Some(sql.clone()),
        sql_guarded: Some(format!("{sql}{GUARD_LIMIT}")),
        tables_read: build_dataco_read(),
        understanding: Some(Understanding {
            measures: vec![
                build_revenue_item(),
                build_item("Số đơn: số Order Id khác nhau", r#"COUNT(DISTINCT "Order Id")"#),
            ],
            filters: vec![],
            group_by: vec![build_item("Quý của ngày đặt hàng", &quarter)],
            order_by: vec![build_item("Quý, tăng dần", "quarter")],
            limit: None,
            time_range: Some(build_item("Quý 3/2017 so với quý 2/2017", &period)),
            assumptions: vec![
                build_revenue_choice(),
                Choice {
                    id: "base_period".to_string(),
                    label: "So sánh với".to_string(),
                    selected: Some("previous_quarter".to_string()),
                    options: vec![
                        build_option("previous_quarter", "Quý liền trước (quý 2/2017)"),
                        build_option("same_quarter_last_year", "Cùng kỳ năm trước (quý 3/2016)"),
                    ],
                },
            ],
        }),
        result: Some(ResultTable {
            columns: vec![
                build_column("quarter", ValueKind::Timestamp, ColumnRole::Time),
                build_column("revenue", ValueKind::Number, ColumnRole::Measure),
                build_column("orders", ValueKind::Decimal, ColumnRole::Measure),
            ],
            rows: vec![
                vec![
                    string_cell("2017-04-01T00:00:00"),
                    Some(CellValue::Number(9402500.0)),
                    string_cell("5000"),
                ],
                vec![
                    string_cell("2017-07-01T00:00:00"),
                    Some(CellValue::Number(8119100.0)),
                    string_cell("4400"),
                ],
            ],
            total_rows: 2,
            truncated: false,
            limit_reached: false,
        }),
        // Hai quý nên cột đứng; doanh thu và số đơn khác đơn vị nên mỗi số đo một biểu đồ nhỏ (ask/chart.rs).
        chart: Some(ChartSuggestion {
            kind: ChartKind::Column,
            x: "quarter".to_string(),
            y: vec!["revenue".to_string(), "orders".to_string()],
            layout: ChartLayout::Panels,
            series: None,
            ratio: vec![],
            top: None,
            other_bucket: false,
        }),
        warnings: vec![Warning {
            code: WarningCode::UnusualMagnitude,
            message: "Số đơn quý 3/2017 thấp hơn mọi quý trước đó trong dữ liệu; nên kiểm dữ liệu quý này đã nạp đủ \
                      chưa."
                .to_string(),
        }],
        explanation: Some(build_why_explanation()),
        steps: vec![
            build_step(StepName::Context, 21, true, None),
            build_step(
                StepName::Classify,
                710,
                true,
                Some("Câu hỏi tìm nguyên nhân: câu hỏi hỏi vì sao doanh thu quý 3 giảm."),
            ),
            build_step(StepName::Llm, 1380, true, None),
            build_step(StepName::Guard, 4, true, Some("Thêm LIMIT 100000")),
            build_step(StepName::Run, 57, true, None),
            build_step(
                StepName::Explain,
                64,
                true,
                Some("Phân rã theo 2 chiều: Market, Customer Segment"),
            ),
            build_step(StepName::Narrate, 1840, true, None),
            build_step(StepName::Record, 7, true, None),
        ],
        question_type: Some(QuestionType::Causal),
        ..build_base(AskStatus::Answered, "Tại sao doanh thu quý 3 năm 2017 giảm?", Some(2))
    }
}

fn build_why_explanation() -> Explanation {
    let base = build_period_filter("2017-04-01", "2017-07-01");
    let compare = build_period_filter("2017-07-01", "2017-10-01");
    let period = build_period_filter("2017-04-01", "2017-10-01");
    // Độ đo đổi sang DECIMAL ở từng dòng trước khi cộng, như quy tắc ở Explanation (ask/contract.rs).
    let revenue = r#"CAST("Order Item Total" AS DECIMAL(18, 2))"#;
    let both_slices = format!(
        "SUM(CASE WHEN {base} THEN {revenue} END) AS base_value, \
         SUM(CASE WHEN {compare} THEN {revenue} END) AS compare_value"
    );
    let by = |column: &str| {
        format!(
            r#"SELECT "{column}", {both_slices} FROM dataco WHERE {period} GROUP BY "{column}"{GUARD_LIMIT}"#
        )
    };
    let volume_sql = format!(
        r#"SELECT COUNT(DISTINCT CASE WHEN {base} THEN "Order Id" END) AS base_volume, COUNT(DISTINCT CASE WHEN {compare} THEN "Order Id" END) AS compare_volume, {both_slices} FROM dataco WHERE {period}{GUARD_LIMIT}"#
    );
    let mut explanation = Explanation {
        measure: "Doanh thu".to_string(),
        measure_kind: MeasureKind::Total,
        comparison: Comparison::Periods,
        counts_rows: false,
        slice_column: None,
        slice_column_label: None,
        base: ExplanationSlice {
            label: "Quý 2/2017".to_string(),
            filter: base,
            value: "9402500.00".to_string(),
            rows: "6200".to_string(),
        },
        compare: ExplanationSlice {
            label: "Quý 3/2017".to_string(),
            filter: compare,
            value: "8119100.00".to_string(),
            rows: "5480".to_string(),
        },
        change: "-1283400.00".to_string(),
        percent_change: None,
        dimensions: vec![
            build_breakdown(
                "Market",
                Some("thị trường"),
                by("Market"),
                &[
                    ("Europe", "2850120.40", "2210340.15", "-639780.25"),
                    ("LATAM", "2640300.10", "2301250.60", "-339049.50"),
                    ("Pacific Asia", "2105430.25", "1960120.00", "-145310.25"),
                    ("USCA", "1210500.00", "1090230.00", "-120270.00"),
                    ("Africa", "596149.25", "557159.25", "-38990.00"),
                ],
            ),
            build_breakdown(
                "Customer Segment",
                Some("phân khúc khách hàng"),
                by("Customer Segment"),
                &[
                    ("Consumer", "4880200.15", "4150100.50", "-730099.65"),
                    ("Corporate", "2850000.10", "2480300.25", "-369699.85"),
                    ("Home Office", "1672299.75", "1488699.25", "-183600.50"),
                ],
            ),
        ],
        volume_price: Some(VolumePrice {
            volume_label: "Số đơn".to_string(),
            price_label: "Giá trị đơn trung bình".to_string(),
            sql: volume_sql,
            base_volume: "5000".to_string(),
            compare_volume: "4400".to_string(),
            base_price: "1880.50".to_string(),
            compare_price: "1845.25".to_string(),
            volume_effect: "-1117725.00".to_string(),
            price_effect: "-165675.00".to_string(),
            if_volume_unchanged: "9226250.00".to_string(),
        }),
        summary: String::new(),
        summary_title: String::new(),
        summary_points: Vec::new(),
        summary_by_model: true,
        summary_warnings: Vec::new(),
        summary_verdict: Some(AnswerVerdict::Match),
        dropped_dimensions: Vec::new(),
        notes: Default::default(),
    };
    // % thay đổi, ghi chú và các ý tóm tắt tính bằng đúng hàm của bước explain, để mẫu khớp response thật.
    let totals = Totals {
        base: Decimal::new(940250000, 2),
        compare: Decimal::new(811910000, 2),
        change: Decimal::new(-128340000, 2),
    };
    let mut presences: Vec<_> = explanation
        .dimensions
        .iter()
        .map(build_value_presences)
        .collect();
    explanation.percent_change = build_percent_change(totals).map(build_percent_text);
    apply_group_notes(&mut explanation.dimensions, &mut presences, totals);
    explanation.notes = annotate_dimensions(&explanation.dimensions, &presences, totals);
    explanation.dropped_dimensions = explanation.notes.dropped.clone();
    apply_summary_parts(&mut explanation);
    apply_model_insight(&mut explanation);
    explanation
}

/// Insight mô hình viết cho mẫu why (cũng là câu trả lời narrate của mock, engine/llm/mock_answers.json): câu mở đầu
/// và các ý thay câu khuôn, summary là các phần đó nối lại như NarrateReply::build_text.
fn apply_model_insight(explanation: &mut Explanation) {
    let reason = |lead: &str, text: &str| SummaryPoint {
        kind: SummaryPointKind::Reason,
        lead: lead.to_string(),
        text: text.to_string(),
    };
    explanation.summary_title =
        "Doanh thu Quý 3/2017 giảm 1.283.400,00 (13,6%) so với Quý 2/2017, từ \
                                 9.402.500,00 xuống 8.119.100,00."
            .to_string();
    explanation.summary_points = vec![
        reason(
            "Số đơn ít đi",
            "Số đơn giảm từ 5.000 xuống 4.400, làm doanh thu giảm 1.117.725,00, chiếm 87,1% mức giảm.",
        ),
        reason(
            "Giá trị đơn trung bình thấp hơn",
            "Giá trị đơn trung bình giảm từ 1.880,50 xuống 1.845,25, làm doanh thu giảm thêm 165.675,00.",
        ),
        reason(
            "Thị trường Europe giảm nhiều nhất",
            "Europe giảm 639.780,25, sau đó là LATAM giảm 339.049,50.",
        ),
    ];
    let mut text = explanation.summary_title.clone();
    for point in &explanation.summary_points {
        text.push_str(&format!(" {}: {}", point.lead, point.text));
    }
    explanation.summary = text;
}

/// "Doanh thu từng tháng năm 2017" trên dataset chưa có chỉ số: mô hình hỏi lại doanh thu tính bằng cột nào.
pub fn build_needs_choice_example() -> AskResponse {
    let question = "Doanh thu tính bằng cột nào?";
    AskResponse {
        message: Some(question.to_string()),
        clarifications: vec![Choice {
            id: "revenue_measure".to_string(),
            label: question.to_string(),
            selected: None,
            options: vec![
                build_option("order_item_total", "Order Item Total, sau giảm giá"),
                build_option("sales", "Sales, trước giảm giá"),
            ],
        }],
        steps: vec![
            build_step(StepName::Context, 12, true, None),
            build_step(StepName::Llm, 940, true, None),
        ],
        question_type: Some(QuestionType::Quantitative),
        ..build_base(
            AskStatus::NeedsChoice,
            "Doanh thu từng tháng năm 2017",
            None,
        )
    }
}

/// "Xóa các đơn hàng bị hủy": mô hình viết câu DELETE, guard từ chối, câu sửa vẫn là DELETE và bị từ chối lần nữa.
pub fn build_rejected_example() -> AskResponse {
    let reason = "Chỉ chạy được câu SELECT đọc dữ liệu";
    AskResponse {
        message: Some(REFUSED_MESSAGE.to_string()),
        sql_generated: Some(r#"DELETE FROM dataco WHERE "Order Status" = 'CANCELED'"#.to_string()),
        steps: vec![
            build_step(StepName::Context, 14, true, None),
            build_step(StepName::Llm, 980, true, None),
            build_step(StepName::Guard, 2, false, Some(reason)),
            build_step(
                StepName::Repair,
                1150,
                true,
                Some("Mô hình sửa SQL theo lý do guard từ chối"),
            ),
            build_step(StepName::Guard, 2, false, Some(reason)),
            build_step(StepName::Record, 5, true, None),
        ],
        question_type: Some(QuestionType::Quantitative),
        ..build_base(AskStatus::Rejected, "Xóa các đơn hàng bị hủy", Some(3))
    }
}

/// "Doanh thu theo ngày giao hàng": SQL đọc sai định dạng ngày, vòng sửa vẫn sai.
pub fn build_failed_example() -> AskResponse {
    let first_error =
        r#"Could not parse string "1/31/2018 22:56" according to format specifier "%d/%m/%Y""#;
    let second_error = r#"Could not parse string "1/31/2018 22:56" according to format specifier "%d/%m/%Y %H:%M""#;
    let sql = r#"SELECT CAST(strptime("shipping date (DateOrders)", '%d/%m/%Y %H:%M') AS DATE) AS ship_day, SUM("Order Item Total") AS revenue FROM dataco GROUP BY ship_day ORDER BY ship_day"#;
    AskResponse {
        message: Some(format!(
            "Không chạy được câu SQL, kể cả sau khi mô hình sửa một lần: {second_error}. Thử hỏi lại, nói rõ cột \
             ngày cần dùng."
        )),
        choices: vec![
            build_chosen("region_column", "market"),
            build_chosen("revenue_measure", "order_item_total"),
        ],
        sql_generated: Some(sql.to_string()),
        sql_guarded: Some(format!("{sql}{GUARD_LIMIT}")),
        steps: vec![
            build_step(StepName::Context, 16, true, None),
            build_step(StepName::Llm, 1210, true, None),
            build_step(StepName::Guard, 3, true, Some("Thêm LIMIT 100000")),
            build_step(StepName::Run, 9, false, Some(first_error)),
            build_step(StepName::Repair, 1150, true, Some("Mô hình sửa SQL theo lỗi của lần chạy đầu")),
            build_step(StepName::Guard, 3, true, Some("Thêm LIMIT 100000")),
            build_step(StepName::Run, 8, false, Some(second_error)),
            build_step(StepName::Record, 6, true, None),
        ],
        question_type: Some(QuestionType::Quantitative),
        ..build_base(AskStatus::Failed, "Doanh thu theo ngày giao hàng", Some(4))
    }
}

// Phần chung của mọi mẫu: mỗi mẫu chỉ ghi các trường của nó, phần còn lại lấy từ đây bằng cú pháp `..build_base(..)`.
// query_number phân biệt id câu hỏi giữa các mẫu; None cho needs_choice (không ghi lịch sử).
fn build_base(status: AskStatus, question: &str, query_number: Option<u128>) -> AskResponse {
    AskResponse {
        status,
        question: question.to_string(),
        message: None,
        query_id: query_number.map(|n| Uuid::from_u128(0x5b2f0c1e_7a4d_4e8b_9c61_0d3e2f1a4b00 + n)),
        conversation_id: CONVERSATION_ID,
        table_id: DATASET_ID,
        version_id: VERSION_ID,
        model: MODEL.to_string(),
        choices: vec![],
        sql_generated: None,
        sql_guarded: None,
        tables_read: vec![],
        understanding: None,
        result: None,
        chart: None,
        warnings: vec![],
        explanation: None,
        insight: None,
        answer_text: None,
        answer_text_by_model: None,
        question_type: None,
        clarifications: vec![],
        steps: vec![],
        model_calls: vec![],
    }
}

fn build_call(task: LlmTask, duration_ms: u64, input: u64, output: u64) -> LlmCall {
    LlmCall {
        task,
        duration_ms,
        ok: true,
        input_tokens: Some(input),
        output_tokens: Some(output),
    }
}

/// Phần giải thích của mẫu theo nhóm, dựng bằng đúng code của service/insight/ từ các dòng của mẫu, nên mẫu luôn khớp
/// cách backend viết. Lời là câu do code viết (mock không có lời viết sẵn cho bước này).
fn build_group_insight(rows: &[(&str, f64)]) -> Insight {
    let points: Vec<Point> = rows
        .iter()
        .map(|(label, value)| Point {
            label: label.to_string(),
            value: value.to_string().parse().unwrap_or_default(),
        })
        .collect();
    let total = points.iter().map(|p| p.value).sum();
    let parts = build_group_parts("Doanh thu", "Thị trường (Market)", &points, Some(total));
    Insight {
        kind: InsightKind::Groups,
        summary: build_code_text(&parts),
        title: parts.title,
        points: parts.points,
        summary_by_model: false,
        summary_warnings: Vec::new(),
        summary_verdict: None,
        notable: parts.notable,
    }
}

fn build_dataco_read() -> Vec<TableRead> {
    vec![TableRead {
        dataset_id: DATASET_ID,
        name: "DataCo".to_string(),
        logical_table: "dataco".to_string(),
    }]
}

fn build_period_filter(from: &str, until: &str) -> String {
    format!("{ORDER_DATE} >= TIMESTAMP '{from}' AND {ORDER_DATE} < TIMESTAMP '{until}'")
}

fn build_revenue_item() -> UnderstoodItem {
    build_item(
        "Doanh thu: tổng Order Item Total",
        r#"SUM("Order Item Total")"#,
    )
}

// Giả định "khu vực" là cột nào: cột đang dùng đứng đầu, cột còn lại là phương án đổi được.
fn build_region_choice() -> Choice {
    Choice {
        id: "region_column".to_string(),
        label: "Hiểu \"khu vực\" là".to_string(),
        selected: Some("market".to_string()),
        options: vec![
            build_option("market", "5 nhóm, như Europe, LATAM, Pacific Asia (Market)"),
            build_option(
                "order_region",
                "23 nhóm, như Western Europe, Central America, South America (Order Region)",
            ),
        ],
    }
}

fn build_revenue_choice() -> Choice {
    Choice {
        id: "revenue_measure".to_string(),
        label: "Doanh thu tính theo".to_string(),
        selected: Some("order_item_total".to_string()),
        options: vec![
            build_option("order_item_total", "Order Item Total, sau giảm giá"),
            build_option("sales", "Sales, trước giảm giá"),
        ],
    }
}

fn build_breakdown(
    column: &str,
    label: Option<&str>,
    sql: String,
    groups: &[(&str, &str, &str, &str)],
) -> DimensionBreakdown {
    DimensionBreakdown {
        column: column.to_string(),
        label: label.map(str::to_string),
        kind: ValueKind::Text,
        sql,
        groups: groups
            .iter()
            .map(|(value, base, compare, contribution)| GroupContribution {
                value: string_cell(value),
                base_value: base.to_string(),
                compare_value: compare.to_string(),
                contribution: contribution.to_string(),
                mix_effect: None,
                rate_effect: None,
                ..Default::default()
            })
            .collect(),
        mix_effect: None,
        rate_effect: None,
        has_outliers: false,
    }
}

fn build_item(label: &str, sql: &str) -> UnderstoodItem {
    UnderstoodItem {
        label: label.to_string(),
        sql: sql.to_string(),
        from_previous: false,
    }
}

fn build_option(id: &str, label: &str) -> ChoiceOption {
    ChoiceOption {
        id: id.to_string(),
        label: label.to_string(),
    }
}

fn build_chosen(choice_id: &str, option_id: &str) -> ChosenOption {
    ChosenOption {
        choice_id: choice_id.to_string(),
        option_id: option_id.to_string(),
    }
}

fn build_column(name: &str, kind: ValueKind, role: ColumnRole) -> ResultColumn {
    ResultColumn {
        name: name.to_string(),
        kind,
        role,
    }
}

// Ô dạng chuỗi JSON: dùng cho cột text, và cả cột decimal, timestamp (xem ValueKind).
fn string_cell(value: &str) -> Option<CellValue> {
    Some(CellValue::String(value.to_string()))
}

fn build_step(name: StepName, duration_ms: u64, ok: bool, detail: Option<&str>) -> AskStep {
    AskStep {
        name,
        duration_ms,
        ok,
        detail: detail.map(str::to_string),
    }
}
