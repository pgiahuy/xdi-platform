// SPDX-License-Identifier: Apache-2.0
/* service/ask/fanout.rs - chặn câu SQL cộng lặp khi nối bảng một-nhiều (#214). ask/run.rs gọi find_double_count sau
khi guard nhận câu và trước khi chạy; câu bị bắt không chạy mà vào vòng sửa một lần (ask/run.rs) kèm build_hint. Test
ở ask/fanout_tests.rs. Phần đọc cây cú pháp (bảng của FROM, summed_grain) ở ask/fanout_grain.rs.

Vì sao: câu hỏi trên dataset chụp từ database đọc được các bảng liên quan (ask/related.rs), với quan hệ ghi trong
dataset_relations: bảng "from" giữ khóa ngoại (bên nhiều), bảng "to" là bên một. Nối students với student_assessments
rồi SUM(s.studied_credits) thì tín chỉ của một sinh viên được cộng một lần cho mỗi bài nộp: trên OULAD của database
mẫu (data/postgres-demo, đo bằng psql), vùng East Anglian Region ra 1 357 190 thay vì 260 380. Con số sai mà trông hợp
lý, nên luật trong prompt thôi chưa đủ.

Luật: với mỗi SELECT, dựng bảng bí danh -> bảng từ FROM và các JOIN. Bị bắt khi đối số của SUM hay AVG chỉ gồm cột
của một bảng P (cộng hằng số), và FROM có bảng C làm mỗi dòng P lặp nhiều lần: C trỏ tới P theo quan hệ many_to_one,
hay trỏ tới một bảng Q mà từ P đi tới được không nhân dòng (P -> Q nhiều-một, hay qua quan hệ một-một). Ví dụ orders,
order_items và returns cùng trỏ orders: SUM(oi.qty) vẫn bị bắt vì returns nhân mỗi dòng orders, kéo theo order_items.
Quan hệ one_to_one không nhân dòng. Cách đọc đối số (summed_grain trong ask/fanout_grain.rs): bỏ qua ngoặc, CAST,
dấu âm, COALESCE, IFNULL, ROUND, ABS, DISTINCT; CASE chỉ xét nhánh THEN và ELSE, không xét điều kiện WHEN (lọc theo
cột của P không làm cộng lặp). Nhân cột của P với cột của C (số lượng nhân giá) không bị bắt: đó là giá trị
của từng dòng C. SUM(DISTINCT p.price) vẫn bị bắt: gộp giá trị trùng làm mất hai dòng P cùng giá.

Không chắc thì không bắt, vì bắt nhầm làm hỏng câu đúng; luật nối bảng trong prompt (prompts/sql_joins.txt) vẫn còn:
cột không ghi bảng mà có ở hai bảng, hay FROM có bảng WITH hoặc câu con; bí danh lạ; hàm khác; quan hệ của một bảng
với chính nó (employees.manager_id): luật theo tên bảng không phân biệt được hai bí danh của cùng bảng.

Giới hạn: không lần theo WITH hay bảng con. Một SELECT nằm trong WITH hay câu con vẫn được xét với FROM của chính nó,
nhưng SUM trên một bảng WITH chưa gộp đúng thì không bị bắt. Không xét ON có đúng theo quan hệ hay không, và không xét
GROUP BY: nhóm theo khóa của C thì SUM cột P không còn lặp nhưng vẫn bị bắt. COUNT(*) sau khi nối cũng đếm lặp nhưng
chưa xét ở đây; COUNT(DISTINCT ...) không bị ảnh hưởng. MIN, MAX không đổi khi dòng lặp. */

use std::ops::ControlFlow;

use sqlparser::ast::{Expr, Query, Select, SetExpr, Visit, Visitor};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::guard::AllowedTable;
use crate::service::ask::fanout_grain::{
    build_from_tables, function_name, plain_args, summed_grain, FromTables, Grain,
};
use crate::service::ask::related::TableGroup;

/// Một quan hệ giữa hai bảng của câu hỏi, theo tên bảng logic (tên câu SQL dùng).
#[derive(Debug, Clone)]
pub struct JoinLink {
    /// Bảng giữ khóa ngoại (bên nhiều).
    pub many: String,
    pub many_columns: Vec<String>,
    /// Bảng được trỏ tới (bên một).
    pub one: String,
    pub one_columns: Vec<String>,
    /// true với many_to_one: một dòng bên một ứng với nhiều dòng bên nhiều. false với one_to_one.
    pub fans_out: bool,
}

