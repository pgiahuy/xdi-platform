// SPDX-License-Identifier: Apache-2.0
/* service/ask/summary_tests.rs - test của đoạn trả lời câu thường (ask/summary.rs): bước kiểm số, câu khuôn, prompt
chỉ gửi tối đa SUMMARY_ROWS dòng, và tắt bằng ANSWER_TEXT thì không gọi mô hình. Mô hình là mô hình giả trong file. */

use std::time::Duration;

use async_trait::async_trait;

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;
use crate::service::ask::contract::{
    CellValue, ColumnRole, ResultColumn, ResultTable, Understanding, UnderstoodItem, ValueKind,
};
use crate::service::ask::summary::{
    build_answer_check, build_summary_prompt, build_template_answer, summarize, SummaryInput,
};

fn column(name: &str, kind: ValueKind, role: ColumnRole) -> ResultColumn {
    ResultColumn {
        name: name.to_string(),
        kind,
        role,
    }
}

fn item(label: &str) -> UnderstoodItem {
    UnderstoodItem {
        label: label.to_string(),
        sql: String::new(),
    }
}

fn understanding() -> Understanding {
    Understanding {
        measures: vec![item("Tổng doanh thu (Sales)")],
        filters: vec![],
        group_by: vec![item("Danh mục (Category Name)")],
        order_by: vec![],
        limit: None,
        time_range: None,
        assumptions: vec![],
    }
}

fn table(rows: &[(&str, f64)]) -> ResultTable {
    ResultTable {
        columns: vec![
            column("category", ValueKind::Text, ColumnRole::Dimension),
            column("sales", ValueKind::Number, ColumnRole::Measure),
        ],
        rows: rows
            .iter()
            .map(|(name, value)| {
                vec![
                    Some(CellValue::String(name.to_string())),
                    Some(CellValue::Number(*value)),
                ]
            })
            .collect(),
        total_rows: rows.len() as u64,
        truncated: false,
        limit_reached: false,
    }
}

fn sales() -> ResultTable {
    table(&[
        ("Fishing", 6929656.49),
        ("Cleats", 4431942.09),
        ("Camping & Hiking", 3667633.2),
    ])
}

#[test]
fn answer_with_numbers_from_the_rows_passes() {
    let result = sales();
    let u = understanding();
    let input = SummaryInput {
        question: "Danh mục nào bán chạy nhất?",
        understanding: Some(&u),
        result: &result,
    };
    let text = "Fishing bán chạy nhất với 6.929.656,49, tiếp theo là Cleats (4.431.942,09) và Camping & Hiking \
                (3.667.633,20); có 3 danh mục.";
    assert_eq!(build_answer_check(text, &input), Ok(()));
}

#[test]
fn check_catches_unknown_numbers_and_ratios() {
    let result = sales();
    let input = SummaryInput {
        question: "Danh mục nào bán chạy nhất?",
        understanding: None,
        result: &result,
    };
    // Làm tròn khác bảng, số tự tính và số kèm phần trăm đều sai.
    let wrong = build_answer_check("Fishing đạt 6.929.656 và chiếm 3%.", &input).unwrap_err();
    assert_eq!(wrong, ["6.929.656", "3"]);
    let wrong = build_answer_check("Fishing gấp 2 lần Cleats.", &input).unwrap_err();
    assert_eq!(wrong, ["2"]);
}

#[test]
fn template_names_the_top_group_when_sorted() {
    let result = sales();
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    assert_eq!(
        build_template_answer(&input),
        "Có 3 dòng; cao nhất là Fishing với 6.929.656,49."
    );
}

#[test]
fn template_without_order_only_counts_rows() {
    let result = table(&[("Cleats", 10.0), ("Fishing", 20.0)]);
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    assert_eq!(build_template_answer(&input), "Có 2 dòng kết quả.");
}

#[test]
fn template_of_one_cell_names_the_measure() {
    let result = ResultTable {
        columns: vec![column("total", ValueKind::Decimal, ColumnRole::Measure)],
        rows: vec![vec![Some(CellValue::String("36784735.01".to_string()))]],
        total_rows: 1,
        truncated: false,
        limit_reached: false,
    };
    let u = understanding();
    let input = SummaryInput {
        question: "Tổng doanh thu?",
        understanding: Some(&u),
        result: &result,
    };
    assert_eq!(
        build_template_answer(&input),
        "Tổng doanh thu (Sales) là 36.784.735,01."
    );
}

#[test]
fn prompt_sends_at_most_ten_rows() {
    let names: Vec<String> = (1..=15).map(|i| format!("group-{i:02}")).collect();
    let rows: Vec<(&str, f64)> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), (100 - i) as f64))
        .collect();
    let result = table(&rows);
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    let prompt = build_summary_prompt(&input);
    assert!(prompt.contains("group-10"), "{prompt}");
    assert!(!prompt.contains("group-11"), "{prompt}");
    assert!(prompt.contains("\"total_rows\": \"15\""), "{prompt}");
}

struct StubLlm(&'static str);

#[async_trait]
impl LlmProvider for StubLlm {
    fn name(&self) -> String {
        "stub".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        assert_eq!(request.task, LlmTask::Summary);
        Ok(self.0.to_string())
    }
}

struct NeverCalled;

#[async_trait]
impl LlmProvider for NeverCalled {
    fn name(&self) -> String {
        "never".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        panic!("the model must not be called");
    }
}

#[tokio::test]
async fn model_answer_is_kept_or_replaced_by_the_template() {
    let result = sales();
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    let timeout = Duration::from_secs(5);
    let good = StubLlm("Fishing bán chạy nhất với 6.929.656,49.");
    let outcome = summarize(&good, &input, true, timeout).await.unwrap();
    assert!(outcome.by_model);
    assert!(outcome.step.ok);
    assert_eq!(outcome.text, "Fishing bán chạy nhất với 6.929.656,49.");

    let wrong = StubLlm("Fishing bán chạy nhất với 7.000.000.");
    let outcome = summarize(&wrong, &input, true, timeout).await.unwrap();
    assert!(!outcome.by_model);
    assert!(!outcome.step.ok);
    assert_eq!(
        outcome.text,
        "Có 3 dòng; cao nhất là Fishing với 6.929.656,49."
    );
}

#[tokio::test]
async fn disabled_answer_text_never_calls_the_model() {
    let result = sales();
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    let outcome = summarize(&NeverCalled, &input, false, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(!outcome.by_model);
    assert!(outcome.step.ok);
}

#[tokio::test]
async fn empty_result_has_no_answer_text() {
    let result = table(&[]);
    let input = SummaryInput {
        question: "q",
        understanding: None,
        result: &result,
    };
    let outcome = summarize(&NeverCalled, &input, true, Duration::from_secs(5)).await;
    assert!(outcome.is_none());
}
