// SPDX-License-Identifier: Apache-2.0
/* service/insight/facts.rs - câu giải thích do code viết cho từng hình dạng kết quả (#227): một câu chính và vài ý ngắn,
mỗi số tính bằng Decimal từ bảng kết quả (hay từ bảng phân rã khi có), viết kiểu Việt Nam như bảng (explain/number.rs).
Mô hình chỉ được diễn đạt lại đúng các câu này (insight/narrate.rs); mô hình lỗi hay viết sai thì các câu này là phần
giải thích. Hàm thuần, test ở insight/tests.rs.

Không nói quá: chênh lệch dưới ngưỡng hay mẫu ít dòng (explain/significance.rs) thì câu chính nói "không có thay đổi đáng
kể"; không dùng từ chỉ nguyên nhân ("do", "vì", "gây ra"): phân rã chỉ nói phần chênh lệch nằm ở nhóm nào. Không viết tỷ
số kiểu "gấp 3 lần": bước kiểm số coi đó là số mô hình tự tính. */

use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::ask::contract::{Comparison, Explanation, InsightPoint};
use crate::service::explain::number::{
    apply_readable_numbers, build_number_text, build_number_text_with,
};
use crate::service::explain::significance::{build_change_weight, ChangeWeight};
use crate::service::explain::summary::build_column_text;
use crate::service::explain::{build_direction_text, build_value_text, lower_first};

/// Tỷ trọng cộng dồn cần đạt khi đếm số nhóm chiếm phần lớn tổng.
const CONCENTRATION_PERCENT: i64 = 80;
/// Nhóm có giá trị từ ngần này lần trung bình mỗi nhóm trở lên, hay từ nghịch đảo của nó trở xuống, là nhóm lệch hẳn
/// mức chung. Chọn 2 để chỉ nêu nhóm khác hẳn; chưa đo trên dữ liệu nào.
const OUTLIER_FACTOR: i64 = 2;
/// Số nhóm nêu tên trong một ý.
const LIST_MAX: usize = 3;

/// Một điểm của bảng: nhãn cho người đọc và giá trị độ đo.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub label: String,
    pub value: Decimal,
}

/// Phần giải thích do code viết, cùng những gì bước kiểm lời của mô hình cần.
#[derive(Debug, Clone, PartialEq)]
pub struct InsightParts {
    pub title: String,
    pub points: Vec<InsightPoint>,
    /// False khi số liệu không cho thấy điều gì đáng kể (insight/mod.rs ghi vào Insight.notable).
    pub notable: bool,
    /// Chênh lệch có chiều đã viết: (độ lớn không dấu như trong câu, "tăng" hay "giảm"). Lời của mô hình phải nói đúng
    /// chiều trước mỗi độ lớn này.
    pub changes: Vec<(String, &'static str)>,
    /// Tên nhóm, kỳ, độ đo đã nêu: đoạn chữ dữ liệu, số nằm trong đó không bị coi là số mô hình tự viết.
    pub names: Vec<String>,
}

impl InsightParts {
    /// Ý "Cách hiểu": cụm của câu hỏi khớp nhiều cột và cột đã chọn, bằng lời. Cụm và tên cột là chữ dữ liệu (names),
    /// để số nằm trong đó không bị bước kiểm coi là số mô hình tự viết.
    pub fn add_reading(&mut self, phrase: &str, label: &str) {
        self.push("Cách hiểu", format!("\"{phrase}\" được tính theo {label}."));
        self.names.push(phrase.to_string());
        self.names.push(label.to_string());
    }

    /// Số từ một triệu trở lên trong câu chính và các ý viết thêm dạng rút gọn (explain/number.rs,
    /// apply_readable_numbers). Gọi một lần sau khi dựng xong các câu. Độ lớn ghi cho bước kiểm chiều (changes) vẫn là số
    /// đủ, và số đủ vẫn đứng trong ngoặc, nên bước kiểm vẫn tìm thấy nó.
    pub fn apply_readable_numbers(&mut self) {
        self.title = apply_readable_numbers(&self.title);
        for point in &mut self.points {
            point.text = apply_readable_numbers(&point.text);
        }
    }

