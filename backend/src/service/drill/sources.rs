// SPDX-License-Identifier: Apache-2.0
/* service/drill/sources.rs - đọc phần FROM của câu đã chạy cho truy dòng nguồn (drill/sql.rs): các chỗ đọc dòng của câu
ngoài cùng (bảng, hộp lọc của guard, câu con và bảng tạm chỉ lọc, chọn cột hay nối bảng), bảng nào là bảng của các
dòng sẽ trả (bảng sự kiện mà độ đo cộng), và cột của bảng nối khác cần hiện cạnh dòng. Test qua drill/tests.rs và
drill/join_tests.rs.

Vì sao truy được câu nối bảng: guard chỉ nhận phép nối đúng khóa ngoại đã khai (#256), và câu đã qua kiểm cộng lặp
(ask/fanout.rs), nên mỗi dòng của phép nối là một dòng đã góp vào con số. Câu truy giữ nguyên FROM và JOIN, nên các
dòng trả về đúng là các dòng đã vào phép tổng hợp; chọn bảng sự kiện chỉ quyết định cột nào hiện, không đổi tập dòng.

Câu con và bảng tạm (WITH) chỉ được truy khi mỗi dòng của chúng vẫn là một dòng dữ liệu: không GROUP BY, hàm tổng hợp,
HAVING, DISTINCT, LIMIT, hàm cửa sổ, phép tập hợp, WITH lồng bên trong hay WITH RECURSIVE. Bảng tạm gộp nhóm (tổng theo
đơn rồi nối khách) thì một dòng của nó là tổng của nhiều dòng, nên truy dòng nguồn từ chối thay vì trả dòng đã gộp. */

use std::collections::HashSet;
use std::ops::ControlFlow;

use sqlparser::ast::{
    Expr, GroupByExpr, Ident, Query, Select, SelectItem, SetExpr, TableFactor, TableWithJoins,
    Visit, Visitor,
};

use crate::engine::guard::{AllowedTable, JoinLink};
use crate::engine::sql_shape::{is_aggregate, read_box_table};
use crate::error::AppError;

/// Một chỗ đọc dòng trong FROM của câu ngoài cùng.
#[derive(Debug, Clone)]
pub struct Source {
    /// Tên gọi cột của chỗ đọc này: bí danh, hay tên bảng khi không có bí danh. None với câu con không bí danh.
    pub qualifier: Option<Ident>,
    /// Bảng dữ liệu chỗ này đọc, khi là bảng hay hộp lọc. None với bảng tạm và câu con có cột riêng.
    pub table: Option<String>,
    /// Bảng dữ liệu đầu tiên trong FROM của câu con hay bảng tạm (bảng chính của phân rã #227), để báo dòng đến từ
    /// đâu khi câu chỉ đọc một câu con.
    pub base_table: Option<String>,
    /// Tên các cột câu con thêm cạnh `t.*` (cột của bảng nối trong câu con của phân rã, như "students.region").
    pub added_columns: Vec<String>,
}

/// Các chỗ đọc của câu ngoài cùng, theo thứ tự viết. Err (400) khi một chỗ đọc không giữ dòng dữ liệu.
pub fn build_sources(query: &Query, select: &Select) -> Result<Vec<Source>, AppError> {
    let ctes = check_ctes(query)?;
    let mut sources = Vec::new();
    for from in &select.from {
        collect_sources(from, &ctes, &mut sources)?;
    }
    if sources.is_empty() {
        return Err(unsupported("câu không đọc bảng nào"));
    }
    Ok(sources)
}

/// Bảng tạm: tên viết thường cùng câu của nó. Mọi bảng tạm phải giữ dòng dữ liệu, kể cả bảng tạm câu ngoài không đọc
/// thẳng (bảng tạm khác đọc nó).
fn check_ctes(query: &Query) -> Result<Vec<(String, Query)>, AppError> {
    let Some(with) = &query.with else {
        return Ok(Vec::new());
    };
    if with.recursive {
        return Err(unsupported("câu dùng bảng tạm đệ quy (WITH RECURSIVE)"));
    }
    let mut ctes = Vec::new();
    for cte in &with.cte_tables {
        let name = &cte.alias.name.value;
        check_row_level(&cte.query).map_err(|reason| {
            unsupported(&format!(
                "bảng tạm (WITH) {name} {reason}, nên mỗi dòng của nó không còn là một dòng dữ liệu"
            ))
        })?;
        ctes.push((name.to_lowercase(), (*cte.query).clone()));
    }
    Ok(ctes)
}

