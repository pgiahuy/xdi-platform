// SPDX-License-Identifier: Apache-2.0
/* service/explain/dimension_pick.rs - chọn chiều cho phân rã sau khi đã chạy câu từng chiều của mọi cột ứng viên
(explain/check.rs, build_dimensions): bỏ chiều không so được gì và chiều chia dữ liệu y hệt một chiều khác, trước khi
explain/breakdown.rs và explain/ratio_breakdown.rs chấm điểm rồi xếp các chiều còn lại, điểm cao trước.

Trước đây chỉ 8 cột đầu tiên theo thứ tự cột trong file được chạy: trên DataCo, cột mã Department Id (trùng
Department Name) và cờ 0/1 chiếm chỗ, còn Market, Order Region, Order Status, Shipping Mode nằm sau trong file nên không
có bảng khi so hai phân khúc. Giờ mọi ứng viên đều chạy; chiều qua bốn luật dưới đây không bị cắt theo số bảng, chỉ xếp theo điểm.

Bốn luật bỏ, không đọc tên cột (cộng luật restates_measure cho độ đo tỷ lệ, ratio_breakdown.rs gọi):
- Ít hơn hai nhóm: một nhóm gánh cả chênh lệch, bảng không giải thích gì (cột chỉ một giá trị, hay cột trùng với cột
  đang lọc bằng một giá trị).
- Một phía, chỉ khi so hai nhóm: không nhóm nào có dòng ở cả hai lát cắt. Bảng như vậy chỉ chia lại chính hai bên đang
  so, ví dụ Department Id khi đang so hai Department Name. Khi so hai kỳ thì giữ: nhóm chỉ có ở một kỳ là thay đổi thật
  (khách chuyển từ Consumer sang Corporate, sản phẩm mới, kênh ngừng bán).
- Đi theo hai nhóm, chỉ khi so hai nhóm: mỗi giá trị của cột đang so ứng với đúng một giá trị của cột chiều, như năm
  và tháng bắt đầu của một mã kỳ (2014J là năm 2014, tháng 10). Khi so 2014J với trung bình các mã kỳ còn lại, nhóm năm
  2014 gồm 2014J và 2014B nên có ở cả hai bên và luật một phía không bắt được, mà bảng chỉ chia lại các mã kỳ: nhóm
  "năm 2014" cao hơn chỉ vì 2014J nằm trong nó. Câu SQL đếm số cặp (giá trị cột đang so, giá trị cột chiều) và số giá
  trị cột đang so trên đúng tập dòng của câu hỏi; bằng nhau thì bỏ. Chỉ chạy câu này khi bên được so có không quá một
  nhóm có số, vì cột đi theo hai nhóm thì nhóm được so (một giá trị) chỉ ứng với một nhóm. So hai nhóm một với một thì
  luật ít hơn hai nhóm hay luật một phía đã bắt trước. So hai kỳ không cần luật này: cột như năm khi so hai năm có mọi
  nhóm chỉ ở một kỳ, explain/annotate.rs bỏ nó như chiều tách hẳn hai lát cắt.
- Trùng: hai chiều cùng số nhóm và cùng bộ số của các nhóm thì có thể đang chia y hệt nhau. Một câu SQL đếm số cặp giá
  trị khác nhau của hai cột trên đúng tập dòng của câu hỏi xác nhận: bằng số nhóm nghĩa là mỗi giá trị bên này ứng với
  đúng một giá trị bên kia. Giữ cột chữ, bỏ cột kia (tên dễ đọc hơn mã); cùng loại thì giữ cột đứng trước trong file.
  Cùng bộ số mà SQL không xác nhận (hay câu SQL lỗi) thì giữ cả hai: trùng số chỉ là tình cờ, bỏ nhầm là mất một bảng.

Chiều bỏ vì một phía, vì đi theo hai nhóm hay vì trùng được trả kèm (Picked.dropped) để breakdown.rs và
ratio_breakdown.rs thêm vào ghi chú của explain/annotate.rs: lời tóm tắt (explain/summary.rs) vẫn có ý "Bỏ qua <cột>"
kèm lý do, dù bảng của cột đó không còn trong tab Phân rã. Chiều ít hơn hai nhóm thì không nhắc: không có gì để giải thích. */

