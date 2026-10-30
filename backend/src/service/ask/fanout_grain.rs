// SPDX-License-Identifier: Apache-2.0
/* service/ask/fanout_grain.rs - phần đọc cây cú pháp của ask/fanout.rs: các bảng trong FROM của một SELECT (bí danh nào
là bảng nào), và giá trị trong SUM hay AVG đến từ bảng nào (summed_grain). ask/fanout.rs dùng hai phần này để quyết
định câu có cộng lặp không; luật và giới hạn ghi ở đầu file đó. */

use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Select,
    TableFactor, UnaryOperator,
};

use crate::engine::guard::AllowedTable;

/// Các bảng của nhóm trong FROM của một SELECT.
pub(super) struct FromTables<'a> {
    /// (tên đứng trước dấu chấm, bảng): bí danh, hay tên bảng khi không đặt bí danh. Tên viết thường.
    pub(super) entries: Vec<(String, &'a AllowedTable)>,
    /// FROM có nguồn khác bảng của nhóm (bảng WITH, câu con): cột không ghi bảng có thể đến từ đó.
    pub(super) has_other: bool,
}

pub(super) fn build_from_tables<'a>(select: &Select, tables: &'a [AllowedTable]) -> FromTables<'a> {
    let mut from = FromTables {
        entries: Vec::new(),
        has_other: false,
    };
    let factors = select
        .from
        .iter()
        .flat_map(|f| std::iter::once(&f.relation).chain(f.joins.iter().map(|j| &j.relation)));
    for factor in factors {
        let TableFactor::Table { name, alias, .. } = factor else {
            from.has_other = true;
            continue;
        };
        // Guard chỉ nhận tên bảng một phần (không schema), nên phần cuối là tên bảng.
        let table_name = name.0.last().and_then(|part| part.as_ident());
        let table = table_name.and_then(|ident| {
            tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(&ident.value))
        });
        match (table, table_name) {
            (Some(table), Some(ident)) => {
                let key = alias.as_ref().map_or(&ident.value, |a| &a.name.value);
                from.entries.push((key.to_lowercase(), table));
            }
            // Tên không phải bảng của nhóm: bảng WITH.
            _ => from.has_other = true,
        }
    }
    from
}

impl FromTables<'_> {
    /// Tên bảng chứa cột, hay None khi không chắc.
    fn resolve(&self, qualifier: Option<&str>, column: &str) -> Option<&str> {
        if let Some(qualifier) = qualifier {
            let qualifier = qualifier.to_lowercase();
            return self
                .entries
                .iter()
                .find(|(key, _)| *key == qualifier)
                .map(|(_, table)| table.name.as_str());
        }
        if self.has_other {
            return None;
        }
        let mut owners: Vec<&str> = self
            .entries
            .iter()
            .filter(|(_, table)| table.columns.iter().any(|c| c.eq_ignore_ascii_case(column)))
            .map(|(_, table)| table.name.as_str())
            .collect();
        // Hai bí danh của cùng một bảng vẫn là một bảng.
        owners.sort_unstable();
        owners.dedup();
        match owners.as_slice() {
            [only] => Some(only),
            // Không bảng nào có cột (tên câu tự đặt) hay nhiều bảng cùng có: không chắc.
            _ => None,
        }
    }

    pub(super) fn contains(&self, table: &str) -> bool {
        self.entries
            .iter()
            .any(|(_, t)| t.name.eq_ignore_ascii_case(table))
    }
}
/// Phần giá trị của một biểu thức đến từ đâu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Grain<'a> {
    /// Chỉ có hằng số (1.1, 0, NULL).
    Constant,
    /// Mọi cột đều của cùng một bảng (tên bảng logic).
    Table(&'a str),
    /// Cột của hai bảng (số lượng nhân giá), hay dạng không xét (hàm khác, câu con, cột không rõ bảng): không bắt.
    Row,
}

/// Hàm chỉ đổi dạng giá trị mà không trộn thêm dòng nào: đi vào đối số của chúng như đi vào ngoặc.
const PASS_THROUGH_FUNCTIONS: [&str; 4] = ["coalesce", "ifnull", "round", "abs"];

/// Xét phần giá trị của biểu thức trong SUM: bỏ qua ngoặc, CAST, dấu âm, COALESCE; với phép tính + - * / % thì gộp
/// hai vế; với CASE thì xét các nhánh THEN và ELSE, bỏ qua điều kiện WHEN.
pub(super) fn summed_grain<'a>(expr: &Expr, from: &'a FromTables) -> Grain<'a> {
    match expr {
        Expr::Identifier(ident) => column_grain(None, &ident.value, from),
        // s.studied_credits: phần áp cuối là bí danh, phần cuối là cột.
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [.., qualifier, column] => column_grain(Some(&qualifier.value), &column.value, from),
            _ => Grain::Row,
        },
        Expr::Value(_) => Grain::Constant,
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => summed_grain(inner, from),
        Expr::UnaryOp {
            op: UnaryOperator::Minus | UnaryOperator::Plus,
            expr: inner,
        } => summed_grain(inner, from),
        Expr::BinaryOp {
            left,
            op:
                BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo,
            right,
        } => combine(summed_grain(left, from), summed_grain(right, from)),
        Expr::Case {
            conditions,
            else_result,
            ..
        } => {
            // Thiếu ELSE thì nhánh đó là NULL, tức một hằng số.
            let mut grain = match else_result {
                Some(other) => summed_grain(other, from),
                None => Grain::Constant,
            };
            for when in conditions {
                grain = combine(grain, summed_grain(&when.result, from));
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
                combine(grain, summed_grain(arg, from))
            })
        }
        _ => Grain::Row,
    }
}

/// Gộp hai phần của một phép tính: cột của hai bảng khác nhau thì là giá trị của từng dòng nối, không cộng lặp; hằng
/// số không đổi gì.
fn combine<'a>(left: Grain<'a>, right: Grain<'a>) -> Grain<'a> {
    match (left, right) {
        (Grain::Row, _) | (_, Grain::Row) => Grain::Row,
        (Grain::Constant, other) | (other, Grain::Constant) => other,
        (Grain::Table(a), Grain::Table(b)) if a.eq_ignore_ascii_case(b) => Grain::Table(a),
        (Grain::Table(_), Grain::Table(_)) => Grain::Row,
    }
}

fn column_grain<'a>(qualifier: Option<&str>, column: &str, from: &'a FromTables) -> Grain<'a> {
    match from.resolve(qualifier, column) {
        Some(table) => Grain::Table(table),
        None => Grain::Row,
    }
}

pub(super) fn function_name(function: &Function) -> String {
    function.name.to_string().to_lowercase()
}

/// Các đối số viết thường (không tên, không dấu *) của một lời gọi hàm; DISTINCT nằm ngoài danh sách này.
pub(super) fn plain_args(function: &Function) -> Vec<&Expr> {
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
