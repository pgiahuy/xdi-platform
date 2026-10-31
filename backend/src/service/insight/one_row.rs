// SPDX-License-Identifier: Apache-2.0
/* service/insight/one_row.rs - câu của code cho kết quả đúng một dòng ít cột mà không phải một con số (insight kind
rows), như "thành phố nào có doanh thu cao nhất" (LIMIT 1) hay "thành phố đó thuộc quốc gia nào". Câu đếm dòng của
danh sách dòng ("Kết quả có 1 dòng.") không nêu được tên nào, nên một dòng ngắn được kể bằng lời:
- xếp theo một cột số đo rồi lấy một dòng: "Caguas có tổng doanh thu cao nhất: 13.610.266,21.";
- còn lại: "Quốc gia của khách hàng là Puerto Rico, thành phố của khách hàng là Caguas."

Mô hình được viết lại câu này (insight/narrate.rs, hai cổng kiểm) chỉ khi mọi ô gửi được: ô tổng hợp ra số là con số
đã tính, ô còn lại phải đọc từ cột phân loại hay thời gian không nhạy cảm (find_unsendable_key), như tên nhóm của bảng
theo nhóm. Không thì dùng câu của code. insight/mod.rs gọi build_one_row trước build_parts. Test ở insight/tests.rs. */

use sqlparser::ast::Expr;

use crate::engine::sql_shape::QueryShape;
use crate::service::ask::contract::{ColumnRole, ResultTable, ValueKind};
use crate::service::ask::summary::build_cell_text;
use crate::service::explain::{drop_column_name, lower_first, upper_first};
use crate::service::insight::facts::InsightParts;
use crate::service::insight::{build_measure_label, find_unsendable_key, InsightInput};

/// Kết quả một dòng có tới chừng này cột thì kể từng ô; nhiều hơn thì câu dài như một bảng, giữ câu đếm dòng.
const MAX_COLUMNS: usize = 5;

/// Chữ của một ô dài quá thì cắt, để đoạn dữ liệu dài không lọt nguyên văn vào câu.
const MAX_CELL_CHARS: usize = 80;

/// Câu của code cho một dòng, và lý do không gửi cho mô hình (None khi gửi được).
pub struct OneRow {
    pub parts: InsightParts,
    pub code_only: Option<String>,
}

/// None khi kết quả không phải một dòng ngắn hay mọi ô đều trống.
pub fn build_one_row(input: &InsightInput<'_>, shape: &QueryShape) -> Option<OneRow> {
    let result = input.result;
    if result.total_rows != 1 || result.rows.len() != 1 || result.columns.len() > MAX_COLUMNS {
        return None;
    }
    let cells: Vec<(usize, String)> = result.rows[0]
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| {
            let text = build_cell_text(cell.as_ref()?, result.columns[i].kind);
            Some((i, text.chars().take(MAX_CELL_CHARS).collect()))
        })
        .collect();
    if cells.is_empty() {
        return None;
    }
    let mut parts = InsightParts::new(String::new());
    let ranked = find_ranked_measure(result, shape, input.limit_kept);
    parts.title = match ranked {
        Some((measure, highest)) => {
            build_ranked_title(input, shape, &cells, measure, highest, &mut parts)
        }
        None => build_cells_title(input, shape, &cells, &mut parts),
    };
    // Ô tổng hợp ra số là con số đã tính, gửi như số của các hình dạng khác. Ô tổng hợp ra chữ (max của cột email) vẫn
    // là một giá trị của cột, nên kiểm như ô thường.
    let is_number = |i: usize| {
        shape.outputs.get(i).is_some_and(|o| o.is_aggregate)
            && matches!(
                result.columns[i].kind,
                ValueKind::Number | ValueKind::Decimal
            )
    };
    let code_only = cells
        .iter()
        .filter(|(i, _)| !is_number(*i))
        .find_map(|(i, _)| find_unsendable_key(input.tables, shape, *i))
        .map(|column| {
            format!("Một dòng có ô đọc từ {column}, cột không gửi được cho mô hình; dùng câu do code viết")
        });
    Some(OneRow { parts, code_only })
}

