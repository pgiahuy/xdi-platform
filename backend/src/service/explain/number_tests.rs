// SPDX-License-Identifier: Apache-2.0
/* service/explain/number_tests.rs - test dạng rút gọn của số lớn (explain/number.rs): số từ một triệu trở lên có thêm
"khoảng X triệu" hay "khoảng X tỷ" đứng trước số đủ, số nhỏ hơn và số có dấu trừ giữ nguyên, gọi hai lần không thêm lần
nữa. */

use crate::service::explain::number::{apply_readable_numbers, build_readable_number};

#[test]
fn large_numbers_get_three_significant_digits() {
    assert_eq!(
        build_readable_number("12.340.831,43").as_deref(),
        Some("khoảng 12,3 triệu")
    );
    assert_eq!(
        build_readable_number("1.234.567").as_deref(),
        Some("khoảng 1,23 triệu")
    );
    assert_eq!(
        build_readable_number("123.456.789").as_deref(),
        Some("khoảng 123 triệu")
    );
    assert_eq!(
        build_readable_number("2.500.000.000").as_deref(),
        Some("khoảng 2,5 tỷ")
    );
}

#[test]
fn small_or_machine_numbers_have_no_short_form() {
    assert_eq!(build_readable_number("999.999"), None);
    assert_eq!(build_readable_number("12340831.43"), None);
    assert_eq!(build_readable_number("2017"), None);
}

#[test]
fn text_gets_the_short_form_before_the_full_number() {
    let text = "Doanh thu đạt 12.340.831,43, tăng 4.521,5 (13,27%).";
    let once = apply_readable_numbers(text);
    assert_eq!(
        once,
        "Doanh thu đạt khoảng 12,3 triệu (12.340.831,43), tăng 4.521,5 (13,27%)."
    );
    assert_eq!(apply_readable_numbers(&once), once);
}

/// Số đã nằm trong ngoặc giữ nguyên, để không ra ngoặc lồng ngoặc.
#[test]
fn numbers_inside_parentheses_stay_as_written() {
    assert_eq!(
        apply_readable_numbers("Thấp hẳn: Africa (2.294.452,93); trung bình 7.356.947."),
        "Thấp hẳn: Africa (2.294.452,93); trung bình khoảng 7,36 triệu (7.356.947)."
    );
}

#[test]
fn negative_numbers_stay_as_written() {
    assert_eq!(
        apply_readable_numbers("Lệch -12.340.831,43."),
        "Lệch -12.340.831,43."
    );
}