use rust_decimal::Decimal;

use crate::error::AppError;
use crate::service::ask::contract::{CellValue, ValueKind};
use crate::service::explain::annotate::{DropReason, DroppedDimension};
use crate::service::explain::breakdown::{
    read_decimal, run_code_query, CodeQuery, DimensionRows, QueryEnv,
};
use crate::service::explain::build_value_text;
use crate::service::explain::check::{PlanSlices, WhyPlan};
use crate::service::explain::sql::{build_follow_count_sql, build_pair_count_sql};

/// Một cột ứng viên đã chạy câu từng chiều.
pub(super) struct Candidate {
    pub(super) column: String,
    pub(super) rows: DimensionRows,
}

/// Ô số nào của một nhóm cho biết nhóm có dòng ở mỗi lát cắt: với độ đo cộng được là hai ô độ đo, với độ đo tỷ lệ là
/// hai ô mẫu số (tử số bằng 0 vẫn có thể là nhóm có dòng). Câu từng chiều cho 0 ở lát cắt không có dòng (explain/sql.rs,
/// COALESCE và COUNT).
#[derive(Debug, Clone, Copy)]
pub(super) struct Presence {
    pub(super) base: usize,
    pub(super) compare: usize,
}

/// Kết quả chọn chiều: ứng viên còn lại, và chiều bị bỏ mà lời tóm tắt nhắc tới.
pub(super) struct Picked {
    /// Theo thứ tự cột trong file; cột chữ thay cho một cột trùng thì đứng vào chỗ của cột đó. Thứ tự không quan trọng:
    /// bước chấm điểm xếp lại.
    pub(super) kept: Vec<Candidate>,
    pub(super) dropped: Vec<DroppedDimension>,
}

pub(super) async fn pick_dimensions(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    candidates: Vec<Candidate>,
    presence: Presence,
) -> Result<Picked, AppError> {
    let mut kept: Vec<Candidate> = Vec::new();
    let mut dropped: Vec<DroppedDimension> = Vec::new();
    let groups_compared = matches!(plan.slices, PlanSlices::Groups { .. });
    for candidate in candidates {
        if let Some(reason) = build_useless_reason(&candidate.rows, presence, groups_compared) {
            tracing::info!(column = %candidate.column, reason, "dimension dropped from the breakdown");
            if candidate.rows.groups.len() >= 2 {
                dropped.push(build_separated(plan, &candidate, presence));
            }
            continue;
        }
        if let Some(slice) = build_compared_column(plan) {
            if may_follow_slices(&candidate.rows, presence)
                && follows_slices(env, plan, slice, &candidate).await?
            {
                tracing::info!(column = %candidate.column, %slice, "dimension dropped from the breakdown: it follows the compared column");
                dropped.push(DroppedDimension {
                    label: plan.labels.get(&candidate.column).cloned(),
                    column: candidate.column,
                    reason: DropReason::FollowsSlices,
                    duplicate_of: Some(slice.to_string()),
                    base_only: Vec::new(),
                    compare_only: Vec::new(),
                });
                continue;
            }
        }
        let mut duplicate = None;
        for (index, other) in kept.iter().enumerate() {
            if has_same_numbers(&other.rows, &candidate.rows)
                && is_same_split(env, plan, other, &candidate).await?
            {
                duplicate = Some(index);
                break;
            }
        }
        let Some(index) = duplicate else {
            kept.push(candidate);
            continue;
        };
        let (keep, drop) = if prefers(candidate.rows.kind, kept[index].rows.kind) {
            (
                candidate.column.clone(),
                std::mem::replace(&mut kept[index], candidate).column,
            )
        } else {
            (kept[index].column.clone(), candidate.column)
        };
        tracing::info!(%keep, %drop, "dimension dropped from the breakdown: same groups as another column");
        // Cột vừa bị thay có thể là cột mà chiều bỏ trước đó trùng với: trỏ các chiều đó sang cột giữ lại.
        for earlier in dropped.iter_mut() {
            if earlier.duplicate_of.as_deref() == Some(drop.as_str()) {
                earlier.duplicate_of = Some(keep.clone());
            }
        }
        dropped.push(DroppedDimension {
            label: plan.labels.get(&drop).cloned(),
            column: drop,
            reason: DropReason::Duplicates,
            duplicate_of: Some(keep),
            base_only: Vec::new(),
            compare_only: Vec::new(),
        });
    }
    Ok(Picked { kept, dropped })
}

