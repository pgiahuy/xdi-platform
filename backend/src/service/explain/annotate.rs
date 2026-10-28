// SPDX-License-Identifier: Apache-2.0
/* service/explain/annotate.rs - đọc bảng phân rã đã xếp (#186) để người dùng hiểu được: mỗi nhóm có phần trăm của
chênh lệch (% tác động) và phần trăm thay đổi trong nhóm (% thay đổi), cờ "khác thường"; bỏ các chiều không giải thích
được gì. breakdown.rs và ratio_breakdown.rs gọi sau khi xếp chiều; lời tóm tắt (explain/summary.rs) và giao diện đọc kết
quả. Hàm thuần trên số của hợp đồng (chuỗi chữ số), test ở explain/annotate_tests.rs.

Hai loại chiều bị bỏ:
- chiều tách hẳn hai lát cắt: mọi nhóm chỉ có ở một bên (đơn COMPLETE đều trả DEBIT, đơn CANCELED đều TRANSFER). Chiều
  đó chỉ là tên khác của hai lát đang so, nhưng Adtributor lại chấm nó giải thích tốt nhất vì nó tách hoàn hảo. Chỉ bỏ
  khi MỌI nhóm một bên: khi so hai kỳ, vài nhóm chỉ có ở một kỳ (bang mới có đơn) thường là nguyên nhân thật, nên giữ
  và gắn nhãn "mới", "không còn".
- chiều trùng nhóm với một chiều đã giữ (Department Id với Department Name): cùng các cặp giá trị hai lát cắt. Giữ chiều
  có giá trị chữ, vì "Fan Shop" dễ đọc hơn "7".

Luật "khác thường" theo gợi ý của nhóm: % thay đổi lệch % thay đổi chung từ 10 điểm phần trăm trở lên và % tác động từ
1% trở lên; nhóm chỉ có ở một bên và % tác động từ 1% cũng là khác thường. Chiều có nhóm khác thường xếp trước. */

use std::collections::HashMap;

use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::ask::contract::{
    DimensionBreakdown, DropReason, DroppedDimension, GroupPresence, ValueKind,
};
use crate::service::explain::build_value_text;

/// Lệch % thay đổi chung từ ngần này điểm phần trăm trở lên mới là khác thường.
const OUTLIER_POINTS: i64 = 10;
/// Nhóm chiếm dưới ngần này phần trăm chênh lệch thì không gọi là khác thường dù lệch bao nhiêu: nhóm nhỏ dao động
/// mạnh là chuyện thường.
const OUTLIER_MIN_SHARE: i64 = 1;
/// Chênh lệch tổng nhỏ hơn ngần này phần nghìn của lát cắt lớn hơn thì coi như gần 0: % tác động khi đó phình ra hàng
/// nghìn phần trăm và không còn nghĩa, nên không tính.
const NEAR_ZERO_PER_MILLE: i64 = 5;

/// Nhóm có ở cả hai lát cắt, chỉ ở lát so sánh (mới), hay chỉ ở lát gốc (không còn).
pub fn build_presence(base_empty: bool, compare_empty: bool) -> GroupPresence {
    match (base_empty, compare_empty) {
        (true, false) => GroupPresence::New,
        (false, true) => GroupPresence::Gone,
        _ => GroupPresence::Both,
    }
}

/// Tổng của hai lát cắt và chênh lệch, đã đọc thành Decimal.
#[derive(Debug, Clone, Copy)]
pub struct Totals {
    pub base: Decimal,
    pub compare: Decimal,
    pub change: Decimal,
}

/// Kết quả: chiều giữ lại (đã gắn số, đã xếp), chiều bỏ, và % thay đổi chung.
pub struct Annotated {
    pub dimensions: Vec<DimensionBreakdown>,
    pub dropped: Vec<DroppedDimension>,
    pub percent_change: Option<Decimal>,
}

/// Gắn nhãn tiếng Việt, % tác động, % thay đổi, cờ khác thường cho mọi nhóm; bỏ chiều tách hẳn hai lát cắt và chiều
/// trùng; xếp chiều có nhóm khác thường lên trước, giữ thứ tự xếp hạng trong từng phần.
pub fn annotate_dimensions(
    dimensions: Vec<DimensionBreakdown>,
    totals: Totals,
    labels: &HashMap<String, String>,
) -> Annotated {
    let percent_change = build_percent(totals.change, totals.base);
    let near_zero = is_near_zero(totals);
    let mut kept: Vec<DimensionBreakdown> = Vec::new();
    let mut dropped = Vec::new();
    for mut dimension in dimensions {
        dimension.label = labels.get(&dimension.column).cloned();
        if separates_slices(&dimension) {
            dropped.push(build_separated(&dimension));
            continue;
        }
        if let Some(i) = kept.iter().position(|k| same_groups(k, &dimension)) {
            // Giữ chiều có giá trị chữ: "Fan Shop" dễ đọc hơn "7". Cùng kiểu thì giữ chiều xếp trước.
            if is_number_kind(kept[i].kind) && !is_number_kind(dimension.kind) {
                let replaced = std::mem::replace(&mut kept[i], dimension);
                dropped.push(build_duplicate(&replaced, &kept[i]));
            } else {
                dropped.push(build_duplicate(&dimension, &kept[i]));
            }
            continue;
        }
        kept.push(dimension);
    }
    for dimension in &mut kept {
        apply_group_numbers(dimension, totals.change, percent_change, near_zero);
    }
    // sort_by_key ổn định: trong mỗi phần, chiều giữ thứ tự xếp hạng.
    kept.sort_by_key(|d| !d.has_outliers);
    Annotated {
        dimensions: kept,
        dropped,
        percent_change,
    }
}

