// SPDX-License-Identifier: Apache-2.0
/* service/insight/rate.rs - nhận ra độ đo là một tỷ lệ từ 0 tới 1 (tỷ lệ giao trễ, tỷ lệ đạt), để phần giải thích viết
"95,3%" và "chênh 56,7 điểm phần trăm" thay cho "0,95" (insight/facts.rs, ValueStyle::Rate). Đọc cây cú pháp của cột
kết quả trong câu đã chạy (engine/sql_shape.rs) và hồ sơ cột. insight/mod.rs gọi. Hàm thuần, test ở insight/tests.rs.

Là tỷ lệ khi biểu thức (bỏ qua ngoặc, CAST, ROUND, COALESCE) là:
- AVG của một cờ: cột BOOLEAN, cột số chỉ có 0 và 1 theo hồ sơ, CASE WHEN ... THEN 1 ELSE 0, hay một phép so sánh;
- AVG của cột có đơn vị percent theo hồ sơ;
- một phép chia hai số đếm: COUNT(...), count_if(...), SUM của một cờ, chia cho COUNT(...) (có thể bọc NULLIF(..., 0)).
Thêm một điều kiện trên số liệu: mọi giá trị nằm trong [0, 1] (cột percent: trong [-1, 1], tỷ suất lợi nhuận có thể âm).
Một trong các dạng trên nhân 100 (X * 100, 100 * X, A * 100 / B) là tỷ lệ đã viết bằng phần trăm, giá trị phải nằm
trong [0, 100] (ValueStyle::Percent). Cột percent ghi 0 tới 100 thì không qua điều kiện nào và giá trị viết như số thường. */

use rust_decimal::Decimal;
use sqlparser::ast::{
    BinaryOperator, DataType, DuplicateTreatment, Expr, Function, FunctionArg, FunctionArgExpr,
    FunctionArguments, Value,
};

use crate::engine::store::ProfileColumn;
use crate::service::insight::facts::ValueStyle;
use crate::service::insight::plan::PlainTable;

/// Cách viết giá trị của độ đo: Rate khi biểu thức là một tỷ lệ và mọi giá trị đọc được nằm trong khoảng của tỷ lệ.
pub fn build_value_style(
    expr: Option<&Expr>,
    tables: &[PlainTable],
    values: &[Decimal],
) -> ValueStyle {
    let Some(expr) = expr else {
        return ValueStyle::Number;
    };
    let within = |low: Decimal, high: Decimal| {
        !values.is_empty() && values.iter().all(|v| *v >= low && *v <= high)
    };
    let fits = match read_rate(expr, tables) {
        Some(RateKind::Share) => within(Decimal::ZERO, Decimal::ONE),
        Some(RateKind::UnitPercent) => within(Decimal::NEGATIVE_ONE, Decimal::ONE),
        None if is_hundred_rate(expr, tables) => {
            return match within(Decimal::ZERO, Decimal::ONE_HUNDRED) {
                true => ValueStyle::Percent,
                false => ValueStyle::Number,
            };
        }
        None => false,
    };
    if fits {
        ValueStyle::Rate
    } else {
        ValueStyle::Number
    }
}

/// Tỷ lệ đã nhân 100: X * 100 hay 100 * X với X là tỷ lệ, hay A * 100 / B với A, B là số đếm (câu "tỷ lệ giao trễ (%)").
fn is_hundred_rate(expr: &Expr, tables: &[PlainTable]) -> bool {
    match strip(expr) {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Multiply,
            right,
        } => {
            (is_hundred(right) && read_rate(left, tables).is_some())
                || (is_hundred(left) && read_rate(right, tables).is_some())
        }
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Divide,
            right,
        } => {
            let Expr::BinaryOp {
                left: a,
                op: BinaryOperator::Multiply,
                right: b,
            } = strip(left)
            else {
                return false;
            };
            let counted = (is_hundred(b) && is_count(strip(a), tables))
                || (is_hundred(a) && is_count(strip(b), tables));
            counted && is_count(strip(right), tables)
        }
        _ => false,
    }
}

fn is_hundred(expr: &Expr) -> bool {
    is_number(expr, Decimal::ONE_HUNDRED)
}

/// Hằng số bằng wanted (100 và 100.0 như nhau).
fn is_number(expr: &Expr, wanted: Decimal) -> bool {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => is_number(inner, wanted),
        Expr::Value(value) => match &value.value {
            Value::Number(number, _) => number.parse::<Decimal>() == Ok(wanted),
            _ => false,
        },
        _ => false,
    }
}

/// Loại tỷ lệ: phần của các dòng (từ 0 tới 1), hay trung bình một cột có đơn vị percent (có thể âm).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RateKind {
    Share,
    UnitPercent,
}

