// SPDX-License-Identifier: Apache-2.0
/* service/insight/plan.rs - đọc câu SQL của câu thường (sql_generated: trên tên bảng logic, trước guard) thành các phần
phân rã tự động cần (#227), rồi dựng WhyPlan cho hai kỳ, để dùng lại nguyên đường phân rã của câu "tại sao"
(explain/sql.rs, explain/breakdown.rs) mà không gọi mô hình điền form. Test ở insight/tests.rs.

Bảng chính là bảng của độ đo: mọi bảng khác trong FROM phải đi tới được từ nó theo quan hệ nhiều-một (explain/source.rs),
như luật chặn cộng lặp của ask/fanout.rs; câu nối theo chiều khác, có câu con, WITH hay phép tập hợp thì không phân rã.
Mọi biểu thức (độ đo, vế lọc, khóa thời gian) được viết lại theo nguồn của plan: cột của bảng chính thành tên trơn, cột
của bảng liên quan thành "bảng.cột" (tên cột của câu con nối bảng, explain/source.rs). Cột không chỉ rõ bảng mà có ở
nhiều bảng thì không đoán, không phân rã.

Vế lọc lấy nguyên từ câu của mô hình, nên phân rã đếm đúng tập dòng của câu hỏi; câu do code dựng vẫn qua guard với bộ
lọc dòng của người hỏi. Câu của mô hình nối bằng điều kiện khác quan hệ đã ghi thì tổng do code tính lệch ô của bảng
kết quả, và insight/mod.rs bỏ phân rã. */

use std::collections::HashMap;
use std::ops::ControlFlow;

use sqlparser::ast::{
    visit_expressions, visit_expressions_mut, BinaryOperator, Expr, Ident, Query, SelectItem,
    SetExpr, Statement, TableFactor,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::store::ProfileColumn;
use crate::service::ask::fanout::JoinLink;
use crate::service::ask::values::ValueIndex;
use crate::service::explain::check::{
    apply_related, build_dimension_candidates, build_labels, build_volume, PlanSlices, PlanTime,
    WhyPlan, WhyProfile,
};
use crate::service::explain::measure::{build_measure_from_expr, PlanMeasure};
use crate::service::explain::period::Period;
use crate::service::explain::source::{build_related_tables, RelatedTables, SourceTable};

/// Một bảng của nhóm và hồ sơ cột, kèm số dòng (để biết cột key có lặp lại không, lượng của #40).
pub struct PlainTable<'a> {
    pub table: &'a str,
    pub columns: &'a [ProfileColumn],
    pub row_count: Option<i64>,
}

/// Câu thường đã đọc, mọi biểu thức đã viết lại theo nguồn của plan.
pub struct PlainQuery {
    pub table: String,
    pub columns: Vec<ProfileColumn>,
    pub row_count: Option<i64>,
    pub related: RelatedTables,
    pub measure: PlanMeasure,
    /// Các vế AND của WHERE.
    pub filters: Vec<Expr>,
    /// Biểu thức của các cột kết quả, theo thứ tự SELECT; None với `*`.
    pub outputs: Vec<Option<Expr>>,
}

