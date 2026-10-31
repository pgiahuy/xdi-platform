// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_conditions.rs - đọc điều kiện nối của một SELECT cho luật nối bảng (guard/joins.rs): gom phép bằng
giữa cột của hai nguồn từ ON, USING và WHERE, rồi chuyển từng cặp nguồn cho guard/join_pairs.rs kiểm theo khóa ngoại
và kiểm mọi bảng trong FROM được nối với nhau. Luật đầy đủ ở đầu joins.rs. */

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use sqlparser::ast::{BinaryOperator, Expr, Query, Visit, Visitor};

use super::join_keys::Keys;
use super::join_pairs::{build_origin_pairs, check_connected, check_loose, check_strict};
use super::join_sources::{resolve_column, Found, Resolved, Source};

/// Vị trí của một nguồn: (tầng câu, thứ tự trong FROM của tầng đó).
type SourceAt = (usize, usize);

/// Một phép bằng giữa cột của hai nguồn khác nhau, như câu viết.
#[derive(Debug, Clone)]
pub(super) struct Equality {
    pub a: Found,
    pub b: Found,
    pub text: String,
}

/// Phép bằng giữa hai cột trơn của hai nguồn khác nhau, hay None. Err khi một cột không ghi bảng mà có ở hai nguồn:
/// không biết nó thuộc bảng nào thì không kiểm được khóa.
pub(super) fn read_equality(expr: &Expr, levels: &[&[Source]]) -> Result<Option<Equality>, String> {
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Eq,
        right,
    } = strip(expr)
    else {
        return Ok(None);
    };
    let (a, b) = match (resolve_column(left, levels), resolve_column(right, levels)) {
        (Resolved::Found(a), Resolved::Found(b)) => (a, b),
        (Resolved::Ambiguous(column), _) | (_, Resolved::Ambiguous(column)) => {
            return Err(ambiguous(&column))
        }
        _ => return Ok(None),
    };
    if (a.level, a.index) == (b.level, b.index) {
        return Ok(None);
    }
    Ok(Some(Equality {
        a,
        b,
        text: expr.to_string(),
    }))
}

/// Các vế nối bằng AND ở ngoài cùng.
pub(super) fn split_and(expr: &Expr) -> Vec<&Expr> {
    match strip(expr) {
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

fn strip(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) => strip(inner),
        other => other,
    }
}

/// Điều kiện của câu: phép bằng (mọi tầng) và cặp nguồn của tầng này nối bằng phép bằng không phải giữa hai cột trơn
/// (`m.month = date_trunc('month', o.order_date)`), chỉ dùng để xét các nguồn có nối với nhau không.
#[derive(Default)]
pub(super) struct Conditions {
    pub equalities: Vec<Equality>,
    pub edges: Vec<(usize, usize)>,
}

impl Conditions {
    /// Một vế AND của ON. Vế đụng hai bảng giữ nguyên dòng mà không phải phép bằng giữa hai cột thì bị từ chối: OR,
    /// so sánh lớn nhỏ, phép bằng trên biểu thức tính ra đều làm cặp dòng khớp không còn theo khóa.
    pub fn read_on(&mut self, conjunct: &Expr, levels: &[&[Source]]) -> Result<(), String> {
        if let Some(equality) = read_equality(conjunct, levels)? {
            self.equalities.push(equality);
            return Ok(());
        }
        let sources = levels[levels.len() - 1];
        let rows = |i: &usize| sources[*i].rows_table().is_some();
        match read_edge(conjunct, levels)? {
            Some((x, y)) if rows(&x) && rows(&y) => Err(not_equality(conjunct)),
            Some(edge) => {
                self.edges.push(edge);
                Ok(())
            }
            None if own_sources(conjunct, levels)?
                .iter()
                .filter(|i| rows(i))
                .count()
                >= 2 =>
            {
                Err(not_equality(conjunct))
            }
            None => Ok(()),
        }
    }

