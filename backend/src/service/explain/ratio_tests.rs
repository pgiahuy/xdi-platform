// SPDX-License-Identifier: Apache-2.0
/* service/explain/ratio_tests.rs - test phần toán của độ đo tỷ lệ (explain/ratio.rs) bằng ví dụ tính tay: tổng hai phần
bằng đúng chênh lệch, nghịch lý Simpson, nhóm chỉ có ở một lát cắt, mẫu bằng 0, làm tròn. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{CellValue, GroupContribution};
use crate::service::explain::ratio::{
    build_ratio_change, build_ratio_split, build_rounded_parts, RatioGroup, RatioParts, RatioSplit,
};
use crate::service::explain::ScoreError;

fn d(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn parts(numerator: &str, denominator: &str) -> RatioParts {
    RatioParts {
        numerator: d(numerator),
        denominator: d(denominator),
    }
}

fn group(name: &str, base: (&str, &str), compare: (&str, &str)) -> RatioGroup {
    RatioGroup {
        value: Some(CellValue::String(name.to_string())),
        base: parts(base.0, base.1),
        compare: parts(compare.0, compare.1),
    }
}

// Tổng tử và mẫu của các nhóm, để test không phải tự cộng.
fn totals(groups: &[RatioGroup]) -> (RatioParts, RatioParts) {
    let sum = |pick: fn(&RatioGroup) -> RatioParts| RatioParts {
        numerator: groups.iter().map(|g| pick(g).numerator).sum(),
        denominator: groups.iter().map(|g| pick(g).denominator).sum(),
    };
    (sum(|g| g.base), sum(|g| g.compare))
}

fn split(groups: &[RatioGroup]) -> Result<RatioSplit, ScoreError> {
    let (base, compare) = totals(groups);
    build_ratio_split(groups, base, compare)
}

// (nhóm, tỷ lệ base, tỷ lệ compare, đóng góp, phần cơ cấu, phần hiệu suất)
fn rows(groups: &[GroupContribution]) -> Vec<(String, String, String, String, String, String)> {
    groups
        .iter()
        .map(|g| {
            let name = match &g.value {
                Some(CellValue::String(text)) => text.clone(),
                other => format!("{other:?}"),
            };
            (
                name,
                g.base_value.clone(),
                g.compare_value.clone(),
                g.contribution.clone(),
                g.mix_effect.clone().unwrap(),
                g.rate_effect.clone().unwrap(),
            )
        })
        .collect()
}

fn row(values: [&str; 6]) -> (String, String, String, String, String, String) {
    let [a, b, c, e, f, g] = values.map(str::to_string);
    (a, b, c, e, f, g)
}

fn contributions_sum(split: &RatioSplit) -> Decimal {
    split.groups.iter().map(|g| d(&g.contribution)).sum()
}

/// Nghịch lý Simpson, tính tay: cả hai nhóm giảm tỷ lệ (A 0.1 thành 0.05, B 0.5 thành 0.4) mà tỷ lệ chung tăng từ 0.18
/// lên 0.33, vì mẫu dồn từ A (80 thành 20) sang B (20 thành 80), nhóm có tỷ lệ cao.
/// A: cơ cấu (0.2 - 0.8) * (0.1 + 0.05) / 2 = -0.045, hiệu suất (0.8 + 0.2) / 2 * (0.05 - 0.1) = -0.025.
/// B: cơ cấu (0.8 - 0.2) * (0.5 + 0.4) / 2 = 0.27, hiệu suất (0.2 + 0.8) / 2 * (0.4 - 0.5) = -0.05.
#[test]
fn simpson_paradox_is_a_mix_effect() {
    let groups = [
        group("A", ("8", "80"), ("1", "20")),
        group("B", ("10", "20"), ("32", "80")),
    ];
    let (base, compare) = totals(&groups);
    let change = build_ratio_change(base, compare).unwrap();
    assert_eq!(
        (change.base_rate, change.compare_rate, change.change),
        (d("0.18"), d("0.33"), d("0.15"))
    );
    let split = split(&groups).unwrap();
    assert_eq!(
        rows(&split.groups),
        [
            row(["B", "0.5000", "0.4000", "0.2200", "0.2700", "-0.0500"]),
            row(["A", "0.1000", "0.0500", "-0.0700", "-0.0450", "-0.0250"]),
        ]
    );
    assert_eq!(
        (split.mix_effect, split.rate_effect),
        (d("0.225"), d("-0.075"))
    );
    assert_eq!(split.mix_effect + split.rate_effect, change.change);
    assert_eq!(contributions_sum(&split), change.change);
    // Đóng góp dạng GroupValues để xếp chiều cộng ra đúng chênh lệch.
    let scored: Decimal = split.scored.iter().map(|g| g.compare_value).sum();
    assert_eq!(scored, change.change);
}

/// Nhóm chỉ có ở một lát cắt lấy tỷ lệ của lát còn lại: toàn bộ đóng góp là cơ cấu. Base A 0.2, B 0.4 (R0 = 0.3);
/// compare A 0.2, C 0.8 (R1 = 0.5). B biến mất: cơ cấu (0 - 0.5) * 0.4 = -0.2; C xuất hiện: 0.5 * 0.8 = 0.4.
#[test]
fn group_in_one_slice_only_is_all_mix() {
    let groups = [
        group("A", ("10", "50"), ("10", "50")),
        group("B", ("20", "50"), ("0", "0")),
        group("C", ("0", "0"), ("40", "50")),
    ];
    let split = split(&groups).unwrap();
    assert_eq!(
        rows(&split.groups),
        [
            row(["C", "0.8000", "0.8000", "0.4000", "0.4000", "0.0000"]),
            row(["B", "0.4000", "0.4000", "-0.2000", "-0.2000", "0.0000"]),
            row(["A", "0.2000", "0.2000", "0.0000", "0.0000", "0.0000"]),
        ]
    );
    assert_eq!(contributions_sum(&split), d("0.2"));
}

/// Mẫu tổng bằng 0 thì không có tỷ lệ; nhóm có tử mà mẫu bằng 0 làm tổng sai nên bỏ chiều; nhóm không cộng ra tổng
/// cũng vậy; hai tỷ lệ bằng nhau thì không có gì để phân rã.
#[test]
fn zero_denominators_and_mismatches_are_refused() {
    assert_eq!(build_ratio_change(parts("0", "0"), parts("1", "2")), None);
    let groups = [group("A", ("1", "2"), ("1", "4"))];
    assert_eq!(
        build_ratio_split(&groups, parts("1", "2"), parts("0", "0")).unwrap_err(),
        ScoreError::TotalsMismatch
    );
    // SUM(a) / SUM(b) với b bằng 0 ở nhóm B mà a khác 0.
    let lost = [
        group("A", ("1", "2"), ("1", "4")),
        group("B", ("3", "0"), ("0", "0")),
    ];
    assert_eq!(split(&lost).unwrap_err(), ScoreError::TotalsMismatch);
    assert_eq!(
        build_ratio_split(&groups, parts("1", "3"), parts("1", "4")).unwrap_err(),
        ScoreError::TotalsMismatch
    );
    let flat = [group("A", ("1", "2"), ("2", "4"))];
    assert_eq!(split(&flat).unwrap_err(), ScoreError::NoChange);
}

/// Tỷ lệ là số thập phân vô hạn (1/3, 2/7): mỗi phần làm tròn bốn chữ số, phần lệch chia lại để tổng đóng góp bằng đúng
/// chênh lệch của hai tỷ lệ đã làm tròn, mỗi số lệch số đúng không quá một đơn vị cuối.
#[test]
fn rounded_parts_add_up_to_the_rounded_change() {
    let groups = [
        group("X", ("1", "3"), ("3", "7")),
        group("Y", ("1", "3"), ("1", "2")),
        group("Z", ("2", "3"), ("1", "5")),
        group("W", ("0", "1"), ("2", "3")),
    ];
    let (base, compare) = totals(&groups);
    let change = build_ratio_change(base, compare).unwrap();
    let split = split(&groups).unwrap();
    assert_eq!(contributions_sum(&split), change.change);
    assert_eq!(split.mix_effect + split.rate_effect, change.change);
    for g in &split.groups {
        let (mix, rate) = (
            d(g.mix_effect.as_deref().unwrap()),
            d(g.rate_effect.as_deref().unwrap()),
        );
        assert_eq!(mix.scale(), 4, "{g:?}");
        assert_eq!(mix + rate, d(&g.contribution));
    }

    // Ba phần 0.00004 làm tròn thành 0; thiếu một đơn vị so với 0.0001, đơn vị đó vào phần đầu (phần dư bằng nhau).
    let parts = [d("0.00004"), d("0.00004"), d("0.00004")];
    assert_eq!(
        build_rounded_parts(&parts, d("0.0001")).unwrap(),
        [d("0.0001"), d("0"), d("0")]
    );
    // Thừa: 0.00005 và 0.00006 cùng làm tròn lên 0.0001, tổng 0.0002; phần bị làm tròn lên nhiều nhất (0.00005) bị trừ.
    let parts = [d("0.00005"), d("0.00006"), d("-0.00001")];
    assert_eq!(
        build_rounded_parts(&parts, d("0.0001")).unwrap(),
        [d("0"), d("0.0001"), d("0")]
    );
}
