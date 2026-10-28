// SPDX-License-Identifier: Apache-2.0
/* service/explain/annotate.rs - đọc bảng phân rã đã xếp (#186) cho lời tóm tắt do code viết: phần trăm của chênh lệch
(% tác động) và phần trăm thay đổi trong nhóm (% thay đổi), cờ "khác thường", các chiều không giải thích được gì. Kết quả
là ghi chú nội bộ (BreakdownNotes, trường `notes` của Explanation, không gửi qua API): annotate không bỏ chiều nào khỏi
bảng phân rã trả cho giao diện, chỉ explain/summary.rs và prompt narrate đọc ghi chú. breakdown.rs và
ratio_breakdown.rs gọi sau khi xếp chiều, rồi thêm vào `dropped` các chiều explain/dimension_pick.rs đã bỏ khỏi bảng
trước đó (trùng một chiều khác, có câu SQL xác nhận; hay tách hẳn hai nhóm đang so): các chiều đó annotate không thấy
nữa, còn ở đây là phần chiều trùng hay tách hẳn mà dimension_pick.rs chưa bỏ (khi so hai kỳ, hay trùng số mà SQL
không xác nhận). Hàm thuần trên số của hợp đồng (chuỗi chữ số), test ở
explain/annotate_tests.rs.

Hai loại chiều lời tóm tắt bỏ qua:
- chiều tách hẳn hai lát cắt: mọi nhóm chỉ có ở một bên (đơn COMPLETE đều trả DEBIT, đơn CANCELED đều TRANSFER). Chiều
  đó chỉ là tên khác của hai lát đang so, nhưng Adtributor lại chấm nó giải thích tốt nhất vì nó tách hoàn hảo. Chỉ bỏ
  khi MỌI nhóm một bên: khi so hai kỳ, vài nhóm chỉ có ở một kỳ (bang mới có đơn) thường là nguyên nhân thật, nên giữ
  và gắn nhãn "mới", "không còn".
- chiều trùng nhóm với một chiều đã giữ (Department Id với Department Name): cùng các cặp giá trị hai lát cắt. Giữ chiều
  có giá trị chữ, vì "Fan Shop" dễ đọc hơn "7".

Luật "khác thường" theo gợi ý của nhóm: % thay đổi lệch % thay đổi chung từ 10 điểm phần trăm trở lên và % tác động từ
1% trở lên; nhóm chỉ có ở một bên và % tác động từ 1% cũng là khác thường. Lời tóm tắt đọc chiều có nhóm khác thường
trước. */

use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::ask::contract::{DimensionBreakdown, ValueKind};
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GroupPresence {
    #[default]
    Both,
    New,
    Gone,
}

/// Ghi chú của cả bảng phân rã. Rỗng (Default) khi không có phân rã để đọc, như Explanation dựng tay trong test.
#[derive(Debug, Clone, Default)]
pub struct BreakdownNotes {
    /// Chiều lời tóm tắt dùng, chiều có nhóm khác thường trước, trong mỗi phần giữ thứ tự xếp hạng.
    pub dimensions: Vec<DimensionNotes>,
    /// Chiều lời tóm tắt bỏ qua, kèm lý do.
    pub dropped: Vec<DroppedDimension>,
}

/// Ghi chú của một chiều: `index` trỏ vào Explanation.dimensions, `groups` thẳng hàng với các nhóm của chiều đó.
#[derive(Debug, Clone)]
pub struct DimensionNotes {
    pub index: usize,
    pub has_outliers: bool,
    pub groups: Vec<GroupNotes>,
}

#[derive(Debug, Clone)]
pub struct GroupNotes {
    pub presence: GroupPresence,
    /// % tác động, chuỗi một chữ số lẻ ("41.5"); âm khi nhóm đi ngược chiều; None khi chênh lệch gần 0.
    pub share_of_change: Option<String>,
    /// % thay đổi trong nhóm; None khi nhóm không có ở cả hai bên hay lát gốc không dương.
    pub percent_change: Option<String>,
    pub outlier: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// Mọi giá trị của cột chỉ có ở một bên: tên khác của hai lát cắt.
    SeparatesSlices,
    /// Cột chia nhóm giống hệt một chiều đã giữ.
    Duplicates,
}

#[derive(Debug, Clone)]
pub struct DroppedDimension {
    pub column: String,
    pub label: Option<String>,
    pub reason: DropReason,
    pub duplicate_of: Option<String>,
    /// Giá trị chỉ có ở lát gốc, dạng chữ.
    pub base_only: Vec<String>,
    /// Giá trị chỉ có ở lát so sánh, dạng chữ.
    pub compare_only: Vec<String>,
}

/// Tổng của hai lát cắt và chênh lệch, đã đọc thành Decimal.
#[derive(Debug, Clone, Copy)]
pub struct Totals {
    pub base: Decimal,
    pub compare: Decimal,
    pub change: Decimal,
}

