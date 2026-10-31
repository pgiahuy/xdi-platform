// SPDX-License-Identifier: Apache-2.0
/* service/explain/ratio.rs - phần toán của câu "tại sao" cho độ đo tỷ lệ (AVG(x), SUM(a) / SUM(b), xem
explain/measure.rs): tách chênh lệch của tỷ lệ thành phần do cơ cấu và phần do hiệu suất của từng nhóm. Hàm thuần như
phần toán ở explain.rs: đầu vào là tử và mẫu đã đọc từ DuckDB, đầu ra là GroupContribution của hợp đồng và các số để
xếp chiều. explain/breakdown.rs gọi; test ở explain/ratio_tests.rs.

Tỷ lệ không cộng được theo nhóm: tỷ lệ giao trễ của cả quý không bằng tổng tỷ lệ của các thị trường, nên phép trừ từng
nhóm của #39 không dùng được. Với R = N / D (tử N, mẫu D), nhóm i có tỷ trọng w_i = D_i / D và tỷ lệ r_i = N_i / D_i,
nên R = tổng w_i * r_i. Chênh lệch giữa hai lát cắt (0 là base, 1 là compare) tách đối xứng theo Kitagawa (1955), cùng
cách chia đôi phần tương tác như Shapley hai nhân tố của #40:
  phần cơ cấu của nhóm i     = (w1_i - w0_i) * (r0_i + r1_i) / 2
  phần hiệu suất của nhóm i  = (w0_i + w1_i) / 2 * (r1_i - r0_i)
Cộng hai phần của mọi nhóm ra đúng R1 - R0 (khai triển hai tích, các số hạng chéo triệt tiêu). Phần cơ cấu khác 0 khi
mẫu dồn sang nhóm khác (nhiều đơn hơn đi đường thủy), phần hiệu suất khác 0 khi tỷ lệ trong nhóm đổi. Vì vậy mọi nhóm
có thể cùng giảm tỷ lệ mà tổng vẫn tăng, khi mẫu dồn sang nhóm có tỷ lệ cao (nghịch lý Simpson): phần cơ cấu dương,
phần hiệu suất âm.

Nhóm chỉ có ở một lát cắt (mẫu bằng 0 ở lát kia) không có tỷ lệ ở lát đó. Quy ước: lấy tỷ lệ của lát còn lại. Tỷ trọng
ở lát thiếu là 0 nên tổng vẫn đúng với mọi giá trị chọn, và quy ước này cho phần hiệu suất bằng 0: nhóm mới xuất hiện
hay biến mất chỉ đổi cơ cấu. Nhóm có tử khác 0 mà mẫu bằng 0 (SUM(a) / SUM(b) với b trống ở nhóm đó) làm tổng không
còn đúng, nên cả chiều bị bỏ như khi các nhóm không cộng ra tổng.

Làm tròn: tỷ lệ là thương nên phải làm tròn; mọi số hiện ra giữ RATE_SCALE chữ số thập phân. Hai tỷ lệ của cả lát cắt
làm tròn trước, chênh lệch lấy bằng hiệu hai số đã làm tròn (để ba số lớn trên giao diện tự khớp nhau). Từng phần cơ
cấu và hiệu suất làm tròn riêng; tổng của chúng có thể lệch chênh lệch vài đơn vị ở chữ số cuối, phần lệch chia cho các
phần có phần dư làm tròn lớn nhất, mỗi phần nhận tối đa một đơn vị (cách chia phần dư lớn nhất). Như vậy tổng khớp đúng
chênh lệch và mỗi số hiện ra lệch số đúng không quá một đơn vị ở chữ số cuối, thay vì dồn cả phần lệch vào một nhóm. */

use std::cmp::Reverse;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::ask::contract::{CellValue, GroupContribution};
use crate::service::explain::annotate::{build_presence, GroupPresence};
use crate::service::explain::{build_decimal_text, compare_group_values, GroupValues, ScoreError};

