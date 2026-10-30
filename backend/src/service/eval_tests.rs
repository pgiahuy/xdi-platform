// SPDX-License-Identifier: Apache-2.0
/* service/eval_tests.rs - ca test của bộ so kết quả đánh giá (build_eval_verdict), thuần không cần Postgres:
cùng dòng khác thứ tự, lệch một dòng, số lệch quanh ngưỡng, null với null, cột đổi tên, ordered khác thứ tự. Thêm phần
của định dạng #202: khớp một trong nhiều đáp án, chọn dataset theo tables, câu phải từ chối, nạp thư mục CSV. */

use std::collections::BTreeSet;

use super::eval::{
    build_any_verdict, build_eval_verdict, build_refuse_verdict, pick_dataset, EvalCase,
};
use super::eval_prepare::{build_data_files, Datasets};
use crate::service::ask::contract::CellValue;
use crate::service::ask::examples::{build_needs_choice_example, build_rejected_example};

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

#[test]
fn any_of_several_answers_passes() {
    let actual = vec![row(vec![num(2.0)])];
    let first = vec![row(vec![num(1.0)])];
    let second = vec![row(vec![num(2.0)])];
    assert!(build_any_verdict(&actual, &[first.clone(), second], false).is_ok());
    let why = build_any_verdict(&actual, &[first], false).unwrap_err();
    assert!(why.contains("dòng 0"), "{why}");
}

fn case_with_tables(tables: &[&str]) -> EvalCase {
    let json = serde_json::json!({
        "id": "c1",
        "question": "q",
        "expected_sql": ["SELECT 1"],
        "tables": tables,
    });
    serde_json::from_value(json).expect("case")
}

#[test]
fn dataset_is_picked_by_the_first_table() {
    let mut datasets = Datasets::new();
    datasets.insert("studentinfo".to_string(), "id-1".to_string());
    datasets.insert("vle".to_string(), "id-2".to_string());
    assert_eq!(
        pick_dataset(&datasets, &case_with_tables(&["vle"])),
        Ok("id-2")
    );
    assert!(pick_dataset(&datasets, &case_with_tables(&["courses"])).is_err());
    // Ca DataCo không ghi tables: chỉ hợp lệ khi đã nạp đúng một dataset.
    assert!(pick_dataset(&datasets, &case_with_tables(&[])).is_err());
    datasets.remove("vle");
    assert_eq!(pick_dataset(&datasets, &case_with_tables(&[])), Ok("id-1"));
}

#[test]
fn only_rejected_counts_as_a_refusal() {
    assert!(build_refuse_verdict(&build_rejected_example()).is_ok());
    assert!(build_refuse_verdict(&build_needs_choice_example()).is_err());
}

#[test]
fn a_directory_loads_only_the_csv_files_the_cases_need() {
    let dir = std::env::temp_dir().join(format!("xdi-eval-data-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("dir");
    for name in ["studentInfo.csv", "vle.csv", "notes.txt"] {
        std::fs::write(dir.join(name), "a\n1\n").expect("write");
    }
    let wanted: BTreeSet<String> = ["studentinfo".to_string()].into();
    let files = build_data_files(&dir, &wanted).expect("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].1, "studentInfo");
    // Không chỉ bảng nào: nạp mọi file CSV, bỏ file khác.
    assert_eq!(
        build_data_files(&dir, &BTreeSet::new()).expect("all").len(),
        2
    );
    let missing: BTreeSet<String> = ["courses".to_string()].into();
    assert!(build_data_files(&dir, &missing).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