/// Cột số đo mà câu xếp theo rồi lấy số dòng câu hỏi muốn (LIMIT của mô hình): (vị trí cột, true khi xếp giảm dần).
fn find_ranked_measure(
    result: &ResultTable,
    shape: &QueryShape,
    limit_kept: bool,
) -> Option<(usize, bool)> {
    if !limit_kept || shape.limit.is_none() {
        return None;
    }
    let order = shape.order_by.first()?;
    let measure = order.output?;
    (result.columns.get(measure)?.role == ColumnRole::Measure)
        .then_some((measure, order.descending))
}

// "Caguas có tổng doanh thu cao nhất: 13.610.266,21." Các ô không phải số đo nối bằng dấu phẩy làm chủ ngữ; số đo khác
// trong dòng kể thêm sau.
fn build_ranked_title(
    input: &InsightInput<'_>,
    shape: &QueryShape,
    cells: &[(usize, String)],
    measure: usize,
    highest: bool,
    parts: &mut InsightParts,
) -> String {
    let result = input.result;
    let is_measure = |i: usize| result.columns[i].role == ColumnRole::Measure;
    let subject: Vec<&str> = cells
        .iter()
        .filter(|(i, _)| !is_measure(*i))
        .map(|(_, t)| t.as_str())
        .collect();
    let Some((_, value)) = cells.iter().find(|(i, _)| *i == measure) else {
        return build_cells_title(input, shape, cells, parts);
    };
    if subject.is_empty() {
        return build_cells_title(input, shape, cells, parts);
    }
    let label = lower_first(&drop_column_name(&build_measure_label(input, measure)));
    parts.names.extend(subject.iter().map(|s| s.to_string()));
    parts.names.push(label.clone());
    let word = if highest {
        "cao nhất"
    } else {
        "thấp nhất"
    };
    let mut title = format!("{} có {label} {word}: {value}.", subject.join(", "));
    let others: Vec<(usize, String)> = cells
        .iter()
        .filter(|(i, _)| is_measure(*i) && *i != measure)
        .cloned()
        .collect();
    if !others.is_empty() {
        title.push(' ');
        title.push_str(&build_cells_title(input, shape, &others, parts));
    }
    title
}

// "Quốc gia của khách hàng là Puerto Rico, thành phố của khách hàng là Caguas."
fn build_cells_title(
    input: &InsightInput<'_>,
    shape: &QueryShape,
    cells: &[(usize, String)],
    parts: &mut InsightParts,
) -> String {
    let said: Vec<String> = cells
        .iter()
        .map(|(i, text)| {
            let label = build_cell_label(input, shape, *i);
            parts.names.push(label.clone());
            if input.result.columns[*i].role != ColumnRole::Measure {
                parts.names.push(text.clone());
            }
            format!("{} là {text}", lower_first(&label))
        })
        .collect();
    format!("{}.", upper_first(&said.join(", ")))
}

/// Tên tiếng Việt của một cột kết quả: số đo lấy nhãn của thẻ "Đã hiểu là"; cột trơn không đặt bí danh lấy bí danh
/// đầu của hồ sơ cột; còn lại là tên cột kết quả (bí danh câu SQL đặt).
fn build_cell_label(input: &InsightInput<'_>, shape: &QueryShape, i: usize) -> String {
    let result = input.result;
    if result.columns[i].role == ColumnRole::Measure {
        return drop_column_name(&build_measure_label(input, i));
    }
    let name = &result.columns[i].name;
    let source = match shape.outputs.get(i).and_then(|o| o.tree.as_ref()) {
        Some(Expr::Identifier(ident)) => Some(&ident.value),
        Some(Expr::CompoundIdentifier(idents)) => idents.last().map(|p| &p.value),
        _ => None,
    };
    let alias = source
        .filter(|s| s.eq_ignore_ascii_case(name))
        .and_then(|s| {
            input
                .tables
                .iter()
                .flat_map(|t| t.columns.iter())
                .find(|c| c.name.eq_ignore_ascii_case(s))?
                .aliases
                .iter()
                .map(|a| a.trim())
                .find(|a| !a.is_empty())
                .map(str::to_string)
        });
    alias.unwrap_or_else(|| name.clone())
}