    pub(super) fn new(title: String) -> Self {
        InsightParts {
            title,
            points: Vec::new(),
            notable: true,
            changes: Vec::new(),
            names: Vec::new(),
        }
    }

    fn push(&mut self, lead: &str, text: String) {
        self.points.push(InsightPoint {
            lead: lead.to_string(),
            text,
        });
    }

    /// "tăng 1.234,5" và ghi chênh lệch đó cho bước kiểm chiều.
    fn change_text(&mut self, change: Decimal) -> String {
        let text = build_direction_text(&change.normalize().to_string(), 2, Comparison::Periods);
        if let Some((word, amount)) = text.split_once(' ') {
            if let Some(word) = ["tăng", "giảm"].into_iter().find(|w| *w == word) {
                self.changes.push((amount.to_string(), word));
            }
        }
        text
    }
}

/// Số kiểu Việt Nam, tối đa hai chữ số lẻ như bảng kết quả.
pub fn build_value_number(value: Decimal) -> String {
    build_number_text(&value.normalize().to_string())
}

/// "tổng doanh thu"; độ đo đã mở đầu bằng "Tổng" (nhãn "Tổng doanh thu" của thẻ "Đã hiểu là") thì không thêm chữ
/// "tổng" thứ hai.
fn build_total_words(measure: &str) -> String {
    let measure = lower_first(measure);
    if measure.starts_with("tổng ") {
        measure
    } else {
        format!("tổng {measure}")
    }
}

/// part / whole * 100 làm tròn một chữ số lẻ, kèm "%"; None khi whole không dương.
pub fn build_share_text(part: Decimal, whole: Decimal) -> Option<String> {
    if whole <= Decimal::ZERO {
        return None;
    }
    let percent = (part * Decimal::from(100))
        .checked_div(whole)?
        .round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    Some(format!(
        "{}%",
        build_number_text_with(&percent.to_string(), 1)
    ))
}

// "tăng 1.234,5 (12,3%)": phần trăm so với giá trị gốc khi gốc dương.
fn build_change_with_percent(parts: &mut InsightParts, base: Decimal, compare: Decimal) -> String {
    let change = compare - base;
    let text = parts.change_text(change);
    match build_share_text(change.abs(), base) {
        Some(percent) => format!("{text} ({percent})"),
        None => text,
    }
}

/// Một con số so với kỳ liền trước (khi đọc được kỳ của câu hỏi). previous: (nhãn kỳ trước, giá trị, số dòng hai kỳ).
pub fn build_single_parts(
    measure: &str,
    current: &Point,
    previous: Option<(&Point, Option<(i64, i64)>)>,
    breakdown: Option<&Explanation>,
) -> InsightParts {
    let value = build_value_number(current.value);
    let subject = if current.label.is_empty() {
        measure.to_string()
    } else {
        format!("{measure} {}", current.label)
    };
    let mut parts = InsightParts::new(format!("{subject} là {value}."));
    parts
        .names
        .extend([measure.to_string(), current.label.clone()]);
    let Some((base, rows)) = previous else {
        parts.notable = false;
        return parts;
    };
    parts.names.push(base.label.clone());
    let change = build_change_with_percent(&mut parts, base.value, current.value);
    let weight = build_change_weight(base.value, current.value, rows);
    let base_value = build_value_number(base.value);
    match weight {
        ChangeWeight::Notable => {
            parts.title = format!("{subject} là {value}, {change} so với {}.", base.label);
            if let Some(explanation) = breakdown {
                apply_breakdown_point(&mut parts, explanation);
            }
        }
        ChangeWeight::Tiny => {
            parts.notable = false;
            parts.push(
                "Không có thay đổi đáng kể",
                format!("{} là {base_value}, chênh lệch nhỏ: {change}.", base.label),
            );
        }
        ChangeWeight::SmallSample => {
            parts.notable = false;
            apply_small_sample(&mut parts, &base.label, &current.label, rows);
        }
    }
    parts
}

// Ý mẫu nhỏ: số dòng mỗi bên, kết luận có thể chỉ do vài dòng (#199).
fn apply_small_sample(
    parts: &mut InsightParts,
    base: &str,
    compare: &str,
    rows: Option<(i64, i64)>,
) {
    let Some((base_rows, compare_rows)) = rows else {
        return;
    };
    parts.push(
        "Không có thay đổi đáng kể",
        format!(
            "{base} có {} dòng, {compare} có {} dòng: quá ít dòng để kết luận.",
            build_value_number(Decimal::from(base_rows)),
            build_value_number(Decimal::from(compare_rows))
        ),
    );
}

/// Chuỗi thời gian: xu hướng chung (kỳ đầu so với kỳ cuối), kỳ đổi mạnh nhất so với kỳ liền trước, và nhóm chiếm phần
/// lớn nhất của chênh lệch đó khi có phân rã. points theo thứ tự thời gian, ít nhất hai điểm. rows: số dòng hai kỳ của
/// kỳ đổi mạnh nhất, khi đã đếm (phân rã chạy).
pub fn build_series_parts(
    measure: &str,
    points: &[Point],
    breakdown: Option<&Explanation>,
    rows: Option<(i64, i64)>,
) -> InsightParts {
    let mut parts = InsightParts::new(String::new());
    parts.names.push(measure.to_string());
    parts.names.extend(points.iter().map(|p| p.label.clone()));
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return parts;
    };
    let overall = build_change_with_percent(&mut parts, first.value, last.value);
    let overall = match build_change_weight(first.value, last.value, None) {
        ChangeWeight::Notable => format!(
            "Từ {} tới {}, {} {overall}.",
            first.label,
            last.label,
            lower_first(measure)
        ),
        _ => format!(
            "{} của {} và {} gần như bằng nhau ({overall}).",
            measure, first.label, last.label
        ),
    };
    parts.push("Xu hướng chung", overall);
    let Some(index) = find_largest_step(points) else {
        return parts;
    };
    let (before, after) = (&points[index - 1], &points[index]);
    let change = build_change_with_percent(&mut parts, before.value, after.value);
    match build_change_weight(before.value, after.value, rows) {
        ChangeWeight::Notable => {
            parts.title = format!(
                "{} đổi mạnh nhất so với kỳ trước: {change} so với {}.",
                after.label, before.label
            );
            if let Some(explanation) = breakdown {
                apply_breakdown_point(&mut parts, explanation);
            }
        }
        ChangeWeight::Tiny => {
            parts.notable = false;
            parts.title = "Không có thay đổi đáng kể giữa các kỳ.".to_string();
            parts.push(
                "Kỳ đổi nhiều nhất",
                format!("{} so với {}: {change}.", after.label, before.label),
            );
        }
        ChangeWeight::SmallSample => {
            parts.notable = false;
            parts.title = format!("{} đổi nhiều nhất so với kỳ trước: {change}.", after.label);
            apply_small_sample(&mut parts, &before.label, &after.label, rows);
        }
    }
    parts
}