/// Quan hệ của nhóm bảng (ask/related.rs) đổi sang tên bảng logic. Nhóm một bảng thì rỗng, và kiểm tra không làm gì.
pub fn build_join_links(group: &TableGroup) -> Vec<JoinLink> {
    group
        .relations
        .iter()
        .filter_map(|r| {
            let many = group.logical_table(r.from_dataset_id)?;
            let one = group.logical_table(r.to_dataset_id)?;
            Some(JoinLink {
                many: many.to_string(),
                many_columns: r.from_columns.clone(),
                one: one.to_string(),
                one_columns: r.to_columns.clone(),
                fans_out: r.cardinality == "many_to_one",
            })
        })
        .collect()
}

/// Một chỗ cộng lặp tìm được.
#[derive(Debug, Clone)]
pub struct DoubleCount {
    /// "SUM" hay "AVG".
    pub function: String,
    /// Đối số như câu SQL viết, ví dụ s.studied_credits.
    pub argument: String,
    /// Bảng P có cột bị cộng.
    pub table: String,
    /// Quan hệ làm dòng của P lặp lại: link.many là bảng C.
    pub link: JoinLink,
}

/// Chỗ cộng lặp đầu tiên của câu, hay None. Câu không đọc được thì None: guard đã đọc được câu này, và lỗi đọc ở đây
/// không nên chặn câu hỏi.
pub fn find_double_count(
    sql: &str,
    tables: &[AllowedTable],
    links: &[JoinLink],
) -> Option<DoubleCount> {
    if links.is_empty() {
        return None;
    }
    let statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    statements.iter().find_map(|statement| {
        let mut finder = QueryFinder {
            tables,
            links,
            found: None,
        };
        let _ = statement.visit(&mut finder);
        finder.found
    })
}

/// Lời gửi mô hình ở vòng sửa (tiếng Anh như lý do của guard).
pub fn build_hint(found: &DoubleCount) -> String {
    let link = &found.link;
    let many_key = qualified(&link.many, &link.many_columns);
    let one_key = qualified(&link.one, &link.one_columns);
    format!(
        "The query uses {function}({argument}), a column of {table}, while {many} is in the same FROM. {many} has \
         many rows for one {one} row ({many_key} = {one_key}), so every {table} row repeats once per matching {many} \
         row and {function} counts its value many times. Fix it one of two ways: aggregate {many} first in a \
         subquery or CTE grouped by {many_columns}, then join that result to {table}; or, if the question asks for a \
         total of {table} values, keep one row per {table} row first (WITH one_row AS (SELECT DISTINCT the key \
         columns of {table}, the summed column and any column you group by FROM ...) SELECT ... FROM one_row), then \
         sum. If the question does not need {many} at all, read {table} without that join: an inner join also drops \
         the {table} rows that have no {many} row. Multiplying by a column of {many} inside the SUM, such as quantity \
         times price, is not double counting.",
        function = found.function,
        argument = found.argument,
        table = found.table,
        many = link.many,
        one = link.one,
        many_columns = link.many_columns.join(", "),
    )
}

/// Chi tiết bước guard (tiếng Việt, hiện trong danh sách bước).
pub fn build_step_detail(found: &DoubleCount) -> String {
    format!(
        "Câu SQL tính {} trên cột của bảng {} sau khi nối bảng {}: mỗi dòng {} lặp theo số dòng {}, cần sửa",
        found.function, found.table, found.link.many, found.table, found.link.many
    )
}

/// Lời cho người hỏi khi câu sửa vẫn cộng lặp.
pub fn build_refused_message(found: &DoubleCount) -> String {
    format!(
        "Câu truy vấn hệ thống viết vẫn tính {} trên cột của bảng {} sau khi nối bảng {}, kể cả sau khi tự sửa một \
         lần, nên không chạy: mỗi giá trị sẽ bị tính lặp và ra số sai. Thử hỏi lại theo cách khác, ví dụ hỏi riêng \
         trên bảng {}.",
        found.function, found.table, found.link.many, found.table
    )
}

/// "orders.id" hay "(products.maker, products.sku)" với khóa nhiều cột.
fn qualified(table: &str, columns: &[String]) -> String {
    let parts: Vec<String> = columns.iter().map(|c| format!("{table}.{c}")).collect();
    match parts.as_slice() {
        [one] => one.clone(),
        _ => format!("({})", parts.join(", ")),
    }
}