/// Ghi chú cho chiều mà không nhóm nào có dòng ở cả hai lát cắt: giá trị chỉ có ở lát gốc, chỉ có ở lát so sánh, như
/// explain/annotate.rs ghi cho chiều tách hẳn hai lát cắt.
fn build_separated(plan: &WhyPlan, candidate: &Candidate, presence: Presence) -> DroppedDimension {
    let values = |side: usize, other: usize| -> Vec<String> {
        candidate
            .rows
            .groups
            .iter()
            .filter(|(_, numbers)| !numbers[side].is_zero() && numbers[other].is_zero())
            .map(|(value, _)| build_group_text(value.as_ref()))
            .collect()
    };
    DroppedDimension {
        column: candidate.column.clone(),
        label: plan.labels.get(&candidate.column).cloned(),
        reason: DropReason::SeparatesSlices,
        duplicate_of: None,
        base_only: values(presence.base, presence.compare),
        compare_only: values(presence.compare, presence.base),
    }
}

// Nhóm null viết là "trống", như explain/annotate.rs.
fn build_group_text(value: Option<&CellValue>) -> String {
    value
        .map(build_value_text)
        .unwrap_or_else(|| "trống".to_string())
}

/// Lý do bỏ một chiều trước khi so với chiều khác; None khi chiều dùng được. groups_compared: hai lát cắt là hai nhóm
/// của một cột (PlanSlices::Groups), không phải hai kỳ.
pub(super) fn build_useless_reason(
    rows: &DimensionRows,
    presence: Presence,
    groups_compared: bool,
) -> Option<&'static str> {
    if rows.groups.len() < 2 {
        return Some("fewer than two groups");
    }
    if !groups_compared {
        return None;
    }
    let both_sides = rows.groups.iter().any(|(_, numbers)| {
        !numbers[presence.base].is_zero() && !numbers[presence.compare].is_zero()
    });
    (!both_sides).then_some("no group has rows in both slices")
}

/// Chiều chỉ nói lại độ đo tỷ lệ: ở cả hai lát cắt, mỗi nhóm hoặc không có dòng nào được đếm ở tử số, hoặc mọi dòng của
/// nhóm đều được đếm. "Tỷ lệ giao trễ" theo "tình trạng giao": nhóm Late delivery trễ 100%, các nhóm khác 0%, nên nhóm
/// đó gánh trọn chênh lệch mà không giải thích gì; trước đây chiều này đứng thứ hai và đẩy thị trường ra khỏi lời giải
/// thích. Bỏ, không nhắc trong lời (như chiều ít hơn hai nhóm). rows là câu từng chiều của độ đo tỷ lệ: tử base, tử
/// compare, mẫu base, mẫu compare (explain/sql.rs).
pub(super) fn restates_measure(rows: &DimensionRows, column: &str) -> bool {
    let all_or_nothing =
        |numerator: Decimal, denominator: Decimal| numerator.is_zero() || numerator == denominator;
    let restates = rows.groups.len() >= 2
        && rows.groups.iter().all(|(_, numbers)| {
            numbers.len() >= 4
                && all_or_nothing(numbers[0], numbers[2])
                && all_or_nothing(numbers[1], numbers[3])
        });
    if restates {
        tracing::info!(%column, "dimension dropped from the breakdown: it only restates the measure");
    }
    restates
}

