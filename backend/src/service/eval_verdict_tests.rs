// SPDX-License-Identifier: Apache-2.0
/* service/eval_verdict_tests.rs - ca test của bước chấm một câu ở service/eval.rs (#202), thuần không cần Postgres:
ba trạng thái đạt, trượt, cần xem lại theo lỗi của nhà cung cấp mô hình, status, phần giải thích của câu tìm nguyên
nhân, nhiều đáp án. Câu trả lời lấy từ response mẫu ở service/ask/examples.rs. */

use super::ask::contract::{AskResponse, CellValue};
use super::ask::examples::{build_needs_choice_example, build_simple_example, build_why_example};
use super::eval::{build_answer_verdict, build_ask_error_verdict, EvalCase, Verdict};
use crate::engine::llm::GEMINI_QUOTA_MESSAGE;
use crate::error::AppError;

type Table = Vec<Vec<Option<CellValue>>>;

fn case(kind: &str) -> EvalCase {
    serde_json::from_value(serde_json::json!({
        "id": "t1",
        "kind": kind,
        "question": "q",
        "expected_sql": ["SELECT 1"]
    }))
    .expect("test case parses")
}

/// Bảng đáp án bằng đúng bảng kết quả của response, để ca test chỉ đổi đúng thứ nó kiểm.
fn rows_of(response: &AskResponse) -> Table {
    response
        .result
        .as_ref()
        .expect("example has a result")
        .rows
        .clone()
}

fn wrong_table() -> Table {
    vec![vec![Some(CellValue::String("nowhere".to_string()))]]
}

#[test]
fn provider_errors_need_review_not_a_fail() {
    let quota = AppError::Upstream(GEMINI_QUOTA_MESSAGE.to_string());
    assert_eq!(build_ask_error_verdict(&quota).0, Verdict::Review);
    let busy = AppError::Upstream("Gemini đang lỗi hoặc quá tải (mã 503). Thử lại sau".to_string());
    assert_eq!(build_ask_error_verdict(&busy).0, Verdict::Review);
    let no_key = AppError::Unavailable("Thiếu khóa Gemini".to_string());
    assert_eq!(build_ask_error_verdict(&no_key).0, Verdict::Review);
    let bad = AppError::BadRequest("mock không có câu này".to_string());
    assert_eq!(build_ask_error_verdict(&bad).0, Verdict::Fail);
}

#[test]
fn matching_answer_passes_and_wrong_answer_fails() {
    let response = build_simple_example();
    let plain = case("plain_simple");
    assert_eq!(
        build_answer_verdict(&response, &plain, &[rows_of(&response)]).0,
        Verdict::Pass
    );
    let (verdict, detail) = build_answer_verdict(&response, &plain, &[wrong_table()]);
    assert_eq!(verdict, Verdict::Fail);
    assert!(detail.contains("sql sinh ra"), "{detail}");
}

#[test]
fn any_expected_answer_is_enough() {
    let response = build_simple_example();
    let expected = vec![wrong_table(), rows_of(&response)];
    assert_eq!(
        build_answer_verdict(&response, &case("plain_simple"), &expected).0,
        Verdict::Pass
    );
}

#[test]
fn why_question_answered_as_a_plain_one_fails() {
    // Bảng khớp, nhưng response không có phần giải thích: người hỏi "vì sao" không nhận được nguyên nhân.
    let response = build_simple_example();
    let (verdict, detail) =
        build_answer_verdict(&response, &case("why_groups"), &[rows_of(&response)]);
    assert_eq!(verdict, Verdict::Fail);
    assert!(detail.contains("như câu thường"), "{detail}");
    let why = build_why_example();
    assert_eq!(
        build_answer_verdict(&why, &case("why_periods"), &[rows_of(&why)]).0,
        Verdict::Pass
    );
}

#[test]
fn asking_back_needs_review() {
    let response = build_needs_choice_example();
    assert_eq!(
        build_answer_verdict(&response, &case("plain_simple"), &[wrong_table()]).0,
        Verdict::Review
    );
}

#[test]
fn cases_without_kind_are_graded_as_before() {
    // Bộ câu cũ (eval/dataco.json, sales_joins.json, edu.json) không có kind: không bị luật câu "vì sao".
    let response = build_simple_example();
    let old: EvalCase = serde_json::from_value(serde_json::json!({
        "id": "q01", "question": "q", "expected_sql": ["SELECT 1"]
    }))
    .expect("old case parses");
    assert_eq!(
        build_answer_verdict(&response, &old, &[rows_of(&response)]).0,
        Verdict::Pass
    );
}