fn apply_group_numbers(
    dimension: &mut DimensionBreakdown,
    change: Decimal,
    total_percent: Option<Decimal>,
    near_zero: bool,
) {
    let mut has_outliers = false;
    for group in &mut dimension.groups {
        let contribution = parse(&group.contribution);
        let share = if near_zero {
            None
        } else {
            contribution.and_then(|c| build_share(c, change))
        };
        let percent = match group.presence {
            GroupPresence::Both => match (parse(&group.compare_value), parse(&group.base_value)) {
                (Some(compare), Some(base)) => compare
                    .checked_sub(base)
                    .and_then(|diff| build_percent(diff, base)),
                _ => None,
            },
            GroupPresence::New | GroupPresence::Gone => None,
        };
        let big_enough = share.is_some_and(|s| s.abs() >= Decimal::from(OUTLIER_MIN_SHARE));
        let off_the_whole = match (percent, total_percent) {
            (Some(p), Some(t)) => (p - t).abs() >= Decimal::from(OUTLIER_POINTS),
            _ => false,
        };
        let one_sided = group.presence != GroupPresence::Both;
        group.outlier = big_enough && (off_the_whole || one_sided);
        group.share_of_change = share.map(build_percent_text);
        group.percent_change = percent.map(build_percent_text);
        has_outliers |= group.outlier;
    }
    dimension.has_outliers = has_outliers;
}

/// `part / change * 100`: phần của chênh lệch, đúng cả khi chênh lệch âm (nhóm cùng chiều luôn ra số dương). None khi
/// chênh lệch bằng 0 hay tràn.
fn build_share(part: Decimal, change: Decimal) -> Option<Decimal> {
    if change.is_zero() {
        return None;
    }
    part.checked_mul(Decimal::ONE_HUNDRED)?.checked_div(change)
}

/// `part / whole * 100`; None khi whole không dương (phần trăm của số âm hay 0 không có nghĩa thông thường) hay tràn.
fn build_percent(part: Decimal, whole: Decimal) -> Option<Decimal> {
    if whole <= Decimal::ZERO {
        return None;
    }
    part.checked_mul(Decimal::ONE_HUNDRED)?.checked_div(whole)
}

/// Phần trăm làm tròn một chữ số lẻ, bỏ số 0 thừa: "41.5", "-44", không bao giờ "-0".
pub fn build_percent_text(value: Decimal) -> String {
    let rounded = value
        .round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    if rounded.is_zero() {
        "0".to_string()
    } else {
        rounded.to_string()
    }
}

fn is_near_zero(totals: Totals) -> bool {
    let scale = totals.base.abs().max(totals.compare.abs());
    let limit = scale * Decimal::from(NEAR_ZERO_PER_MILLE) / Decimal::from(1000);
    totals.change.abs() <= limit
}

// Mọi nhóm chỉ có ở một bên, và có nhóm ở cả hai bên: chiều chỉ là tên khác của hai lát cắt. Mọi nhóm cùng một bên thì
// một lát cắt rỗng, không phải chuyện của chiều này.
fn separates_slices(dimension: &DimensionBreakdown) -> bool {
    let presences: Vec<GroupPresence> = dimension.groups.iter().map(|g| g.presence).collect();
    presences.iter().all(|p| *p != GroupPresence::Both)
        && presences.contains(&GroupPresence::New)
        && presences.contains(&GroupPresence::Gone)
}

// Cùng các cặp (giá trị lát gốc, giá trị lát so sánh) theo một thứ tự: hai chiều chia dòng thành đúng các nhóm như
// nhau. Cần ít nhất hai nhóm, vì một nhóm thì chiều nào cũng "trùng" (cả bảng).
fn same_groups(a: &DimensionBreakdown, b: &DimensionBreakdown) -> bool {
    if a.groups.len() < 2 || a.groups.len() != b.groups.len() {
        return false;
    }
    let pairs = |d: &DimensionBreakdown| {
        let mut pairs: Vec<(Option<Decimal>, Option<Decimal>)> = d
            .groups
            .iter()
            .map(|g| (parse(&g.base_value), parse(&g.compare_value)))
            .collect();
        pairs.sort();
        pairs
    };
    pairs(a) == pairs(b)
}

fn is_number_kind(kind: ValueKind) -> bool {
    matches!(kind, ValueKind::Number | ValueKind::Decimal)
}

fn build_separated(dimension: &DimensionBreakdown) -> DroppedDimension {
    let values = |presence: GroupPresence| {
        dimension
            .groups
            .iter()
            .filter(|g| g.presence == presence)
            .map(|g| {
                g.value
                    .as_ref()
                    .map(build_value_text)
                    .unwrap_or_else(|| "trống".to_string())
            })
            .collect()
    };
    DroppedDimension {
        column: dimension.column.clone(),
        label: dimension.label.clone(),
        reason: DropReason::SeparatesSlices,
        duplicate_of: None,
        base_only: values(GroupPresence::Gone),
        compare_only: values(GroupPresence::New),
    }
}

fn build_duplicate(dropped: &DimensionBreakdown, kept: &DimensionBreakdown) -> DroppedDimension {
    DroppedDimension {
        column: dropped.column.clone(),
        label: dropped.label.clone(),
        reason: DropReason::Duplicates,
        duplicate_of: Some(kept.column.clone()),
        base_only: Vec::new(),
        compare_only: Vec::new(),
    }
}

fn parse(text: &str) -> Option<Decimal> {
    text.parse().ok()
}
