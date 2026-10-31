// SPDX-License-Identifier: Apache-2.0
/* service/eval_report_tests.rs - ca test của service/eval_report.rs (#202), thuần không cần Postgres: khoảng Wilson
so với số tính tay, bảng theo loại câu, tập con xoay vòng theo loại, cách hỏi khác, câu hồi quy và câu đổi kết quả
giữa các lượt. */

use super::eval::{CaseOutcome, EvalCase, Verdict};
use super::eval_report::{
    build_kind_rows, build_regressions, build_subset, build_unstable, build_variant_cases,
    build_wilson, PreviousCase, NO_KIND,
};

fn outcome(id: &str, kind: Option<&str>, verdict: Verdict) -> CaseOutcome {
    CaseOutcome {
        id: id.to_string(),
        question: "q".to_string(),
        kind: kind.map(str::to_string),
        run: 1,
        ok: verdict == Verdict::Pass,
        verdict,
        detail: String::new(),
    }
}

fn case(id: &str, kind: &str, variants: &[&str]) -> EvalCase {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "kind": kind,
        "question": format!("question {id}"),
        "variants": variants,
        "expected_sql": ["SELECT 1"]
    }))
    .expect("test case parses")
}

fn ids(cases: &[EvalCase]) -> Vec<&str> {
    cases.iter().map(|c| c.id.as_str()).collect()
}

#[test]
fn wilson_matches_hand_computed_values() {
    // 8/10: tính tay theo công thức Wilson với z = 1.96 ra [0.4902, 0.9433].
    let (low, high) = build_wilson(8, 10);
    assert!((low - 0.4902).abs() < 1e-3, "{low}");
    assert!((high - 0.9433).abs() < 1e-3, "{high}");
    // Đạt hết vẫn có khoảng dưới khác 1, khác khoảng p ± 1.96 sqrt(p(1-p)/n) co về một điểm.
    let (low, high) = build_wilson(5, 5);
    assert!(low < 0.6 && low > 0.5, "{low}");
    assert_eq!(high, 1.0);
    assert_eq!(build_wilson(0, 0), (0.0, 1.0));
}

#[test]
fn kind_rows_keep_file_order_and_leave_review_out_of_accuracy() {
    let cases = vec![
        outcome("a", Some("why_groups"), Verdict::Pass),
        outcome("b", Some("plain_simple"), Verdict::Fail),
        outcome("c", Some("why_groups"), Verdict::Fail),
        outcome("d", Some("why_groups"), Verdict::Review),
        outcome("e", None, Verdict::Pass),
    ];
    let rows = build_kind_rows(&cases);
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, ["why_groups", "plain_simple", NO_KIND]);
    let why = &rows[0];
    assert_eq!((why.passed, why.failed, why.review), (1, 1, 1));
    assert_eq!(why.accuracy, 0.5);
    assert!(why.low < 0.5 && why.high > 0.5);
    assert_eq!(rows[1].accuracy, 0.0);
}

#[test]
fn subset_takes_every_kind_in_turn_and_keeps_file_order() {
    let cases = vec![
        case("w1", "why_groups", &[]),
        case("w2", "why_groups", &[]),
        case("w3", "why_groups", &[]),
        case("p1", "plain_simple", &[]),
        case("r1", "why_ratio", &[]),
        case("p2", "plain_simple", &[]),
    ];
    // Vòng đầu lấy w1, p1, r1; vòng hai lấy w2 rồi đủ 4.
    assert_eq!(
        ids(&build_subset(cases.clone(), 4)),
        ["w1", "w2", "p1", "r1"]
    );
    assert_eq!(build_subset(cases.clone(), 100).len(), 6);
    assert!(build_subset(cases, 0).is_empty());
}

#[test]
fn subset_brings_the_parent_of_a_chosen_follow_up() {
    let mut follow = case("f1", "follow_up", &[]);
    follow.follow_up_of = Some("p2".to_string());
    let cases = vec![
        case("w1", "why_groups", &[]),
        case("p1", "plain_simple", &[]),
        case("p2", "plain_simple", &[]),
        follow,
    ];
    // Vòng đầu lấy w1, p1, f1; f1 kéo theo câu gốc p2, giữ thứ tự file để p2 vẫn hỏi trước.
    assert_eq!(ids(&build_subset(cases, 3)), ["w1", "p1", "p2", "f1"]);
}

#[test]
fn variants_become_cases_after_their_original() {
    let cases = vec![
        case("a", "plain_simple", &["a one", "a two"]),
        case("b", "why_groups", &[]),
    ];
    let expanded = build_variant_cases(cases);
    assert_eq!(ids(&expanded), ["a", "a~v1", "a~v2", "b"]);
    assert_eq!(expanded[2].question, "a two");
    assert_eq!(expanded[2].kind.as_deref(), Some("plain_simple"));
    assert_eq!(expanded[2].expected_sql, ["SELECT 1"]);
}

#[test]
fn regressions_are_cases_that_passed_before_and_fail_now() {
    let previous = vec![
        PreviousCase {
            id: "a".to_string(),
            verdict: Verdict::Pass,
        },
        PreviousCase {
            id: "b".to_string(),
            verdict: Verdict::Fail,
        },
        PreviousCase {
            id: "c".to_string(),
            verdict: Verdict::Pass,
        },
    ];
    let now = vec![
        outcome("a", None, Verdict::Fail),
        outcome("b", None, Verdict::Fail),
        // Cần xem lại là chưa chấm được, không phải hồi quy.
        outcome("c", None, Verdict::Review),
        // Câu mới.
        outcome("d", None, Verdict::Fail),
        outcome("a", None, Verdict::Fail),
    ];
    assert_eq!(build_regressions(&previous, &now), ["a"]);
}

#[test]
fn unstable_lists_cases_whose_verdict_changed_between_runs() {
    let cases = vec![
        outcome("a", None, Verdict::Pass),
        outcome("b", None, Verdict::Pass),
        outcome("c", None, Verdict::Review),
        outcome("a", None, Verdict::Fail),
        outcome("b", None, Verdict::Pass),
        outcome("c", None, Verdict::Pass),
        outcome("a", None, Verdict::Pass),
    ];
    assert_eq!(build_unstable(&cases), ["a"]);
}
