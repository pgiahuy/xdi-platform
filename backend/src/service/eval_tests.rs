// SPDX-License-Identifier: Apache-2.0
/* service/eval_tests.rs - ca test của bộ so kết quả đánh giá (build_eval_verdict), thuần không cần Postgres:
cùng dòng khác thứ tự, lệch một dòng, số lệch quanh ngưỡng, null với null, cột đổi tên, ordered khác thứ tự. */

use super::eval::build_eval_verdict;
use crate::service::ask::contract::CellValue;

type Row = Vec<Option<CellValue>>;

fn row(cells: Vec<CellValue>) -> Row {
    cells.into_iter().map(Some).collect()
}

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

fn text(s: &str) -> CellValue {
    CellValue::String(s.to_string())
}

#[test]
fn same_rows_other_order_pass_when_unordered() {
    let actual = vec![
        row(vec![text("b"), num(2.0)]),
        row(vec![text("a"), num(1.0)]),
    ];
    let expected = vec![
        row(vec![text("a"), num(1.0)]),
        row(vec![text("b"), num(2.0)]),
    ];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
}

#[test]
fn same_rows_other_order_fail_when_ordered() {
    let actual = vec![row(vec![text("b")]), row(vec![text("a")])];
    let expected = vec![row(vec![text("a")]), row(vec![text("b")])];
    assert!(build_eval_verdict(&actual, &expected, true).is_err());
}

#[test]
fn one_row_off_is_a_mismatch() {
    let actual = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(3.0)])];
    let why = build_eval_verdict(&actual, &expected, false).unwrap_err();
    assert!(why.contains("dòng 1"), "{why}");
}

#[test]
fn numbers_allow_tiny_relative_drift() {
    let actual = vec![row(vec![num(1.0000000005)])];
    let expected = vec![row(vec![num(1.0)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let actual = vec![row(vec![num(1.5)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_err());
}

#[test]
fn null_matches_null_only() {
    let actual: Vec<Row> = vec![vec![None]];
    let expected: Vec<Row> = vec![vec![None]];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let expected = vec![row(vec![num(0.0)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_err());
}

#[test]
fn column_names_do_not_matter_but_width_does() {
    // Tên cột không vào verdict (bí danh là chi tiết cách viết), chỉ số cột tính.
    let actual = vec![row(vec![text("x")])];
    let expected = vec![row(vec![text("x")])];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let wide = vec![row(vec![text("x"), num(1.0)])];
    assert!(build_eval_verdict(&wide, &expected, false).is_err());
}

#[test]
fn row_count_mismatch_is_reported() {
    let actual = vec![row(vec![num(1.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let why = build_eval_verdict(&actual, &expected, false).unwrap_err();
    assert!(why.contains("số dòng"), "{why}");
}
