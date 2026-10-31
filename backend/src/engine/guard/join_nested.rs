// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_nested.rs - tìm câu con nằm trong biểu thức của một SELECT (danh sách cột, WHERE, HAVING, QUALIFY,
GROUP BY, ON) cho luật nối bảng (guard/joins.rs). Chỉ lấy câu con ở tầng ngay dưới: joins.rs kiểm từng câu con với
nguồn của SELECT này là tầng ngoài, rồi câu con tự tìm câu con của nó. Câu con ở FROM không đi qua đây. */

use std::ops::ControlFlow;

use sqlparser::ast::{Expr, GroupByExpr, Query, Select, Visit, Visitor};

pub(super) enum Nested {
    /// `x IN (SELECT y ...)`, cả NOT IN: nửa nối giữa x và cột của câu con.
    In { left: Box<Expr>, query: Box<Query> },
    /// EXISTS, câu con vô hướng, `= ANY (SELECT ...)`.
    Query(Box<Query>),
}

pub(super) fn find_nested(select: &Select, on: &[&Expr]) -> Vec<Nested> {
    let mut finder = NestedFinder {
        depth: 0,
        found: Vec::new(),
    };
    for item in &select.projection {
        let _ = item.visit(&mut finder);
    }
    let mut exprs: Vec<&Expr> = on.to_vec();
    exprs.extend(&select.selection);
    exprs.extend(&select.having);
    exprs.extend(&select.qualify);
    if let GroupByExpr::Expressions(group, _) = &select.group_by {
        exprs.extend(group);
    }
    for expr in exprs {
        let _ = expr.visit(&mut finder);
    }
    finder.found
}

struct NestedFinder {
    depth: usize,
    found: Vec<Nested>,
}

impl Visitor for NestedFinder {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        if let (0, Expr::InSubquery { expr, subquery, .. }) = (self.depth, expr) {
            self.found.push(Nested::In {
                left: expr.clone(),
                query: subquery.clone(),
            });
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        // Câu con của IN đã lấy ở pre_visit_expr; Visitor vẫn đi vào nó nên bỏ qua ở đây để không kiểm hai lần.
        let taken = self
            .found
            .iter()
            .any(|n| matches!(n, Nested::In { query: q, .. } if **q == *query));
        if self.depth == 0 && !taken {
            self.found.push(Nested::Query(Box::new(query.clone())));
        }
        self.depth += 1;
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, _query: &Query) -> ControlFlow<()> {
        self.depth -= 1;
        ControlFlow::Continue(())
    }
}
