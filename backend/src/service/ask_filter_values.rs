// SPDX-License-Identifier: Apache-2.0
/* service/ask_filter_values.rs - phần tra cứu của luật filter_value_not_found trong kiểm hợp lý kết quả (#36,
ask_checks.rs): lấy giá trị chữ câu hỏi lọc theo từ cây cú pháp của sql_guarded và từ terms của mô hình, so với danh
sách giá trị của cột (ValueIndex của ask_values.rs, đọc từ column_values), giá trị không có thì kèm gợi ý giá trị gần
nhất. Chỉ cột chiều không nhạy cảm có danh sách; cột khác không kiểm. Test ở ask_checks_tests.rs. */

use sqlparser::ast::{BinaryOperator, Expr, Value};

use super::ask_phrases::fold;
use super::ask_reply::{Term, TermKind};
use super::ask_values::{ValueEntry, ValueIndex};
use crate::engine::sql_shape::QueryShape;
use crate::engine::store::ProfileColumn;

/// Số giá trị gợi ý tối đa cho một giá trị không có.
const SUGGESTIONS_MAX: usize = 3;

/// Một giá trị chữ câu hỏi lọc theo trên một cột; column là tên cột đúng như hồ sơ.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterValue {
    pub column: String,
    pub value: String,
}

/// Giá trị lọc mà cột không có, kèm giá trị gần nhất của cột (có thể rỗng).
#[derive(Debug, Clone, PartialEq)]
pub struct MissingValue {
    pub column: String,
    pub value: String,
    pub suggestions: Vec<String>,
}

/// Giá trị chữ câu lọc theo, mỗi cặp (cột, giá trị) một lần: vế `cột = 'x'`, `'x' = cột`, `cột IN ('x', ...)` của câu,
/// rồi terms kind value của mô hình. Vế của câu chỉ đọc khi câu đọc thẳng bảng dữ liệu (QueryShape::table): đọc từ bảng
/// tạm (WITH) thì tên cột trong WHERE có thể là cột tính ra, không phải cột của hồ sơ.
pub fn build_filter_values(
    shape: Option<&QueryShape>,
    row_filters: &[String],
    terms: &[Term],
    columns: &[ProfileColumn],
) -> Vec<FilterValue> {
    let mut found: Vec<(String, String)> = Vec::new();
    if let Some(shape) = shape.filter(|s| s.table.is_some()) {
        for filter in &shape.filters {
            if filter.in_table_box && row_filters.contains(&filter.sql) {
                continue;
            }
            found.extend(build_compared_values(&filter.tree));
        }
    }
    for term in terms.iter().filter(|t| t.kind == TermKind::Value) {
        if let (Some(column), Some(value)) = (&term.column, &term.value) {
            found.push((column.clone(), value.clone()));
        }
    }
    let mut values: Vec<FilterValue> = Vec::new();
    for (name, value) in found {
        let Some(column) = find_column(columns, &name) else {
            continue;
        };
        let item = FilterValue {
            column: column.name.clone(),
            value,
        };
        if !values.contains(&item) {
            values.push(item);
        }
    }
    values
}

/// (tên cột như đã viết, giá trị) của một vế bằng hay IN với hằng chữ. Vế khác (LIKE, lower(cột) = ..., NOT IN) không
/// đọc: không biết chắc giá trị nào phải có trong cột.
fn build_compared_values(expr: &Expr) -> Vec<(String, String)> {
    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Eq,
            right,
        } => {
            let pair = match (column_name(left), text_literal(right)) {
                (Some(column), Some(value)) => Some((column, value)),
                _ => column_name(right).zip(text_literal(left)),
            };
            pair.into_iter().collect()
        }
        Expr::InList {
            expr,
            list,
            negated: false,
        } => match column_name(expr) {
            Some(column) => list
                .iter()
                .filter_map(text_literal)
                .map(|value| (column.clone(), value))
                .collect(),
            None => Vec::new(),
        },
        Expr::Nested(inner) => build_compared_values(inner),
        _ => Vec::new(),
    }
}

/// Tên cột của một biểu thức chỉ là cột (`"Market"`, `t."Market"`).
pub(super) fn column_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(ident) => Some(ident.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.clone()),
        Expr::Nested(inner) => column_name(inner),
        _ => None,
    }
}

fn text_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(value) => match &value.value {
            Value::SingleQuotedString(text) => Some(text.clone()),
            _ => None,
        },
        Expr::Nested(inner) => text_literal(inner),
        _ => None,
    }
}

/// Cột của hồ sơ theo tên: đúng chữ trước, rồi không phân biệt hoa thường như DuckDB đọc tên cột.
pub(super) fn find_column<'a>(
    columns: &'a [ProfileColumn],
    name: &str,
) -> Option<&'a ProfileColumn> {
    columns
        .iter()
        .find(|c| c.name == name)
        .or_else(|| columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)))
}

/// Giá trị lọc không có trong danh sách giá trị của cột. Cột không có trong index (không phải cột chiều, cột nhạy cảm,
/// hay cột không có giá trị nào) và cột có giá trị dài bị bỏ khỏi index (danh sách không đủ) thì không kiểm.
pub fn build_missing_values(wanted: &[FilterValue], index: &ValueIndex) -> Vec<MissingValue> {
    wanted
        .iter()
        .filter_map(|filter| {
            let column = index
                .columns
                .iter()
                .find(|c| c.column_name == filter.column && !c.dropped)?;
            if column.values.iter().any(|e| e.value == filter.value) {
                return None;
            }
            Some(MissingValue {
                column: filter.column.clone(),
                value: filter.value.clone(),
                suggestions: build_suggestions(&filter.value, &column.values),
            })
        })
        .collect()
}

/// Giá trị gần nhất của cột, nhiều dòng trước: trước hết giá trị hay bí danh bằng nhau khi bỏ dấu và hoa thường
/// ("europe", "Ha Noi"); không có thì giá trị khác tối đa một chữ (từ 5 chữ trở lên thì hai chữ), như "Europa" với
/// "Europe". Ngưỡng nhỏ để không gợi ý một giá trị chẳng liên quan.
fn build_suggestions(value: &str, values: &[ValueEntry]) -> Vec<String> {
    let folded = fold(value);
    let same: Vec<String> = values
        .iter()
        .filter(|e| fold(&e.value) == folded || e.aliases.iter().any(|a| fold(a) == folded))
        .map(|e| e.value.clone())
        .take(SUGGESTIONS_MAX)
        .collect();
    if !same.is_empty() {
        return same;
    }
    let limit = if folded.chars().count() >= 5 { 2 } else { 1 };
    let mut near: Vec<(usize, &str)> = values
        .iter()
        .map(|e| (edit_distance(&folded, &fold(&e.value)), e.value.as_str()))
        .filter(|(distance, _)| *distance <= limit)
        .collect();
    // sort_by_key giữ thứ tự cũ khi cùng khoảng cách, tức giá trị nhiều dòng trước.
    near.sort_by_key(|(distance, _)| *distance);
    near.into_iter()
        .take(SUGGESTIONS_MAX)
        .map(|(_, v)| v.to_string())
        .collect()
}

/// Số chữ phải thêm, bớt hay thay để a thành b (khoảng cách Levenshtein), đếm theo ký tự chứ không theo byte.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    // previous[j]: khoảng cách giữa phần a đã đọc và j chữ đầu của b.
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let replace = previous[j] + usize::from(ca != *cb);
            current[j + 1] = replace.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        previous = current;
    }
    previous[b.len()]
}
