// SPDX-License-Identifier: Apache-2.0
/* service/explain/per_unit_tests.rs - test luật tên tổng của cột "mỗi đơn vị" (explain/per_unit.rs, #260): bí danh tiếng
Việt và tên cột tiếng Anh có phần "mỗi đơn vị" thành "Tổng ...", tên không có phần đó giữ nguyên, chỉ SUM mới đổi. */

use crate::service::explain::per_unit::{
    build_alias_label, build_total_name, drop_total_word, strip_per_unit,
};

#[test]
fn vietnamese_per_unit_names_become_totals() {
    assert_eq!(
        build_total_name("lợi nhuận mỗi đơn").as_deref(),
        Some("Tổng lợi nhuận")
    );
    assert_eq!(
        build_total_name("Lợi nhuận trên mỗi đơn hàng").as_deref(),
        Some("Tổng lợi nhuận")
    );
    assert_eq!(
        build_total_name("Số lượng của mỗi dòng đơn hàng").as_deref(),
        Some("Tổng số lượng")
    );
    // Tên đã bắt đầu bằng "tổng" thì không thành "Tổng tổng".
    assert_eq!(
        build_total_name("tổng tiền mỗi dòng").as_deref(),
        Some("Tổng tiền")
    );
}

#[test]
fn english_per_unit_names_become_totals() {
    assert_eq!(
        build_total_name("Benefit per order").as_deref(),
        Some("Tổng Benefit")
    );
    assert_eq!(
        build_total_name("Order Profit Per Order").as_deref(),
        Some("Tổng Order Profit")
    );
    assert_eq!(
        build_total_name("profit_per_item").as_deref(),
        Some("Tổng profit")
    );
}

#[test]
fn names_without_a_per_unit_part_are_kept() {
    for name in [
        "Doanh thu",
        "lợi nhuận đơn",
        "Sales",
        "Tỷ lệ mỗi",
        "mỗi đơn",
        "Percent",
        "Operating expenses",
    ] {
        assert_eq!(build_total_name(name), None, "{name}");
    }
    assert_eq!(strip_per_unit("Tổng lợi nhuận"), None);
}

#[test]
fn only_a_sum_alias_is_renamed() {
    let alias = "Lợi nhuận mỗi đơn";
    assert_eq!(
        build_alias_label(alias, "SUM(\"Benefit per order\")"),
        "Tổng lợi nhuận"
    );
    assert_eq!(
        build_alias_label(alias, "ROUND(SUM(\"Benefit per order\"), 2)"),
        "Tổng lợi nhuận"
    );
    assert_eq!(build_alias_label(alias, "AVG(\"Benefit per order\")"), alias);
    assert_eq!(
        build_alias_label(alias, "COUNT(\"Benefit per order\")"),
        alias
    );
    assert_eq!(build_alias_label("Doanh thu", "SUM(\"Sales\")"), "Doanh thu");
}

#[test]
fn the_total_word_is_dropped_for_the_price_label() {
    assert_eq!(drop_total_word("Tổng lợi nhuận"), "Lợi nhuận");
    assert_eq!(drop_total_word("Tổng Benefit"), "Benefit");
    assert_eq!(drop_total_word("Doanh thu"), "Doanh thu");
    assert_eq!(drop_total_word("Tổng"), "Tổng");
}