/// Số chữ số thập phân của mọi tỷ lệ và mọi phần đóng góp: đủ đọc tỷ lệ phần trăm tới hai chữ số lẻ (0.0123 là
/// 1,23%), và đủ cho trung bình kiểu tiền.
pub const RATE_SCALE: u32 = 4;

/// Tử và mẫu của một lát cắt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatioParts {
    pub numerator: Decimal,
    pub denominator: Decimal,
}

/// Một nhóm của một chiều, như câu từng chiều trả về: tử và mẫu của hai lát cắt.
#[derive(Debug, Clone)]
pub struct RatioGroup {
    /// None cho nhóm các dòng có cột chiều là null.
    pub value: Option<CellValue>,
    pub base: RatioParts,
    pub compare: RatioParts,
}

/// Tỷ lệ hai lát cắt và chênh lệch, đã làm tròn RATE_SCALE chữ số.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatioChange {
    pub base_rate: Decimal,
    pub compare_rate: Decimal,
    pub change: Decimal,
}

/// Phân rã của một chiều.
#[derive(Debug, Clone)]
pub struct RatioSplit {
    /// Nhóm đóng góp lớn nhất (theo trị tuyệt đối) trước, như build_group_contributions.
    pub groups: Vec<GroupContribution>,
    /// Tổng phần cơ cấu và tổng phần hiệu suất của mọi nhóm, cộng lại bằng đúng chênh lệch.
    pub mix_effect: Decimal,
    pub rate_effect: Decimal,
    /// Đóng góp của từng nhóm dạng GroupValues (base 0, compare là đóng góp), để xếp chiều bằng
    /// build_dimension_score theo cách Effect với tổng base 0 và tổng compare là chênh lệch.
    pub scored: Vec<GroupValues>,
    /// Nhóm có ở lát cắt nào, thẳng hàng với `groups`, cho lời tóm tắt (explain/annotate.rs).
    pub presences: Vec<GroupPresence>,
}

/// Tỷ lệ hai lát cắt. None khi một mẫu bằng 0 (lát cắt không có tỷ lệ) hay phép chia tràn.
pub fn build_ratio_change(base: RatioParts, compare: RatioParts) -> Option<RatioChange> {
    let base_rate = build_rounded(build_rate(base)?);
    let compare_rate = build_rounded(build_rate(compare)?);
    Some(RatioChange {
        base_rate,
        compare_rate,
        change: compare_rate.checked_sub(base_rate)?,
    })
}

fn build_rate(parts: RatioParts) -> Option<Decimal> {
    parts.numerator.checked_div(parts.denominator)
}

// Nửa xa số 0, như giá của #40: cách làm tròn người dùng quen.
fn build_rounded(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(RATE_SCALE, RoundingStrategy::MidpointAwayFromZero)
}

// Số đúng (28 chữ số) của một nhóm, trước khi làm tròn.
struct ExactGroup<'a> {
    group: &'a RatioGroup,
    base_rate: Decimal,
    compare_rate: Decimal,
    mix: Decimal,
    rate: Decimal,
}

