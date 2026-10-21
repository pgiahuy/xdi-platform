// SPDX-License-Identifier: Apache-2.0
/* engine/guard/rewrite.rs - sửa câu đã qua kiểm tên: thêm giới hạn dòng, rồi tạo bản cho DuckDB chạy bằng cách đổi
bảng logic sang hàm đọc Parquet và đổi tên bảng tạm (WITH).

Đổi sang Parquet làm sau cùng: chuỗi thay vào là read_parquet(...), đúng loại hàm mà bước kiểm tên chặn.

Đổi tên bảng tạm sang tên do guard đặt (guard_cte_1, ...): bước kiểm tên nhận tên bảng tạm ở mọi chỗ trong câu, không
theo phạm vi. Nếu câu gọi một bảng tạm ở ngoài chỗ nó được định nghĩa, DuckDB sẽ tìm bảng hay file trùng tên, ví dụ
bảng tạm tên "x.csv" thành đọc file x.csv. Sau khi đổi tên, chỗ gọi sai đó chỉ còn là tên không tồn tại và DuckDB báo
lỗi. */

use std::collections::{BTreeSet, HashMap};
use std::convert::Infallible;
use std::ops::ControlFlow;
use std::path::Path;

use sqlparser::ast::{
    Expr, Ident, LimitClause, ObjectName, Query, SetExpr, Statement, TableAlias, TableFactor,
    Value, VisitMut, VisitorMut,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use super::{name_key, rejected, single_ident, AllowedTable};
use crate::engine::analytics::parquet_glob;
use crate::error::AppError;

/// Giới hạn số dòng câu ngoài cùng trả về: chưa có LIMIT thì thêm, lớn hơn max_rows thì hạ xuống, nhỏ hơn thì giữ.
pub(super) fn apply_row_limit(query: &mut Query, max_rows: u64) -> Result<(), AppError> {
    if query.fetch.is_some() {
        return Err(rejected("Dùng LIMIT thay cho FETCH FIRST"));
    }
    let max = Expr::value(Value::Number(max_rows.to_string(), false));
    match &mut query.limit_clause {
        None => {
            query.limit_clause = Some(LimitClause::LimitOffset {
                limit: Some(max),
                offset: None,
                limit_by: vec![],
            });
        }
        Some(LimitClause::LimitOffset {
            limit, limit_by, ..
        }) if limit_by.is_empty() => {
            // LIMIT là biểu thức hay câu con thì không biết trước số dòng, nên thay bằng max_rows.
            if !limit
                .as_ref()
                .is_some_and(|l| is_number_at_most(l, max_rows))
            {
                *limit = Some(max);
            }
        }
        Some(_) => return Err(rejected("Chỉ hỗ trợ LIMIT n hoặc LIMIT n OFFSET m")),
    }
    Ok(())
}

fn is_number_at_most(expr: &Expr, max: u64) -> bool {
    match expr {
        Expr::Value(v) => {
            matches!(&v.value, Value::Number(n, _) if n.parse::<u64>().is_ok_and(|n| n <= max))
        }
        _ => false,
    }
}

/// Bản cho DuckDB chạy: bảng logic thành read_parquet của phiên bản, bảng tạm thành guard_cte_1, guard_cte_2, ...
pub(super) fn build_runnable(
    mut query: Query,
    tables: &[AllowedTable],
    cte_names: &BTreeSet<String>,
) -> Result<Query, AppError> {
    let mut sources = HashMap::new();
    for table in tables {
        sources.insert(
            table.name.to_lowercase(),
            parquet_source(&table.parquet_dir)?,
        );
    }
    let renames = cte_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), format!("guard_cte_{}", i + 1)))
        .collect();
    let _ = query.visit(&mut Resolver { sources, renames });
    Ok(query)
}

/// Phần FROM đọc Parquet của một phiên bản. Lấy chuỗi từ analytics::parquet_glob rồi đọc thành cây, để cách đọc
/// Parquet (gồm union_by_name) chỉ viết ở một chỗ.
fn parquet_source(dir: &Path) -> Result<TableFactor, AppError> {
    let sql = format!("SELECT * FROM {}", parquet_glob(dir)?);
    let statement = Parser::parse_sql(&DuckDbDialect {}, &sql)
        .ok()
        .and_then(|mut statements| statements.pop());
    if let Some(Statement::Query(query)) = statement {
        if let SetExpr::Select(select) = *query.body {
            if let Some(from) = select.from.into_iter().next() {
                return Ok(from.relation);
            }
        }
    }
    Err(AppError::Internal(format!(
        "cannot build parquet source from {sql}"
    )))
}

struct Resolver {
    /// Tên bảng logic viết thường, và phần FROM đọc Parquet thay vào.
    sources: HashMap<String, TableFactor>,
    /// Tên bảng tạm viết thường, và tên mới.
    renames: HashMap<String, String>,
}

impl VisitorMut for Resolver {
    type Break = Infallible;

    fn pre_visit_query(&mut self, query: &mut Query) -> ControlFlow<Infallible> {
        for cte in query.with.iter_mut().flat_map(|with| &mut with.cte_tables) {
            if let Some(new_name) = self.renames.get(&name_key(&cte.alias.name)) {
                cte.alias.name = Ident::new(new_name);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &mut TableFactor) -> ControlFlow<Infallible> {
        let TableFactor::Table { name, alias, .. } = factor else {
            return ControlFlow::Continue(());
        };
        let Some(ident) = single_ident(name).cloned() else {
            return ControlFlow::Continue(());
        };
        let key = name_key(&ident);
        // Tên cũ thành bí danh khi câu chưa đặt, để `sales.amount` hay `t.region` vẫn trỏ đúng bảng.
        let kept_alias = alias.clone().unwrap_or(TableAlias {
            explicit: true,
            name: ident,
            columns: vec![],
            at: None,
        });
        if let Some(source) = self.sources.get(&key) {
            let mut source = source.clone();
            if let TableFactor::Table { alias, .. } = &mut source {
                *alias = Some(kept_alias);
            }
            *factor = source;
        } else if let Some(new_name) = self.renames.get(&key) {
            *name = ObjectName::from(vec![Ident::new(new_name)]);
            *alias = Some(kept_alias);
        }
        ControlFlow::Continue(())
    }
}