/// None khi câu không thuộc dạng phân rã được (xem đầu file). measure là vị trí cột độ đo trong SELECT; label là tên
/// cột đó trong bảng kết quả, dùng làm tên độ đo.
pub fn read_plain_query(
    sql: &str,
    measure: usize,
    label: &str,
    tables: &[PlainTable],
    links: &[JoinLink],
) -> Option<PlainQuery> {
    let query = parse_query(sql)?;
    if query.with.is_some() {
        return None;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let aliases = build_aliases(select, tables)?;
    let outputs: Vec<Option<Expr>> = select
        .projection
        .iter()
        .map(|item| match item {
            SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                Some(expr.clone())
            }
            _ => None,
        })
        .collect();
    let measure_expr = outputs.get(measure)?.clone()?;
    let root = find_root(&measure_expr, &aliases, tables, links)?;
    let root_table = tables.iter().find(|t| t.table == root)?;
    let others: Vec<SourceTable> = tables
        .iter()
        .filter(|t| t.table != root)
        .map(|t| SourceTable {
            table: t.table,
            columns: t.columns,
        })
        .collect();
    let related = build_related_tables(&root, &others, links);
    let resolver = Resolver {
        root: root_table,
        aliases: &aliases,
        tables,
        related: &related,
    };
    let measure_expr = resolver.rewrite(&measure_expr)?;
    let plan_measure = build_measure_from_expr(label, &measure_expr, root_table.columns)?;
    let mut filters = Vec::new();
    if let Some(selection) = &select.selection {
        for conjunct in split_and(selection) {
            filters.push(resolver.rewrite(conjunct)?);
        }
    }
    let outputs = outputs
        .iter()
        .map(|o| o.as_ref().and_then(|e| resolver.rewrite(e)))
        .collect();
    Some(PlainQuery {
        table: root,
        columns: root_table.columns.to_vec(),
        row_count: root_table.row_count,
        related,
        measure: plan_measure,
        filters,
        outputs,
    })
}

impl PlainQuery {
    /// Hồ sơ của một tên cột trong nguồn (tên trơn của bảng chính, "bảng.cột" của bảng liên quan), đổi tên thành tên
    /// trong nguồn để build_time_expr dựng đúng biểu thức.
    pub fn find_column(&self, name: &str, tables: &[PlainTable]) -> Option<ProfileColumn> {
        if let Some(column) = self.columns.iter().find(|c| c.name == name) {
            return Some(column.clone());
        }
        let related = self
            .related
            .source
            .columns
            .iter()
            .find(|c| c.name == name)?;
        let join = &self.related.source.joins[related.join];
        let table = tables.iter().find(|t| t.table == join.table)?;
        let mut column = table
            .columns
            .iter()
            .find(|c| c.name == related.column)?
            .clone();
        column.name = name.to_string();
        Some(column)
    }

    /// WhyPlan so hai kỳ. filters là các vế lọc dùng (đã bỏ vế thời gian khi so kỳ trước của một con số).
    pub fn build_period_plan(
        &self,
        time: PlanTime,
        base: Period,
        compare: Period,
        filters: Vec<Expr>,
    ) -> WhyPlan {
        let fixed = build_fixed_columns(&filters);
        let time_column = time.column.clone();
        let mut excluded: Vec<&str> = fixed.iter().map(String::as_str).collect();
        excluded.push(&time_column);
        let empty = ValueIndex::default();
        let profile = WhyProfile {
            table: &self.table,
            columns: &self.columns,
            values: &empty,
            metrics: &[],
            row_count: self.row_count,
        };
        let used = build_used_names(&filters, &time);
        let mut plan = WhyPlan {
            table: self.table.clone(),
            measure: self.measure.clone(),
            slices: PlanSlices::Periods {
                time,
                base,
                compare,
                base_by_default: true,
            },
            filters,
            dimensions: build_dimension_candidates(&self.columns, &excluded),
            volume: build_volume(&profile),
            labels: build_labels(&profile),
            source: Default::default(),
        };
        apply_related(&mut plan, &self.related, &fixed);
        // Cột liên quan của vế lọc và cột thời gian: mọi câu của phân rã đều cần, kể cả câu tổng hai kỳ.
        plan.source.always = used
            .into_iter()
            .filter(|name| plan.source.columns.iter().any(|c| c.name == *name))
            .collect();
        plan
    }
}

/// Tên cột độ đo trong câu tổng của top N.
const TOTAL_MEASURE: &str = "insight_measure";

