// SPDX-License-Identifier: Apache-2.0
/* service/ask/summary.rs - đoạn trả lời ngắn ở đầu thẻ của câu hỏi thường đã answered (AskResponse.answer_text). Bước
này không gọi mô hình và không gửi dòng kết quả nào ra ngoài: đoạn trả lời là lời của phần giải thích
(service/insight/narrate.rs) khi mô hình đã viết lại các câu do code tính và lời đó qua bước kiểm số, chiều tăng giảm,
từ chỉ nguyên nhân; còn lại là câu khuôn do code viết từ bảng kết quả. ask/mod.rs gọi build_answer sau phần giải thích.

Không gửi dòng kết quả vì dòng có thể chứa giá trị của mọi cột câu SQL chọn, kể cả cột nhạy cảm; mô hình chỉ nhận số
đã tính qua phần giải thích (docs/PRIVACY.md). Dùng lại lời của phần giải thích cũng tránh một lần gọi mô hình thứ hai
viết lại cùng các con số.

Danh sách dòng không gộp (insight kind rows) luôn dùng câu khuôn: số dòng và điều kiện lọc. ANSWER_TEXT=false thì luôn
dùng câu khuôn. Test ở ask/summary_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{
    CellValue, ColumnRole, Insight, InsightKind, ResultTable, Understanding, ValueKind,
};
use crate::service::explain::number::{build_number_text, build_number_text_with};

/// Giá trị ô có chữ dài quá thì cắt khi đưa vào câu khuôn, để đoạn dữ liệu dài không lọt nguyên văn ra giao diện.
const TEMPLATE_CELL_CHARS: usize = 80;

/// Câu trả lời khi mọi ô của kết quả đều trống, thường là tổng hợp trên tập không có dòng nào (hỏi năm 2019 trên dữ liệu
/// tới tháng 1/2018 ra một dòng SUM là null). Câu khuôn "<độ đo> là <giá trị>." khi đó thành "Tổng Sales là ." vì giá trị
/// rỗng; cảnh báo all_null (ask/checks.rs) nói thêm khoảng thời gian dữ liệu có khi câu lọc theo thời gian.
pub const NO_DATA_ANSWER: &str =
    "Không có dữ liệu khớp với câu hỏi, nên chưa có con số để trả lời.";

/// Đầu vào của build_answer: thẻ "Đã hiểu là", bảng kết quả đã có vai trò cột và phần giải thích nếu có.
pub struct SummaryInput<'a> {
    pub understanding: Option<&'a Understanding>,
    pub result: &'a ResultTable,
    /// None khi không dựng được phần giải thích (đọc không được câu đã chạy); khi đó dùng câu khuôn.
    pub insight: Option<&'a Insight>,
    /// Config::answer_text: false thì không dùng lời của mô hình cho đoạn trả lời.
    pub enabled: bool,
}

/// Đoạn trả lời và cờ "do mô hình viết".
pub struct SummaryOutcome {
    pub text: String,
    pub by_model: bool,
}

/// None khi kết quả không có dòng nào: thẻ cảnh báo kết quả rỗng (ask/checks.rs) đã nói điều đó. Mọi ô đều trống thì
/// trả NO_DATA_ANSWER.
pub fn build_answer(input: &SummaryInput<'_>) -> Option<SummaryOutcome> {
    let result = input.result;
    if result.rows.is_empty() {
        return None;
    }
    let code = |text: String| {
        Some(SummaryOutcome {
            text,
            by_model: false,
        })
    };
    if result.rows.iter().flatten().all(Option::is_none) {
        return code(NO_DATA_ANSWER.to_string());
    }
    match input.insight {
        Some(insight) if insight.kind == InsightKind::Rows => {
            code(build_rows_answer(result, input.understanding))
        }
        // Lời này đã qua bước kiểm của phần giải thích (insight/narrate.rs): mọi số có trong câu do code tính.
        Some(insight) if input.enabled && insight.summary_by_model => Some(SummaryOutcome {
            text: insight.summary.clone(),
            by_model: true,
        }),
        _ => code(build_template_answer(input)),
    }
}

/// Câu khuôn của danh sách dòng không gộp: "Có 37 dòng khớp điều kiện: <lọc>." Điều kiện là nhãn khoảng thời gian và
/// bộ lọc của thẻ "Đã hiểu là"; limit_reached (bảng chạm trần số dòng) thì nói "ít nhất".
pub fn build_rows_answer(result: &ResultTable, understanding: Option<&Understanding>) -> String {
    let count = build_number_text(&result.total_rows.to_string());
    let count = if result.limit_reached {
        format!("ít nhất {count}")
    } else {
        count
    };
    let filters: Vec<String> = understanding
        .map(|u| {
            u.time_range
                .iter()
                .chain(&u.filters)
                .map(|item| item.label.clone())
                .collect()
        })
        .unwrap_or_default();
    if filters.is_empty() {
        format!("Có {count} dòng kết quả.")
    } else {
        format!("Có {count} dòng khớp điều kiện: {}.", filters.join("; "))
    }
}

/// Câu khuôn: kết quả một ô là "<độ đo> là <số>."; nhiều dòng có cột số đo xếp giảm dần và một cột nhóm là "Có N
/// dòng; cao nhất là <nhóm> với <số>."; còn lại "Có N dòng kết quả.".
pub fn build_template_answer(input: &SummaryInput<'_>) -> String {
    let result = input.result;
    let total = build_number_text(&result.total_rows.to_string());
    if result.columns.len() == 1 && result.total_rows == 1 {
        let measure = input
            .understanding
            .and_then(|u| u.measures.first())
            .map_or(result.columns[0].name.as_str(), |m| m.label.as_str());
        let value = build_row_texts(result, &result.rows[0]).remove(0);
        return format!("{measure} là {value}.");
    }
    if let Some((group, value)) = build_top_group(result) {
        return format!("Có {total} dòng; cao nhất là {group} với {value}.");
    }
    format!("Có {total} dòng kết quả.")
}

/// Chữ của từng ô trong một dòng, như giao diện hiện (cellFormat.ts): số kiểu Việt Nam, ngày dd/mm/yyyy, ô trống là
/// chuỗi rỗng.
fn build_row_texts(result: &ResultTable, row: &[Option<CellValue>]) -> Vec<String> {
    row.iter()
        .zip(&result.columns)
        .map(|(cell, column)| match cell {
            None => String::new(),
            Some(cell) => build_cell_text(cell, column.kind),
        })
        .collect()
}

pub(crate) fn build_cell_text(cell: &CellValue, kind: ValueKind) -> String {
    match (cell, kind) {
        (CellValue::Number(number), _) => build_float_text(*number),
        (CellValue::String(text), ValueKind::Decimal) => build_number_text_with(text, u32::MAX),
        (CellValue::String(text), ValueKind::Date) => build_date_text(text),
        (CellValue::String(text), _) => text.clone(),
        (CellValue::Boolean(true), _) => "Đúng".to_string(),
        (CellValue::Boolean(false), _) => "Sai".to_string(),
    }
}

/// Số nguyên giữ mọi chữ số; số lẻ từ 1 trở lên đúng hai chữ số lẻ; số lẻ dưới 1 giữ hai chữ số có nghĩa, như
/// formatNumber của cellFormat.ts.
fn build_float_text(number: f64) -> String {
    let Ok(value) = number.to_string().parse::<Decimal>() else {
        return number.to_string();
    };
    let shown = if value.fract().is_zero() {
        value.trunc()
    } else if value.abs() < Decimal::ONE {
        value.round_sf(2).unwrap_or(value).normalize()
    } else {
        let mut rounded = value.round_dp(2);
        rounded.rescale(2);
        rounded
    };
    build_number_text_with(&shown.to_string(), u32::MAX)
}

// "2018-01-31" thành "31/01/2018"; chuỗi khác dạng thì giữ nguyên.
fn build_date_text(text: &str) -> String {
    let parts: Vec<&str> = text.split('-').collect();
    match parts.as_slice() {
        [year, month, day] => format!("{day}/{month}/{year}"),
        _ => text.to_string(),
    }
}

/// Nhóm đứng đầu và số đo của nó, khi bảng có đúng một cột số đo, ít nhất một cột nhóm (hay thời gian) và cột số đo
/// xếp giảm dần trên các dòng đã có. Không xếp giảm dần thì dòng đầu không phải "cao nhất", nên không nói.
fn build_top_group(result: &ResultTable) -> Option<(String, String)> {
    let measures: Vec<usize> = indexes_with(result, |role| role == ColumnRole::Measure);
    let groups: Vec<usize> = indexes_with(result, |role| {
        matches!(role, ColumnRole::Dimension | ColumnRole::Time)
    });
    let (&[measure], Some(&group)) = (measures.as_slice(), groups.first()) else {
        return None;
    };
    let values: Option<Vec<Decimal>> = result
        .rows
        .iter()
        .map(|row| build_decimal(row[measure].as_ref()?))
        .collect();
    let values = values?;
    if values.len() < 2 || values.windows(2).any(|pair| pair[0] < pair[1]) {
        return None;
    }
    let texts = build_row_texts(result, &result.rows[0]);
    let name: String = texts[group].chars().take(TEMPLATE_CELL_CHARS).collect();
    if name.is_empty() {
        return None;
    }
    Some((name, texts[measure].clone()))
}

fn indexes_with(result: &ResultTable, wanted: impl Fn(ColumnRole) -> bool) -> Vec<usize> {
    result
        .columns
        .iter()
        .enumerate()
        .filter(|(_, column)| wanted(column.role))
        .map(|(i, _)| i)
        .collect()
}

fn build_decimal(cell: &CellValue) -> Option<Decimal> {
    match cell {
        CellValue::Number(number) => number.to_string().parse().ok(),
        CellValue::String(text) => text.parse().ok(),
        CellValue::Boolean(_) => None,
    }
}