fn read_rate(expr: &Expr, tables: &[PlainTable]) -> Option<RateKind> {
    match strip(expr) {
        Expr::Function(function) if is_named(function, &["avg", "mean"]) => {
            let arg = strip(single_arg(function)?);
            if is_flag(arg, tables) {
                Some(RateKind::Share)
            } else if find_column(arg, tables).is_some_and(|c| c.unit.as_deref() == Some("percent"))
            {
                Some(RateKind::UnitPercent)
            } else {
                None
            }
        }
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Divide,
            right,
        } => (is_count(strip(left), tables) && is_count(strip(right), tables))
            .then_some(RateKind::Share),
        _ => None,
    }
}

/// Bỏ lớp không đổi nghĩa của số: ngoặc, CAST, ROUND(x, n), COALESCE(x, 0), NULLIF(x, 0), nhân với 1.0 (mô hình hay
/// viết "1.0 * COUNT_IF(...)" để chia ra số lẻ).
fn strip(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => strip(inner),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Multiply,
            right,
        } if is_number(left, Decimal::ONE) => strip(right),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Multiply,
            right,
        } if is_number(right, Decimal::ONE) => strip(left),
        Expr::Function(function) if is_named(function, &["round", "coalesce", "nullif"]) => {
            match first_arg(function) {
                Some(inner) => strip(inner),
                None => expr,
            }
        }
        _ => expr,
    }
}

/// Số đếm: COUNT(...) hay count_if(...) không có OVER, hay SUM của một cờ.
fn is_count(expr: &Expr, tables: &[PlainTable]) -> bool {
    let Expr::Function(function) = expr else {
        return false;
    };
    if function.over.is_some() {
        return false;
    }
    if is_named(function, &["count", "count_if", "countif"]) {
        return true;
    }
    is_named(function, &["sum"])
        && single_arg(function).is_some_and(|arg| is_flag(strip(arg), tables))
}

/// Biểu thức chỉ ra 0 hay 1 (hay đúng, sai): cột BOOLEAN, cột số chỉ có 0 và 1, phép so sánh, CASE chỉ ra 0, 1.
fn is_flag(expr: &Expr, tables: &[PlainTable]) -> bool {
    match expr {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => {
            find_column(expr, tables).is_some_and(is_flag_column)
        }
        Expr::BinaryOp { op, .. } => matches!(
            op,
            BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
        ),
        Expr::InList { .. } | Expr::IsNull(_) | Expr::IsNotNull(_) => true,
        Expr::Case {
            conditions,
            else_result,
            ..
        } => {
            conditions.iter().all(|when| is_zero_or_one(&when.result))
                && else_result.as_deref().is_none_or(is_zero_or_one)
        }
        Expr::Cast {
            expr: inner,
            data_type,
            ..
        } => !matches!(data_type, DataType::Varchar(_) | DataType::Text) && is_flag(inner, tables),
        Expr::Nested(inner) => is_flag(inner, tables),
        _ => false,
    }
}

fn is_flag_column(column: &ProfileColumn) -> bool {
    let data_type = column.data_type.to_uppercase();
    if data_type.starts_with("BOOL") {
        return true;
    }
    column.min_value.as_deref() == Some("0")
        && column.max_value.as_deref() == Some("1")
        && column.distinct_count.is_some_and(|n| n <= 2)
}

fn is_zero_or_one(expr: &Expr) -> bool {
    match expr {
        Expr::Value(value) => match &value.value {
            Value::Number(number, _) => number
                .parse::<Decimal>()
                .is_ok_and(|n| n.is_zero() || n == Decimal::ONE),
            Value::Null => true,
            _ => false,
        },
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => is_zero_or_one(inner),
        _ => false,
    }
}

fn find_column<'a>(expr: &Expr, tables: &'a [PlainTable]) -> Option<&'a ProfileColumn> {
    let name = match expr {
        Expr::Identifier(ident) => &ident.value,
        Expr::CompoundIdentifier(parts) => &parts.last()?.value,
        _ => return None,
    };
    tables
        .iter()
        .flat_map(|t| t.columns.iter())
        .find(|c| c.name.eq_ignore_ascii_case(name))
}

fn is_named(function: &Function, names: &[&str]) -> bool {
    let name = function.name.to_string().to_lowercase();
    names.contains(&name.as_str())
}

fn first_arg(function: &Function) -> Option<&Expr> {
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    match list.args.first()? {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
        _ => None,
    }
}

/// Đối số duy nhất của hàm; None khi hàm có nhiều đối số, có `*` hay DISTINCT (trung bình các giá trị khác nhau của một
/// cờ không phải tỷ lệ dòng).
fn single_arg(function: &Function) -> Option<&Expr> {
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
    if distinct || list.args.len() != 1 {
        return None;
    }
    first_arg(function)
}
