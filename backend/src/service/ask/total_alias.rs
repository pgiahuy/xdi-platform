// SPDX-License-Identifier: Apache-2.0
/* service/ask/total_alias.rs - đổi bí danh "mỗi đơn vị" của một SUM trong câu SQL mô hình viết thành tên tổng (#260):
`SUM("Benefit per order") AS "Lợi nhuận mỗi đơn"` thành `... AS "Tổng lợi nhuận"`. Prompt viết SQL (backend/prompts/
sql.txt) đã dặn không làm vậy, nhưng mô hình vẫn hay chép bí danh của hồ sơ cột; tên cột kết quả là tiêu đề bảng, tên
trục biểu đồ và chữ lời trả lời đọc, nên sửa một lần ở đây thì mọi chỗ sau cùng đúng. Luật đặt tên ở
explain/per_unit.rs.

ask/run.rs gọi trên câu mô hình viết, trước guard, nên sql_generated, câu đã chạy và tên cột kết quả là một. Chỉ đổi bí
danh tiếng Việt (có chữ ngoài ASCII, như is_vietnamese_alias) ở SELECT ngoài cùng, cùng chỗ ORDER BY gọi đúng bí danh đó.
Câu còn chỗ khác gọi tên đó (HAVING, câu ngoài một CTE, một cột nguồn trùng tên) thì giữ nguyên bí danh: đổi một nửa
thì câu hỏng hay đổi nghĩa. Câu không đọc được thì để nguyên cho guard báo lỗi. Test ở ask/total_alias_tests.rs. */

use std::ops::ControlFlow;

use sqlparser::ast::{
    visit_expressions, Expr, Ident, OrderByKind, Query, SelectItem, SetExpr, Statement,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::service::explain::per_unit::{build_total_name, is_sum_expr};

/// Câu SQL đã đổi bí danh; None khi không có gì để đổi (bên gọi giữ nguyên câu của mô hình, không in lại).
pub fn build_total_aliases(sql: &str) -> Option<String> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    let [Statement::Query(query)] = statements.as_mut_slice() else {
        return None;
    };
    let mut renamed = false;
    for (old, new) in find_renames(query) {
        renamed |= apply_rename(query, &old, &new);
    }
    renamed.then(|| statements[0].to_string())
}

/// Các cặp (bí danh cũ, bí danh mới) của SELECT ngoài cùng. Bí danh mới trùng một bí danh khác thì bỏ.
fn find_renames(query: &Query) -> Vec<(String, String)> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Vec::new();
    };
    let aliases: Vec<&str> = select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::ExprWithAlias { alias, .. } => Some(alias.value.as_str()),
            _ => None,
        })
        .collect();
    select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::ExprWithAlias { expr, alias } if !alias.value.is_ascii() => {
                is_sum_expr(expr).then_some(alias.value.as_str())
            }
            _ => None,
        })
        .filter_map(|old| Some((old.to_string(), build_total_name(old)?)))
        .filter(|(_, new)| !aliases.iter().any(|a| a.eq_ignore_ascii_case(new)))
        .collect()
}

/// Đổi bí danh ở SELECT và ở ORDER BY; true khi đã đổi. Còn chỗ khác gọi tên cũ thì không đổi gì.
fn apply_rename(query: &mut Query, old: &str, new: &str) -> bool {
    // Đếm trước (mượn đọc), đổi sau (mượn ghi): Rust không cho giữ hai kiểu mượn query cùng lúc.
    let order_uses = match query.order_by.as_ref().map(|o| &o.kind) {
        Some(OrderByKind::Expressions(items)) => {
            items.iter().filter(|i| is_name(&i.expr, old)).count()
        }
        _ => 0,
    };
    if count_uses(query, old) != order_uses {
        return false;
    }
    if let Some(OrderByKind::Expressions(items)) = query.order_by.as_mut().map(|o| &mut o.kind) {
        for item in items.iter_mut().filter(|i| is_name(&i.expr, old)) {
            item.expr = Expr::Identifier(Ident::with_quote('"', new));
        }
    }
    let SetExpr::Select(select) = query.body.as_mut() else {
        return false;
    };
    for item in &mut select.projection {
        if let SelectItem::ExprWithAlias { alias, .. } = item {
            if alias.value == old {
                *alias = Ident::with_quote('"', new);
            }
        }
    }
    true
}

// Số biểu thức gọi đúng tên này ở bất kỳ đâu trong câu (cả ORDER BY, HAVING, CTE và trong SUM(...)).
fn count_uses(query: &Query, name: &str) -> usize {
    let mut uses = 0;
    let _ = visit_expressions(query, |expr| {
        if is_name(expr, name) {
            uses += 1;
        }
        ControlFlow::<()>::Continue(())
    });
    uses
}

fn is_name(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Identifier(ident) if ident.value == name)
}
