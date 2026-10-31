// SPDX-License-Identifier: Apache-2.0
/* service/drill/measures.rs - độ đo của ô được bấm, để người dùng so dòng nguồn với con số (drill/sql.rs dựng câu,
drill/mod.rs chạy). Mỗi cột tổng hợp của câu đã chạy được tính lại một lần trên mọi dòng nguồn của ô (cùng câu đếm
dòng), nên giá trị đó bằng ô khi tập dòng đúng là tập đã ra ô. SUM(x) còn có cột x của từng dòng trên trang, để người
dùng thấy các dòng cộng ra con số; COUNT(*) thì mỗi dòng đếm một, tổng là số dòng nguồn.

Độ đo khác (AVG, COUNT DISTINCT, tỷ lệ, ROUND(SUM(x), 2)) không cộng theo dòng: chỉ có giá trị tính lại, không có cột
từng dòng. SUM(DISTINCT x) và SUM(x) FILTER (WHERE ...) cũng vào nhóm này, vì cột x của từng dòng cộng lại không ra
con số. SUM(CASE WHEN ... THEN x END) thì vẫn là SUM: cột từng dòng là chính biểu thức CASE, dòng ngoài điều kiện có
giá trị trống. */

use sqlparser::ast::{Expr, FunctionArg, FunctionArgExpr, FunctionArguments};

use crate::engine::sql_shape::QueryShape;
use crate::service::drill::contract::MeasureRule;

/// Một cột tổng hợp của câu đã chạy.
#[derive(Debug, Clone)]
pub struct PlanMeasure {
    /// Chỉ số của cột trong kết quả câu đã chạy.
    pub output: usize,
    pub aggregate: Expr,
    pub rule: MeasureRule,
    /// Biểu thức của từng dòng, với SUM.
    pub row_expr: Option<Expr>,
}

/// Mọi cột tổng hợp, theo thứ tự trong SELECT.
pub fn build_measures(shape: &QueryShape) -> Vec<PlanMeasure> {
    shape
        .outputs
        .iter()
        .enumerate()
        .filter(|(_, output)| output.is_aggregate)
        .filter_map(|(output, column)| {
            let aggregate = column.tree.clone()?;
            let (rule, row_expr) = read_rule(&aggregate);
            Some(PlanMeasure {
                output,
                aggregate,
                rule,
                row_expr,
            })
        })
        .collect()
}

fn read_rule(expr: &Expr) -> (MeasureRule, Option<Expr>) {
    let other = (MeasureRule::Other, None);
    let Expr::Function(function) = expr else {
        return other;
    };
    let FunctionArguments::List(list) = &function.args else {
        return other;
    };
    let plain = function.filter.is_none()
        && function.over.is_none()
        && function.within_group.is_empty()
        && list.duplicate_treatment.is_none()
        && list.clauses.is_empty();
    if !plain {
        return other;
    }
    let name = function.name.to_string().to_lowercase();
    match (name.as_str(), list.args.as_slice()) {
        ("sum", [FunctionArg::Unnamed(FunctionArgExpr::Expr(arg))]) => {
            (MeasureRule::Sum, Some(arg.clone()))
        }
        ("count", [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)]) => (MeasureRule::Count, None),
        _ => other,
    }
}