/// Nhóm có ở cả hai lát cắt, chỉ ở lát so sánh (mới), hay chỉ ở lát gốc (không còn).
pub fn build_presence(base_empty: bool, compare_empty: bool) -> GroupPresence {
    match (base_empty, compare_empty) {
        (true, false) => GroupPresence::New,
        (false, true) => GroupPresence::Gone,
        _ => GroupPresence::Both,
    }
}

/// Độ đo cộng được: giá trị 0 ở một lát cắt coi như nhóm không có ở lát đó (tổng 0 mà có dòng, như lãi lỗ bù trừ
/// nhau, hiếm và cũng không đổi cách đọc). Độ đo tỷ lệ thì ratio_breakdown.rs xét mẫu số.
pub fn build_value_presences(dimension: &DimensionBreakdown) -> Vec<GroupPresence> {
    dimension
        .groups
        .iter()
        .map(|g| build_presence(is_zero(&g.base_value), is_zero(&g.compare_value)))
        .collect()
}

/// % thay đổi chung `change / base * 100`; None khi lát gốc không dương.
pub fn build_percent_change(totals: Totals) -> Option<Decimal> {
    build_percent(totals.change, totals.base)
}

/// Ghi chú cho lời tóm tắt. `presences[i]` thẳng hàng với các nhóm của `dimensions[i]`. Không đổi `dimensions`.
pub fn annotate_dimensions(
    dimensions: &[DimensionBreakdown],
    presences: &[Vec<GroupPresence>],
    totals: Totals,
) -> BreakdownNotes {
    let total_percent = build_percent_change(totals);
    let near_zero = is_near_zero(totals);
    let mut kept: Vec<DimensionNotes> = Vec::new();
    let mut dropped = Vec::new();
    for (index, dimension) in dimensions.iter().enumerate() {
        let presence = &presences[index];
        if separates_slices(presence) {
            dropped.push(build_separated(dimension, presence));
            continue;
        }
        let duplicate = kept
            .iter()
            .position(|k| same_groups(&dimensions[k.index], dimension));
        if let Some(i) = duplicate {
            let earlier = &dimensions[kept[i].index];
            // Giữ chiều có giá trị chữ: "Fan Shop" dễ đọc hơn "7". Cùng kiểu thì giữ chiều xếp trước.
            if is_number_kind(earlier.kind) && !is_number_kind(dimension.kind) {
                dropped.push(build_duplicate(earlier, dimension));
                kept[i] = build_notes(
                    index,
                    dimension,
                    presence,
                    totals.change,
                    total_percent,
                    near_zero,
                );
            } else {
                dropped.push(build_duplicate(dimension, earlier));
            }
            continue;
        }
        kept.push(build_notes(
            index,
            dimension,
            presence,
            totals.change,
            total_percent,
            near_zero,
        ));
    }
    // sort_by_key ổn định: trong mỗi phần, chiều giữ thứ tự xếp hạng.
    kept.sort_by_key(|d| !d.has_outliers);
    BreakdownNotes {
        dimensions: kept,
        dropped,
    }
}

fn build_notes(
    index: usize,
    dimension: &DimensionBreakdown,
    presences: &[GroupPresence],
    change: Decimal,
    total_percent: Option<Decimal>,
    near_zero: bool,
) -> DimensionNotes {
    let groups: Vec<GroupNotes> = dimension
        .groups
        .iter()
        .zip(presences)
        .map(|(group, &presence)| {
            let share = if near_zero {
                None
            } else {
                parse(&group.contribution).and_then(|c| build_share(c, change))
            };
            let percent = match presence {
                GroupPresence::Both => {
                    match (parse(&group.compare_value), parse(&group.base_value)) {
                        (Some(compare), Some(base)) => compare
                            .checked_sub(base)
                            .and_then(|diff| build_percent(diff, base)),
                        _ => None,
                    }
                }
                GroupPresence::New | GroupPresence::Gone => None,
            };
            let big_enough = share.is_some_and(|s| s.abs() >= Decimal::from(OUTLIER_MIN_SHARE));
            let off_the_whole = match (percent, total_percent) {
                (Some(p), Some(t)) => (p - t).abs() >= Decimal::from(OUTLIER_POINTS),
                _ => false,
            };
            let one_sided = presence != GroupPresence::Both;
            GroupNotes {
                presence,
                share_of_change: share.map(build_percent_text),
                percent_change: percent.map(build_percent_text),
                outlier: big_enough && (off_the_whole || one_sided),
            }
        })
        .collect();
    DimensionNotes {
        index,
        has_outliers: groups.iter().any(|g| g.outlier),
        groups,
    }
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
fn separates_slices(presences: &[GroupPresence]) -> bool {
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

fn build_separated(
    dimension: &DimensionBreakdown,
    presences: &[GroupPresence],
) -> DroppedDimension {
    let values = |wanted: GroupPresence| {
        dimension
            .groups
            .iter()
            .zip(presences)
            .filter(|(_, &p)| p == wanted)
            .map(|(g, _)| {
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

fn is_zero(text: &str) -> bool {
    parse(text).is_some_and(|v| v.is_zero())
}

fn parse(text: &str) -> Option<Decimal> {
    text.parse().ok()
}
