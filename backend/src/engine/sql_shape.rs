// SPDX-License-Identifier: Apache-2.0
/* engine/sql_shape.rs - đọc hình dạng của một câu SELECT từ cây sqlparser: cột kết quả (cột nào là tổng hợp), các vế
lọc, khóa nhóm, thứ tự, LIMIT và bảng đọc. Thẻ "Đã hiểu là" (service/understanding/mod.rs, #35), truy dòng nguồn (#34) và
kiểm hợp lý kết quả (#36) cùng đọc câu đã chạy (sql_guarded) qua đây, để ba chỗ hiểu câu giống nhau. Test ở
engine/sql_shape_tests.rs.

Chỉ đọc câu ngoài cùng. Câu có CTE hay câu con ở FROM thì đọc phần SELECT ngoài cùng, bảng đọc là None; câu UNION,
EXCEPT, INTERSECT chỉ có tên cột của vế đầu, ORDER BY và LIMIT (set_operation là true). Đọc không hết thì trả phần
đọc được, không báo lỗi: người dùng vẫn có thẻ, còn #34 tự từ chối dạng nó không truy được.

Mọi đoạn chữ ở đây in từ chính cây của câu đưa vào, nên với câu đã in từ cây (sql_guarded do guard in ra) chúng là
đoạn nguyên văn của câu: Display của sqlparser in một nút bằng cách ghép chữ của các nút con. */

use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, GroupByExpr, LimitClause, OrderByKind, OrderBySort, Query, Select,
    SelectItem, SetExpr, Statement, TableFactor, Value, Visit, Visitor,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::error::AppError;

/// Hàm tổng hợp DuckDB, viết thường. Cột kết quả gọi một hàm này (ngoài câu con, không kèm OVER) là cột tổng hợp.
// Tách khỏi danh sách hàm được phép của guard: danh sách đó trộn mọi loại hàm, ở đây chỉ cần biết hàm nào gộp dòng.
#[rustfmt::skip]
const AGGREGATE_FUNCTIONS: &[&str] = &[
    "count", "sum", "avg", "mean", "min", "max", "median", "mode", "stddev", "stddev_samp", "stddev_pop", "variance",
    "var_samp", "var_pop", "quantile_cont", "quantile_disc", "percentile_cont", "percentile_disc",
    "approx_count_distinct", "count_if", "any_value", "arg_max", "arg_min", "max_by", "min_by", "first", "last",
    "string_agg", "bool_and", "bool_or", "corr", "covar_pop", "covar_samp", "product",
];

/// Hình dạng của câu ngoài cùng.
#[derive(Debug, Clone)]
pub struct QueryShape {
    /// Cột kết quả theo thứ tự trong SELECT.
    pub outputs: Vec<OutputColumn>,
    /// Các vế AND của WHERE: trước là vế trong hộp đọc bảng (in_table_box), trong ra ngoài, rồi tới WHERE của câu
    /// ngoài cùng theo thứ tự viết.
    pub filters: Vec<ShapeFilter>,
    /// Các vế AND của HAVING. Tách khỏi filters vì chúng lọc nhóm sau khi tổng hợp, không lọc dòng nguồn.
    pub having: Vec<ShapeFilter>,
    pub group_by: Vec<ShapeKey>,
    pub order_by: Vec<ShapeOrder>,
    /// Số trong LIMIT của câu ngoài cùng, như đã viết.
    pub limit: Option<String>,
    /// Bảng dữ liệu câu ngoài cùng đọc, như đã viết (không nháy). None khi đọc bảng tạm (WITH), câu con không phải
    /// hộp đọc bảng, JOIN, nhiều bảng, hay câu là phép tập hợp.
    pub table: Option<String>,
    /// Câu ngoài cùng là UNION, EXCEPT hay INTERSECT.
    pub set_operation: bool,
}

/// Một cột kết quả.
#[derive(Debug, Clone)]
pub struct OutputColumn {
    /// Bí danh nếu có, tên cột nếu là cột trơn, còn lại là chữ của biểu thức (DuckDB có thể đặt tên khác).
    pub name: String,
    /// Chữ của biểu thức, không kèm bí danh; với `*` là chính `*`.
    pub expr: String,
    /// Cây của biểu thức; None với `*`, `t.*`, vì không biết trước chúng ra bao nhiêu cột.
    pub tree: Option<Expr>,
    pub is_aggregate: bool,
}

