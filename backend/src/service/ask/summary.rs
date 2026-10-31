// SPDX-License-Identifier: Apache-2.0
/* service/ask/summary.rs - đoạn trả lời ngắn ở đầu thẻ của câu hỏi thường đã answered (AskResponse.answer_text). Bước
này không gọi mô hình và không gửi dòng kết quả nào ra ngoài: đoạn trả lời là lời của phần giải thích
(service/insight/narrate.rs) khi mô hình đã viết lại các câu do code tính và lời đó qua hai cổng kiểm (code kiểm số,
chiều tăng giảm, từ chỉ nguyên nhân; mô hình chấm so nghĩa); còn lại là câu khuôn do code viết từ bảng kết quả.
ask/mod.rs gọi build_answer sau phần giải thích.

Không gửi dòng kết quả vì dòng có thể chứa giá trị của mọi cột câu SQL chọn, kể cả cột nhạy cảm; mô hình chỉ nhận số
đã tính qua phần giải thích (docs/PRIVACY.md). Dùng lại lời của phần giải thích cũng tránh một lần gọi mô hình thứ hai
viết lại cùng các con số.

Danh sách dòng không gộp (insight kind rows) luôn dùng câu khuôn: số dòng và điều kiện lọc, hay từng ô khi chỉ có một
dòng ít cột. ANSWER_TEXT=false thì luôn dùng câu khuôn. Test ở ask/summary_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{
    CellValue, ColumnRole, Insight, InsightKind, ResultTable, Understanding, ValueKind,
};
use crate::service::explain::number::{
    apply_readable_numbers, build_number_text, build_number_text_with,
};
use crate::service::explain::per_unit::build_alias_label;
use crate::service::explain::{drop_column_name, is_vietnamese_alias, lower_first};
use crate::service::insight::narrate::build_facts_text;

/// Giá trị ô có chữ dài quá thì cắt khi đưa vào câu khuôn, để đoạn dữ liệu dài không lọt nguyên văn ra giao diện.
const TEMPLATE_CELL_CHARS: usize = 80;

/// Kết quả có tới chừng này dòng xếp theo số đo thì câu khuôn kể tên mọi nhóm theo sau nhóm đứng đầu, như câu trả lời
/// cho "top 5"; nhiều dòng hơn thì chỉ nêu nhóm đứng đầu.
const TEMPLATE_NAMED_ROWS: usize = 5;

/// Kết quả một dòng có tới chừng này cột thì câu khuôn kể từng ô; nhiều cột hơn thì câu kể dài như một bảng, để người
/// đọc xem bảng.
const TEMPLATE_ROW_COLUMNS: usize = 5;

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
        // Một dòng ngắn (câu "thành phố nào cao nhất" ra đúng một dòng) thì kể từng ô: câu đếm dòng không nêu được tên nào.
        Some(insight) if insight.kind == InsightKind::Rows && !is_short_row(result) => {
            code(build_rows_answer(result, input.understanding))
        }
        // Một dòng ngắn đã được phần giải thích kể bằng lời (insight/one_row.rs): dùng lời đó, hay câu của code khi lời
        // của mô hình không được dùng.
        Some(insight) if insight.kind == InsightKind::Rows => {
            if input.enabled && insight.summary_by_model {
                Some(SummaryOutcome {
                    text: insight.summary.clone(),
                    by_model: true,
                })
            } else {
                code(build_facts_text(&insight.title, &insight.points))
            }
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
    let count = build_row_count(result);
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

/// Câu khuôn, viết như người kể lại kết quả:
/// - một ô: "Tổng doanh thu là khoảng 36,8 triệu (36.784.735,01).";
/// - một dòng ít cột: "Kết quả: Market là Europe, Sales là khoảng 12,3 triệu (12.340.831,43).";
/// - cột số đo xếp giảm dần theo nhóm, tới năm dòng: "Caguas có tổng doanh thu cao nhất: khoảng 13,6 triệu
///   (13.610.266,21), tiếp theo là Chicago, Los Angeles và Brooklyn."; nhiều dòng hơn thì câu sau là "Kết quả có N dòng,
///   xem bảng bên dưới.";
/// - còn lại: "Kết quả có N dòng, xem bảng bên dưới."
///
/// Số từ một triệu trở lên có dạng rút gọn đứng trước số đủ; tên độ đo bỏ tên cột trong ngoặc, tên cột đã có ở thẻ
/// "Đã hiểu là".
pub fn build_template_answer(input: &SummaryInput<'_>) -> String {
    let result = input.result;
    // Tên độ đo: bí danh tiếng Việt câu SQL đặt cho cột, không thì nhãn của thẻ "Đã hiểu là" khi thẻ có đúng một độ đo.
    // None khi chỉ còn tên cột trong dữ liệu (giữ nguyên chữ hoa, chữ thường của nó).
    let measure_label = |column: &str| -> Option<String> {
        let item = input
            .understanding
            .filter(|u| u.measures.len() == 1)?
            .measures
            .first()?;
        Some(match is_vietnamese_alias(column, &item.sql) {
            // Bí danh "Lợi nhuận mỗi đơn" đặt cho một SUM là tổng (#260): gọi "Tổng lợi nhuận".
            true => build_alias_label(column, &item.sql),
            false => drop_column_name(&item.label),
        })
    };
    if result.columns.len() == 1 && result.total_rows == 1 {
        let measure = measure_label(&result.columns[0].name)
            .unwrap_or_else(|| result.columns[0].name.clone());
        let value = build_row_texts(result, &result.rows[0]).remove(0);
        return format!("{measure} là {}.", apply_readable_numbers(&value));
    }
    if is_short_row(result) {
        let texts = build_row_texts(result, &result.rows[0]);
        let cells: Vec<String> = result
            .columns
            .iter()
            .zip(&texts)
            .filter(|(_, text)| !text.is_empty())
            .map(|(column, text)| {
                let text: String = text.chars().take(TEMPLATE_CELL_CHARS).collect();
                format!("{} là {}", column.name, apply_readable_numbers(&text))
            })
            .collect();
        return format!("Kết quả: {}.", cells.join(", "));
    }
    let rows = build_row_count(result);
    if let Some(top) = build_top_group(result) {
        // Tên độ đo đứng giữa câu nên viết thường chữ đầu; tên cột trong dữ liệu giữ nguyên.
        let measure = measure_label(&top.measure).map_or(top.measure, |label| lower_first(&label));
        let lead = format!(
            "{} có {measure} cao nhất: {}",
            top.group,
            apply_readable_numbers(&top.value)
        );
        return match top.next.split_last() {
            Some((last, [])) => format!("{lead}, tiếp theo là {last}."),
            Some((last, rest)) => format!("{lead}, tiếp theo là {} và {last}.", rest.join(", ")),
            None => format!("{lead}. Kết quả có {rows} dòng, xem bảng bên dưới."),
        };
    }
    format!("Kết quả có {rows} dòng, xem bảng bên dưới.")
}

fn is_short_row(result: &ResultTable) -> bool {
    result.total_rows == 1 && result.columns.len() <= TEMPLATE_ROW_COLUMNS
}

/// Số dòng của kết quả bằng chữ; bảng chạm trần số dòng (limit_reached) thì "ít nhất N".
fn build_row_count(result: &ResultTable) -> String {
    let count = build_number_text(&result.total_rows.to_string());
    if result.limit_reached {
        format!("ít nhất {count}")
    } else {
        count
    }
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

/// Nhóm đứng đầu của bảng, tên cột số đo và giá trị của nhóm đó.
struct TopGroup {
    group: String,
    measure: String,
    value: String,
    /// Tên các nhóm theo sau, theo thứ tự; rỗng khi bảng nhiều hơn TEMPLATE_NAMED_ROWS dòng.
    next: Vec<String>,
}

/// Nhóm đứng đầu khi bảng có đúng một cột số đo, ít nhất một cột nhóm (hay thời gian) và cột số đo xếp giảm dần trên
/// các dòng đã có. Không xếp giảm dần thì dòng đầu không phải "cao nhất", nên không nói.
fn build_top_group(result: &ResultTable) -> Option<TopGroup> {
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
    let named = result.total_rows as usize <= TEMPLATE_NAMED_ROWS && !result.limit_reached;
    let next = if named {
        result.rows[1..]
            .iter()
            .map(|row| build_row_texts(result, row).swap_remove(group))
            .map(|text| text.chars().take(TEMPLATE_CELL_CHARS).collect())
            .collect()
    } else {
        Vec::new()
    };
    Some(TopGroup {
        group: name,
        measure: result.columns[measure].name.clone(),
        value: texts[measure].clone(),
        next,
    })
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