fn collect_sources(
    from: &TableWithJoins,
    ctes: &[(String, Query)],
    out: &mut Vec<Source>,
) -> Result<(), AppError> {
    let relations =
        std::iter::once(&from.relation).chain(from.joins.iter().map(|join| &join.relation));
    for relation in relations {
        match relation {
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => collect_sources(table_with_joins, ctes, out)?,
            other => out.push(build_source(other, ctes)?),
        }
    }
    Ok(())
}

fn build_source(relation: &TableFactor, ctes: &[(String, Query)]) -> Result<Source, AppError> {
    match relation {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => {
            let Some(ident) = name.0.last().and_then(|part| part.as_ident()) else {
                return Err(unsupported("câu đọc một chỗ không phải bảng"));
            };
            let cte = find_cte(ctes, &ident.value);
            let qualifier = alias.as_ref().map_or(ident.clone(), |a| a.name.clone());
            Ok(Source {
                qualifier: Some(qualifier),
                table: cte.is_none().then(|| ident.value.clone()),
                base_table: cte.and_then(|q| first_table(q, ctes)),
                added_columns: cte.map(added_columns).unwrap_or_default(),
            })
        }
        TableFactor::Derived {
            lateral: false,
            subquery,
            alias,
            ..
        } => {
            let qualifier = alias.as_ref().map(|a| a.name.clone());
            // Hộp của bảng tạm (`SELECT * FROM t` với t là WITH) không phải bảng dữ liệu.
            let boxed = read_box_table(relation).filter(|t| find_cte(ctes, t).is_none());
            if boxed.is_some() {
                return Ok(Source {
                    qualifier,
                    base_table: boxed.clone(),
                    table: boxed,
                    added_columns: Vec::new(),
                });
            }
            check_row_level(subquery).map_err(|reason| {
                unsupported(&format!(
                    "câu con ở FROM {reason}, nên mỗi dòng của nó không còn là một dòng dữ liệu"
                ))
            })?;
            Ok(Source {
                qualifier,
                table: None,
                base_table: first_table(subquery, ctes),
                added_columns: added_columns(subquery),
            })
        }
        _ => Err(unsupported(
            "câu đọc dữ liệu bằng hàm sinh bảng hay LATERAL",
        )),
    }
}

fn find_cte<'a>(ctes: &'a [(String, Query)], name: &str) -> Option<&'a Query> {
    let key = name.to_lowercase();
    ctes.iter().find(|(n, _)| *n == key).map(|(_, q)| q)
}

/// Err là lý do một câu con không giữ dòng dữ liệu, viết để ghép sau "câu con ở FROM" hay "bảng tạm t".
fn check_row_level(query: &Query) -> Result<(), &'static str> {
    if query.with.is_some() {
        return Err("có bảng tạm (WITH) lồng bên trong");
    }
    if query.limit_clause.is_some() || query.fetch.is_some() {
        return Err("giới hạn số dòng (LIMIT)");
    }
    let select = match query.body.as_ref() {
        SetExpr::Select(select) => select,
        SetExpr::Query(inner) => return check_row_level(inner),
        _ => return Err("ghép nhiều câu SELECT (UNION, EXCEPT, INTERSECT)"),
    };
    let grouped = match &select.group_by {
        GroupByExpr::Expressions(keys, modifiers) => !keys.is_empty() || !modifiers.is_empty(),
        GroupByExpr::All(_) => true,
    };
    if grouped || select.having.is_some() || select.projection.iter().any(item_is_aggregate) {
        return Err("gộp nhóm (GROUP BY hay hàm tổng hợp)");
    }
    if select.distinct.is_some() {
        return Err("bỏ dòng trùng (DISTINCT)");
    }
    if has_window(select) {
        return Err("dùng hàm cửa sổ (OVER)");
    }
    select.from.iter().try_for_each(check_from_row_level)
}

