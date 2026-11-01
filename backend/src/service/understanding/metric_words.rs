// SPDX-License-Identifier: Apache-2.0
/* service/understanding/metric_words.rs - dòng "Đo" của thẻ "Cách tính" cho một chỉ số (#185): tên chỉ số kèm cách
tính bằng lời, để người dùng không phải đọc SQL mới biết "Doanh thu" là cộng cột nào. Có mô tả thì dùng mô tả (admin
viết ở màn Chỉ số); không có thì đọc công thức: "tổng cột X", "trung bình cột X"... Công thức dạng khác (thương, biểu
thức lồng) không đọc thành lời được thì chỉ có tên. Thẻ câu thường (labels.rs) và câu "tại sao" (why.rs) cùng gọi. */

use sqlparser::ast::{DuplicateTreatment, Expr, FunctionArg, FunctionArgExpr, FunctionArguments};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::store::MetricDefinition;

/// "<tên>: <mô tả>", hoặc "<tên>: <cách tính>" khi không có mô tả, hoặc chỉ tên.
pub fn build_metric_label(metric: &MetricDefinition) -> String {
    let description = metric.description.trim();
    if !description.is_empty() {
        return format!("{}: {description}", metric.name);
    }
    match build_expression_words(&metric.expression) {
        Some(words) => format!("{}: {words}", metric.name),
        None => metric.name.clone(),
    }
}

/// Lời của một hàm tổng hợp trên đúng một cột (hay COUNT(*)); None với công thức dạng khác.
pub fn build_expression_words(expression: &str) -> Option<String> {
    let expr = Parser::new(&DuckDbDialect {})
        .try_with_sql(expression)
        .and_then(|mut parser| parser.parse_expr())
        .ok()?;
    let Expr::Function(function) = &expr else {
        return None;
    };
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    if function.filter.is_some() || function.over.is_some() {
        return None;
    }
    let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
    let name = function.name.to_string().to_lowercase();
    let column = match list.args.as_slice() {
        [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)] if name == "count" && !distinct => {
            return Some("số dòng".to_string());
        }
        [FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Identifier(ident)))] => &ident.value,
        _ => return None,
    };
    let words = match (name.as_str(), distinct) {
        ("sum", false) => format!("tổng cột {column}"),
        ("avg" | "mean", false) => format!("trung bình cột {column}"),
        ("count", false) => format!("số dòng có cột {column}"),
        ("count", true) => format!("số giá trị khác nhau của cột {column}"),
        ("min", _) => format!("giá trị nhỏ nhất của cột {column}"),
        ("max", _) => format!("giá trị lớn nhất của cột {column}"),
        _ => return None,
    };
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(description: &str, expression: &str) -> MetricDefinition {
        MetricDefinition {
            name: "Doanh thu".to_string(),
            description: description.to_string(),
            expression: expression.to_string(),
            filter: None,
            time_column: None,
            unit: None,
            confirmed: true,
        }
    }

    #[test]
    fn description_first_then_words_from_the_expression() {
        let cases = [
            (
                "Tiền sau giảm giá",
                "SUM(\"Sales\")",
                "Doanh thu: Tiền sau giảm giá",
            ),
            (
                "",
                "SUM(\"Order Item Total\")",
                "Doanh thu: tổng cột Order Item Total",
            ),
            (
                "",
                "avg(\"studied_credits\")",
                "Doanh thu: trung bình cột studied_credits",
            ),
            (
                "",
                "COUNT(DISTINCT \"Order Id\")",
                "Doanh thu: số giá trị khác nhau của cột Order Id",
            ),
            ("", "COUNT(*)", "Doanh thu: số dòng"),
            ("", "SUM(\"a\") / SUM(\"b\")", "Doanh thu"),
            ("  ", "SUM(\"a\" * 2)", "Doanh thu"),
        ];
        for (description, expression, label) in cases {
            assert_eq!(
                build_metric_label(&metric(description, expression)),
                label,
                "{expression}"
            );
        }
    }
}