/// Câu tổng của top N: câu của mô hình bỏ ORDER BY, LIMIT, đặt tên cột độ đo, rồi cộng độ đo và đếm nhóm của mọi nhóm.
/// None khi độ đo không phải SUM hay COUNT (trung bình hay tỷ lệ của các nhóm không cộng thành tổng được).
pub fn build_total_sql(sql: &str, measure: usize) -> Option<String> {
    let mut query = parse_query(sql)?;
    let SetExpr::Select(select) = query.body.as_mut() else {
        return None;
    };
    let item = select.projection.get_mut(measure)?;
    let expr = match item {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => expr.clone(),
        _ => return None,
    };
    let Expr::Function(function) = &expr else {
        return None;
    };
    let name = function.name.to_string().to_lowercase();
    if name != "sum" && name != "count" {
        return None;
    }
    *item = SelectItem::ExprWithAlias {
        expr,
        alias: Ident::new(TOTAL_MEASURE),
    };
    query.order_by = None;
    query.limit_clause = None;
    Some(format!(
        "SELECT SUM({TOTAL_MEASURE}) AS total_value, COUNT(*) AS group_count FROM ({query}) AS all_groups"
    ))
}

fn parse_query(sql: &str) -> Option<Query> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    match (statements.pop(), statements.is_empty()) {
        (Some(Statement::Query(query)), true) => Some(*query),
        _ => None,
    }
}

/// Bí danh (viết thường) -> bảng của FROM: một nguồn kèm các JOIN, mỗi chỗ là một bảng của nhóm. None khi có câu con,
/// hàm ở chỗ bảng hay bảng ngoài nhóm.
fn build_aliases(
    select: &sqlparser::ast::Select,
    tables: &[PlainTable],
) -> Option<HashMap<String, String>> {
    let [from] = select.from.as_slice() else {
        return None;
    };
    let mut aliases = HashMap::new();
    let factors = std::iter::once(&from.relation).chain(from.joins.iter().map(|j| &j.relation));
    for factor in factors {
        let TableFactor::Table { name, alias, .. } = factor else {
            return None;
        };
        let [part] = name.0.as_slice() else {
            return None;
        };
        let written = part.as_ident()?.value.clone();
        let table = tables
            .iter()
            .find(|t| t.table.eq_ignore_ascii_case(&written))?;
        let key = alias
            .as_ref()
            .map_or(written.clone(), |a| a.name.value.clone());
        aliases.insert(key.to_lowercase(), table.table.to_string());
    }
    Some(aliases)
}

/// Bảng chính: bảng mà cột của độ đo thuộc về (COUNT(*) thì xét lần lượt các bảng của FROM), và từ đó đi tới được mọi
/// bảng khác của FROM theo quan hệ nhiều-một.
fn find_root(
    measure: &Expr,
    aliases: &HashMap<String, String>,
    tables: &[PlainTable],
    links: &[JoinLink],
) -> Option<String> {
    let in_from: Vec<&String> = {
        let mut list: Vec<&String> = aliases.values().collect();
        list.sort();
        list.dedup();
        list
    };
    let mut owners: Vec<String> = Vec::new();
    let _ = visit_expressions(measure, |e| {
        let owner = match e {
            Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
                aliases.get(&parts[0].value.to_lowercase()).cloned()
            }
            Expr::Identifier(ident) => {
                let mut found = in_from
                    .iter()
                    .filter(|t| has_column(tables, t, &ident.value));
                match (found.next(), found.next()) {
                    (Some(table), None) => Some((*table).clone()),
                    _ => None,
                }
            }
            _ => return ControlFlow::<()>::Continue(()),
        };
        match owner {
            Some(owner) if !owners.contains(&owner) => owners.push(owner),
            Some(_) => {}
            // Cột không biết của bảng nào: không chọn được bảng chính.
            None => owners.push(String::new()),
        }
        ControlFlow::Continue(())
    });
    let candidates: Vec<String> = match owners.as_slice() {
        [] => in_from.iter().map(|t| (*t).clone()).collect(),
        [owner] if !owner.is_empty() => vec![owner.clone()],
        _ => return None,
    };
    candidates.into_iter().find(|candidate| {
        let others: Vec<SourceTable> = tables
            .iter()
            .filter(|t| t.table != candidate)
            .map(|t| SourceTable {
                table: t.table,
                columns: t.columns,
            })
            .collect();
        let related = build_related_tables(candidate, &others, links);
        in_from
            .iter()
            .filter(|t| **t != candidate)
            .all(|t| related.source.joins.iter().any(|j| j.table == **t))
    })
}

