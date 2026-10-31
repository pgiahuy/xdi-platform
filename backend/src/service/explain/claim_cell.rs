// SPDX-License-Identifier: Apache-2.0
/* service/explain/claim_cell.rs - phần của cổng 1 (explain/claims.rs, #201) xét từng khẳng định: tìm ô của bảng phân rã
mà khẳng định trỏ vào (tổng, một nhóm hay một chiều, lượng và giá, phần trăm trong ghi chú của annotate.rs), so giá trị
chép, kể cả dấu, với ô đó, và xét nhóm được nêu là nguyên nhân chính hay kéo lại. Hàm thuần, test qua
explain/claims_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::Explanation;
use crate::service::explain::annotate::GroupPresence;
use crate::service::explain::claims::{Claim, ClaimField};
use crate::service::explain::number::build_number_text_with;
use crate::service::explain::split::build_split;
use crate::service::explain::{
    build_fraction_digits, build_value_text, same_direction, NARRATE_GROUPS,
};

pub(super) fn check_claim(claim: &Claim, explanation: &Explanation) -> Result<(), String> {
    if matches!(claim.field, ClaimField::Cause | ClaimField::Against) {
        return check_named_group(claim, explanation);
    }
    if matches!(claim.field, ClaimField::New | ClaimField::Gone) {
        return check_presence(claim, explanation);
    }
    let label = build_claim_label(claim);
    let truth = build_truth(claim, explanation)?;
    // Dấu cộng đứng trước số dương không đổi nghĩa, nên bỏ; dấu trừ thì giữ vì nó là chiều.
    let copied = claim
        .value
        .trim()
        .trim_start_matches('+')
        .trim_end_matches('%')
        .trim();
    if copied != truth {
        return Err(format!(
            "{label}: giá trị chép là {copied}, bảng ghi {truth}"
        ));
    }
    Ok(())
}

// Tên khẳng định trong lý do trượt, để mô hình biết sửa mục nào: "contribution của Europe (Market)".
fn build_claim_label(claim: &Claim) -> String {
    let field = claim.field.name();
    let column = claim.column.trim();
    let group = claim.group.trim();
    match (column.is_empty(), group.is_empty()) {
        (_, false) => format!("{field} của {group} ({column})"),
        (false, true) => format!("{field} của cột {column}"),
        (true, true) => field.to_string(),
    }
}

// Nguyên nhân chính phải cùng chiều chênh lệch và nằm trong NARRATE_GROUPS nhóm đầu của cột (nhóm đã xếp theo độ lớn
// đóng góp), như ba nhóm prompt gửi; nhóm kéo lại phải ngược chiều. Chênh lệch không đọc được thì không xét được chiều,
// nên khẳng định trượt.
fn check_named_group(claim: &Claim, explanation: &Explanation) -> Result<(), String> {
    let label = build_claim_label(claim);
    let (d, g) = find_group(explanation, &claim.column, &claim.group).ok_or_else(|| {
        format!(
            "{label}: không có nhóm {} trong cột {}",
            claim.group, claim.column
        )
    })?;
    let contribution = &explanation.dimensions[d].groups[g].contribution;
    let change = explanation
        .change
        .parse::<Decimal>()
        .map_err(|_| format!("{label}: chênh lệch của bảng không đọc được"))?;
    let is_zero = contribution
        .parse::<Decimal>()
        .map(|c| c.is_zero())
        .unwrap_or(true);
    let along = !is_zero && same_direction(contribution, change);
    match claim.field {
        ClaimField::Cause if !along => Err(format!(
            "{label}: nhóm {} không đi cùng chiều chênh lệch nên không phải nguyên nhân chính",
            claim.group
        )),
        ClaimField::Cause if g >= NARRATE_GROUPS => Err(format!(
            "{label}: nhóm {} không nằm trong {NARRATE_GROUPS} nhóm đóng góp lớn nhất của cột {}",
            claim.group, claim.column
        )),
        ClaimField::Against if along || is_zero => Err(format!(
            "{label}: nhóm {} không đi ngược chiều chênh lệch",
            claim.group
        )),
        _ => Ok(()),
    }
}

// Nhóm nêu là chỉ có ở bên được so (new) hay chỉ có ở bên gốc (gone) phải đúng như ghi chú của annotate.rs.
fn check_presence(claim: &Claim, explanation: &Explanation) -> Result<(), String> {
    let label = build_claim_label(claim);
    let (d, g) = find_group(explanation, &claim.column, &claim.group).ok_or_else(|| {
        format!(
            "{label}: không có nhóm {} trong cột {}",
            claim.group, claim.column
        )
    })?;
    let presence = explanation
        .notes
        .dimensions
        .iter()
        .find(|notes| notes.index == d)
        .and_then(|notes| notes.groups.get(g))
        .map(|notes| notes.presence);
    let expected = match claim.field {
        ClaimField::New => GroupPresence::New,
        _ => GroupPresence::Gone,
    };
    if presence == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "{label}: nhóm {} có ở cả hai bên hay ở bên kia",
            claim.group
        ))
    }
}

/// Vị trí (chiều, nhóm) của một nhóm. Cột nhận cả tên trong dữ liệu lẫn tên tiếng Việt của hồ sơ cột.
fn find_group(explanation: &Explanation, column: &str, group: &str) -> Option<(usize, usize)> {
    let column = column.trim();
    let group = group.trim();
    explanation
        .dimensions
        .iter()
        .enumerate()
        .find_map(|(d, dimension)| {
            let same_column =
                dimension.column == column || dimension.label.as_deref() == Some(column);
            if !same_column {
                return None;
            }
            let g = dimension.groups.iter().position(|item| {
                item.value.as_ref().map(build_value_text).as_deref() == Some(group)
            })?;
            Some((d, g))
        })
}

// Chuỗi đúng của ô một khẳng định trỏ vào, có dấu, viết như prompt in (phần trăm: như trong các ý, một chữ số lẻ,
// không có "%"); tìm theo field và theo việc khẳng định có nêu nhóm, nêu cột hay không.
fn build_truth(claim: &Claim, explanation: &Explanation) -> Result<String, String> {
    use ClaimField as F;
    let label = build_claim_label(claim);
    let digits = build_fraction_digits(explanation);
    let missing = || format!("{label}: bảng phân rã không có ô này");
    let amount = |raw: Option<&String>, scale: u32| -> Result<String, String> {
        let raw = raw.ok_or_else(missing)?;
        raw.parse::<Decimal>().map_err(|_| missing())?;
        Ok(build_number_text_with(raw, scale))
    };
    let percent = |raw: Option<&String>| amount(raw, 1);
    if !claim.group.trim().is_empty() {
        let (d, g) = find_group(explanation, &claim.column, &claim.group).ok_or_else(|| {
            format!(
                "{label}: không có nhóm {} trong cột {}",
                claim.group, claim.column
            )
        })?;
        let group = &explanation.dimensions[d].groups[g];
        let notes = explanation
            .notes
            .dimensions
            .iter()
            .find(|notes| notes.index == d)
            .and_then(|notes| notes.groups.get(g));
        return match claim.field {
            F::BaseValue => amount(Some(&group.base_value), digits),
            F::CompareValue => amount(Some(&group.compare_value), digits),
            F::Contribution => amount(Some(&group.contribution), digits),
            F::MixEffect => amount(group.mix_effect.as_ref(), digits),
            F::RateEffect => amount(group.rate_effect.as_ref(), digits),
            F::ShareOfChange => percent(notes.and_then(|n| n.share_of_change.as_ref())),
            F::PercentChange => percent(notes.and_then(|n| n.percent_change.as_ref())),
            _ => Err(missing()),
        };
    }
    if !claim.column.trim().is_empty() {
        let column = claim.column.trim();
        let dimension = explanation
            .dimensions
            .iter()
            .find(|d| d.column == column || d.label.as_deref() == Some(column))
            .ok_or_else(missing)?;
        return match claim.field {
            F::MixEffect => amount(dimension.mix_effect.as_ref(), digits),
            F::RateEffect => amount(dimension.rate_effect.as_ref(), digits),
            _ => Err(missing()),
        };
    }
    // Lượng và giá lấy từ cùng phần tách mà prompt gửi (có volume_price thì là nó, không thì theo số dòng).
    let split = build_split(explanation);
    let numbers = split.as_ref().map(|s| &s.numbers);
    match claim.field {
        F::BaseValue => amount(Some(&explanation.base.value), digits),
        F::CompareValue => amount(Some(&explanation.compare.value), digits),
        F::Change => amount(Some(&explanation.change), digits),
        F::PercentChange => percent(explanation.percent_change.as_ref()),
        F::BaseVolume => amount(numbers.map(|n| &n.base_volume), digits),
        F::CompareVolume => amount(numbers.map(|n| &n.compare_volume), digits),
        F::BasePrice => amount(numbers.map(|n| &n.base_price), digits),
        F::ComparePrice => amount(numbers.map(|n| &n.compare_price), digits),
        F::VolumeEffect => amount(numbers.map(|n| &n.volume_effect), digits),
        F::PriceEffect => amount(numbers.map(|n| &n.price_effect), digits),
        F::IfVolumeUnchanged => amount(numbers.map(|n| &n.if_volume_unchanged), digits),
        F::VolumeShare => percent(split.as_ref().and_then(|s| s.volume_share.as_ref())),
        F::PriceShare => percent(split.as_ref().and_then(|s| s.price_share.as_ref())),
        _ => Err(missing()),
    }
}