/// Mọi chỗ đọc trong một mục FROM của câu con đều giữ dòng dữ liệu, kể cả trong JOIN trong ngoặc.
fn check_from_row_level(from: &TableWithJoins) -> Result<(), &'static str> {
    let relations =
        std::iter::once(&from.relation).chain(from.joins.iter().map(|join| &join.relation));
    for relation in relations {
        match relation {
            TableFactor::Table { args: None, .. } => {}
            TableFactor::Derived {
                lateral: false,
                subquery,
                ..
            } => check_row_level(subquery)?,
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => check_from_row_level(table_with_joins)?,
            _ => return Err("đọc dữ liệu bằng hàm sinh bảng hay LATERAL"),
        }
    }
    Ok(())
}

fn item_is_aggregate(item: &SelectItem) -> bool {
    match item {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            is_aggregate(expr)
        }
        _ => false,
    }
}

/// Câu có hàm cửa sổ ở danh sách cột hay QUALIFY. Hàm cửa sổ tính trên cả kết quả, không trên từng dòng hay nhóm.
pub fn has_window(select: &Select) -> bool {
    select.qualify.is_some()
        || select
            .projection
            .iter()
            .any(|item| item.visit(&mut WindowFinder).is_break())
}

struct WindowFinder;

impl Visitor for WindowFinder {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        match expr {
            Expr::Function(function) if function.over.is_some() => ControlFlow::Break(()),
            _ => ControlFlow::Continue(()),
        }
    }
}

/// Bảng dữ liệu đầu tiên trong FROM của một câu con, đi qua câu con và bảng tạm lồng nhau.
fn first_table(query: &Query, ctes: &[(String, Query)]) -> Option<String> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let relation = &select.from.first()?.relation;
    match relation {
        TableFactor::Table { name, .. } => {
            let ident = name.0.last()?.as_ident()?;
            match find_cte(ctes, &ident.value) {
                Some(cte) => first_table(cte, ctes),
                None => Some(ident.value.clone()),
            }
        }
        TableFactor::Derived { subquery, .. } => {
            read_box_table(relation).or_else(|| first_table(subquery, ctes))
        }
        _ => None,
    }
}

/// Tên các cột có bí danh trong danh sách cột của câu con: cột nó thêm cạnh `t.*`.
fn added_columns(query: &Query) -> Vec<String> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Vec::new();
    };
    select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::ExprWithAlias { alias, .. } => Some(alias.value.clone()),
            _ => None,
        })
        .collect()
}

/// Chỉ số trong sources của bảng có dòng sẽ trả, khi FROM có nhiều chỗ đọc. Chỉ chọn chỗ đọc là bảng dữ liệu có tên
/// gọi, vì trang lấy `<tên gọi>.*`, và guard chỉ đếm được số cột của `t.*` khi t là bảng hay hộp lọc (guard/width.rs).
/// Thứ tự chọn: bảng duy nhất mà độ đo đọc cột; không thì bảng không làm bên một của quan hệ nào với bảng khác trong
/// câu (gốc của chuỗi nhiều-một, như order_items dưới orders dưới customers); không thì bảng viết đầu tiên. Err là lý do
/// cho người dùng khi độ đo chỉ đọc bảng tạm hay câu con: liệt kê dòng của bảng khác thì dòng không khớp con số.
pub fn choose_fact(
    sources: &[Source],
    measures: &[&Expr],
    tables: &[AllowedTable],
    links: &[JoinLink],
) -> Result<usize, AppError> {
    let candidates: Vec<usize> = (0..sources.len())
        .filter(|&i| sources[i].table.is_some() && sources[i].qualifier.is_some())
        .collect();
    let mut referenced: Vec<usize> = Vec::new();
    for expr in measures {
        for column in build_column_refs(expr) {
            if let Some(owner) = find_owner(sources, tables, &column) {
                if !referenced.contains(&owner) {
                    referenced.push(owner);
                }
            }
        }
    }
    let on_tables: Vec<usize> = referenced
        .iter()
        .copied()
        .filter(|i| candidates.contains(i))
        .collect();
    if on_tables.is_empty() && !referenced.is_empty() {
        return Err(unsupported(
            "độ đo cộng trên bảng tạm (WITH) hay câu con nối với bảng khác",
        ));
    }
    if let [only] = on_tables.as_slice() {
        return Ok(*only);
    }
    let pool = if on_tables.is_empty() {
        candidates.clone()
    } else {
        on_tables
    };
    let roots: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&i| !is_one_side(i, &candidates, sources, links))
        .collect();
    match (roots.as_slice(), pool.first()) {
        ([only], _) => Ok(*only),
        (_, Some(first)) => Ok(*first),
        (_, None) => Err(unsupported(
            "câu chỉ nối bảng tạm (WITH) hay câu con với nhau, không có bảng dữ liệu để lấy dòng",
        )),
    }
}