/// Vị trí của điểm có chênh lệch với điểm liền trước lớn nhất (theo trị tuyệt đối); bằng nhau thì lấy điểm sớm hơn.
pub fn find_largest_step(points: &[Point]) -> Option<usize> {
    (1..points.len()).fold(None, |best: Option<usize>, i| {
        let step = (points[i].value - points[i - 1].value).abs();
        match best {
            Some(b) if (points[b].value - points[b - 1].value).abs() >= step => Some(b),
            _ => Some(i),
        }
    })
}

/// Ý từ bảng phân rã: chiều đầu (giải thích chênh lệch rõ nhất, explain/mod.rs xếp) và nhóm có phần chênh lệch lớn
/// nhất cùng chiều với chênh lệch chung. Nói "nằm nhiều nhất ở", không nói "do".
fn apply_breakdown_point(parts: &mut InsightParts, explanation: &Explanation) {
    let Ok(change) = explanation.change.parse::<Decimal>() else {
        return;
    };
    let Some(dimension) = explanation.dimensions.first() else {
        return;
    };
    let main = dimension.groups.iter().find(|g| {
        g.contribution
            .parse::<Decimal>()
            .is_ok_and(|c| !c.is_zero() && (c < Decimal::ZERO) == (change < Decimal::ZERO))
    });
    let Some(group) = main else {
        return;
    };
    let Ok(contribution) = group.contribution.parse::<Decimal>() else {
        return;
    };
    let name = group
        .value
        .as_ref()
        .map_or_else(|| "trống".to_string(), build_value_text);
    let column = build_column_text(&dimension.column, dimension.label.as_deref());
    let part = parts.change_text(contribution);
    // Nhóm lớn hơn cả chênh lệch chung (các nhóm khác đi ngược chiều) thì "756% của chênh lệch" khó đọc: nêu chênh lệch
    // chung bên cạnh thay cho phần trăm.
    let share = if contribution.abs() > change.abs() {
        format!(", trong khi chênh lệch chung {}", parts.change_text(change))
    } else {
        build_share_text(contribution.abs(), change.abs())
            .map(|s| format!(", {s} của chênh lệch"))
            .unwrap_or_default()
    };
    parts.names.extend([name.clone(), column.clone()]);
    parts.push(
        "Phần chênh lệch nằm nhiều nhất ở",
        format!("{name} theo {column}: {part}{share}."),
    );
}

