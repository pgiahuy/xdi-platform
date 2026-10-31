// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_shape.rs - hình dạng kết quả của một SELECT cho luật nối bảng (guard/joins.rs): các cột câu trả ra
kèm cột gốc, và câu có giữ nguyên dòng của nguồn duy nhất không (Grain::Rows) hay đã gộp nhóm (Grain::Derived). Kiểu
Shape, Source ở guard/join_sources.rs. */

use std::ops::ControlFlow;

use sqlparser::ast::{
    Distinct, Expr, GroupByExpr, Query, Select, SelectItem, SelectItemQualifiedWildcardKind, Visit,
    Visitor,
};

use super::join_keys::Origin;
use super::join_sources::{resolve_column, Grain, Resolved, Shape, Source};
use super::name_key;

/// Hình dạng kết quả của một SELECT với các nguồn của nó (tầng cuối của levels).
pub(super) fn build_select_shape(select: &Select, levels: &[&[Source]]) -> Shape {
    let sources = levels.last().copied().unwrap_or(&[]);
    let mut columns = Vec::new();
    for item in &select.projection {
        match item {
            SelectItem::Wildcard(_) => {
                for source in sources {
                    columns.extend(source.shape.columns.iter().cloned());
                }
            }
            SelectItem::QualifiedWildcard(SelectItemQualifiedWildcardKind::ObjectName(name), _) => {
                let key = name.0.last().and_then(|p| p.as_ident()).map(name_key);
                if let Some(source) = sources.iter().find(|s| Some(&s.key) == key.as_ref()) {
                    columns.extend(source.shape.columns.iter().cloned());
                }
            }
            SelectItem::UnnamedExpr(expr) => {
                if let Some(name) = column_name(expr) {
                    columns.push((name, origin_of(expr, levels)));
                }
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                columns.push((name_key(alias), origin_of(expr, levels)));
            }
            SelectItem::ExprWithAliases { aliases, .. } => {
                columns.extend(aliases.iter().map(|a| (name_key(a), None)));
            }
            SelectItem::QualifiedWildcard(..) => {}
        }
    }
    let grain = match sources {
        [only] if keeps_rows(select) => only.shape.grain.clone(),
        _ => Grain::Derived,
    };
    Shape { grain, columns }
}

/// Cột gốc của một biểu thức chỉ là một cột; biểu thức tính ra thì None.
pub(super) fn origin_of(expr: &Expr, levels: &[&[Source]]) -> Option<Origin> {
    match resolve_column(expr, levels) {
        Resolved::Found(found) => found.origin,
        _ => None,
    }
}

/// Tên DuckDB đặt cho cột chọn thẳng: phần cuối của tên.
fn column_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Nested(inner) => column_name(inner),
        Expr::Identifier(ident) => Some(name_key(ident)),
        Expr::CompoundIdentifier(parts) => parts.last().map(name_key),
        _ => None,
    }
}

/// SELECT giữ nguyên dòng của nguồn duy nhất: không gộp nhóm, không DISTINCT, không hàm gộp (hàm gộp không GROUP BY
/// thu cả bảng về một dòng, như `SELECT SUM(x) AS total FROM orders`).
fn keeps_rows(select: &Select) -> bool {
    let grouped = match &select.group_by {
        GroupByExpr::All(_) => true,
        GroupByExpr::Expressions(exprs, _) => !exprs.is_empty(),
    };
    let distinct = matches!(
        select.distinct,
        Some(Distinct::Distinct) | Some(Distinct::On(_))
    );
    !grouped && !distinct && select.having.is_none() && !has_aggregate(select)
}

/// Hàm gộp trong ALLOWED_FUNCTIONS (guard/mod.rs). Cùng tên mà có OVER là hàm cửa sổ, giữ nguyên dòng.
const AGGREGATE_FUNCTIONS: &[&str] = &[
    "count",
    "sum",
    "avg",
    "mean",
    "min",
    "max",
    "median",
    "mode",
    "stddev",
    "stddev_samp",
    "stddev_pop",
    "variance",
    "var_samp",
    "var_pop",
    "quantile_cont",
    "quantile_disc",
    "percentile_cont",
    "percentile_disc",
    "approx_count_distinct",
    "count_if",
    "any_value",
    "arg_max",
    "arg_min",
    "max_by",
    "min_by",
    "first",
    "last",
    "string_agg",
    "bool_and",
    "bool_or",
    "corr",
    "covar_pop",
    "covar_samp",
    "product",
];

fn has_aggregate(select: &Select) -> bool {
    let mut finder = AggregateFinder { depth: 0 };
    select
        .projection
        .iter()
        .any(|item| item.visit(&mut finder).is_break())
}

/// Chỉ xét hàm của chính SELECT này, không của câu con trong danh sách cột.
struct AggregateFinder {
    depth: usize,
}

impl Visitor for AggregateFinder {
    type Break = ();

    fn pre_visit_query(&mut self, _query: &Query) -> ControlFlow<()> {
        self.depth += 1;
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, _query: &Query) -> ControlFlow<()> {
        self.depth -= 1;
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        if let Expr::Function(function) = expr {
            let last = function.name.0.last().and_then(|part| part.as_ident());
            let name = last.map(name_key).unwrap_or_default();
            if self.depth == 0
                && function.over.is_none()
                && AGGREGATE_FUNCTIONS.contains(&name.as_str())
            {
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }
}
