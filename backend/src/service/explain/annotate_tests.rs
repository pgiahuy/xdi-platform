// SPDX-License-Identifier: Apache-2.0
/* service/explain/annotate_tests.rs - test của annotate.rs theo bảng trường hợp của #186: chiều tách hẳn hai lát cắt
bị bỏ (so hai nhóm và so hai kỳ), chiều chỉ có vài nhóm một bên thì giữ và gắn "mới", chiều trùng nhóm bị bỏ và giữ
chiều có giá trị chữ, % tác động và % thay đổi, cờ khác thường theo luật 10 điểm và 1%, lát gốc không dương, chênh lệch
gần 0, nhóm ngược chiều, thứ tự chiều. Dựng chiều từ giá trị hai lát cắt bằng đúng build_group_contributions của bước
explain. Bảng phân rã đưa vào không đổi: mọi kết quả nằm trong ghi chú. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{CellValue, DimensionBreakdown, ValueKind};
use crate::service::explain::annotate::{
    annotate_dimensions, build_percent_change, build_percent_text, build_value_presences,
    BreakdownNotes, DimensionNotes, DropReason, GroupNotes, GroupPresence, Totals,
};
use crate::service::explain::{build_group_contributions, GroupValues};

fn d(text: &str) -> Decimal {
    text.parse().expect("decimal")
}

/// Một chiều từ các cặp (giá trị nhóm, lát gốc, lát so sánh).
fn dimension(column: &str, kind: ValueKind, groups: &[(&str, &str, &str)]) -> DimensionBreakdown {
    let values: Vec<GroupValues> = groups
        .iter()
        .map(|(value, base, compare)| GroupValues {
            value: Some(match kind {
                ValueKind::Number => CellValue::Number(value.parse().expect("number")),
                _ => CellValue::String(value.to_string()),
            }),
            base_value: d(base),
            compare_value: d(compare),
        })
        .collect();
    DimensionBreakdown {
        column: column.to_string(),
        label: None,
        kind,
        sql: String::new(),
        groups: build_group_contributions(&values).expect("contributions"),
        mix_effect: None,
        rate_effect: None,
    }
}

fn totals(base: &str, compare: &str) -> Totals {
    Totals {
        base: d(base),
        compare: d(compare),
        change: d(compare) - d(base),
    }
}

/// Ghi chú của các chiều, nhóm có mặt ở đâu theo giá trị 0 như breakdown.rs.
fn annotate(dimensions: &[DimensionBreakdown], totals: Totals) -> BreakdownNotes {
    let presences: Vec<_> = dimensions.iter().map(build_value_presences).collect();
    annotate_dimensions(dimensions, &presences, totals)
}

fn columns<'a>(notes: &BreakdownNotes, dimensions: &'a [DimensionBreakdown]) -> Vec<&'a str> {
    notes
        .dimensions
        .iter()
        .map(|n| dimensions[n.index].column.as_str())
        .collect()
}

fn group<'a>(
    notes: &'a DimensionNotes,
    dimensions: &[DimensionBreakdown],
    value: &str,
) -> &'a GroupNotes {
    let i = dimensions[notes.index]
        .groups
        .iter()
        .position(|g| matches!(&g.value, Some(CellValue::String(v)) if v == value))
        .expect("group");
    &notes.groups[i]
}

/// COMPLETE với CANCELED: Type tách hẳn hai nhóm (DEBIT chỉ ở COMPLETE, TRANSFER chỉ ở CANCELED) nên bị bỏ, kèm giá
/// trị mỗi bên; Market thì giữ.
#[test]
fn a_dimension_that_splits_the_two_groups_is_dropped() {
    let mut kind = dimension(
        "Type",
        ValueKind::Text,
        &[("DEBIT", "1000", "0"), ("TRANSFER", "0", "60")],
    );
    kind.label = Some("hình thức thanh toán".to_string());
    let dimensions = [
        kind,
        dimension(
            "Market",
            ValueKind::Text,
            &[("Europe", "600", "35"), ("LATAM", "400", "25")],
        ),
    ];
    let annotated = annotate(&dimensions, totals("1000", "60"));
    assert_eq!(columns(&annotated, &dimensions), ["Market"]);
    let dropped = &annotated.dropped[0];
    assert_eq!(dropped.column, "Type");
    assert_eq!(dropped.label.as_deref(), Some("hình thức thanh toán"));
    assert_eq!(dropped.reason, DropReason::SeparatesSlices);
    assert_eq!(dropped.base_only, ["DEBIT"]);
    assert_eq!(dropped.compare_only, ["TRANSFER"]);
}

/// So hai kỳ: vài bang chỉ có đơn ở kỳ sau là nguyên nhân thật, nên chiều được giữ và các bang đó là "mới", khác
/// thường khi chiếm từ 1% chênh lệch.
#[test]
fn a_few_one_sided_groups_are_kept_and_marked_new() {
    let dimensions = [dimension(
        "Customer State",
        ValueKind::Text,
        &[
            ("CA", "771.12", "1319.99"),
            ("IL", "0", "524.96"),
            ("PR", "1809.33", "1323.69"),
        ],
    )];
    let annotated = annotate(&dimensions, totals("2580.45", "3168.64"));
    assert!(annotated.dropped.is_empty());
    let states = &annotated.dimensions[0];
    let il = group(states, &dimensions, "IL");
    assert_eq!(il.presence, GroupPresence::New);
    assert!(il.outlier);
    assert_eq!(il.percent_change, None);
    assert!(states.has_outliers);
}

/// Department Id và Department Name chia đúng các nhóm như nhau: giữ Name (chữ), bỏ Id, dù Id xếp trước.
#[test]
fn a_duplicate_dimension_keeps_the_text_one() {
    let groups = [("7", "800", "500"), ("4", "200", "120")];
    let named = [("Fan Shop", "800", "500"), ("Apparel", "200", "120")];
    let dimensions = [
        dimension("Department Id", ValueKind::Number, &groups),
        dimension("Department Name", ValueKind::Text, &named),
    ];
    let annotated = annotate(&dimensions, totals("1000", "620"));
    assert_eq!(columns(&annotated, &dimensions), ["Department Name"]);
    assert_eq!(annotated.dropped[0].column, "Department Id");
    assert_eq!(annotated.dropped[0].reason, DropReason::Duplicates);
    assert_eq!(
        annotated.dropped[0].duplicate_of.as_deref(),
        Some("Department Name")
    );
}

/// Consumer với Corporate: mọi nhóm giảm gần như mức chung, nên không nhóm nào khác thường; % tác động cộng lại 100.
#[test]
fn an_even_change_has_no_outlier() {
    let dimensions = [dimension(
        "Department Name",
        ValueKind::Text,
        &[
            ("Fan Shop", "8869203.67", "5216292.76"),
            ("Apparel", "4130801.34", "2408676.13"),
            ("Golf", "2388833.71", "1405012.47"),
        ],
    )];
    let slices = totals("15388838.72", "9029981.36");
    let annotated = annotate(&dimensions, slices);
    let departments = &annotated.dimensions[0];
    assert!(!departments.has_outliers);
    let shares: Decimal = departments
        .groups
        .iter()
        .map(|g| d(g.share_of_change.as_deref().expect("share")))
        .sum();
    assert!(
        (shares - Decimal::ONE_HUNDRED).abs() <= d("0.2"),
        "{shares}"
    );
    assert_eq!(
        group(departments, &dimensions, "Fan Shop")
            .percent_change
            .as_deref(),
        Some("-41.2")
    );
    assert_eq!(
        build_percent_change(slices)
            .map(build_percent_text)
            .as_deref(),
        Some("-41.3")
    );
}

/// Một nhóm giảm mạnh hơn hẳn mức chung và chiếm nhiều chênh lệch là khác thường; nhóm lệch mạnh nhưng chiếm dưới 1%
/// thì không. Chiều có nhóm khác thường xếp lên trước chiều không có.
#[test]
fn outliers_follow_the_ten_points_and_one_percent_rule() {
    let dimensions = [
        dimension(
            "Market",
            ValueKind::Text,
            &[("Europe", "500", "450"), ("LATAM", "500", "450")],
        ),
        dimension(
            "Category",
            ValueKind::Text,
            &[
                ("Fishing", "400", "260"),
                ("Golf", "595", "639"),
                ("Tiny", "5", "1"),
            ],
        ),
    ];
    let annotated = annotate(&dimensions, totals("1000", "900"));
    assert_eq!(columns(&annotated, &dimensions), ["Category", "Market"]);
    let category = &annotated.dimensions[0];
    assert!(group(category, &dimensions, "Fishing").outlier);
    // Golf đi ngược chiều (tăng 7,4% khi chung giảm 10%) cũng lệch hẳn mức chung.
    assert!(group(category, &dimensions, "Golf").outlier);
    assert!(!annotated.dimensions[1].has_outliers);
}

/// Nhóm dưới 1% chênh lệch không khác thường dù lệch mức chung rất xa.
#[test]
fn a_tiny_group_is_not_an_outlier() {
    let dimensions = [dimension(
        "Category",
        ValueKind::Text,
        &[("Big", "9990", "8991.5"), ("Tiny", "10", "1")],
    )];
    let annotated = annotate(&dimensions, totals("10000", "8992.5"));
    let tiny = group(&annotated.dimensions[0], &dimensions, "Tiny");
    assert_eq!(tiny.percent_change.as_deref(), Some("-90"));
    assert_eq!(tiny.share_of_change.as_deref(), Some("0.9"));
    assert!(!tiny.outlier);
}

/// Nhóm đi ngược chiều tổng có % tác động âm; các nhóm cùng chiều cộng lại hơn 100%.
#[test]
fn a_group_against_the_change_has_a_negative_share() {
    let dimensions = [dimension(
        "Market",
        ValueKind::Text,
        &[("Europe", "500", "300"), ("LATAM", "500", "600")],
    )];
    let annotated = annotate(&dimensions, totals("1000", "900"));
    let markets = &annotated.dimensions[0];
    let share = |value| {
        group(markets, &dimensions, value)
            .share_of_change
            .as_deref()
    };
    assert_eq!(share("Europe"), Some("200"));
    assert_eq!(share("LATAM"), Some("-100"));
}

/// Lợi nhuận âm ở lát gốc: không có % thay đổi chung hay của nhóm có lát gốc âm; % tác động vẫn có.
#[test]
fn a_negative_base_has_no_percent_change() {
    let dimensions = [dimension(
        "Market",
        ValueKind::Text,
        &[("Europe", "-300", "100"), ("LATAM", "200", "250")],
    )];
    let slices = totals("-100", "350");
    let annotated = annotate(&dimensions, slices);
    assert_eq!(build_percent_change(slices), None);
    let markets = &annotated.dimensions[0];
    let europe = group(markets, &dimensions, "Europe");
    assert_eq!(europe.percent_change, None);
    assert!(europe.share_of_change.is_some());
    assert_eq!(
        group(markets, &dimensions, "LATAM")
            .percent_change
            .as_deref(),
        Some("25")
    );
}

/// Chênh lệch tổng gần 0 so với quy mô hai lát cắt: % tác động phình ra vô nghĩa nên không tính, không nhóm nào khác
/// thường.
#[test]
fn a_change_near_zero_has_no_share() {
    let dimensions = [dimension(
        "Market",
        ValueKind::Text,
        &[("Europe", "5000", "6000"), ("LATAM", "5000", "4001")],
    )];
    let annotated = annotate(&dimensions, totals("10000", "10001"));
    let markets = &annotated.dimensions[0];
    assert!(markets.groups.iter().all(|g| g.share_of_change.is_none()));
    assert!(!markets.has_outliers);
}

/// Phần trăm in một chữ số lẻ, bỏ số 0 thừa, không có "-0".
#[test]
fn percent_text_is_short() {
    assert_eq!(build_percent_text(d("41.46")), "41.5");
    assert_eq!(build_percent_text(d("-44.0")), "-44");
    assert_eq!(build_percent_text(d("-0.04")), "0");
}
