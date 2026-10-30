// SPDX-License-Identifier: Apache-2.0
/* service/ask/repeated.rs - chặn câu SQL cộng (SUM) riêng một cột chép từ bảng nối nhiều-một của bản chụp database
(service/sources/, docs/DATA.md mục Nối bảng theo khóa ngoại). Test ở ask/repeated_tests.rs.

Vì sao: dataset "order_items nối products" lặp giá của một sản phẩm trên mọi dòng hàng của sản phẩm đó; SUM("products.price")
trên bảng cộng giá đó một lần cho mỗi dòng hàng, ra một con số sai mà trông hợp lý. Prompt SQL đã có luật cấm, nhưng mô
hình vẫn có lúc viết (chạy thật với Gemini: "Tổng tín chỉ đăng ký theo vùng" ra SUM("students.studied_credits") trên
bảng bài nộp). Nên ask/run.rs kiểm sau guard: câu cộng cột như vậy ngay trên bảng dataset thì không chạy mà gửi mô hình
sửa, kèm cách viết đúng (giữ một dòng cho mỗi dòng của bảng gốc bằng SELECT DISTINCT theo cột khóa, rồi mới cộng). Câu
sửa vẫn cộng sai thì câu hỏi bị từ chối.

Luật chặn (summed_grain): chỉ chặn khi phần giá trị được cộng chỉ gồm cột của cùng một bảng nối và hằng số. Ví dụ bị
chặn: SUM("products.price"), SUM(-CAST("products.price" AS DOUBLE)), SUM(COALESCE("products.price", 0) * 1.1),
SUM("students.credits" + "students.extra"), SUM(DISTINCT "products.price"), SUM(CASE WHEN ... THEN "products.price"
ELSE 0 END). Không chặn: SUM("quantity" * "products.price") (giá nhân số lượng của từng dòng hàng là doanh thu của dòng
đó, không cộng lặp), SUM(CASE WHEN "orders.status" = 'Delivered' THEN "amount" ELSE 0 END) (cột nối chỉ nằm trong điều
kiện), SUM("amount" * "currencies.rate"). Đi theo cây cú pháp của sqlparser, không so chữ: so chữ thì mọi SUM có nhắc
tên cột nối đều bị chặn, kể cả các câu đúng ở trên.

Giới hạn: chỉ xét SUM trong một SELECT đọc thẳng bảng dataset (câu ngoài, hay một SELECT nằm trong WITH hay câu con).
Không lần theo tên cột qua WITH hay bảng con: SUM trên một bảng WITH tự đặt tên không bị xét, dù bảng đó chưa DISTINCT.
Cột từ hai bảng nối khác nhau trong cùng một SUM cũng không bị chặn. So tên cột không phân biệt hoa thường. */

use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Query, Select,
    SetExpr, TableFactor, UnaryOperator, Visit, Visitor,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::store::SourceModel;

/// Một cột chép từ bảng nối nhiều-một.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatedColumn {
    pub name: String,
    /// schema.bảng gốc.
    pub table: String,
    /// Cột của dataset xác định một dòng của bảng gốc.
    pub key: Vec<String>,
}

pub fn build_repeated_columns(model: Option<&SourceModel>) -> Vec<RepeatedColumn> {
    let Some(model) = model else {
        return Vec::new();
    };
    model
        .joins
        .iter()
        .flat_map(|join| {
            join.columns.iter().map(|name| RepeatedColumn {
                name: name.clone(),
                table: join.to_table.clone(),
                key: join.key_columns.clone(),
            })
        })
        .collect()
}