/// Đi qua mọi Query (câu ngoài, WITH, câu con): visitor của sqlparser gọi pre_visit_query cho từng Query.
struct QueryFinder<'a> {
    tables: &'a [AllowedTable],
    links: &'a [JoinLink],
    found: Option<DoubleCount>,
}

impl Visitor for QueryFinder<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        self.found = self.find_in_set(&query.body);
        if self.found.is_some() {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}

impl QueryFinder<'_> {
    /// SELECT, hay từng vế của UNION, INTERSECT, EXCEPT.
    fn find_in_set(&self, body: &SetExpr) -> Option<DoubleCount> {
        match body {
            SetExpr::Select(select) => self.find_in_select(select),
            SetExpr::SetOperation { left, right, .. } => {
                self.find_in_set(left).or_else(|| self.find_in_set(right))
            }
            // Câu trong ngoặc là một Query riêng, visitor sẽ tới nó.
            _ => None,
        }
    }

    fn find_in_select(&self, select: &Select) -> Option<DoubleCount> {
        let from = build_from_tables(select, self.tables);
        if from.entries.len() < 2 {
            return None;
        }
        let mut finder = SumFinder {
            from: &from,
            links: self.links,
            depth: 0,
            found: None,
        };
        for item in &select.projection {
            let _ = item.visit(&mut finder);
        }
        if let Some(having) = &select.having {
            let _ = having.visit(&mut finder);
        }
        finder.found
    }
}

/// Hàm gộp bị ảnh hưởng khi dòng lặp: SUM cộng lặp, AVG nghiêng về dòng lặp nhiều. MIN, MAX không đổi.
const CHECKED_FUNCTIONS: [&str; 3] = ["sum", "avg", "mean"];

/// Tìm SUM, AVG trong danh sách cột kết quả hay HAVING của một SELECT. depth đếm câu con lồng trong biểu thức: SUM
/// của câu con thuộc FROM của câu con, QueryFinder xét riêng.
struct SumFinder<'a> {
    from: &'a FromTables<'a>,
    links: &'a [JoinLink],
    depth: usize,
    found: Option<DoubleCount>,
}

impl Visitor for SumFinder<'_> {
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
        let Expr::Function(function) = expr else {
            return ControlFlow::Continue(());
        };
        let name = function_name(function);
        if self.depth > 0 || !CHECKED_FUNCTIONS.contains(&name.as_str()) {
            return ControlFlow::Continue(());
        }
        // SUM(DISTINCT x) vẫn xét x: plain_args bỏ DISTINCT.
        let args = plain_args(function);
        let [arg] = args.as_slice() else {
            return ControlFlow::Continue(());
        };
        let Grain::Table(table) = summed_grain(arg, self.from) else {
            return ControlFlow::Continue(());
        };
        if let Some(link) = find_fan_out(table, self.from, self.links) {
            self.found = Some(DoubleCount {
                function: name.to_uppercase(),
                argument: arg.to_string(),
                table: table.to_string(),
                link: link.clone(),
            });
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}

/// Quan hệ trong FROM làm mỗi dòng của table lặp lại, hay None. Đi từ table theo các quan hệ không nhân dòng (từ bên
/// nhiều lên bên một, hay qua quan hệ một-một); gặp một bảng trong FROM trỏ nhiều-một vào bảng đã tới thì dòng của
/// table lặp theo bảng đó.
fn find_fan_out<'a>(table: &str, from: &FromTables, links: &'a [JoinLink]) -> Option<&'a JoinLink> {
    // Chỉ quan hệ có cả hai đầu trong FROM; bỏ quan hệ của một bảng với chính nó (xem đầu file).
    let used: Vec<&JoinLink> = links
        .iter()
        .filter(|l| from.contains(&l.many) && from.contains(&l.one))
        .filter(|l| !l.many.eq_ignore_ascii_case(&l.one))
        .collect();
    let mut reached = vec![table.to_lowercase()];
    let mut next = 0;
    while next < reached.len() {
        let current = reached[next].clone();
        next += 1;
        for link in &used {
            let many = link.many.to_lowercase();
            let one = link.one.to_lowercase();
            // Bên nhiều đã nằm trên đường đi (bảng vừa đi lên từ đó) thì không nhân thêm dòng nào.
            if one == current && link.fans_out && !reached.contains(&many) {
                return Some(link);
            }
            let other = if many == current {
                one
            } else if one == current {
                // Quan hệ một-một (nhiều-một đã trả ở trên): đi xuống không nhân dòng.
                many
            } else {
                continue;
            };
            if !reached.contains(&other) {
                reached.push(other);
            }
        }
    }
    None
}