/// Theo nhóm: nhóm đứng đầu chiếm bao nhiêu phần tổng, cần mấy nhóm để đạt 80% tổng, nhóm lệch hẳn mức chung. total là
/// tổng mọi nhóm khi bảng đủ dòng; None thì không nói tỷ trọng.
pub fn build_group_parts(
    measure: &str,
    column: &str,
    groups: &[Point],
    total: Option<Decimal>,
) -> InsightParts {
    let mut parts = InsightParts::new(String::new());
    parts
        .names
        .extend([measure.to_string(), column.to_string()]);
    let mut sorted: Vec<&Point> = groups.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.value));
    let Some(top) = sorted.first() else {
        return parts;
    };
    parts
        .names
        .extend(sorted.iter().take(LIST_MAX).map(|p| p.label.clone()));
    // Tỷ trọng chỉ có nghĩa khi mọi nhóm không âm (lợi nhuận âm làm tổng nhỏ hơn một nhóm).
    let total =
        total.filter(|t| *t > Decimal::ZERO && sorted.iter().all(|p| p.value >= Decimal::ZERO));
    let value = build_value_number(top.value);
    parts.title = match total.and_then(|t| build_share_text(top.value, t)) {
        Some(share) => format!(
            "{} đứng đầu với {value}, chiếm {share} {}.",
            top.label,
            build_total_words(measure)
        ),
        None => format!("{} đứng đầu với {value}.", top.label),
    };
    let mut even = true;
    if let Some(total) = total {
        let needed = count_to_reach(&sorted, total);
        parts.push(
            "Mức tập trung",
            format!(
                "{} trên {} nhóm chiếm từ {CONCENTRATION_PERCENT}% tổng trở lên.",
                build_value_number(Decimal::from(needed)),
                build_value_number(Decimal::from(sorted.len()))
            ),
        );
        // Cần ít hơn một nửa số nhóm để đạt 80% thì tổng tập trung ở vài nhóm.
        even = needed * 2 > sorted.len();
    }
    if apply_outliers(&mut parts, &sorted) {
        even = false;
    }
    if even {
        parts.notable = false;
        parts.push(
            "Không có nhóm nào nổi bật",
            "Các nhóm khá đều nhau, không nhóm nào lệch hẳn mức trung bình.".to_string(),
        );
    }
    parts
}

// Số nhóm đầu (đã xếp giảm dần) cộng dồn đạt CONCENTRATION_PERCENT của total.
fn count_to_reach(sorted: &[&Point], total: Decimal) -> usize {
    let target = total * Decimal::from(CONCENTRATION_PERCENT);
    let mut sum = Decimal::ZERO;
    for (i, point) in sorted.iter().enumerate() {
        sum += point.value;
        if sum * Decimal::from(100) >= target {
            return i + 1;
        }
    }
    sorted.len()
}