/// Lý do (tiếng Anh, gửi mô hình ở vòng sửa) khi câu cộng riêng một cột lặp lại ngay trên bảng dataset; None khi không.
/// Câu không đọc được thì None: guard đã đọc được câu này, và lỗi đọc ở đây không nên chặn câu hỏi.
pub fn find_repeated_sum(sql: &str, table: &str, repeated: &[RepeatedColumn]) -> Option<String> {
    if repeated.is_empty() {
        return None;
    }
    let statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    let column = statements.iter().find_map(|statement| {
        let mut finder = QueryFinder {
            table,
            repeated,
            found: None,
        };
        let _ = statement.visit(&mut finder);
        finder.found
    })?;
    let key = column
        .key
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(", ");
    // Lời gợi ý nói đúng luật ở trên: nhân với cột của bảng chính thì được, cộng riêng cột lặp thì phải DISTINCT trước.
    Some(format!(
        "The query sums \"{name}\" on its own, and that column was copied from the joined table {source}: its value \
         repeats on every row that links to the same {source} row, so SUM over the rows of {table} counts it many \
         times. If the question asks for an amount per row of {table}, such as quantity times price, multiply it by \
         that column of {table} inside the SUM: that is not double counting. If it asks for a total of {source} \
         values, keep one row per {source} row first, then sum. The columns {key} together identify one {source} \
         row: keep every one of them in the DISTINCT, or rows of different {source} rows with equal values merge. \
         WITH one_row AS (SELECT DISTINCT {key}, \"{name}\" (and any column you group by) FROM {table}) SELECT \
         SUM(\"{name}\") FROM one_row. If the question does not need a total, use MIN, MAX or AVG instead.",
        name = column.name,
        source = column.table,
    ))
}

/// Đi qua mọi câu SELECT (câu ngoài, WITH, câu con): visitor của sqlparser gọi pre_visit_query cho từng Query.
struct QueryFinder<'a> {
    table: &'a str,
    repeated: &'a [RepeatedColumn],
    found: Option<&'a RepeatedColumn>,
}

impl Visitor for QueryFinder<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let SetExpr::Select(select) = query.body.as_ref() {
            if reads_table(select, self.table) {
                self.found = find_in_select(select, self.repeated);
                if self.found.is_some() {
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }
}

fn reads_table(select: &Select, table: &str) -> bool {
    select.from.iter().any(|from| {
        std::iter::once(&from.relation)
            .chain(from.joins.iter().map(|j| &j.relation))
            .any(|factor| match factor {
                TableFactor::Table { name, .. } => name
                    .0
                    .last()
                    .and_then(|part| part.as_ident())
                    .is_some_and(|ident| ident.value.eq_ignore_ascii_case(table)),
                _ => false,
            })
    })
}

/// SUM trong danh sách cột kết quả hay HAVING của một SELECT mà phần được cộng là riêng một cột lặp lại.
fn find_in_select<'a>(
    select: &Select,
    repeated: &'a [RepeatedColumn],
) -> Option<&'a RepeatedColumn> {
    let mut finder = SumFinder {
        repeated,
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

struct SumFinder<'a> {
    repeated: &'a [RepeatedColumn],
    found: Option<&'a RepeatedColumn>,
}

