// SPDX-License-Identifier: Apache-2.0
/* service/ask_narrate_tests.rs - test bước narrate (#41, service/ask_narrate.rs và phần chữ ở explain.rs): bước kiểm
số nhận summary của mẫu why và câu khuôn, loại số lạ, số thiếu chữ số thập phân, cách viết có dấu ngăn và số trước %;
mô hình lỗi, quá giờ, trả câu trống hay quá dài thì rơi về câu khuôn với bước narrate ok = false; chênh lệch bằng 0
không gọi mô hình. Mô hình giả trả câu viết sẵn; không cần DuckDB hay Postgres. */

use std::time::Duration;

use async_trait::async_trait;

use super::ask_contract::{Explanation, StepName};
use super::ask_examples::build_why_example;
use super::ask_narrate::narrate;
use super::explain::{
    build_allowed_numbers, build_narrate_prompt, build_number_check, build_template_summary,
    MAX_SUMMARY_CHARS,
};
use crate::engine::llm::{LlmProvider, LlmRequest};
use crate::error::AppError;

const QUESTION: &str = "Tại sao doanh thu quý 3 năm 2017 giảm?";
const TIMEOUT: Duration = Duration::from_secs(30);

fn example() -> Explanation {
    build_why_example().explanation.expect("why example")
}

fn check(text: &str, explanation: &Explanation) -> Result<(), Vec<String>> {
    build_number_check(text, &build_allowed_numbers(QUESTION, explanation))
}

/// Mô hình giả: trả câu viết sẵn, hoặc lỗi Upstream.
struct StubLlm {
    reply: Result<String, String>,
}

#[async_trait]
impl LlmProvider for StubLlm {
    fn name(&self) -> String {
        "stub".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        match &self.reply {
            Ok(text) => Ok(text.clone()),
            Err(text) => Err(AppError::Upstream(text.clone())),
        }
    }
}

/// Mô hình không bao giờ trả lời xong, để thử nhánh quá giờ.
struct SlowLlm;

#[async_trait]
impl LlmProvider for SlowLlm {
    fn name(&self) -> String {
        "slow".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok("không tới được đây".to_string())
    }
}

/// Mô hình mà test không cho phép gọi: gọi là panic.
struct NeverCalled;

#[async_trait]
impl LlmProvider for NeverCalled {
    fn name(&self) -> String {
        "never-called".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        panic!("change = 0 must not call the model")
    }
}

/// Tiêu chí của #41: summary của mẫu why (ask_examples.rs) qua bước kiểm số.
#[test]
fn sample_summary_passes_the_number_check() {
    let explanation = example();
    check(&explanation.summary, &explanation).unwrap();
}

/// Tiêu chí của #41: câu khuôn cũng qua chính bước kiểm đó.
#[test]
fn template_summary_passes_the_number_check() {
    let explanation = example();
    let template = build_template_summary(&explanation);
    check(&template, &explanation).unwrap();
}

/// Câu khuôn của câu hỏi không có tách lượng giá và không có chiều vẫn đọc được và qua kiểm số.
#[test]
fn template_works_without_dimensions_and_volume_price() {
    let mut explanation = example();
    explanation.dimensions = vec![];
    explanation.volume_price = None;
    let template = build_template_summary(&explanation);
    assert_eq!(
        template,
        "Doanh thu Quý 3/2017 giảm 1283400.00 so với Quý 2/2017, từ 9402500.00 xuống 8119100.00."
    );
    check(&template, &explanation).unwrap();
}

#[test]
fn absolute_value_of_a_negative_change_is_accepted() {
    let explanation = example();
    // change của mẫu là "-1283400.00"; nói "giảm 1283400.00" vẫn nhận.
    check(
        "Doanh thu quý 3/2017 giảm 1283400.00 so với quý 2/2017.",
        &explanation,
    )
    .unwrap();
}

/// Số sai, số thiếu chữ số thập phân, cách viết có dấu ngăn và số trước % đều không qua.
#[test]
fn wrong_numbers_are_rejected() {
    let explanation = example();
    for bad in [
        "Doanh thu giảm 1283401.00 so với quý trước.",
        "Doanh thu giảm 1283400 so với quý trước.",
        "Doanh thu giảm 1.283.400,00 so với quý trước.",
        "Doanh thu quý 3/2017 giảm 13,6% so với quý 2/2017.",
        "Doanh thu quý 3/2017 giảm 13.6% so với quý 2/2017.",
    ] {
        assert!(check(bad, &explanation).is_err(), "should reject: {bad}");
    }
}

/// Số gần đúng bị loại vì thiếu chữ số thập phân: so nguyên văn chuỗi, không nhận "1283400" thay "1283400.00".
#[test]
fn the_rejected_numbers_are_reported() {
    let explanation = example();
    let wrong = check("Doanh thu giảm 1283400 so với quý trước.", &explanation).unwrap_err();
    assert_eq!(wrong, vec!["1283400".to_string()]);
}