/// Một vế lọc.
#[derive(Debug, Clone)]
pub struct ShapeFilter {
    pub sql: String,
    pub tree: Expr,
    /// Vế nằm trong hộp `(SELECT * FROM bảng WHERE ...)` ở FROM. Guard bọc bảng như vậy để lọc theo phân quyền (xem
    /// engine/guard/mod.rs, bước 3), mô hình cũng có thể tự viết; service so chữ với GuardedSql::row_filters để biết vế
    /// nào là phân quyền.
    pub in_table_box: bool,
}

/// Một khóa GROUP BY.
#[derive(Debug, Clone)]
pub struct ShapeKey {
    /// Chữ của khóa như đã viết: biểu thức, bí danh (`month`) hay số thứ tự cột (`1`).
    pub sql: String,
    pub tree: Expr,
    /// Cột kết quả khóa trỏ tới (theo số thứ tự, cùng biểu thức, hay bí danh); None khi khóa không có trong SELECT.
    pub output: Option<usize>,
}

/// Một mục ORDER BY.
#[derive(Debug, Clone)]
pub struct ShapeOrder {
    /// Chữ của cả mục, kể cả ASC, DESC: `revenue DESC`.
    pub sql: String,
    pub tree: Expr,
    /// Như ShapeKey::output.
    pub output: Option<usize>,
    pub descending: bool,
}

/// Lỗi chỉ khi chữ không phải đúng một câu truy vấn; câu đọc được một phần vẫn ra Ok. Câu đưa vào đã qua guard nên
/// lỗi ở đây là lỗi của máy chủ.
pub fn build_query_shape(sql: &str) -> Result<QueryShape, AppError> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql)
        .map_err(|e| AppError::Internal(format!("query shape: cannot parse sql: {e}")))?;
    match (statements.pop(), statements.is_empty()) {
        (Some(Statement::Query(query)), true) => Ok(build_shape_of_query(&query)),
        _ => Err(AppError::Internal(
            "query shape: sql is not a single query".to_string(),
        )),
    }
}

fn build_shape_of_query(query: &Query) -> QueryShape {
    let mut shape = QueryShape {
        outputs: Vec::new(),
        filters: Vec::new(),
        having: Vec::new(),
        group_by: Vec::new(),
        order_by: Vec::new(),
        limit: build_limit(query.limit_clause.as_ref()),
        table: None,
        set_operation: false,
    };
    match outer_select(&query.body) {
        Some(select) => apply_select(&mut shape, select, query),
        None => {
            shape.set_operation = true;
            if let Some(first) = first_select(&query.body) {
                shape.outputs = build_outputs(first);
            }
        }
    }
    shape.order_by = build_order_by(query, &shape.outputs);
    shape
}

/// SELECT của câu ngoài cùng; None khi câu là phép tập hợp. `(SELECT ...)` trong ngoặc vẫn là một SELECT.
fn outer_select(body: &SetExpr) -> Option<&Select> {
    match body {
        SetExpr::Select(select) => Some(select),
        SetExpr::Query(inner) => outer_select(&inner.body),
        _ => None,
    }
}

/// SELECT vế đầu của phép tập hợp: tên cột kết quả lấy theo vế đầu.
fn first_select(body: &SetExpr) -> Option<&Select> {
    match body {
        SetExpr::Select(select) => Some(select),
        SetExpr::Query(inner) => first_select(&inner.body),
        SetExpr::SetOperation { left, .. } => first_select(left),
        _ => None,
    }
}

fn apply_select(shape: &mut QueryShape, select: &Select, query: &Query) {
    shape.outputs = build_outputs(select);
    // Một chỗ đọc, không JOIN: mới biết bảng. Hộp đọc bảng góp vế lọc của nó vào filters.
    if let [from] = select.from.as_slice() {
        if from.joins.is_empty() {
            let table = read_table_box(&from.relation, &mut shape.filters);
            let cte_names: Vec<String> = query
                .with
                .iter()
                .flat_map(|with| &with.cte_tables)
                .map(|cte| cte.alias.name.value.to_lowercase())
                .collect();
            shape.table = table.filter(|name| !cte_names.contains(&name.to_lowercase()));
        }
    }
    if let Some(selection) = &select.selection {
        apply_conjuncts(selection, false, &mut shape.filters);
    }
    if let Some(having) = &select.having {
        apply_conjuncts(having, false, &mut shape.having);
    }
    shape.group_by = build_group_by(&select.group_by, &shape.outputs);
}