    /// Một vế AND của WHERE: phép bằng giữa hai cột là điều kiện nối (FROM a, b WHERE ...); vế khác là điều kiện lọc.
    /// Cột không rõ bảng ở đây thì bỏ qua: DuckDB tự báo lỗi, và vế đó không nối thêm bảng nào.
    pub fn read_where(&mut self, conjunct: &Expr, levels: &[&[Source]]) {
        if let Ok(Some(equality)) = read_equality(conjunct, levels) {
            self.equalities.push(equality);
            return;
        }
        let sources = levels[levels.len() - 1];
        if let Ok(Some((x, y))) = read_edge(conjunct, levels) {
            let both_rows = sources[x].rows_table().is_some() && sources[y].rows_table().is_some();
            if !both_rows {
                self.edges.push((x, y));
            }
        }
    }

    /// Kiểm từng cặp nguồn có phép bằng, rồi kiểm các bảng của tầng này có nối với nhau không.
    pub fn check(&self, levels: &[&[Source]], keys: &Keys) -> Result<(), String> {
        let own = levels.len() - 1;
        let mut groups: BTreeMap<(SourceAt, SourceAt), Vec<&Equality>> = BTreeMap::new();
        for equality in &self.equalities {
            let x = (equality.a.level, equality.a.index);
            let y = (equality.b.level, equality.b.index);
            groups
                .entry((x.min(y), x.max(y)))
                .or_default()
                .push(equality);
        }
        for ((x, y), group) in &groups {
            if x.0 != own && y.0 != own {
                continue;
            }
            let rows_x = levels[x.0][x.1].rows_table();
            let rows_y = levels[y.0][y.1].rows_table();
            match (x.0 == own && y.0 == own, rows_x, rows_y) {
                (true, Some(tx), Some(ty)) => check_strict(tx, ty, *x, group, keys)?,
                _ => check_loose(&build_origin_pairs(group), keys)?,
            }
        }
        check_connected(self, levels[own], own, keys)
    }
}

/// Phép bằng mà mỗi vế đụng đúng một nguồn của tầng này, hai nguồn khác nhau.
fn read_edge(expr: &Expr, levels: &[&[Source]]) -> Result<Option<(usize, usize)>, String> {
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Eq,
        right,
    } = strip(expr)
    else {
        return Ok(None);
    };
    let (left, right) = (own_sources(left, levels)?, own_sources(right, levels)?);
    match (left.as_slice(), right.as_slice()) {
        ([x], [y]) if x != y => Ok(Some((*x, *y))),
        _ => Ok(None),
    }
}

/// Các nguồn của tầng này mà biểu thức đọc cột, không tính câu con bên trong (câu con được kiểm riêng).
fn own_sources(expr: &Expr, levels: &[&[Source]]) -> Result<Vec<usize>, String> {
    let mut collector = SourceCollector {
        levels,
        depth: 0,
        found: Vec::new(),
    };
    match expr.visit(&mut collector) {
        ControlFlow::Break(column) => Err(ambiguous(&column)),
        ControlFlow::Continue(()) => Ok(collector.found),
    }
}

struct SourceCollector<'l, 's> {
    levels: &'l [&'s [Source]],
    depth: usize,
    found: Vec<usize>,
}

impl Visitor for SourceCollector<'_, '_> {
    type Break = String;

    fn pre_visit_query(&mut self, _query: &Query) -> ControlFlow<String> {
        self.depth += 1;
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, _query: &Query) -> ControlFlow<String> {
        self.depth -= 1;
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<String> {
        if self.depth > 0 || !matches!(expr, Expr::Identifier(_) | Expr::CompoundIdentifier(_)) {
            return ControlFlow::Continue(());
        }
        match resolve_column(expr, self.levels) {
            Resolved::Found(found) if found.level == self.levels.len() - 1 => {
                if !self.found.contains(&found.index) {
                    self.found.push(found.index);
                }
            }
            Resolved::Ambiguous(column) => return ControlFlow::Break(column),
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn not_equality(conjunct: &Expr) -> String {
    format!(
        "Điều kiện ON giữa hai bảng chỉ được là phép bằng giữa cột khóa của hai bảng, nối bằng AND, không có OR, so \
         sánh lớn nhỏ hay biểu thức tính ra: {conjunct}. Điều kiện chỉ lọc một bảng thì được"
    )
}

pub(super) fn ambiguous(column: &str) -> String {
    format!(
        "Cột {column} có ở nhiều bảng trong câu: ghi rõ bảng hay bí danh trước tên cột, ví dụ o.{column}"
    )
}