/// Phân rã một chiều. `totals` là tử và mẫu của cả hai lát cắt; các nhóm phải cộng ra đúng hai tổng này.
/// Err(TotalsMismatch) khi các nhóm không cộng ra tổng, một nhóm có tử mà không có mẫu, một mẫu tổng bằng 0, hay phép
/// tính tràn; Err(NoChange) khi chênh lệch đã làm tròn bằng 0.
pub fn build_ratio_split(
    groups: &[RatioGroup],
    base: RatioParts,
    compare: RatioParts,
) -> Result<RatioSplit, ScoreError> {
    let mismatch = ScoreError::TotalsMismatch;
    if !parts_add_up(groups.iter().map(|g| g.base), base)
        || !parts_add_up(groups.iter().map(|g| g.compare), compare)
    {
        return Err(mismatch);
    }
    let totals = build_ratio_change(base, compare).ok_or(mismatch)?;
    if totals.change.is_zero() {
        return Err(ScoreError::NoChange);
    }
    let exact: Option<Vec<ExactGroup>> = groups
        .iter()
        .map(|group| build_exact_group(group, base.denominator, compare.denominator))
        .collect();
    let mut exact = exact.ok_or(mismatch)?;
    check_exact_sum(&exact, base, compare)?;
    exact.sort_by(|a, b| {
        let size = |g: &ExactGroup| (g.mix + g.rate).abs();
        size(b)
            .cmp(&size(a))
            .then_with(|| compare_group_values(&a.group.value, &b.group.value))
    });
    // Hai phần của nhóm i nằm ở vị trí 2i và 2i + 1.
    let parts: Vec<Decimal> = exact.iter().flat_map(|g| [g.mix, g.rate]).collect();
    let rounded = build_rounded_parts(&parts, totals.change).ok_or(mismatch)?;
    let mut split = RatioSplit {
        groups: Vec::with_capacity(exact.len()),
        mix_effect: Decimal::ZERO,
        rate_effect: Decimal::ZERO,
        scored: Vec::with_capacity(exact.len()),
        presences: Vec::with_capacity(exact.len()),
    };
    for (i, group) in exact.iter().enumerate() {
        let (mix, rate) = (rounded[2 * i], rounded[2 * i + 1]);
        let contribution = mix + rate;
        split.mix_effect += mix;
        split.rate_effect += rate;
        split.groups.push(GroupContribution {
            value: group.group.value.clone(),
            base_value: build_rate_text(build_rounded(group.base_rate)),
            compare_value: build_rate_text(build_rounded(group.compare_rate)),
            contribution: build_rate_text(contribution),
            mix_effect: Some(build_rate_text(mix)),
            rate_effect: Some(build_rate_text(rate)),
            ..Default::default()
        });
        split.scored.push(GroupValues {
            value: group.group.value.clone(),
            base_value: Decimal::ZERO,
            compare_value: contribution,
        });
        // Không có mẫu số ở một lát cắt nghĩa là nhóm không có dòng nào ở lát đó.
        split.presences.push(build_presence(
            group.group.base.denominator.is_zero(),
            group.group.compare.denominator.is_zero(),
        ));
    }
    Ok(split)
}

/// Một số tỷ lệ in thành chuỗi của hợp đồng, luôn đủ RATE_SCALE chữ số.
pub fn build_rate_text(value: Decimal) -> String {
    build_decimal_text(value, RATE_SCALE)
}

// Tử và mẫu của các nhóm cộng ra đúng tổng của lát cắt (cộng bằng Decimal nên so được bằng ==).
fn parts_add_up(mut parts: impl Iterator<Item = RatioParts>, total: RatioParts) -> bool {
    let sum = parts.try_fold((Decimal::ZERO, Decimal::ZERO), |(n, d), part| {
        Some((
            n.checked_add(part.numerator)?,
            d.checked_add(part.denominator)?,
        ))
    });
    sum == Some((total.numerator, total.denominator))
}

// Tỷ trọng, tỷ lệ và hai phần của một nhóm. None khi nhóm có tử mà không có mẫu ở một lát cắt, hay phép tính tràn.
fn build_exact_group(
    group: &RatioGroup,
    base_total: Decimal,
    compare_total: Decimal,
) -> Option<ExactGroup<'_>> {
    let base_weight = group.base.denominator.checked_div(base_total)?;
    let compare_weight = group.compare.denominator.checked_div(compare_total)?;
    let (base_rate, compare_rate) = match (
        build_group_rate(group.base)?,
        build_group_rate(group.compare)?,
    ) {
        (Some(base), Some(compare)) => (base, compare),
        // Nhóm chỉ có ở một lát cắt: tỷ lệ của lát thiếu lấy bằng lát còn lại (quy ước ở đầu file).
        (Some(base), None) => (base, base),
        (None, Some(compare)) => (compare, compare),
        (None, None) => (Decimal::ZERO, Decimal::ZERO),
    };
    let two = Decimal::TWO;
    let mix = compare_weight
        .checked_sub(base_weight)?
        .checked_mul(base_rate.checked_add(compare_rate)?)?
        .checked_div(two)?;
    let rate = base_weight
        .checked_add(compare_weight)?
        .checked_div(two)?
        .checked_mul(compare_rate.checked_sub(base_rate)?)?;
    Some(ExactGroup {
        group,
        base_rate,
        compare_rate,
        mix,
        rate,
    })
}

