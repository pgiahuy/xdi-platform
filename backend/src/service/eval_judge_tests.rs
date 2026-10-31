// SPDX-License-Identifier: Apache-2.0
/* service/eval_judge_tests.rs - test bộ đo mô hình chấm (service/eval_judge.rs, #201): file ca eval/judge.json đọc được,
có cả ca đúng lẫn ca sai, có ca của cả câu "tại sao" lẫn câu thường; báo cáo đếm đúng số ca đúng, bỏ lọt, chỉ cảnh báo, chặn nhầm, cảnh báo thừa và không chấm
được. */

use std::path::Path;

use crate::service::eval_judge::{build_report, load_judge_cases, CaseFacts, JudgeOutcome};

#[test]
fn judge_cases_file_is_valid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../eval/judge.json");
    let cases = load_judge_cases(&path).expect("eval/judge.json");
    let matches = cases.cases.iter().filter(|c| c.expected == "match").count();
    assert!(matches > 0 && matches < cases.cases.len());
    let plain = cases
        .cases
        .iter()
        .filter(|c| matches!(c.facts, CaseFacts::Plain(_)))
        .count();
    assert!(plain > 0 && plain < cases.cases.len());
    for case in &cases.cases {
        assert_eq!(
            case.expected == "match",
            case.error == "none",
            "{}",
            case.id
        );
        match &case.facts {
            CaseFacts::Why(facts) => {
                assert!(!facts.claims.is_empty(), "{}", case.id);
                assert!(
                    !facts.base.is_empty() && !facts.compare.is_empty(),
                    "{}",
                    case.id
                );
            }
            CaseFacts::Plain(facts) => assert!(!facts.title.is_empty(), "{}", case.id),
        }
    }
}

fn outcome(expected: &str, got: Option<&str>) -> JudgeOutcome {
    JudgeOutcome {
        id: "x".to_string(),
        expected: expected.to_string(),
        error: String::new(),
        got: got.map(str::to_string),
        detail: String::new(),
        duration_ms: 10,
    }
}

#[test]
fn report_counts_misses_warnings_and_false_alarms() {
    let report = build_report(vec![
        outcome("match", Some("match")),
        outcome("match", Some("mismatch")),
        outcome("match", Some("warning")),
        outcome("mismatch", Some("mismatch")),
        outcome("mismatch", Some("match")),
        outcome("mismatch", Some("warning")),
        outcome("mismatch", None),
    ]);
    assert_eq!(report.total, 7);
    assert_eq!(report.correct, 2);
    assert_eq!((report.missed, report.mismatch_total), (1, 4));
    assert_eq!(report.warned_wrong, 1);
    assert_eq!((report.false_alarms, report.match_total), (1, 3));
    assert_eq!(report.warned_right, 1);
    assert_eq!(report.unavailable, 1);
}