fn build_outputs(select: &Select) -> Vec<OutputColumn> {
    select.projection.iter().map(build_output).collect()
}

fn build_output(item: &SelectItem) -> OutputColumn {
    let (expr, alias) = match item {
        SelectItem::UnnamedExpr(expr) => (expr, None),
        SelectItem::ExprWithAlias { expr, alias } => (expr, Some(alias)),
        SelectItem::ExprWithAliases { expr, aliases } => (expr, aliases.first()),
        // `*`, `t.*`: không biết trước ra những cột nào.
        other => {
            return OutputColumn {
                name: other.to_string(),
                expr: other.to_string(),
                tree: None,
                is_aggregate: false,
            }
        }
    };
    let name = match (alias, expr) {
        (Some(alias), _) => alias.value.clone(),
        (None, Expr::Identifier(ident)) => ident.value.clone(),
        (None, Expr::CompoundIdentifier(parts)) => {
            parts.last().map_or(expr.to_string(), |p| p.value.clone())
        }
        (None, _) => expr.to_string(),
    };
    OutputColumn {
        name,
        expr: expr.to_string(),
        tree: Some(expr.clone()),
        is_aggregate: is_aggregate(expr),
    }
}

/// Có gọi hàm tổng hợp ở chính câu này: hàm trong câu con thuộc câu khác, hàm kèm OVER là hàm cửa sổ, không gộp dòng.
pub fn is_aggregate(expr: &Expr) -> bool {
    expr.visit(&mut AggregateFinder { depth: 0 }).is_break()
}

