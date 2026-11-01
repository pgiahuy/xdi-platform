// SPDX-License-Identifier: Apache-2.0
/* service/explain/rest_average.rs - một nhóm so với trung bình các nhóm còn lại (#201, việc 7). Câu "vì sao Europe có
doanh thu cao nhất" so Europe với tổng của mọi thị trường khác gộp lại thì Europe luôn "thấp hơn", ngược câu hỏi. Với độ
đo cộng được (tổng, đếm, đếm phân biệt), lát gốc "các nhóm còn lại" vì vậy là trung bình mỗi nhóm còn lại: tổng, số dòng và lượng của
lát gốc chia cho số nhóm còn lại (k, câu tổng hai lát cắt đếm, explain/sql.rs). Độ đo tỷ lệ và trung bình không chia:
tỷ lệ của phần còn lại gộp lại vốn đã là mức chung của các nhóm đó.

Mỗi nhóm của một chiều phân rã cũng chia cho k, làm tròn tới cùng số chữ số với tổng rồi chia phần dư lớn nhất (như
explain/ratio.rs), để các nhóm cộng lại đúng bằng tổng lát gốc đã chia và phần đóng góp vẫn cộng ra đúng chênh lệch.
explain/breakdown.rs gọi các hàm ở đây; check.rs, sql.rs và nhãn (group_base.rs) hỏi averages_rest. */

use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::explain::check::{PlanSlices, WhyPlan};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::measure::Additive;

/// Số chữ số thập phân tối thiểu của số đã chia: trung bình của một số đếm thường lẻ (3,5 đơn mỗi nhóm).
const MIN_SCALE: u32 = 2;

/// Lát gốc là các nhóm còn lại và độ đo là tổng, đếm dòng hay đếm phân biệt: so với trung bình mỗi nhóm còn lại. Đếm
/// phân biệt đếm cặp (nhóm, giá trị) khi so với phần còn lại (apply_rest_distinct của explain/check.rs), nên tổng các nhóm
/// còn lại chia số nhóm cũng là trung bình mỗi nhóm.
pub fn averages_rest(plan: &WhyPlan) -> bool {
    let rest = matches!(
        &plan.slices,
        PlanSlices::Groups {
            base: GroupBase::Rest,
            ..
        }
    );
    let additive = matches!(
        plan.measure.additive,
        Additive::Sum { .. } | Additive::Count { .. } | Additive::CountDistinct { .. }
    );
    rest && additive && plan.measure.denominator.is_none()
}

/// Số chữ số thập phân của số đã chia cho k.
pub fn build_average_scale(value: Decimal) -> u32 {
    value.scale().max(MIN_SCALE)
}

/// value / k làm tròn tới scale chữ số; None khi k không dương.
pub fn build_average(value: Decimal, groups: Decimal, scale: u32) -> Option<Decimal> {
    if groups <= Decimal::ZERO {
        return None;
    }
    Some(
        value
            .checked_div(groups)?
            .round_dp_with_strategy(scale, RoundingStrategy::MidpointAwayFromZero),
    )
}

/// Giá trị lát gốc của từng nhóm một chiều, đã chia cho k, cộng lại đúng bằng total (tổng lát gốc đã chia, cùng scale).
/// None khi phép chia tràn hay phần dư không chia hết được (không xảy ra khi các nhóm cộng ra đúng tổng chưa chia).
pub fn build_group_averages(
    values: &[Decimal],
    groups: Decimal,
    total: Decimal,
    scale: u32,
) -> Option<Vec<Decimal>> {
    if groups <= Decimal::ZERO {
        return None;
    }
    let unit = Decimal::new(1, scale);
    let exact: Vec<Decimal> = values
        .iter()
        .map(|v| v.checked_div(groups))
        .collect::<Option<_>>()?;
    let mut parts: Vec<Decimal> = exact
        .iter()
        .map(|q| q.round_dp_with_strategy(scale, RoundingStrategy::ToNegativeInfinity))
        .collect();
    let sum: Decimal = parts.iter().copied().sum();
    let missing = (total.checked_sub(sum)? / unit).round();
    let missing = usize::try_from(i64::try_from(missing).ok()?).ok()?;
    if missing > parts.len() {
        return None;
    }
    // Phần dư lớn nhất nhận thêm một đơn vị trước; bằng nhau thì nhóm đứng trước.
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - parts[b]).cmp(&(exact[a] - parts[a])));
    for &i in order.iter().take(missing) {
        parts[i] += unit;
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    #[test]
    fn groups_add_up_to_the_averaged_total() {
        // Tổng lát gốc 10,00 trên 3 nhóm còn lại: trung bình 3,33; ba nhóm phân rã 3,33 + 3,34 + 3,33 không được ra
        // 9,99 hay 10,01.
        let total = build_average(d("10.00"), d("3"), 2).unwrap();
        assert_eq!(total, d("3.33"));
        let parts =
            build_group_averages(&[d("4.00"), d("3.00"), d("3.00")], d("3"), total, 2).unwrap();
        assert_eq!(parts.iter().copied().sum::<Decimal>(), total);
        assert_eq!(parts, [d("1.33"), d("1.00"), d("1.00")]);
    }

    #[test]
    fn negative_groups_keep_the_sum() {
        let total = build_average(d("-7.01"), d("4"), 2).unwrap();
        let parts = build_group_averages(&[d("-9.00"), d("1.99")], d("4"), total, 2).unwrap();
        assert_eq!(parts.iter().copied().sum::<Decimal>(), total);
    }

    #[test]
    fn counts_get_two_decimals() {
        assert_eq!(build_average_scale(d("7")), 2);
        assert_eq!(build_average(d("7"), d("2"), 2), Some(d("3.50")));
        assert_eq!(build_average(d("7"), Decimal::ZERO, 2), None);
    }
}