/// Bảng của sources[i] là bên một của một quan hệ nhiều-một với bảng khác trong câu.
fn is_one_side(i: usize, candidates: &[usize], sources: &[Source], links: &[JoinLink]) -> bool {
    let table = |j: usize| {
        sources[j]
            .table
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
    };
    let own = table(i);
    candidates.iter().filter(|&&j| table(j) != own).any(|&j| {
        let other = table(j);
        links.iter().any(|link| {
            link.fans_out && link.one.to_lowercase() == own && link.many.to_lowercase() == other
        })
    })
}

/// Một cột câu nhắc tới: tên gọi bảng (nếu có ghi) và tên cột.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnRef {
    pub qualifier: Option<Ident>,
    pub column: Ident,
}

/// Cột nhắc tới ở chính câu này, không tính cột trong câu con (thuộc phạm vi tên khác).
pub fn build_column_refs(expr: &Expr) -> Vec<ColumnRef> {
    let mut finder = ColumnFinder {
        depth: 0,
        found: Vec::new(),
    };
    let _ = expr.visit(&mut finder);
    finder.found
}

struct ColumnFinder {
    depth: usize,
    found: Vec<ColumnRef>,
}

impl Visitor for ColumnFinder {
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
        if self.depth > 0 {
            return ControlFlow::Continue(());
        }
        let found = match expr {
            Expr::Identifier(column) => Some(ColumnRef {
                qualifier: None,
                column: column.clone(),
            }),
            Expr::CompoundIdentifier(parts) => match parts.as_slice() {
                [qualifier, column] => Some(ColumnRef {
                    qualifier: Some(qualifier.clone()),
                    column: column.clone(),
                }),
                _ => None,
            },
            _ => None,
        };
        if let Some(found) = found {
            if !self.found.contains(&found) {
                self.found.push(found);
            }
        }
        ControlFlow::Continue(())
    }
}

/// Chỗ đọc mà một cột thuộc về: theo tên gọi khi cột có ghi, không thì bảng duy nhất có cột cùng tên trong hồ sơ.
pub fn find_owner(
    sources: &[Source],
    tables: &[AllowedTable],
    column: &ColumnRef,
) -> Option<usize> {
    if let Some(qualifier) = &column.qualifier {
        return sources.iter().position(|s| {
            s.qualifier
                .as_ref()
                .is_some_and(|q| q.value.eq_ignore_ascii_case(&qualifier.value))
        });
    }
    let owners: HashSet<usize> = (0..sources.len())
        .filter(|&i| {
            let Some(table) = &sources[i].table else {
                return false;
            };
            tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(table))
                .is_some_and(|t| {
                    t.columns
                        .iter()
                        .any(|c| c.eq_ignore_ascii_case(&column.column.value))
                })
        })
        .collect();
    match owners.len() {
        1 => owners.into_iter().next(),
        _ => None,
    }
}

fn unsupported(reason: &str) -> AppError {
    AppError::BadRequest(format!("Câu hỏi này chưa truy được dòng nguồn: {reason}"))
}
