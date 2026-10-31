// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_conditions.rs - đọc điều kiện nối của một SELECT cho luật nối bảng (guard/joins.rs): gom phép bằng
giữa cột của hai nguồn từ ON, USING và WHERE, kiểm từng cặp nguồn theo khóa ngoại (guard/join_keys.rs), và kiểm mọi
bảng trong FROM được nối với nhau bằng ít nhất một phép bằng (không có CROSS JOIN ngầm). Luật đầy đủ ở đầu joins.rs. */

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use sqlparser::ast::{BinaryOperator, Expr, Query, Visit, Visitor};

use super::join_keys::{Keys, Origin};
use super::join_sources::{resolve_column, Found, Resolved, Source};

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
        let mut groups: BTreeMap<((usize, usize), (usize, usize)), Vec<&Equality>> =
            BTreeMap::new();
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

/// Hai bảng giữ nguyên dòng nối với nhau: các phép bằng phải đúng bằng cột của một khóa ngoại giữa hai bảng.
fn check_strict(
    tx: &str,
    ty: &str,
    x: (usize, usize),
    group: &[&Equality],
    keys: &Keys,
) -> Result<(), String> {
    let mut pairs = Vec::new();
    for equality in group {
        // Đặt cột của nguồn x trước, vì câu viết a.c = b.d hay b.d = a.c đều được.
        let (on_x, on_y) = if (equality.a.level, equality.a.index) == x {
            (&equality.a.origin, &equality.b.origin)
        } else {
            (&equality.b.origin, &equality.a.origin)
        };
        match (on_x, on_y) {
            (Some(ox), Some(oy)) => pairs.push((ox.column.clone(), oy.column.clone())),
            // Cột tính ra trong câu con chỉ lọc một bảng: không phải cột khóa.
            _ => pairs.push((String::new(), String::new())),
        }
    }
    if keys.key_matches(tx, ty, &pairs) {
        return Ok(());
    }
    let written: Vec<&str> = group.iter().map(|e| e.text.as_str()).collect();
    Err(format!(
        "Nối bảng {tx} với {ty} sai điều kiện ({}). {}",
        written.join(" AND "),
        keys.describe_keys(tx, ty)
    ))
}

fn build_origin_pairs(group: &[&Equality]) -> Vec<(Origin, Origin, String)> {
    group
        .iter()
        .filter_map(|e| match (&e.a.origin, &e.b.origin) {
            (Some(a), Some(b)) => Some((a.clone(), b.clone(), e.text.clone())),
            _ => None,
        })
        .collect()
}

/// Nối có bảng tạm, câu con đã gộp nhóm, hay nửa nối (IN, EXISTS, câu con tương quan): mỗi phép bằng có cột gốc biết
/// được phải so hai cột cùng miền giá trị (Keys::same_domain), và các cặp thuộc khóa ngoại phải đủ cột của khóa đó.
/// Cột tính ra (tháng, năm) không kiểm: nó không phải cột của bảng nào.
pub(super) fn check_loose(pairs: &[(Origin, Origin, String)], keys: &Keys) -> Result<(), String> {
    let mut by_tables: BTreeMap<(&str, &str), Vec<(String, String)>> = BTreeMap::new();
    for (a, b, text) in pairs {
        if !keys.same_domain(a, b) {
            return Err(format!(
                "Phép bằng {text} so cột {a} với {b}, hai cột này không nối với nhau qua khóa ngoại nào. {}",
                keys.describe_keys(&a.table, &b.table)
            ));
        }
        if a != b && keys.is_key_pair(a, b) {
            let pair = (a.column.clone(), b.column.clone());
            by_tables
                .entry((&a.table, &b.table))
                .or_default()
                .push(pair);
        }
    }
    for ((ta, tb), pairs) in &by_tables {
        if !keys.key_matches(ta, tb, pairs) {
            return Err(format!(
                "Câu chỉ so một phần khóa ngoại giữa {ta} và {tb}. {}",
                keys.describe_keys(ta, tb)
            ));
        }
    }
    Ok(())
}

/// Mọi bảng giữ nguyên dòng trong FROM phải nối với nhau qua phép bằng, trực tiếp hay qua nguồn khác. Thiếu thì là
/// tích chéo: mỗi dòng bảng này ghép với mọi dòng bảng kia.
fn check_connected(
    conditions: &Conditions,
    sources: &[Source],
    own: usize,
    keys: &Keys,
) -> Result<(), String> {
    let mut group: Vec<usize> = (0..sources.len()).collect();
    let mut links: Vec<(usize, usize)> = conditions.edges.clone();
    for e in &conditions.equalities {
        if e.a.level == own && e.b.level == own {
            links.push((e.a.index, e.b.index));
        }
    }
    // Gộp nhóm đơn giản: lặp tới khi không đổi; số nguồn tối đa 6 (guard/width.rs) nên không cần cấu trúc nhanh hơn.
    let mut changed = true;
    while changed {
        changed = false;
        for &(x, y) in &links {
            let low = group[x].min(group[y]);
            if group[x] != low || group[y] != low {
                group[x] = low;
                group[y] = low;
                changed = true;
            }
        }
    }
    let rows: Vec<usize> = (0..sources.len())
        .filter(|&i| sources[i].rows_table().is_some())
        .collect();
    let Some(&first) = rows.first() else {
        return Ok(());
    };
    let Some(&apart) = rows.iter().find(|&&i| group[i] != group[first]) else {
        return Ok(());
    };
    let (tx, ty) = (
        sources[first].rows_table().unwrap_or_default(),
        sources[apart].rows_table().unwrap_or_default(),
    );
    Err(format!(
        "Bảng {tx} và {ty} được nối mà không có phép bằng nào giữa cột của hai bảng (CROSS JOIN, ON true hay thiếu \
         điều kiện nối trong WHERE). {}",
        keys.describe_keys(tx, ty)
    ))
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