// Nhóm cao hẳn hay thấp hẳn mức trung bình mỗi nhóm; cần ít nhất ba nhóm và mọi giá trị dương. true khi có nhóm nào.
fn apply_outliers(parts: &mut InsightParts, sorted: &[&Point]) -> bool {
    if sorted.len() < 3 || sorted.iter().any(|p| p.value <= Decimal::ZERO) {
        return false;
    }
    let sum: Decimal = sorted.iter().map(|p| p.value).sum();
    let mean = (sum / Decimal::from(sorted.len())).round_dp(2);
    let factor = Decimal::from(OUTLIER_FACTOR);
    let list = |points: Vec<&&Point>| -> Vec<String> {
        points
            .into_iter()
            .take(LIST_MAX)
            .map(|p| format!("{} ({})", p.label, build_value_number(p.value)))
            .collect()
    };
    let high = list(sorted.iter().filter(|p| p.value >= mean * factor).collect());
    let low = list(
        sorted
            .iter()
            .rev()
            .filter(|p| p.value * factor <= mean)
            .collect(),
    );
    let mean_text = build_value_number(mean);
    if !high.is_empty() {
        parts.push(
            "Cao hẳn mức trung bình",
            format!("{}; trung bình mỗi nhóm {mean_text}.", high.join(", ")),
        );
    }
    if !low.is_empty() {
        parts.push(
            "Thấp hẳn mức trung bình",
            format!("{}; trung bình mỗi nhóm {mean_text}.", low.join(", ")),
        );
    }
    parts.names.extend(sorted.iter().map(|p| p.label.clone()));
    !high.is_empty() || !low.is_empty()
}

/// Top N: N nhóm đầu chiếm bao nhiêu phần tổng (total: tổng mọi nhóm, kể cả nhóm ngoài top; groups: số nhóm khi có),
/// và chênh giữa hạng đầu với hạng kế. shown theo thứ tự của bảng (giảm dần).
pub fn build_top_parts(
    measure: &str,
    shown: &[Point],
    all: Option<(Decimal, i64)>,
) -> InsightParts {
    let mut parts = InsightParts::new(String::new());
    parts.names.push(measure.to_string());
    parts.names.extend(shown.iter().map(|p| p.label.clone()));
    let (Some(first), Some(second)) = (shown.first(), shown.get(1)) else {
        return parts;
    };
    let count = build_value_number(Decimal::from(shown.len()));
    let sum: Decimal = shown.iter().map(|p| p.value).sum();
    let share =
        all.and_then(|(total, groups)| Some((build_share_text(sum, total)?, total, groups)));
    parts.title = match share {
        Some((share, total, groups)) => format!(
            "{count} nhóm đầu chiếm {share} {} ({} trên {} nhóm).",
            build_total_words(measure),
            build_value_number(total),
            build_value_number(Decimal::from(groups))
        ),
        None => format!(
            "{} đứng đầu với {}.",
            first.label,
            build_value_number(first.value)
        ),
    };
    // Hai nhóm không có trước sau nên chữ chiều là "cao hơn", không phải "tăng"; hạng đầu luôn không thấp hơn hạng kế.
    let gap = build_value_number(first.value - second.value);
    let gap = match build_share_text(first.value - second.value, second.value) {
        Some(percent) => format!("{gap} ({percent})"),
        None => gap,
    };
    let text = match build_change_weight(second.value, first.value, None) {
        ChangeWeight::Notable => format!("{} cao hơn {} {gap}.", first.label, second.label),
        _ => {
            parts.notable = false;
            format!(
                "{} và {} gần như bằng nhau, chênh {gap}.",
                first.label, second.label
            )
        }
    };
    parts.push("Hạng đầu so với hạng kế", text);
    parts
}

/// Danh sách dòng không gộp: chỉ số dòng và điều kiện lọc (nhãn của thẻ "Đã hiểu là").
pub fn build_rows_parts(total_rows: u64, limit_reached: bool, filters: &[String]) -> InsightParts {
    let count = build_value_number(Decimal::from(total_rows));
    let title = if limit_reached {
        format!("Kết quả có ít nhất {count} dòng.")
    } else {
        format!("Kết quả có {count} dòng.")
    };
    let mut parts = InsightParts::new(title);
    parts.notable = false;
    let text = if filters.is_empty() {
        "Không có điều kiện lọc.".to_string()
    } else {
        format!("{}.", filters.join("; "))
    };
    parts.names.extend(filters.iter().cloned());
    parts.push("Điều kiện lọc", text);
    parts
}
