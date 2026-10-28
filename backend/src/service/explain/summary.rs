// SPDX-License-Identifier: Apache-2.0
/* service/explain/summary.rs - lời tóm tắt do code viết cho câu hỏi tìm nguyên nhân (#186): một câu trả lời chính và
vài ý ngắn, mỗi ý chỉ nói khi số đã tính xác nhận. Đọc bảng phân rã cùng ghi chú nội bộ của explain/annotate.rs
(Explanation.notes: % tác động, % thay đổi, nhóm khác thường, chiều bỏ qua); không đổi bảng phân rã. run.rs ghi summary_title, summary_points vào Explanation; narrate.rs dùng
build_template_summary khi không dùng lời của mô hình; prompt narrate (explain/mod.rs) gửi đúng các ý này để mô hình chỉ
diễn đạt lại, không tự tìm nguyên nhân.

Các ý, theo thứ tự:
- "Lý do chính": độ đo tỷ lệ thì phần do cơ cấu hay phần do hiệu suất; độ đo tổng có tách lượng và giá thì phần nào
  lớn hơn; độ đo tổng (không phải đếm) mà trung bình mỗi dòng hai bên gần nhau thì chênh lệch chủ yếu do số dòng.
- "Khác thường theo <cột>": các nhóm lệch hẳn mức chung hay chỉ có ở một bên, nhóm chiếm nhiều chênh lệch trước.
- "Không có nơi nào đặc biệt": không chiều nào có nhóm khác thường; nhóm chênh nhiều nhất chỉ vì là nhóm lớn nhất khi
  đúng là nhóm lớn nhất.
- "Nhóm góp nhiều nhất": khi không so được nhóm với mức chung (lát gốc không dương, chênh lệch gần 0), nêu nhóm có
  phần chênh lệch lớn nhất cùng chiều với chênh lệch chung.
- "Ngược chiều": một nhóm đi ngược chênh lệch chung và chiếm từ 5% trở lên.
- "Bỏ qua <cột>": chiều tách hẳn hai lát cắt hay trùng chiều khác (annotate.rs).

Mọi số lấy từ Explanation, viết kiểu Việt Nam như bảng (explain/number.rs); phần trăm do annotate.rs tính và nằm trong
tập số được phép (build_allowed_numbers), nên lời này qua được bước kiểm số. Tên cột kèm nghĩa tiếng Việt trong ngoặc
khi hồ sơ có bí danh: "Type (hình thức thanh toán)". Test ở explain/summary_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{
    Comparison, DimensionBreakdown, Explanation, GroupContribution, MeasureKind, SummaryPoint,
    SummaryPointKind,
};
use crate::service::explain::annotate::{
    DimensionNotes, DropReason, DroppedDimension, GroupNotes, GroupPresence,
};
use crate::service::explain::number::build_number_text_with;
use crate::service::explain::{
    between, build_direction_text, build_direction_word, build_fraction_digits, build_value_text,
    lower_first, truncate_text, MAX_SUMMARY_CHARS, TEMPLATE_GROUP_CHARS,
};

/// Trung bình mỗi dòng hai bên lệch nhau không quá ngần này phần trăm thì coi là gần nhau.
const SIMILAR_AVERAGE_PERCENT: i64 = 10;
/// Nhóm ngược chiều chiếm từ ngần này phần trăm chênh lệch (âm) trở lên mới nêu.
const AGAINST_MIN_SHARE: i64 = 5;
/// Số nhóm, số cột nêu tên trong một ý; nhiều hơn thì câu dài mà không thêm ý.
const LIST_MAX: usize = 3;

/// Một chiều lời tóm tắt dùng: bảng phân rã của nó và ghi chú của annotate.rs.
#[derive(Clone, Copy)]
struct Dimension<'a> {
    breakdown: &'a DimensionBreakdown,
    notes: &'a DimensionNotes,
}

impl<'a> Dimension<'a> {
    /// Từng nhóm kèm ghi chú của nó (hai danh sách thẳng hàng).
    fn groups(&self) -> impl Iterator<Item = (&'a GroupContribution, &'a GroupNotes)> {
        self.breakdown.groups.iter().zip(&self.notes.groups)
    }

    fn column_text(&self) -> String {
        build_column_text(&self.breakdown.column, self.breakdown.label.as_deref())
    }
}

// Chiều lời tóm tắt dùng, theo thứ tự của ghi chú: chiều có nhóm khác thường trước.
fn kept_dimensions(e: &Explanation) -> Vec<Dimension<'_>> {
    e.notes
        .dimensions
        .iter()
        .filter_map(|notes| {
            let breakdown = e.dimensions.get(notes.index)?;
            Some(Dimension { breakdown, notes })
        })
        .collect()
}

/// Câu trả lời chính và các ý, đọc từ Explanation.
pub fn build_summary_parts(explanation: &Explanation) -> (String, Vec<SummaryPoint>) {
    let title = build_title(explanation);
    let mut points = Vec::new();
    let change = explanation.change.parse::<Decimal>().ok();
    if change.is_some_and(|c| !c.is_zero()) {
        let kept = kept_dimensions(explanation);
        points.extend(build_reason(explanation, &kept));
        match kept.iter().find(|d| d.notes.has_outliers) {
            Some(dimension) => points.push(build_outliers(explanation, *dimension)),
            None => points.extend(
                build_even(explanation, &kept).or_else(|| build_largest(explanation, &kept)),
            ),
        }
        points.extend(build_against(explanation, &kept));
    }
    points.extend(build_dropped(explanation));
    (title, points)
}

/// Lời tóm tắt dạng một đoạn chữ: câu chính rồi từng ý "Đầu ý: nội dung". Dùng khi không dùng lời của mô hình, và
/// cho người gọi API chỉ đọc `summary`.
pub fn build_template_summary(explanation: &Explanation) -> String {
    let (title, points) = build_summary_parts(explanation);
    let mut text = title;
    for point in points {
        text.push_str(&format!(" {}: {}", point.lead, point.text));
    }
    truncate_text(&text, MAX_SUMMARY_CHARS)
}

/// Ghi câu chính và các ý vào Explanation (run.rs gọi sau bước explain).
pub fn apply_summary_parts(explanation: &mut Explanation) {
    let (title, points) = build_summary_parts(explanation);
    explanation.summary_title = title;
    explanation.summary_points = points;
}

// Câu chính: so cái gì với cái gì, chênh bao nhiêu, kèm hai giá trị. Hai kỳ đọc "từ ... lên ...", hai nhóm không có
// trước sau nên đọc "... so với ...". So hai nhóm thì nêu cột của hai nhóm: "Order Status (trạng thái đơn) CANCELED".
fn build_title(e: &Explanation) -> String {
    let digits = build_fraction_digits(e);
    let comparison = e.comparison;
    let change = e.change.parse::<Decimal>().ok();
    let measure = &e.measure;
    let (base, compare) = (&e.base.label, &e.compare.label);
    let base_value = build_number_text_with(&e.base.value, digits);
    let compare_value = build_number_text_with(&e.compare.value, digits);
    let of = match &e.slice_column {
        Some(column) => format!(
            "{} ",
            build_column_text(column, e.slice_column_label.as_deref())
        ),
        None => String::new(),
    };
    match (change, comparison) {
        (Some(c), Comparison::Periods) if c.is_zero() => {
            format!("{measure} {compare} không đổi so với {base}: {compare_value}.")
        }
        (Some(c), Comparison::Groups) if c.is_zero() => {
            format!("{measure} của {of}{compare} bằng {base}: {compare_value}.")
        }
        (Some(c), Comparison::Periods) => {
            let towards = if c < Decimal::ZERO { "xuống" } else { "lên" };
            format!(
                "{measure} {compare} {} so với {base}, từ {base_value} {towards} {compare_value}.",
                build_direction_text(&e.change, digits, comparison)
            )
        }
        (Some(_), Comparison::Groups) => format!(
            "{measure} của {of}{compare} {} ({compare_value} so với {base_value}).",
            between(&build_direction_text(&e.change, digits, comparison), base)
        ),
        (None, _) => {
            format!("{measure} {compare} là {compare_value}, so với {base_value} ở {base}.")
        }
    }
}

fn build_reason(e: &Explanation, kept: &[Dimension]) -> Option<SummaryPoint> {
    let digits = build_fraction_digits(e);
    let comparison = e.comparison;
    if e.measure_kind == MeasureKind::Ratio {
        let dimension = kept.first()?.breakdown;
        let (mix, rate) = (
            dimension.mix_effect.as_deref()?,
            dimension.rate_effect.as_deref()?,
        );
        let mostly = if parse(mix)?.abs() >= parse(rate)?.abs() {
            "chủ yếu do cơ cấu (tỷ trọng các nhóm đổi)"
        } else {
            "chủ yếu do tỷ lệ trong từng nhóm đổi"
        };
        return Some(point(
            SummaryPointKind::Reason,
            "Lý do chính".to_string(),
            format!(
                "{mostly}. Theo {}, phần do cơ cấu {}, phần do tỷ lệ trong từng nhóm {}.",
                build_column_text(&dimension.column, dimension.label.as_deref()),
                build_direction_text(mix, digits, comparison),
                build_direction_text(rate, digits, comparison)
            ),
        ));
    }
    if let Some(split) = &e.volume_price {
        let volume_bigger = parse(&split.volume_effect)?.abs() >= parse(&split.price_effect)?.abs();
        let (base, compare) = (&e.base.label, &e.compare.label);
        let lead_text = if volume_bigger {
            format!(
                "chủ yếu do {}: {base} có {}, {compare} có {}",
                lower_first(&split.volume_label),
                build_number_text_with(&split.base_volume, digits),
                build_number_text_with(&split.compare_volume, digits)
            )
        } else {
            format!(
                "chủ yếu do {}: {} ở {base}, {} ở {compare}",
                lower_first(&split.price_label),
                build_number_text_with(&split.base_price, digits),
                build_number_text_with(&split.compare_price, digits)
            )
        };
        return Some(point(
            SummaryPointKind::Reason,
            "Lý do chính".to_string(),
            format!(
                "{lead_text}. Phần do {} {}, phần do {} {}. Nếu {} giữ nguyên như {base}, {} {compare} là {}.",
                lower_first(&split.volume_label),
                build_direction_text(&split.volume_effect, digits, comparison),
                lower_first(&split.price_label),
                build_direction_text(&split.price_effect, digits, comparison),
                lower_first(&split.volume_label),
                lower_first(&e.measure),
                build_number_text_with(&split.if_volume_unchanged, digits)
            ),
        ));
    }
    // Độ đo đếm chính là quy mô của lát cắt: nói "do số dòng" thì chỉ nhắc lại câu chính.
    if e.counts_rows || !similar_row_averages(e)? {
        return None;
    }
    Some(point(
        SummaryPointKind::Reason,
        "Lý do chính".to_string(),
        format!(
            "số dòng dữ liệu hai bên khác nhau: {} có {} dòng, {} có {} dòng, còn trung bình mỗi dòng hai bên gần như \
             nhau.",
            e.base.label,
            build_number_text_with(&e.base.rows, 0),
            e.compare.label,
            build_number_text_with(&e.compare.rows, 0)
        ),
    ))
}

// Trung bình mỗi dòng (giá trị chia số dòng) hai bên lệch nhau không quá SIMILAR_AVERAGE_PERCENT. None khi thiếu số
// dòng hay giá trị không dương (trung bình của lợi nhuận âm không so được theo phần trăm).
fn similar_row_averages(e: &Explanation) -> Option<bool> {
    let (base, compare) = (parse(&e.base.value)?, parse(&e.compare.value)?);
    let (base_rows, compare_rows) = (parse(&e.base.rows)?, parse(&e.compare.rows)?);
    if base <= Decimal::ZERO
        || compare <= Decimal::ZERO
        || base_rows.is_zero()
        || compare_rows.is_zero()
    {
        return None;
    }
    let base_average = base.checked_div(base_rows)?;
    let compare_average = compare.checked_div(compare_rows)?;
    let gap = (compare_average - base_average).abs() * Decimal::ONE_HUNDRED;
    Some(gap <= base_average * Decimal::from(SIMILAR_AVERAGE_PERCENT))
}

fn build_outliers(e: &Explanation, dimension: Dimension) -> SummaryPoint {
    let mut groups: Vec<(&GroupContribution, &GroupNotes)> =
        dimension.groups().filter(|(_, n)| n.outlier).collect();
    groups.sort_by_key(|(_, n)| std::cmp::Reverse(share_abs(n)));
    let items: Vec<String> = groups
        .iter()
        .take(LIST_MAX)
        .map(|(g, n)| build_outlier_item(e, g, n))
        .collect();
    point(
        SummaryPointKind::Outliers,
        format!("Khác thường theo {}", dimension.column_text()),
        format!("{}.", items.join("; ")),
    )
}

fn build_outlier_item(e: &Explanation, group: &GroupContribution, notes: &GroupNotes) -> String {
    let digits = build_fraction_digits(e);
    let comparison = e.comparison;
    let name = group_name(group);
    let amount = build_direction_text(&group.contribution, digits, comparison);
    let what = match (notes.presence, comparison) {
        (GroupPresence::New, Comparison::Periods) => {
            format!("{name} mới có ở {} ({amount})", e.compare.label)
        }
        (GroupPresence::Gone, Comparison::Periods) => {
            format!("{name} không còn ở {} ({amount})", e.compare.label)
        }
        (GroupPresence::New, Comparison::Groups) => {
            format!("{name} chỉ có ở {} ({amount})", e.compare.label)
        }
        (GroupPresence::Gone, Comparison::Groups) => {
            format!("{name} chỉ có ở {} ({amount})", e.base.label)
        }
        (GroupPresence::Both, _) => match (&notes.percent_change, &e.percent_change) {
            (Some(own), Some(whole)) => format!(
                "{name} {} trong khi mức chung {}",
                build_percent_change_text(own, comparison),
                build_percent_change_text(whole, comparison)
            ),
            _ => format!("{name} {amount}"),
        },
    };
    // Phần âm nghĩa là nhóm đi ngược chênh lệch chung: "chiếm -36%" khó hiểu, nên nói "kéo ngược 36%".
    match notes.share_of_change.as_deref().and_then(parse) {
        Some(share) if share < Decimal::ZERO => format!(
            "{what}, kéo ngược {} chênh lệch",
            build_percent_text(&share.abs().to_string())
        ),
        Some(share) => format!(
            "{what}, chiếm {} chênh lệch",
            build_percent_text(&share.to_string())
        ),
        None => what,
    }
}

fn build_even(e: &Explanation, kept: &[Dimension]) -> Option<SummaryPoint> {
    let whole = e.percent_change.as_deref()?;
    let first = kept.first()?;
    let columns: Vec<String> = kept
        .iter()
        .take(LIST_MAX)
        .map(|d| d.column_text())
        .collect();
    let mut text = format!(
        "theo {}, không nhóm nào lệch hẳn mức chung ({}).",
        join_words(&columns),
        build_percent_change_text(whole, e.comparison)
    );
    if e.measure_kind == MeasureKind::Total {
        if let Some(largest) = largest_groups(*first) {
            text.push_str(&format!(
                " Chênh lệch lớn nhất ở {} chỉ vì đây là những nhóm có {} lớn nhất.",
                join_words(&largest),
                lower_first(&e.measure)
            ));
        }
    }
    Some(point(
        SummaryPointKind::Even,
        "Không có nơi nào đặc biệt".to_string(),
        text,
    ))
}

// Hai nhóm chênh nhiều nhất cũng nằm trong ba nhóm có giá trị lát gốc lớn nhất: chênh lệch lớn vì nhóm lớn. Không
// đúng như vậy thì không nói câu này.
fn largest_groups(dimension: Dimension) -> Option<Vec<String>> {
    let both: Vec<&GroupContribution> = dimension
        .groups()
        .filter(|(g, n)| n.presence == GroupPresence::Both && g.value.is_some())
        .map(|(g, _)| g)
        .collect();
    if both.len() < 3 {
        return None;
    }
    let mut by_size = both.clone();
    by_size.sort_by_key(|g| std::cmp::Reverse(parse(&g.base_value)));
    let biggest = &by_size[..3];
    // groups đã xếp theo trị tuyệt đối của đóng góp, lớn trước.
    let top = &both[..2];
    top.iter()
        .all(|g| biggest.iter().any(|b| std::ptr::eq(*b, *g)))
        .then(|| top.iter().map(|g| group_name(g)).collect())
}

// Nhóm lớn nhất cùng chiều với chênh lệch chung: nhóm đầu bảng có thể đi ngược chiều (chung giảm mà nhóm biến động
// mạnh nhất lại tăng), gọi nó là nhóm góp nhiều nhất thì sai.
fn build_largest(e: &Explanation, kept: &[Dimension]) -> Option<SummaryPoint> {
    let change = parse(&e.change)?;
    let dimension = kept.first()?;
    let group = dimension.breakdown.groups.iter().find(|g| {
        parse(&g.contribution).is_some_and(|c| (c < Decimal::ZERO) == (change < Decimal::ZERO))
    })?;
    let digits = build_fraction_digits(e);
    let amount = build_direction_text(&group.contribution, digits, e.comparison);
    let column = dimension.column_text();
    let text = match e.comparison {
        Comparison::Periods => format!(
            "theo {column}, riêng nhóm {}, {} {amount}.",
            group_name(group),
            lower_first(&e.measure)
        ),
        Comparison::Groups => format!(
            "theo {column}, ở nhóm {}, {} {}.",
            group_name(group),
            e.compare.label,
            between(&amount, &e.base.label)
        ),
    };
    Some(point(
        SummaryPointKind::Largest,
        "Nhóm góp nhiều nhất".to_string(),
        text,
    ))
}

fn build_against(e: &Explanation, kept: &[Dimension]) -> Option<SummaryPoint> {
    let dimension = kept.first()?;
    let limit = -Decimal::from(AGAINST_MIN_SHARE);
    let share = |n: &GroupNotes| n.share_of_change.as_deref().and_then(parse);
    let (group, _) = dimension
        .groups()
        .filter(|(_, n)| !(dimension.notes.has_outliers && n.outlier))
        .filter(|(_, n)| share(n).is_some_and(|s| s <= limit))
        .min_by_key(|(_, n)| share(n))?;
    let digits = build_fraction_digits(e);
    Some(point(
        SummaryPointKind::Against,
        "Ngược chiều".to_string(),
        format!(
            "theo {}, {} {}.",
            dimension.column_text(),
            group_name(group),
            build_direction_text(&group.contribution, digits, e.comparison)
        ),
    ))
}

fn build_dropped(e: &Explanation) -> Option<SummaryPoint> {
    if e.notes.dropped.is_empty() {
        return None;
    }
    let columns: Vec<String> = e
        .notes
        .dropped
        .iter()
        .map(|d| build_column_text(&d.column, d.label.as_deref()))
        .collect();
    let separated: Vec<String> = e
        .notes
        .dropped
        .iter()
        .filter(|d| d.reason == DropReason::SeparatesSlices)
        .map(|d| build_separated_text(e, d))
        .collect();
    let mut sentences = Vec::new();
    if !separated.is_empty() {
        let kind = match e.comparison {
            Comparison::Periods => "hai kỳ",
            Comparison::Groups => "hai nhóm",
        };
        let which = if separated.len() == 1 {
            "Cột này"
        } else {
            "Các cột này"
        };
        sentences.push(format!(
            "{}. {which} chỉ là tên khác của {kind} đang so, không giải thích được gì.",
            separated.join("; ")
        ));
    }
    for dropped in e
        .notes
        .dropped
        .iter()
        .filter(|d| d.reason == DropReason::Duplicates)
    {
        sentences.push(format!(
            "{} chia nhóm giống hệt {}.",
            build_column_text(&dropped.column, dropped.label.as_deref()),
            dropped.duplicate_of.as_deref().unwrap_or("một cột khác")
        ));
    }
    Some(point(
        SummaryPointKind::Dropped,
        format!("Bỏ qua {}", join_words(&columns)),
        sentences.join(" "),
    ))
}

// "theo Type, COMPLETE chỉ có DEBIT, CANCELED chỉ có TRANSFER": tên cột không kèm nghĩa, vì đầu ý đã ghi.
fn build_separated_text(e: &Explanation, dropped: &DroppedDimension) -> String {
    format!(
        "theo {}, {} chỉ có {}, {} chỉ có {}",
        dropped.column,
        e.base.label,
        join_values(&dropped.base_only),
        e.compare.label,
        join_values(&dropped.compare_only)
    )
}

/// "Type (hình thức thanh toán)"; không có nghĩa tiếng Việt thì chỉ tên cột.
pub fn build_column_text(column: &str, label: Option<&str>) -> String {
    match label {
        Some(label) => format!("{column} ({label})"),
        None => column.to_string(),
    }
}

// Phần trăm thay đổi viết bằng chữ chiều: "giảm 41,5%", "thấp hơn 41,5%".
fn build_percent_change_text(percent: &str, comparison: Comparison) -> String {
    let Some(value) = parse(percent) else {
        return format!("{percent}%");
    };
    let word = if value.is_zero() {
        "không đổi"
    } else if value < Decimal::ZERO {
        "giảm"
    } else {
        "tăng"
    };
    let word = build_direction_word(word, comparison);
    if value.is_zero() {
        word.to_string()
    } else {
        format!("{word} {}", build_percent_text(&value.abs().to_string()))
    }
}

/// "41.5" thành "41,5%", "-44" thành "-44%".
pub fn build_percent_text(percent: &str) -> String {
    format!("{}%", build_number_text_with(percent, 1))
}

fn group_name(group: &GroupContribution) -> String {
    let name = group
        .value
        .as_ref()
        .map(build_value_text)
        .unwrap_or_else(|| "giá trị trống".to_string());
    truncate_text(&name, TEMPLATE_GROUP_CHARS)
}

fn join_values(values: &[String]) -> String {
    let shown: Vec<String> = values
        .iter()
        .take(LIST_MAX)
        .map(|v| truncate_text(v, TEMPLATE_GROUP_CHARS))
        .collect();
    let rest = values.len().saturating_sub(LIST_MAX);
    if rest > 0 {
        format!("{} và {rest} giá trị khác", shown.join(", "))
    } else {
        join_words(&shown)
    }
}

// "A", "A và B", "A, B và C".
fn join_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} và {last}", rest.join(", ")),
    }
}

fn share_abs(notes: &GroupNotes) -> Option<Decimal> {
    notes
        .share_of_change
        .as_deref()
        .and_then(parse)
        .map(|s| s.abs())
}

fn point(kind: SummaryPointKind, lead: String, text: String) -> SummaryPoint {
    SummaryPoint { kind, lead, text }
}

fn parse(text: &str) -> Option<Decimal> {
    text.parse().ok()
}
