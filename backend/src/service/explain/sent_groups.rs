// SPDX-License-Identifier: Apache-2.0
/* service/explain/sent_groups.rs - các nhóm của bảng phân rã mà lời câu "tại sao" được nói tới, dùng chung cho hai lần gọi
mô hình: prompt viết lời (explain/narrate_prompt.rs) gửi số của đúng các nhóm này, và JudgeFacts của mô hình chấm
(explain/judge.rs) nhận số của cùng các nhóm đó dưới dạng khẳng định do code thêm (build_group_claims).

Vì sao mô hình chấm cần số của nhóm: trước đây nó chỉ nhận các khẳng định mô hình viết tự khai. Thử với Gemini trên hai câu
thật (số đơn 2016 so với 2015 trên sales_demo, số sinh viên 2014J so với trung bình các mã kỳ còn lại trên edu_demo):
mô hình viết hay chỉ khai nhóm là nguyên nhân (cause) hay không còn (gone) mà không khai số, rồi vẫn viết số đúng của
nhóm trong lời ("Europe giảm 7.041", "năm 2014 cao hơn 8.225,67 (336,5%)"). Mô hình chấm không thấy số đó ở đâu nên
chấm là sai, và lời đúng bị thay bằng câu khuôn. Giờ hai bên thấy cùng các số. Nhóm được nêu là nguyên nhân chính hay
đi ngược chiều vẫn phải do mô hình viết khai (cause, against): đó là nhận định cổng 1 xét, code không tự thêm.

Hàm thuần, test ở explain/sent_groups_tests.rs. */

use std::collections::HashSet;

use rust_decimal::Decimal;

use crate::service::ask::contract::{DimensionBreakdown, Explanation, GroupContribution};
use crate::service::explain::annotate::{GroupNotes, GroupPresence};
use crate::service::explain::claims::{Claim, ClaimField};
use crate::service::explain::narrate_prompt::is_negligible;
use crate::service::explain::number::build_number_text_with;
use crate::service::explain::{
    build_fraction_digits, build_value_text, NARRATE_DIMENSIONS, NARRATE_GROUPS,
};

/// Một chiều gửi cho mô hình viết lời, cùng các nhóm của nó được gửi.
pub(crate) struct SentDimension<'a> {
    pub dimension: &'a DimensionBreakdown,
    pub has_outliers: bool,
    pub groups: Vec<(&'a GroupContribution, &'a GroupNotes)>,
}

/// NARRATE_DIMENSIONS chiều đầu theo ghi chú (chiều có nhóm khác thường trước), mỗi chiều NARRATE_GROUPS nhóm đầu, bỏ
/// nhóm không đáng kể (is_negligible). Các nhóm đã xếp theo phần chênh lệch giảm dần nên các nhóm đầu là phần đáng kể.
pub(crate) fn build_sent_dimensions(e: &Explanation) -> Vec<SentDimension<'_>> {
    e.notes
        .dimensions
        .iter()
        .filter_map(|notes| Some((e.dimensions.get(notes.index)?, notes)))
        .take(NARRATE_DIMENSIONS)
        .map(|(dimension, notes)| SentDimension {
            dimension,
            has_outliers: notes.has_outliers,
            groups: dimension
                .groups
                .iter()
                .zip(&notes.groups)
                .take(NARRATE_GROUPS)
                .filter(|(group, _)| !is_negligible(e, group))
                .collect(),
        })
        .collect()
}

/// Phần của chênh lệch gửi cho mô hình: chỉ khi cùng chiều và không quá 100%. Phần âm hay quá 100% (các nhóm bù trừ
/// nhau) người đọc không hiểu được, mô hình nói bằng số thay vào. Một chữ số lẻ, như cổng 1 so.
pub(crate) fn build_sent_share(notes: &GroupNotes) -> Option<String> {
    notes
        .share_of_change
        .as_deref()
        .filter(|s| {
            s.parse::<Decimal>()
                .is_ok_and(|v| v > Decimal::ZERO && v <= Decimal::ONE_HUNDRED)
        })
        .map(|s| build_number_text_with(s, 1))
}