/// Cùng số nhóm và cùng bộ số của các nhóm, không kể giá trị nhóm và thứ tự: điều kiện cần để hai chiều chia y hệt nhau.
pub(super) fn has_same_numbers(first: &DimensionRows, second: &DimensionRows) -> bool {
    first.groups.len() == second.groups.len() && sorted_numbers(first) == sorted_numbers(second)
}

/// Giữ chiều mới thay cho chiều cũ khi chiều mới là cột chữ còn chiều cũ thì không.
pub(super) fn prefers(new: ValueKind, old: ValueKind) -> bool {
    new == ValueKind::Text && old != ValueKind::Text
}

fn sorted_numbers(rows: &DimensionRows) -> Vec<&[Decimal]> {
    let mut numbers: Vec<&[Decimal]> = rows.groups.iter().map(|(_, n)| n.as_slice()).collect();
    numbers.sort();
    numbers
}

/// Cột của hai nhóm đang so; None khi so hai kỳ.
fn build_compared_column(plan: &WhyPlan) -> Option<&str> {
    match &plan.slices {
        PlanSlices::Groups { column, .. } => Some(column.as_str()),
        PlanSlices::Periods { .. } => None,
    }
}

/// Điều kiện cần để một chiều đi theo cột đang so: bên được so (một giá trị của cột đó) có số ở không quá một nhóm. Sai
/// thì không cần chạy câu đếm của follows_slices.
pub(super) fn may_follow_slices(rows: &DimensionRows, presence: Presence) -> bool {
    let compared = rows
        .groups
        .iter()
        .filter(|(_, numbers)| !numbers[presence.compare].is_zero())
        .count();
    compared <= 1
}

// Mỗi giá trị của cột đang so ứng với đúng một giá trị của cột chiều: số cặp bằng số giá trị của cột đang so. Câu lỗi
// thì giữ chiều, như is_same_split: bỏ nhầm là mất một bảng.
async fn follows_slices(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    slice: &str,
    candidate: &Candidate,
) -> Result<bool, AppError> {
    let sql = build_follow_count_sql(plan, slice, &candidate.column);
    let result = match run_code_query(env, &sql, None).await? {
        CodeQuery::Ran { result, .. } => result,
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(column = %candidate.column, %slice, %reason, "follow count query failed");
            return Ok(false);
        }
    };
    let Some(row) = result.rows.first() else {
        return Ok(false);
    };
    let cell = |i: usize| read_decimal(row.get(i).and_then(|c| c.as_ref()));
    Ok(
        matches!((cell(0), cell(1)), (Some(pairs), Some(slices)) if slices > Decimal::ZERO && pairs == slices),
    )
}

// Số cặp giá trị khác nhau của hai cột bằng số nhóm của mỗi cột thì mỗi giá trị bên này ứng với đúng một giá trị bên
// kia. Đếm trên cùng điều kiện với câu từng chiều, nên nhóm null cũng được tính như một giá trị.
async fn is_same_split(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    first: &Candidate,
    second: &Candidate,
) -> Result<bool, AppError> {
    let sql = build_pair_count_sql(plan, &first.column, &second.column);
    let result = match run_code_query(env, &sql, None).await? {
        CodeQuery::Ran { result, .. } => result,
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(first = %first.column, second = %second.column, %reason, "pair count query failed");
            return Ok(false);
        }
    };
    let pairs = result
        .rows
        .first()
        .and_then(|row| read_decimal(row.first().and_then(|c| c.as_ref())));
    Ok(pairs == Some(Decimal::from(first.rows.groups.len())))
}
