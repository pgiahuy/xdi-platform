// SPDX-License-Identifier: Apache-2.0
/* service/eval_grade_tests.rs - ca test của bộ so kết quả đánh giá (build_eval_verdict), thuần không cần Postgres.
Mỗi ca là một kết quả sai cố ý hay một cách viết khác mà vẫn đúng (#202): cùng dòng khác thứ tự, lệch một dòng, số
lệch quanh ngưỡng làm tròn, số dạng chữ, null với null, cột thừa, cột khác thứ tự, thiếu cột bắt buộc. */

use super::eval_grade::build_eval_verdict;
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
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_ok());
}

#[test]
fn same_rows_other_order_fail_when_ordered() {
    let actual = vec![row(vec![text("b")]), row(vec![text("a")])];
    let expected = vec![row(vec![text("a")]), row(vec![text("b")])];
    assert!(build_eval_verdict(&actual, &expected, true, &[]).is_err());
}

#[test]
fn one_row_off_is_a_mismatch() {
    let actual = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(3.0)])];
    let why = build_eval_verdict(&actual, &expected, false, &[]).unwrap_err();
    assert!(why.contains("dòng 1"), "{why}");
}

#[test]
fn fractions_match_at_four_significant_digits() {
    // Mô hình viết ROUND(x, 4), đáp án để nguyên: vẫn khớp. Lệch ở chữ số thứ tư thì trượt.
    let expected = vec![row(vec![num(0.445_37)])];
    let rounded = vec![row(vec![num(0.4454)])];
    assert!(build_eval_verdict(&rounded, &expected, false, &[]).is_ok());
    let off = vec![row(vec![num(0.4461)])];
    assert!(build_eval_verdict(&off, &expected, false, &[]).is_err());
    let two_digits = vec![row(vec![num(0.45)])];
    assert!(build_eval_verdict(&two_digits, &expected, false, &[]).is_err());
}

#[test]
fn integers_must_match_exactly() {
    // Đếm lệch một dòng là sai, dù hai số giống nhau ở 4 chữ số có nghĩa.
    let actual = vec![row(vec![num(32_594.0)])];
    let expected = vec![row(vec![num(32_593.0)])];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_err());
}

#[test]
fn numeric_text_matches_numbers() {
    // BIGINT và DECIMAL tới bộ chấm là chuỗi; SUM trên cột thực là số. Hai cách viết cùng giá trị phải khớp.
    let actual = vec![row(vec![text("Europe"), text("1234.50")])];
    let expected = vec![row(vec![text("Europe"), num(1234.5)])];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_ok());
    // Chữ trông như số nhưng không đọc được thành số hữu hạn vẫn là chữ.
    let actual = vec![row(vec![text("NaN")])];
    let expected = vec![row(vec![num(0.0)])];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_err());
}

#[test]
fn null_matches_null_only() {
    let actual: Vec<Row> = vec![vec![None]];
    let expected: Vec<Row> = vec![vec![None]];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_ok());
    let expected = vec![row(vec![num(0.0)])];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_err());
}

#[test]
fn column_names_do_not_matter() {
    // Tên cột không vào verdict: bí danh là chi tiết cách viết.
    let actual = vec![row(vec![text("x")])];
    let expected = vec![row(vec![text("x")])];
    assert_eq!(
        build_eval_verdict(&actual, &expected, false, &[]).unwrap(),
        "khớp"
    );
}

#[test]
fn extra_columns_pass_with_a_note() {
    let expected = vec![row(vec![text("x")])];
    let wide = vec![row(vec![num(1.0), text("x")])];
    let note = build_eval_verdict(&wide, &expected, false, &[]).unwrap();
    assert!(note.contains("thừa 1 cột"), "{note}");
}

#[test]
fn swapped_columns_pass() {
    let actual = vec![
        row(vec![num(1.0), text("a")]),
        row(vec![num(2.0), text("b")]),
    ];
    let expected = vec![
        row(vec![text("a"), num(1.0)]),
        row(vec![text("b"), num(2.0)]),
    ];
    let note = build_eval_verdict(&actual, &expected, false, &[]).unwrap();
    assert!(note.contains("khác thứ tự"), "{note}");
}

#[test]
fn columns_must_match_row_by_row_not_only_one_by_one() {
    // Từng cột có cùng tập giá trị, nhưng cặp giá trị trên mỗi dòng khác: số của a và b bị tráo.
    let actual = vec![
        row(vec![text("a"), num(2.0)]),
        row(vec![text("b"), num(1.0)]),
    ];
    let expected = vec![
        row(vec![text("a"), num(1.0)]),
        row(vec![text("b"), num(2.0)]),
    ];
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_err());
}

#[test]
fn missing_required_column_fails() {
    let expected = vec![row(vec![text("a"), num(1.0), num(9.0)])];
    // Thiếu cột 2 của đáp án: đạt khi chỉ cột 0 và 1 bắt buộc, trượt khi mọi cột bắt buộc.
    let actual = vec![row(vec![text("a"), num(1.0)])];
    assert!(build_eval_verdict(&actual, &expected, false, &[0, 1]).is_ok());
    assert!(build_eval_verdict(&actual, &expected, false, &[]).is_err());
    // Đủ số cột mà cột bắt buộc sai giá trị.
    let wrong = vec![row(vec![text("a"), num(5.0)])];
    let why = build_eval_verdict(&wrong, &expected, false, &[0, 1]).unwrap_err();
    assert!(why.contains("cột 1"), "{why}");
}

#[test]
fn required_column_out_of_range_is_reported() {
    let expected = vec![row(vec![text("a")])];
    let why = build_eval_verdict(&expected, &expected, false, &[3]).unwrap_err();
    assert!(why.contains("required_columns"), "{why}");
}

#[test]
fn row_count_mismatch_is_reported() {
    let actual = vec![row(vec![num(1.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let why = build_eval_verdict(&actual, &expected, false, &[]).unwrap_err();
    assert!(why.contains("số dòng"), "{why}");
}