/// Khẳng định code thêm cho mô hình chấm: mọi số của từng nhóm đã gửi cho mô hình viết lời, cùng nhóm nào khác mà một
/// khẳng định của mô hình viết nêu tên; nhóm chỉ có ở một bên thì thêm khẳng định new hay gone. Số viết như prompt viết
/// lời và như cổng 1 so (explain/claim_cell.rs). Bỏ khẳng định trùng một khẳng định mô hình viết đã khai.
pub(crate) fn build_group_claims(e: &Explanation, written: &[Claim]) -> Vec<Claim> {
    let mut groups: Vec<(&DimensionBreakdown, &GroupContribution, &GroupNotes)> = Vec::new();
    for sent in build_sent_dimensions(e) {
        for (group, notes) in sent.groups {
            groups.push((sent.dimension, group, notes));
        }
    }
    for claim in written {
        let Some(found) = find_named_group(e, claim) else {
            continue;
        };
        if !groups
            .iter()
            .any(|(d, g, _)| std::ptr::eq(*d, found.0) && std::ptr::eq(*g, found.1))
        {
            groups.push(found);
        }
    }
    let seen: HashSet<(ClaimField, String, String)> = written
        .iter()
        .map(|c| {
            (
                c.field,
                resolve_column(e, &c.column),
                c.group.trim().to_string(),
            )
        })
        .collect();
    groups
        .into_iter()
        .flat_map(|(dimension, group, notes)| build_one_group(e, dimension, group, notes))
        .filter(|c| !seen.contains(&(c.field, c.column.clone(), c.group.clone())))
        .collect()
}

// Các khẳng định của một nhóm: hai giá trị, phần chênh lệch, % thay đổi và phần của chênh lệch khi có, phần cơ cấu và
// phần hiệu suất khi độ đo là tỷ lệ, có mặt ở một bên.
fn build_one_group(
    e: &Explanation,
    dimension: &DimensionBreakdown,
    group: &GroupContribution,
    notes: &GroupNotes,
) -> Vec<Claim> {
    let Some(value) = group.value.as_ref().map(build_value_text) else {
        // Nhóm null không có tên để mô hình viết nêu; prompt viết lời gửi giá trị null cho nó.
        return Vec::new();
    };
    let digits = build_fraction_digits(e);
    let claim = |field: ClaimField, number: String| Claim {
        field,
        column: dimension.column.clone(),
        group: value.clone(),
        value: number,
    };
    let mut claims = vec![
        claim(
            ClaimField::BaseValue,
            build_number_text_with(&group.base_value, digits),
        ),
        claim(
            ClaimField::CompareValue,
            build_number_text_with(&group.compare_value, digits),
        ),
        claim(
            ClaimField::Contribution,
            build_number_text_with(&group.contribution, digits),
        ),
    ];
    if let Some(percent) = &notes.percent_change {
        claims.push(claim(
            ClaimField::PercentChange,
            build_number_text_with(percent, 1),
        ));
    }
    if let Some(share) = build_sent_share(notes) {
        claims.push(claim(ClaimField::ShareOfChange, share));
    }
    if let (Some(mix), Some(rate)) = (&group.mix_effect, &group.rate_effect) {
        claims.push(claim(
            ClaimField::MixEffect,
            build_number_text_with(mix, digits),
        ));
        claims.push(claim(
            ClaimField::RateEffect,
            build_number_text_with(rate, digits),
        ));
    }
    match notes.presence {
        GroupPresence::New => claims.push(claim(ClaimField::New, String::new())),
        GroupPresence::Gone => claims.push(claim(ClaimField::Gone, String::new())),
        GroupPresence::Both => {}
    }
    claims
}

// Nhóm mà một khẳng định nêu tên, kèm ghi chú của nó. Cột nhận cả tên trong dữ liệu lẫn tên tiếng Việt, như cổng 1.
fn find_named_group<'a>(
    e: &'a Explanation,
    claim: &Claim,
) -> Option<(
    &'a DimensionBreakdown,
    &'a GroupContribution,
    &'a GroupNotes,
)> {
    let group_text = claim.group.trim();
    if group_text.is_empty() {
        return None;
    }
    let column = resolve_column(e, &claim.column);
    e.notes.dimensions.iter().find_map(|notes| {
        let dimension = e.dimensions.get(notes.index)?;
        if dimension.column != column {
            return None;
        }
        let (group, group_notes) =
            dimension.groups.iter().zip(&notes.groups).find(|(g, _)| {
                g.value.as_ref().map(build_value_text).as_deref() == Some(group_text)
            })?;
        Some((dimension, group, group_notes))
    })
}

// Tên cột trong dữ liệu của một khẳng định: mô hình viết có thể khai bằng tên tiếng Việt của cột.
fn resolve_column(e: &Explanation, column: &str) -> String {
    let column = column.trim();
    e.dimensions
        .iter()
        .find(|d| d.column == column || d.label.as_deref() == Some(column))
        .map(|d| d.column.clone())
        .unwrap_or_else(|| column.to_string())
}
