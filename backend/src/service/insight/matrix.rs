// SPDX-License-Identifier: Apache-2.0
/* service/insight/matrix.rs - câu giải thích do code viết cho bảng theo hai cột nhóm (câu "doanh thu theo danh mục và
phân khúc khách hàng"): tổ hợp đứng đầu, vài tổ hợp lớn nhất cùng chiếm bao nhiêu phần tổng, và nhóm đứng đầu của từng
cột khi cộng qua cột kia. insight/mod.rs gọi khi bảng có hai cột nhóm (insight/shape.rs, second_key); trước đây bảng
này chỉ được nói "Có 150 dòng kết quả".

Tỷ trọng và nhóm đứng đầu của từng cột chỉ có nghĩa khi độ đo cộng được (SUM, COUNT) và bảng đủ dòng: cộng các trung
bình hay tỷ lệ qua một cột không ra số nào. Khi đó chỉ nêu tổ hợp cao nhất và thấp nhất. Hàm thuần, test ở
insight/tests.rs. */

use rust_decimal::Decimal;

use crate::service::explain::lower_first;
use crate::service::insight::facts::{build_share_text, InsightParts, MeasureText};

/// Số tổ hợp nêu tên trong ý "Các tổ hợp lớn nhất".
const TOP_CELLS: usize = 3;

/// Một ô của bảng hai chiều: giá trị của hai cột nhóm và của độ đo.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub first: String,
    pub second: String,
    pub value: Decimal,
}

/// columns: tên hai cột nhóm như người đọc hiểu ("Danh mục", "Phân khúc khách hàng"). complete: bảng có mọi dòng.
pub fn build_matrix_parts(
    measure: MeasureText,
    columns: (&str, &str),
    cells: &[Cell],
    complete: bool,
) -> InsightParts {
    let mut parts = InsightParts::new(String::new());
    parts.style = measure.style;
    parts.names.extend([
        measure.label.to_string(),
        columns.0.to_string(),
        columns.1.to_string(),
    ]);
    let mut sorted: Vec<&Cell> = cells.iter().collect();
    sorted.sort_by_key(|c| std::cmp::Reverse(c.value));
    let (Some(top), Some(bottom)) = (sorted.first(), sorted.last()) else {
        return parts;
    };
    for cell in sorted.iter().take(TOP_CELLS).chain(std::iter::once(bottom)) {
        parts
            .names
            .extend([cell.first.clone(), cell.second.clone()]);
    }
    let name = |cell: &Cell| format!("{}, {} {}", cell.first, lower_first(columns.1), cell.second);
    let total: Decimal = sorted.iter().map(|c| c.value).sum();
    let shares = measure.additive
        && complete
        && total > Decimal::ZERO
        && sorted.iter().all(|c| c.value >= Decimal::ZERO);
    let value = parts.style.value_text(top.value);
    if !shares {
        parts.title = format!("{} cao nhất với {value}.", name(top));
        if sorted.len() >= 2 {
            let low = parts.style.value_text(bottom.value);
            parts.push("Thấp nhất", format!("{} ({low}).", name(bottom)));
        }
        return parts;
    }
    let total_words = build_total_label(measure.label);
    parts.title = match build_share_text(top.value, total) {
        Some(share) => format!(
            "{} đứng đầu với {value}, chiếm {share} {total_words}.",
            name(top)
        ),
        None => format!("{} đứng đầu với {value}.", name(top)),
    };
    if sorted.len() > TOP_CELLS {
        let shown: Vec<&&Cell> = sorted.iter().take(TOP_CELLS).collect();
        let sum: Decimal = shown.iter().map(|c| c.value).sum();
        let list: Vec<String> = shown
            .iter()
            .map(|c| format!("{} ({})", name(c), parts.style.value_text(c.value)))
            .collect();
        if let Some(share) = build_share_text(sum, total) {
            parts.push(
                "Các tổ hợp lớn nhất",
                format!("{}: cùng chiếm {share} tổng.", list.join("; ")),
            );
        }
    }
    for (column, by_first) in [(columns.0, true), (columns.1, false)] {
        let Some((group, sum)) = find_leader(&sorted, by_first) else {
            continue;
        };
        let Some(share) = build_share_text(sum, total) else {
            continue;
        };
        let text = format!(
            "{group} với {}, chiếm {share}.",
            parts.style.value_text(sum)
        );
        parts.names.push(group);
        parts.push(&format!("Đứng đầu theo {}", lower_first(column)), text);
    }
    parts
}

/// Nhóm có tổng lớn nhất của một cột khi cộng qua cột kia; bằng nhau thì nhóm gặp trước (của tổ hợp lớn hơn).
fn find_leader(cells: &[&Cell], by_first: bool) -> Option<(String, Decimal)> {
    let mut sums: Vec<(String, Decimal)> = Vec::new();
    for cell in cells {
        let key = if by_first { &cell.first } else { &cell.second };
        match sums.iter_mut().find(|(name, _)| name == key) {
            Some((_, sum)) => *sum += cell.value,
            None => sums.push((key.clone(), cell.value)),
        }
    }
    sums.into_iter().fold(
        None,
        |best: Option<(String, Decimal)>, (name, sum)| match best {
            Some((_, top)) if top >= sum => best,
            _ => Some((name, sum)),
        },
    )
}

/// "tổng doanh thu", không lặp chữ "tổng" khi nhãn đã có.
fn build_total_label(measure: &str) -> String {
    let measure = lower_first(measure);
    if measure.starts_with("tổng ") {
        measure
    } else {
        format!("tổng {measure}")
    }
}
