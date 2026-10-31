// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_sources.rs - nguồn dòng trong FROM của một SELECT cho luật nối bảng (guard/joins.rs): nguồn nào
giữ nguyên dòng của một bảng dữ liệu (Grain::Rows), cột của câu trỏ về cột nào của bảng dữ liệu (Origin), và hình dạng
kết quả của một câu con hay bảng tạm (Shape), để câu ngoài nối với nó vẫn biết cột khóa đến từ bảng nào.

Vì sao phân biệt Rows với Derived: nối hai nguồn giữ nguyên dòng là nối bảng thật, phải đúng khóa ngoại. Câu con đã gộp
nhóm (một dòng mỗi đơn, mỗi tháng) không còn là dòng của bảng nào; nối nó với bảng khác theo cột cùng miền giá trị
(customer_id của hai bảng tạm gộp theo khách) là cách đúng để tránh cộng lặp, nên luật lỏng hơn (joins.rs). */

use std::ops::ControlFlow;

use sqlparser::ast::{
    Distinct, Expr, GroupByExpr, Ident, Query, Select, SelectItem, SelectItemQualifiedWildcardKind,
    TableAlias, Visit, Visitor,
};

use super::join_keys::Origin;
use super::{name_key, AllowedTable};

/// Mỗi dòng của nguồn ứng với gì.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Grain {
    /// Một dòng của bảng dữ liệu (tên viết thường): chính bảng đó, hay câu con, bảng tạm chỉ lọc và chọn cột của nó.
    Rows(String),
    /// Kết quả gộp nhóm, DISTINCT, nối nhiều nguồn, UNION: dòng không còn là dòng của một bảng.
    Derived,
}

/// Kết quả của một câu hay một bảng: mỗi cột (tên viết thường) kèm cột gốc nếu cột chỉ chép từ một cột của bảng dữ
/// liệu, theo thứ tự cột của câu.
#[derive(Debug, Clone)]
pub(super) struct Shape {
    pub grain: Grain,
    pub columns: Vec<(String, Option<Origin>)>,
}

impl Shape {
    pub fn derived(columns: Vec<(String, Option<Origin>)>) -> Shape {
        Shape {
            grain: Grain::Derived,
            columns,
        }
    }

    pub fn of_table(table: &AllowedTable) -> Shape {
        let name = table.name.to_lowercase();
        let columns = table
            .columns
            .iter()
            .map(|c| {
                let column = c.to_lowercase();
                let origin = Origin {
                    table: name.clone(),
                    column: column.clone(),
                };
                (column, Some(origin))
            })
            .collect();
        Shape {
            grain: Grain::Rows(name),
            columns,
        }
    }

    /// `AS t(a, b)`, `WITH t(a, b)`: đổi tên cột theo vị trí.
    pub fn apply_alias_columns(&mut self, alias: Option<&TableAlias>) {
        let Some(alias) = alias else { return };
        for (slot, column) in self.columns.iter_mut().zip(&alias.columns) {
            slot.0 = name_key(&column.name);
        }
    }

    fn find(&self, column: &str) -> Option<&Option<Origin>> {
        self.columns
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, origin)| origin)
    }
}

/// Một nguồn trong FROM.
#[derive(Debug, Clone)]
pub(super) struct Source {
    /// Tên đứng trước dấu chấm: bí danh, hay tên bảng khi không đặt bí danh. Viết thường.
    pub key: String,
    pub shape: Shape,
}

impl Source {
    pub fn rows_table(&self) -> Option<&str> {
        match &self.shape.grain {
            Grain::Rows(table) => Some(table),
            Grain::Derived => None,
        }
    }
}

/// Chỗ một cột của câu trỏ tới: tầng câu (0 là câu ngoài cùng), nguồn thứ mấy của tầng đó, và cột gốc nếu biết.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Found {
    pub level: usize,
    pub index: usize,
    pub origin: Option<Origin>,
}

pub(super) enum Resolved {
    Found(Found),
    /// Cột không ghi bảng mà có ở hai nguồn của cùng tầng.
    Ambiguous(String),
    /// Không phải cột, hay không tìm thấy (tên câu tự đặt, cột của tầng không biết cột).
    Missing,
}

/// Tìm nguồn của một cột, từ tầng trong cùng ra ngoài như SQL. levels[i] là các nguồn của tầng i.
pub(super) fn resolve_column(expr: &Expr, levels: &[&[Source]]) -> Resolved {
    match expr {
        Expr::Nested(inner) => resolve_column(inner, levels),
        Expr::Identifier(column) => resolve_plain(&name_key(column), levels),
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [qualifier, column] => resolve_qualified(qualifier, column, levels),
            _ => Resolved::Missing,
        },
        _ => Resolved::Missing,
    }
}

fn resolve_plain(column: &str, levels: &[&[Source]]) -> Resolved {
    for (level, sources) in levels.iter().enumerate().rev() {
        let owners: Vec<usize> = (0..sources.len())
            .filter(|&i| sources[i].shape.find(column).is_some())
            .collect();
        match owners.as_slice() {
            [] => continue,
            [index] => {
                let origin = sources[*index].shape.find(column).cloned().flatten();
                return Resolved::Found(Found {
                    level,
                    index: *index,
                    origin,
                });
            }
            _ => return Resolved::Ambiguous(column.to_string()),
        }
    }
    Resolved::Missing
}

fn resolve_qualified(qualifier: &Ident, column: &Ident, levels: &[&[Source]]) -> Resolved {
    let key = name_key(qualifier);
    for (level, sources) in levels.iter().enumerate().rev() {
        if let Some(index) = sources.iter().position(|s| s.key == key) {
            let origin = sources[index]
                .shape
                .find(&name_key(column))
                .cloned()
                .flatten();
            return Resolved::Found(Found {
                level,
                index,
                origin,
            });
        }
    }
    Resolved::Missing
}

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
fn origin_of(expr: &Expr, levels: &[&[Source]]) -> Option<Origin> {
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
