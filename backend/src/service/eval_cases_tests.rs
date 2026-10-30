// SPDX-License-Identifier: Apache-2.0
/* service/eval_cases_tests.rs - test của định dạng file bộ câu (service/eval_cases.rs): file mẫu của repo đọc và kiểm
được, mỗi luật kiểm bắt đúng một kiểu file sai, trường lạ bị từ chối, và câu nào được hỏi khi chạy với mock. */

use super::eval::select_cases;
use super::eval_cases::{check_case_file, load_case_file, CaseFile};

/// File một dataset với các câu cho sẵn dạng JSON.
fn file_with(cases: &str) -> CaseFile {
    let text =
        format!(r#"{{"dataset": {{"name": "DataCo", "file": "data/x.csv"}}, "cases": [{cases}]}}"#);
    serde_json::from_str(&text).expect("test case file parses")
}

const PLAIN: &str = r#"{"id": "a", "kind": "plain_simple", "question": "Doanh thu theo thị trường",
    "expected_sql": ["SELECT 1"], "author": "x"}"#;

fn errors_of(cases: &str) -> Vec<String> {
    check_case_file(&file_with(cases)).expect_err("file should have errors")
}

#[test]
fn repo_case_files_load() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../eval/cases/dataco.json");
    let file = load_case_file(&path).expect("eval/cases/dataco.json is valid");
    assert!(!file.cases.is_empty());
}

#[test]
fn minimal_case_is_valid() {
    assert!(check_case_file(&file_with(PLAIN)).is_ok());
}

#[test]
fn unknown_field_is_refused() {
    let text = r#"{"dataset": {"name": "D", "file": "f"}, "cases": [{"id": "a", "kind": "plain_simple",
        "question": "q", "expected_sql": ["SELECT 1"], "author": "x", "orderd": true}]}"#;
    assert!(serde_json::from_str::<CaseFile>(text).is_err());
}

#[test]
fn unknown_kind_is_refused() {
    let text = r#"{"dataset": {"name": "D", "file": "f"}, "cases": [{"id": "a", "kind": "plain",
        "question": "q", "expected_sql": ["SELECT 1"], "author": "x"}]}"#;
    assert!(serde_json::from_str::<CaseFile>(text).is_err());
}

#[test]
fn duplicate_id_is_an_error() {
    let errors = errors_of(&format!("{PLAIN}, {PLAIN}"));
    assert_eq!(errors, vec!["a: duplicate id"]);
}

#[test]
fn case_without_expected_sql_is_an_error() {
    let errors =
        errors_of(r#"{"id": "a", "kind": "plain_simple", "question": "q", "author": "x"}"#);
    assert_eq!(errors, vec!["a: expected_sql is empty"]);
}

#[test]
fn refuse_case_with_expected_sql_is_an_error() {
    let errors = errors_of(
        r#"{"id": "a", "kind": "refuse", "question": "q", "must_refuse": true,
            "expected_sql": ["SELECT 1"], "author": "x"}"#,
    );
    assert_eq!(errors, vec!["a: must_refuse case has expected_sql"]);
}

#[test]
fn kind_refuse_needs_must_refuse() {
    let errors = errors_of(
        r#"{"id": "a", "kind": "refuse", "question": "q", "expected_sql": ["SELECT 1"], "author": "x"}"#,
    );
    assert_eq!(errors, vec!["a: kind refuse needs must_refuse"]);
}

#[test]
fn variant_equal_to_the_question_is_an_error() {
    let errors = errors_of(
        r#"{"id": "a", "kind": "plain_simple", "question": "q", "variants": ["q "],
            "expected_sql": ["SELECT 1"], "author": "x"}"#,
    );
    assert_eq!(errors, vec!["a: variant repeats a question: q "]);
}

#[test]
fn follow_up_must_point_to_an_earlier_case() {
    let later = r#"{"id": "b", "kind": "follow_up", "question": "còn Europe?", "follow_up_of": "a",
        "expected_sql": ["SELECT 1"], "author": "x"}"#;
    assert!(check_case_file(&file_with(&format!("{PLAIN}, {later}"))).is_ok());
    let errors = errors_of(&format!("{later}, {PLAIN}"));
    assert_eq!(errors, vec!["b: follow_up_of a is not an earlier case"]);
}

#[test]
fn kind_follow_up_needs_follow_up_of() {
    let errors = errors_of(
        r#"{"id": "a", "kind": "follow_up", "question": "q", "expected_sql": ["SELECT 1"], "author": "x"}"#,
    );
    assert_eq!(errors, vec!["a: kind follow_up needs follow_up_of"]);
}

#[test]
fn author_cannot_review_their_own_case() {
    let errors = errors_of(
        r#"{"id": "a", "kind": "plain_simple", "question": "q", "expected_sql": ["SELECT 1"],
            "author": "x", "reviewers": ["y", "x"]}"#,
    );
    assert_eq!(errors, vec!["a: the author cannot review their own case"]);
}

#[test]
fn mock_runs_only_cases_with_a_mock_answer() {
    let mocked = r#"{"id": "m", "kind": "plain_simple", "question": "q", "expected_sql": ["SELECT 1"],
        "mock": true, "author": "x"}"#;
    let file = file_with(&format!("{PLAIN}, {mocked}"));
    let (run, skipped) = select_cases(&file.cases, true);
    assert_eq!(
        run.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec!["m"]
    );
    assert_eq!(skipped.len(), 1);
    let (run, skipped) = select_cases(&file.cases, false);
    assert_eq!(run.len(), 2);
    assert!(skipped.is_empty());
}

#[test]
fn follow_up_is_skipped_for_now() {
    let later = r#"{"id": "b", "kind": "follow_up", "question": "còn Europe?", "follow_up_of": "a",
        "expected_sql": ["SELECT 1"], "author": "x"}"#;
    let file = file_with(&format!("{PLAIN}, {later}"));
    let (run, skipped) = select_cases(&file.cases, false);
    assert_eq!(run.len(), 1);
    assert_eq!(skipped[0].id, "b");
}
