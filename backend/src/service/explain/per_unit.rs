// SPDX-License-Identifier: Apache-2.0
/* service/explain/per_unit.rs - tên của một tổng cộng trên cột "mỗi đơn vị" (#260). Cột như "Benefit per order" (bí
danh "lợi nhuận mỗi đơn") mang giá trị của một đơn hay một dòng; SUM cộng các giá trị đó thành tổng lợi nhuận, nên tên
hiện cho người dùng phải là "Tổng lợi nhuận", không phải "Lợi nhuận mỗi đơn". Trước đây câu trả lời ghi "Lợi nhuận mỗi
đơn của COMPLETE cao hơn 14.971,35" khi con số là tổng của 597 đơn.

Luật đọc từ chính tên (bí danh, mô tả hay tên cột), không ghi cứng tên cột nào: tìm từ "mỗi" hay "per" đứng thành một từ
riêng và có ít nhất một từ sau nó (đơn vị), bỏ từ đó trở về sau cùng các từ nối đứng ngay trước ("trên", "của"), rồi
thêm "Tổng". Tên không có phần "mỗi đơn vị" thì giữ như cũ. Chỉ áp cho SUM: AVG giữ "Trung bình ...", COUNT giữ "Số dòng
có ...", vì hai cách đó không đổi nghĩa "mỗi đơn vị" của cột.

Bên dùng: explain/measure_column.rs (nhãn độ đo của câu "tại sao": thẻ "Đã hiểu là", câu chính, tiêu đề bảng phân rã,
JSON gửi mô hình viết lời), explain/breakdown.rs (nhãn giá trị trung bình mỗi đơn vị), understanding/labels.rs (thẻ của
câu thường), insight/mod.rs và ask/summary.rs (bí danh tiếng Việt câu SQL đặt), ask/total_alias.rs (đổi chính bí danh
trong câu SQL). Test ở explain/per_unit_tests.rs. */

use sqlparser::ast::{Expr, FunctionArg, FunctionArgExpr, FunctionArguments};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::service::explain::{lower_first, upper_first};

/// Từ mở đầu phần "mỗi đơn vị" của một tên, viết thường. "từng" không có ở đây: "đã từng mua" không phải "mỗi".
const PER_WORDS: &[&str] = &["mỗi", "per"];
/// Từ nối đứng ngay trước phần "mỗi đơn vị", bỏ theo: "lợi nhuận trên mỗi đơn hàng" thành "lợi nhuận".
const LINK_WORDS: &[&str] = &["trên", "của", "cho", "theo"];
/// Chữ đầu nhãn của một tổng, như nhãn dựng sẵn "Tổng <cột>".
const TOTAL_WORD: &str = "Tổng";

/// Phần trước "mỗi đơn vị" của một tên: "Lợi nhuận trên mỗi đơn hàng" thành "Lợi nhuận", "Benefit per order" thành
/// "Benefit", "profit_per_order" thành "profit". None khi tên không có phần đó, hay khi bỏ đi thì không còn gì.
pub fn strip_per_unit(name: &str) -> Option<&str> {
    let words = build_word_spans(name);
    let is_word = |index: usize, list: &[&str]| {
        let (start, end) = words[index];
        list.contains(&name[start..end].to_lowercase().as_str())
    };
    // Từ "mỗi" phải có một từ đơn vị theo sau: "lợi nhuận mỗi" không đủ để biết là mỗi gì.
    let marker = (1..words.len().saturating_sub(1)).find(|&i| is_word(i, PER_WORDS))?;
    let mut keep = marker;
    while keep > 0 && is_word(keep - 1, LINK_WORDS) {
        keep -= 1;
    }
    if keep == 0 {
        return None;
    }
    Some(&name[..words[keep - 1].1])
}

/// Tên của SUM trên một cột "mỗi đơn vị": "lợi nhuận mỗi đơn" thành "Tổng lợi nhuận". Tên đã bắt đầu bằng "tổng" thì
/// không thêm lần nữa. Tên tiếng Anh giữ hoa thường ("Tổng Benefit"), như nhãn "Tổng <cột>" khi chưa có tên tiếng Việt.
/// None khi tên không có phần "mỗi đơn vị": bên gọi giữ tên của mình.
pub fn build_total_name(name: &str) -> Option<String> {
    let base = strip_per_unit(name.trim())?;
    if starts_with_total(base) {
        return Some(upper_first(base));
    }
    let base = if base.is_ascii() {
        base.to_string()
    } else {
        lower_first(base)
    };
    Some(format!("{TOTAL_WORD} {base}"))
}

/// Tên một bí danh SQL nên hiện: bí danh có phần "mỗi đơn vị" đặt cho một SUM ("SUM(x) AS "Lợi nhuận mỗi đơn"") thì
/// thành tên tổng; còn lại giữ nguyên bí danh. sql là chữ của biểu thức tổng hợp.
pub fn build_alias_label(alias: &str, sql: &str) -> String {
    let total = parse_expr(sql)
        .filter(is_sum_expr)
        .and_then(|_| build_total_name(alias));
    total.unwrap_or_else(|| alias.to_string())
}

/// Nhãn của một tổng bỏ chữ "Tổng" đứng đầu, để ghép thành nhãn trung bình mỗi đơn vị: "Tổng lợi nhuận" thành "Lợi
/// nhuận" rồi "Lợi nhuận trên mỗi đơn hàng". Nhãn không bắt đầu bằng "Tổng" thì giữ nguyên.
pub fn drop_total_word(label: &str) -> String {
    // "Tổng", "tổng" và "TỔNG" cùng số byte, nên cắt theo độ dài của TOTAL_WORD là cắt đúng chữ đó.
    match label.get(TOTAL_WORD.len()..) {
        Some(rest) if starts_with_total(label) && !rest.trim().is_empty() => {
            upper_first(rest.trim_start())
        }
        _ => label.to_string(),
    }
}

/// Biểu thức là một SUM, có thể bọc trong ROUND, CAST hay ngoặc như mô hình hay viết: ROUND(SUM(x), 2).
pub fn is_sum_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => is_sum_expr(inner),
        Expr::Function(function) => {
            let name = function.name.to_string().to_lowercase();
            match name.as_str() {
                "sum" => function.over.is_none(),
                "round" => first_arg(function).is_some_and(is_sum_expr),
                _ => false,
            }
        }
        _ => false,
    }
}

fn first_arg(function: &sqlparser::ast::Function) -> Option<&Expr> {
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    match list.args.first()? {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
        _ => None,
    }
}

fn parse_expr(sql: &str) -> Option<Expr> {
    Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_expr())
        .ok()
}

// "tổng" đứng thành một từ ở đầu tên, không phân biệt hoa thường.
fn starts_with_total(text: &str) -> bool {
    let lower = text.to_lowercase();
    let word = TOTAL_WORD.to_lowercase();
    lower
        .strip_prefix(&word)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Vị trí (byte đầu, byte cuối) của từng từ, ngăn bằng khoảng trắng hay "_" như tên cột "profit_per_order".
fn build_word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    for (index, c) in text.char_indices() {
        let separator = c.is_whitespace() || c == '_';
        match (separator, start) {
            (true, Some(begin)) => {
                spans.push((begin, index));
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    if let Some(begin) = start {
        spans.push((begin, text.len()));
    }
    spans
}
