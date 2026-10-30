// SPDX-License-Identifier: Apache-2.0
/* service/eval_classify_tests.rs - test của bộ câu thử phân loại (service/eval_classify.rs), thuần không cần Postgres:
file eval/classify.json đọc được và có đủ các câu bỏ sót, bắt nhầm của luật từ khóa nêu ở #197, cách đếm và trung vị
của báo cáo. */

use std::collections::HashSet;
use std::path::Path;

use super::eval_classify::{build_report, load_cases, CaseKind, ClassifyOutcome};
use crate::service::explain::form::build_question_kind;

fn cases_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../eval/classify.json")
}

#[test]
fn case_file_has_enough_unique_cases_and_the_known_misses() {
    let cases = load_cases(&cases_path()).unwrap();
    assert!(cases.len() >= 40, "{} cases", cases.len());
    let ids: HashSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), cases.len(), "ids must be unique");
    let kind_of = |question: &str| {
        cases
            .iter()
            .find(|c| c.question == question)
            .map(|c| c.kind)
            .unwrap_or_else(|| panic!("missing case {question:?}"))
    };
    // Luật từ khóa bỏ sót bốn câu này và bắt nhầm hai câu sau; bộ câu phải giữ chúng để đo đúng chỗ luật yếu.
    let misses = [
        "Điều gì khiến doanh thu tháng 5 giảm?",
        "Sao doanh thu giảm vậy?",
        "Doanh thu giảm do gì?",
        "Cái gì kéo lợi nhuận quý 3 xuống?",
    ];
    for question in misses {
        assert_eq!(kind_of(question), CaseKind::Cause, "{question}");
        assert_eq!(
            CaseKind::from(build_question_kind(question)),
            CaseKind::Plain
        );
    }
    let false_positives = [
        "Số đơn theo lý do hủy",
        "Thống kê nguyên nhân giao trễ theo tháng",
    ];
    for question in false_positives {
        assert_eq!(kind_of(question), CaseKind::Plain, "{question}");
        assert_eq!(
            CaseKind::from(build_question_kind(question)),
            CaseKind::Cause
        );
    }
}

fn outcome(expected: CaseKind, keyword: CaseKind, model: CaseKind, ms: u64) -> ClassifyOutcome {
    ClassifyOutcome {
        id: String::new(),
        question: String::new(),
        expected,
        keyword,
        model,
        by_model: model != keyword,
        duration_ms: ms,
        detail: String::new(),
    }
}

#[test]
fn report_counts_both_rules_and_takes_the_median() {
    use CaseKind::{Cause, Plain};
    let report = build_report(vec![
        outcome(Cause, Plain, Cause, 400),
        outcome(Plain, Cause, Plain, 100),
        outcome(Plain, Plain, Plain, 300),
        outcome(Cause, Cause, Cause, 200),
    ]);
    assert_eq!(report.total, 4);
    assert_eq!(report.keyword_correct, 2);
    assert_eq!(report.model_correct, 4);
    assert_eq!(report.fallbacks, 2);
    assert_eq!(report.median_ms, 250);
    let odd = build_report(vec![outcome(Plain, Plain, Plain, 7)]);
    assert_eq!(odd.median_ms, 7);
}