struct AggregateFinder {
    /// Số tầng câu con đang ở trong.
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
            let name = function.name.to_string().to_lowercase();
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

/// Tên bảng của một chỗ đọc: bảng trơn, hay hộp `(SELECT * FROM x WHERE ...)` (hộp lọc của guard). Truy dòng nguồn
/// (service/drill/sources.rs) và luật số cột của guard (guard/width.rs) dùng để biết hộp ra đúng các cột của bảng nào.
pub fn read_box_table(factor: &TableFactor) -> Option<String> {
    read_table_box(factor, &mut Vec::new())
}

/// Câu là đúng một hộp lọc trên bảng trơn: `SELECT * FROM <bảng> [WHERE ...]`, không gì khác. Luật số tầng của guard
/// (guard/depth.rs) không tính tầng của hộp như vậy.
pub fn is_table_box(query: &Query) -> bool {
    table_box_select(query).is_some_and(|select| {
        matches!(
            &select.from[0].relation,
            TableFactor::Table { args: None, .. }
        )
    })
}

/// Tên bảng sau các hộp `(SELECT * FROM x WHERE ...)` lồng nhau; vế lọc của các hộp vào filters, hộp trong trước. None
/// khi gặp thứ khác hộp: câu con có cột riêng, GROUP BY, JOIN, hàm sinh bảng.
fn read_table_box(factor: &TableFactor, filters: &mut Vec<ShapeFilter>) -> Option<String> {
    match factor {
        TableFactor::Table {
            name, args: None, ..
        } => match name.0.as_slice() {
            [part] => part.as_ident().map(|ident| ident.value.clone()),
            _ => None,
        },
        TableFactor::Derived {
            lateral: false,
            subquery,
            ..
        } => {
            let select = table_box_select(subquery)?;
            // Gom vế của hộp trong vào danh sách riêng: hộp trong không phải hộp thì không góp vế nào.
            let mut inner = Vec::new();
            let table = read_table_box(&select.from[0].relation, &mut inner)?;
            filters.append(&mut inner);
            if let Some(selection) = &select.selection {
                apply_conjuncts(selection, true, filters);
            }
            Some(table)
        }
        _ => None,
    }
}

/// SELECT của câu con khi nó chỉ là `SELECT * FROM <một chỗ đọc> [WHERE ...]`, đúng dạng hộp lọc của guard.
fn table_box_select(subquery: &Query) -> Option<&Select> {
    if subquery.with.is_some() || subquery.order_by.is_some() || subquery.limit_clause.is_some() {
        return None;
    }
    let SetExpr::Select(select) = subquery.body.as_ref() else {
        return None;
    };
    let plain_star = matches!(select.projection.as_slice(), [SelectItem::Wildcard(options)]
        if options.to_string().is_empty());
    let no_grouping =
        matches!(&select.group_by, GroupByExpr::Expressions(keys, _) if keys.is_empty());
    let single_read = select.from.len() == 1 && select.from[0].joins.is_empty();
    let simple = plain_star
        && no_grouping
        && single_read
        && select.distinct.is_none()
        && select.having.is_none()
        && select.qualify.is_none();
    simple.then_some(select.as_ref())
}

/// Tách theo AND ở tầng ngoài; vế trong ngoặc giữ nguyên một vế, vì chữ của nó có cả ngoặc.
fn apply_conjuncts(expr: &Expr, in_table_box: bool, out: &mut Vec<ShapeFilter>) {
    if let Expr::BinaryOp {
        left,
        op: BinaryOperator::And,
        right,
    } = expr
    {
        apply_conjuncts(left, in_table_box, out);
        apply_conjuncts(right, in_table_box, out);
    } else {
        out.push(ShapeFilter {
            sql: expr.to_string(),
            tree: expr.clone(),
            in_table_box,
        });
    }
}

fn build_group_by(group_by: &GroupByExpr, outputs: &[OutputColumn]) -> Vec<ShapeKey> {
    match group_by {
        GroupByExpr::Expressions(keys, _) => keys
            .iter()
            .map(|key| ShapeKey {
                sql: key.to_string(),
                tree: key.clone(),
                output: find_output(key, outputs),
            })
            .collect(),
        // GROUP BY ALL: mọi cột kết quả không tổng hợp là khóa.
        GroupByExpr::All(_) => outputs
            .iter()
            .enumerate()
            .filter(|(_, output)| !output.is_aggregate)
            .filter_map(|(i, output)| {
                output.tree.as_ref().map(|tree| ShapeKey {
                    sql: output.expr.clone(),
                    tree: tree.clone(),
                    output: Some(i),
                })
            })
            .collect(),
    }
}

// ORDER BY ALL không đọc: hiếm, và không có biểu thức để trỏ về cột nào.
fn build_order_by(query: &Query, outputs: &[OutputColumn]) -> Vec<ShapeOrder> {
    let Some(order_by) = &query.order_by else {
        return Vec::new();
    };
    let OrderByKind::Expressions(items) = &order_by.kind else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| ShapeOrder {
            sql: item.to_string(),
            tree: item.expr.clone(),
            output: find_output(&item.expr, outputs),
            descending: matches!(item.options.sort, Some(OrderBySort::Desc)),
        })
        .collect()
}

/// Cột kết quả mà khóa nhóm hay mục ORDER BY trỏ tới. So cùng biểu thức trước bí danh: DuckDB hiểu `GROUP BY x` là cột
/// x của bảng khi bảng có cột đó, dù SELECT có bí danh x.
fn find_output(expr: &Expr, outputs: &[OutputColumn]) -> Option<usize> {
    if let Expr::Value(value) = expr {
        // Số thứ tự cột chỉ đếm được khi không có `*` (không biết `*` ra bao nhiêu cột).
        let Value::Number(number, _) = &value.value else {
            return None;
        };
        let position = number.parse::<usize>().ok()?;
        let countable = outputs.iter().all(|output| output.tree.is_some());
        return (countable && (1..=outputs.len()).contains(&position)).then(|| position - 1);
    }
    let text = expr.to_string();
    if let Some(i) = outputs
        .iter()
        .position(|o| o.tree.is_some() && o.expr == text)
    {
        return Some(i);
    }
    let Expr::Identifier(ident) = expr else {
        return None;
    };
    outputs
        .iter()
        .position(|o| o.tree.is_some() && o.name.eq_ignore_ascii_case(&ident.value))
}

// `LIMIT m, n` không đọc: guard đã từ chối dạng này (chỉ nhận LIMIT n, LIMIT n OFFSET m).
fn build_limit(limit: Option<&LimitClause>) -> Option<String> {
    match limit? {
        LimitClause::LimitOffset { limit, .. } => limit.as_ref().map(|l| l.to_string()),
        LimitClause::OffsetCommaLimit { .. } => None,
    }
}