/// Prompt gửi đúng phần đã chốt: câu hỏi, độ đo, hai lát cắt, change, hai chiều đầu với ba nhóm mỗi chiều (chiều
/// Market của mẫu có năm nhóm), và volume_price.
#[test]
fn prompt_sends_two_dimensions_with_three_groups_each() {
    let explanation = example();
    let prompt = build_narrate_prompt(QUESTION, &explanation);
    assert!(prompt.contains(QUESTION));
    assert!(prompt.contains("\"measure\": \"Doanh thu\""));
    assert!(prompt.contains("\"change\": \"-1283400.00\""));
    assert_eq!(prompt.matches("\"contribution\"").count(), 2 * 3);
    assert!(prompt.contains("\"column\": \"Market\""));
    assert!(prompt.contains("\"column\": \"Customer Segment\""));
    assert!(prompt.contains("\"if_volume_unchanged\": \"9226250.00\""));
    // Nhóm thứ tư và năm của Market không được gửi.
    assert!(!prompt.contains("USCA"));
}

#[tokio::test]
async fn model_summary_is_used_when_it_passes_the_check() {
    let explanation = example();
    let summary = explanation.summary.clone();
    let llm = StubLlm {
        reply: Ok(summary.clone()),
    };
    let outcome = narrate(&llm, QUESTION, &explanation, TIMEOUT).await;
    assert_eq!(outcome.summary, summary);
    assert!(outcome.summary_by_model);
    assert!(outcome.step.ok);
    assert_eq!(outcome.step.name, StepName::Narrate);
    assert_eq!(outcome.step.detail, None);
}

/// Mô hình lỗi: dùng câu khuôn, summary_by_model = false, bước narrate ok = false kèm lý do.
#[tokio::test]
async fn model_error_falls_back_to_the_template() {
    let explanation = example();
    let llm = StubLlm {
        reply: Err("Mô hình không trả lời, thử lại sau".to_string()),
    };
    let outcome = narrate(&llm, QUESTION, &explanation, TIMEOUT).await;
    assert_eq!(outcome.summary, build_template_summary(&explanation));
    assert!(!outcome.summary_by_model);
    assert!(!outcome.step.ok);
    let detail = outcome.step.detail.expect("detail");
    assert!(detail.contains("Mô hình không trả lời"), "{detail}");
}

#[tokio::test]
async fn timeout_falls_back_to_the_template() {
    let explanation = example();
    let outcome = narrate(&SlowLlm, QUESTION, &explanation, Duration::from_millis(10)).await;
    assert_eq!(outcome.summary, build_template_summary(&explanation));
    assert!(!outcome.summary_by_model);
    assert!(!outcome.step.ok);
    let detail = outcome.step.detail.expect("detail");
    assert!(detail.contains("quá"), "{detail}");
}

#[tokio::test]
async fn empty_or_too_long_reply_falls_back_to_the_template() {
    let explanation = example();
    for reply in ["  \n ".to_string(), "a".repeat(MAX_SUMMARY_CHARS + 1)] {
        let llm = StubLlm { reply: Ok(reply) };
        let outcome = narrate(&llm, QUESTION, &explanation, TIMEOUT).await;
        assert_eq!(outcome.summary, build_template_summary(&explanation));
        assert!(!outcome.summary_by_model);
        assert!(!outcome.step.ok);
    }
}

/// Câu của mô hình có số ngoài bảng phân rã: dùng câu khuôn, detail liệt kê số sai.
#[tokio::test]
async fn wrong_numbers_fall_back_to_the_template() {
    let explanation = example();
    let llm = StubLlm {
        reply: Ok("Doanh thu quý 3/2017 giảm 13,6% so với quý 2/2017.".to_string()),
    };
    let outcome = narrate(&llm, QUESTION, &explanation, TIMEOUT).await;
    assert_eq!(outcome.summary, build_template_summary(&explanation));
    assert!(!outcome.summary_by_model);
    assert!(!outcome.step.ok);
    let detail = outcome.step.detail.expect("detail");
    assert!(detail.contains("13"), "{detail}");
}

/// Chênh lệch bằng 0: không gọi mô hình, câu khuôn nói "không đổi", bước narrate vẫn ok.
#[tokio::test]
async fn zero_change_does_not_call_the_model() {
    let mut explanation = example();
    explanation.change = "0.00".to_string();
    let outcome = narrate(&NeverCalled, QUESTION, &explanation, TIMEOUT).await;
    assert!(outcome.summary.contains("không đổi"));
    assert!(!outcome.summary_by_model);
    assert!(outcome.step.ok);
}