impl<'a> Visitor for SumFinder<'a> {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        let Expr::Function(function) = expr else {
            return ControlFlow::Continue(());
        };
        if function_name(function) != "sum" {
            return ControlFlow::Continue(());
        }
        // SUM có đúng một đối số. SUM(DISTINCT x) vẫn xét x: cộng các giá khác nhau gộp mất hai sản phẩm cùng giá.
        let args = plain_args(function);
        let [arg] = args.as_slice() else {
            return ControlFlow::Continue(());
        };
        if let Grain::Repeated(column) = summed_grain(arg, self.repeated) {
            self.found = Some(column);
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}

/// Phần giá trị của một biểu thức đến từ đâu.
#[derive(Debug, Clone, Copy)]
enum Grain<'a> {
    /// Chỉ có hằng số (1.1, 0, NULL).
    Constant,
    /// Mọi cột đều chép từ cùng một bảng nối: giá trị lặp lại như cột đó, cộng lại là cộng lặp.
    Repeated(&'a RepeatedColumn),
    /// Có cột của bảng chính, cột của hai bảng nối khác nhau, hay dạng biểu thức không xét (hàm khác, câu con):
    /// không chặn. Không chắc thì không chặn, vì chặn nhầm làm hỏng câu đúng; luật trong prompt vẫn còn đó.
    Row,
}

/// Hàm chỉ đổi dạng giá trị mà không trộn thêm dòng nào: đi vào đối số của chúng như đi vào ngoặc.
const PASS_THROUGH_FUNCTIONS: [&str; 4] = ["coalesce", "ifnull", "round", "abs"];

/// Xét phần giá trị của biểu thức trong SUM: bỏ qua ngoặc, CAST, dấu âm, COALESCE; với phép tính + - * / % thì gộp
/// hai vế; với CASE thì xét các nhánh THEN và ELSE, bỏ qua điều kiện WHEN (lọc theo cột nối không làm cộng lặp).
/// Vì vậy CASE WHEN ... THEN "products.price" ELSE 0 END vẫn là cộng riêng cột lặp và bị chặn.
fn summed_grain<'a>(expr: &Expr, repeated: &'a [RepeatedColumn]) -> Grain<'a> {
    match expr {
        Expr::Identifier(ident) => column_grain(&ident.value, repeated),
        // order_items."products.price": phần cuối là tên cột.
        Expr::CompoundIdentifier(parts) => match parts.last() {
            Some(ident) => column_grain(&ident.value, repeated),
            None => Grain::Row,
        },
        Expr::Value(_) => Grain::Constant,
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => summed_grain(inner, repeated),
        Expr::UnaryOp {
            op: UnaryOperator::Minus | UnaryOperator::Plus,
            expr: inner,
        } => summed_grain(inner, repeated),
        Expr::BinaryOp {
            left,
            op:
                BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo,
            right,
        } => combine(summed_grain(left, repeated), summed_grain(right, repeated)),
        Expr::Case {
            conditions,
            else_result,
            ..
        } => {
            // Thiếu ELSE thì nhánh đó là NULL, tức một hằng số.
            let mut grain = match else_result {
                Some(other) => summed_grain(other, repeated),
                None => Grain::Constant,
            };
            for when in conditions {
                grain = combine(grain, summed_grain(&when.result, repeated));
            }
            grain
        }
        Expr::Function(function)
            if PASS_THROUGH_FUNCTIONS.contains(&function_name(function).as_str()) =>
        {
            let args = plain_args(function);
            if args.is_empty() {
                return Grain::Row;
            }
            args.iter().fold(Grain::Constant, |grain, arg| {
                combine(grain, summed_grain(arg, repeated))
            })
        }
        _ => Grain::Row,
    }
}

/// Gộp hai phần của một phép tính: có cột bảng chính thì không còn là cộng lặp; hằng số không đổi gì.
fn combine<'a>(left: Grain<'a>, right: Grain<'a>) -> Grain<'a> {
    match (left, right) {
        (Grain::Row, _) | (_, Grain::Row) => Grain::Row,
        (Grain::Constant, other) | (other, Grain::Constant) => other,
        (Grain::Repeated(a), Grain::Repeated(b)) if a.table == b.table => Grain::Repeated(a),
        (Grain::Repeated(_), Grain::Repeated(_)) => Grain::Row,
    }
}

/// Cột lặp lại có tên này, hay Row nếu là cột của bảng chính (hoặc tên lạ).
fn column_grain<'a>(name: &str, repeated: &'a [RepeatedColumn]) -> Grain<'a> {
    let name = name.to_lowercase();
    match repeated.iter().find(|c| c.name.to_lowercase() == name) {
        Some(column) => Grain::Repeated(column),
        None => Grain::Row,
    }
}

fn function_name(function: &Function) -> String {
    function.name.to_string().to_lowercase()
}

/// Các đối số viết thường (không tên, không dấu *) của một lời gọi hàm.
fn plain_args(function: &Function) -> Vec<&Expr> {
    let FunctionArguments::List(list) = &function.args else {
        return Vec::new();
    };
    list.args
        .iter()
        .filter_map(|arg| match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
            _ => None,
        })
        .collect()
}