fn has_column(tables: &[PlainTable], table: &str, column: &str) -> bool {
    tables.iter().find(|t| t.table == table).is_some_and(|t| {
        t.columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(column))
    })
}

/// Viết lại tên cột theo nguồn của plan.
struct Resolver<'a> {
    root: &'a PlainTable<'a>,
    aliases: &'a HashMap<String, String>,
    tables: &'a [PlainTable<'a>],
    related: &'a RelatedTables,
}

impl Resolver<'_> {
    /// None khi một cột không xác định được bảng, hay thuộc bảng không nối được.
    fn rewrite(&self, expr: &Expr) -> Option<Expr> {
        let mut expr = expr.clone();
        let mut failed = false;
        let _ = visit_expressions_mut(&mut expr, |e| {
            let name = match e {
                Expr::Identifier(ident) => self.resolve(None, &ident.value),
                Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
                    self.resolve(Some(&parts[0].value), &parts[1].value)
                }
                Expr::CompoundIdentifier(_) => None,
                _ => return ControlFlow::<()>::Continue(()),
            };
            match name {
                Some(name) => *e = Expr::Identifier(Ident::with_quote('"', name)),
                None => failed = true,
            }
            ControlFlow::Continue(())
        });
        (!failed).then_some(expr)
    }

    fn resolve(&self, qualifier: Option<&str>, column: &str) -> Option<String> {
        let table = match qualifier {
            Some(q) => self.aliases.get(&q.to_lowercase())?.clone(),
            None if has_column(self.tables, self.root.table, column) => self.root.table.to_string(),
            None => {
                let mut tables: Vec<&String> = self.aliases.values().collect();
                tables.sort();
                tables.dedup();
                let mut found = tables
                    .into_iter()
                    .filter(|t| has_column(self.tables, t, column));
                match (found.next(), found.next()) {
                    (Some(table), None) => table.clone(),
                    _ => return None,
                }
            }
        };
        let owner = self.tables.iter().find(|t| t.table == table)?;
        let exact = owner
            .columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(column))?;
        if table == self.root.table {
            return Some(exact.name.clone());
        }
        let name = format!("{table}.{}", exact.name);
        self.related
            .source
            .columns
            .iter()
            .any(|c| c.name == name)
            .then_some(name)
    }
}

/// Các vế AND ở ngoài cùng; vế trong ngoặc giữ nguyên.
fn split_and(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => {
            let mut parts = split_and(left);
            parts.extend(split_and(right));
            parts
        }
        other => vec![other],
    }
}

/// Cột bị lọc bằng một giá trị (`cột = 'x'`): chỉ một nhóm, không làm chiều phân rã.
fn build_fixed_columns(filters: &[Expr]) -> Vec<String> {
    filters
        .iter()
        .filter_map(|f| match f {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::Eq,
                right,
            } => match (left.as_ref(), right.as_ref()) {
                (Expr::Identifier(ident), Expr::Value(_))
                | (Expr::Value(_), Expr::Identifier(ident)) => Some(ident.value.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Mọi tên cột các vế lọc và cột thời gian dùng.
fn build_used_names(filters: &[Expr], time: &PlanTime) -> Vec<String> {
    let mut names = vec![time.column.clone()];
    for filter in filters {
        let _ = visit_expressions(filter, |e| {
            if let Expr::Identifier(ident) = e {
                if !names.contains(&ident.value) {
                    names.push(ident.value.clone());
                }
            }
            ControlFlow::<()>::Continue(())
        });
    }
    names
}