// Some(None) khi nhóm không có ở lát cắt (mẫu 0, tử 0); None khi có tử mà mẫu bằng 0: tử đó không nằm trong tổng nào
// của phân rã, cả chiều phải bỏ.
fn build_group_rate(parts: RatioParts) -> Option<Option<Decimal>> {
    if parts.denominator.is_zero() {
        return parts.numerator.is_zero().then_some(None);
    }
    build_rate(parts).map(Some)
}

// Bất biến: tổng hai phần của mọi nhóm bằng R1 - R0 tính đúng. Tỷ trọng và tỷ lệ là thương nên Decimal làm tròn ở chữ
// số thứ 28; sai số cho phép là một phần tỷ của đơn vị cuối RATE_SCALE, nhỏ hơn mọi chữ số hiện ra. Lệch hơn thì có lỗi
// trong dữ liệu đọc vào hay trong phép tính, bỏ chiều thay vì hiện số sai.
fn check_exact_sum(
    exact: &[ExactGroup],
    base: RatioParts,
    compare: RatioParts,
) -> Result<(), ScoreError> {
    let mismatch = ScoreError::TotalsMismatch;
    let change = build_rate(compare)
        .zip(build_rate(base))
        .and_then(|(c, b)| c.checked_sub(b))
        .ok_or(mismatch)?;
    let sum = exact
        .iter()
        .try_fold(Decimal::ZERO, |total, g| {
            total.checked_add(g.mix)?.checked_add(g.rate)
        })
        .ok_or(mismatch)?;
    let tolerance = Decimal::new(1, RATE_SCALE + 9);
    if sum.checked_sub(change).ok_or(mismatch)?.abs() > tolerance {
        return Err(mismatch);
    }
    Ok(())
}

/// Làm tròn từng phần RATE_SCALE chữ số rồi chia phần lệch so với `target` theo cách phần dư lớn nhất: phần lệch tính
/// bằng đơn vị cuối, mỗi đơn vị cộng (hay trừ) vào phần có số bị làm tròn đi nhiều nhất theo đúng chiều đó, mỗi phần
/// tối đa một đơn vị. None khi phần lệch nhiều hơn số phần (không xảy ra khi bất biến đúng, vì mỗi phần lệch tối đa nửa
/// đơn vị và target lệch tổng đúng tối đa một đơn vị).
pub fn build_rounded_parts(parts: &[Decimal], target: Decimal) -> Option<Vec<Decimal>> {
    let mut rounded: Vec<Decimal> = parts.iter().map(|p| build_rounded(*p)).collect();
    let unit = Decimal::new(1, RATE_SCALE);
    let sum = rounded
        .iter()
        .try_fold(Decimal::ZERO, |total, p| total.checked_add(*p))?;
    let missing = target.checked_sub(sum)?.checked_div(unit)?;
    let steps = missing.abs().trunc();
    if steps != missing.abs() || steps > Decimal::from(parts.len()) {
        return None;
    }
    let up = missing > Decimal::ZERO;
    let step = if up { unit } else { -unit };
    // Phần dư theo chiều cần bù: thiếu thì ưu tiên phần bị làm tròn xuống nhiều nhất (số đúng trừ số đã làm tròn lớn
    // nhất), thừa thì ngược lại. sort ổn định nên phần dư bằng nhau giữ thứ tự nhóm, nhóm lớn trước.
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&i| {
        let remainder = parts[i] - rounded[i];
        Reverse(if up { remainder } else { -remainder })
    });
    for &i in order.iter().take(steps.to_usize()?) {
        rounded[i] += step;
    }
    Some(rounded)
}
