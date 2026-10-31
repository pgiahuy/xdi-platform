// SPDX-License-Identifier: Apache-2.0
/* service/ask/total_alias_tests.rs - test đổi bí danh "mỗi đơn vị" của SUM trong câu SQL mô hình viết
(ask/total_alias.rs, #260): đổi ở SELECT và ORDER BY, giữ nguyên khi bí danh không phải của một SUM, khi câu còn chỗ khác
gọi tên đó, hay khi không có gì để đổi. */

use crate::service::ask::total_alias::build_total_aliases;

#[test]
fn a_sum_alias_and_its_order_by_are_renamed() {
    let sql = "SELECT \"Order Status\", SUM(\"Benefit per order\") AS \"Lợi nhuận mỗi đơn\" FROM dataco \
               GROUP BY \"Order Status\" ORDER BY \"Lợi nhuận mỗi đơn\" DESC";
    let renamed = build_total_aliases(sql).expect("renamed");
    assert_eq!(
        renamed,
        "SELECT \"Order Status\", SUM(\"Benefit per order\") AS \"Tổng lợi nhuận\" FROM dataco \
         GROUP BY \"Order Status\" ORDER BY \"Tổng lợi nhuận\" DESC"
    );
}

#[test]
fn round_of_a_sum_is_a_sum() {
    let sql = "SELECT ROUND(SUM(profit), 2) AS \"Lợi nhuận trên mỗi dòng\" FROM order_items";
    let renamed = build_total_aliases(sql).expect("renamed");
    assert!(renamed.contains("AS \"Tổng lợi nhuận\""), "{renamed}");
}

#[test]
fn other_aliases_are_kept() {
    for sql in [
        // AVG của cột mỗi đơn đúng là trung bình mỗi đơn.
        "SELECT AVG(\"Benefit per order\") AS \"Lợi nhuận mỗi đơn\" FROM dataco",
        // Bí danh không có phần "mỗi đơn vị".
        "SELECT SUM(\"Sales\") AS \"Doanh thu\" FROM dataco",
        // Bí danh tiếng Anh: không đổi, như is_vietnamese_alias.
        "SELECT SUM(\"Benefit per order\") AS benefit_per_order FROM dataco",
        // HAVING gọi bí danh: đổi một nửa thì câu hỏng.
        "SELECT \"Market\", SUM(x) AS \"Lợi nhuận mỗi đơn\" FROM t GROUP BY 1 HAVING \"Lợi nhuận mỗi đơn\" > 0",
        "not sql",
    ] {
        assert_eq!(build_total_aliases(sql), None, "{sql}");
    }
}
