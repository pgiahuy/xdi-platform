// SPDX-License-Identifier: Apache-2.0
/* service/ask_magnitude.rs - luật unusual_magnitude của kiểm hợp lý kết quả (#36, ask_checks.rs): một cột tổng hợp ra
số mà dạng của biểu thức không cho phép. Chỉ nhận ra ba dạng, theo cây cú pháp của sql_guarded (engine/sql_shape.rs),
không đoán theo tên cột:
- Tỷ lệ: phép chia hai số đếm (`COUNT(...) / COUNT(*)`, kể cả qua CAST, ROUND hay nhân 1.0), hay AVG của cột chỉ có 0
  và 1 theo hồ sơ (cờ như Late_delivery_risk). Phải nằm trong [0, 1]. Đây là luật bắt được lỗi hay gặp nhất: tử số và
  mẫu số đếm trên hai tập khác nhau hay bị đảo.
- AVG, MIN, MAX của một cột số: phải nằm trong [min, max] của cột trong hồ sơ.
- SUM của một cột số mà hồ sơ có min không âm: không được âm.
Hai luật sau không thể sai trên đúng dữ liệu hồ sơ đã đo, nên chúng bắt chỗ câu đọc cột khác với cột hồ sơ đo (đổi kiểu
sai, cột trùng tên). Mỗi cột kết quả tối đa một cảnh báo, theo ô sai đầu tiên trong các dòng đã gửi. */

use sqlparser::ast::{BinaryOperator, Expr, Value};

use super::ask_contract::{CellValue, ResultTable, ValueKind, Warning, WarningCode};
use super::ask_filter_values::{column_name, find_column};
use super::explain_number::build_number_text;
use super::profile::is_number_type;
use super::understanding_labels::build_plain_args;
use crate::engine::sql_shape::QueryShape;
use crate::engine::store::ProfileColumn;

/// Sai số cho phép khi so số thực với biên: AVG cộng bằng số thực có thể lệch biên ở chữ số cuối.
const TOLERANCE: f64 = 1e-9;

/// Khoảng một cột tổng hợp phải nằm trong, theo dạng biểu thức của nó.
#[derive(Debug, PartialEq)]
enum Bound {
    Ratio,
    Range {
        column: String,
        min: f64,
        max: f64,
        min_text: String,
        max_text: String,
    },
    NotNegative {
        column: String,
    },
}

/// Cảnh báo unusual_magnitude của một câu đã trả lời. Câu đọc bảng tạm (WITH), phép tập hợp hay có `*` thì không biết
/// chắc cột kết quả ứng với biểu thức nào, hay tên cột trong biểu thức là cột nào của hồ sơ, nên không kiểm.
pub fn build_magnitude_warnings(
    result: &ResultTable,
    shape: &QueryShape,
    profile: &[ProfileColumn],
) -> Vec<Warning> {
    let readable = !shape.set_operation
        && shape.table.is_some()
        && shape.outputs.len() == result.columns.len()
        && shape.outputs.iter().all(|o| o.tree.is_some());
    if !readable {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    for (i, output) in shape.outputs.iter().enumerate() {
        let Some(tree) = output.tree.as_ref().filter(|_| output.is_aggregate) else {
            continue;
        };
        let Some(bound) = build_bound(tree, profile) else {
            continue;
        };
        let column = &result.columns[i];
        let outside = result
            .rows
            .iter()
            .filter_map(|row| read_number(row[i].as_ref(), column.kind))
            .find(|value| !is_inside(&bound, *value));
        if let Some(value) = outside {
            warnings.push(Warning {
                code: WarningCode::UnusualMagnitude,
                message: build_message(&bound, &column.name, value),
            });
        }
    }
    warnings
}

fn build_bound(expr: &Expr, profile: &[ProfileColumn]) -> Option<Bound> {
    let expr = strip_wrappers(expr);
    if let Expr::BinaryOp {
        left,
        op: BinaryOperator::Divide,
        right,
    } = expr
    {
        return (is_count(strip_wrappers(left)) && is_count(strip_wrappers(right)))
            .then_some(Bound::Ratio);
    }
    let Expr::Function(function) = expr else {
        return None;
    };
    if function.over.is_some() {
        return None;
    }
    let name = function.name.to_string().to_lowercase();
    let [argument] = build_plain_args(function)[..] else {
        return None;
    };
    let column = find_column(profile, &column_name(strip_casts(argument))?)?;
    if !is_number_type(&column.data_type) {
        return None;
    }
    let min_text = column.min_value.clone()?;
    let max_text = column.max_value.clone()?;
    let (min, max) = (min_text.parse::<f64>().ok()?, max_text.parse::<f64>().ok()?);
    match name.as_str() {
        "avg" | "mean" if min == 0.0 && max == 1.0 => Some(Bound::Ratio),
        "avg" | "mean" | "min" | "max" => Some(Bound::Range {
            column: column.name.clone(),
            min,
            max,
            min_text,
            max_text,
        }),
        "sum" if min >= 0.0 => Some(Bound::NotNegative {
            column: column.name.clone(),
        }),
        _ => None,
    }
}

/// Bỏ lớp không đổi độ lớn: ngoặc, CAST, ROUND, nhân với 1 (`COUNT(x) * 1.0` để DuckDB chia số thực).
fn strip_wrappers(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => strip_wrappers(inner),
        Expr::Function(function) if function.name.to_string().eq_ignore_ascii_case("round") => {
            match build_plain_args(function).first() {
                Some(inner) => strip_wrappers(inner),
                None => expr,
            }
        }
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Multiply,
            right,
        } if is_one(left) => strip_wrappers(right),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Multiply,
            right,
        } if is_one(right) => strip_wrappers(left),
        _ => expr,
    }
}

fn strip_casts(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => strip_casts(inner),
        _ => expr,
    }
}

fn is_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Value(v) if matches!(&v.value, Value::Number(n, _) if n.parse::<f64>() == Ok(1.0)))
}

/// COUNT hay COUNT_IF ở chính câu này (không kèm OVER).
fn is_count(expr: &Expr) -> bool {
    let Expr::Function(function) = expr else {
        return false;
    };
    let name = function.name.to_string().to_lowercase();
    function.over.is_none() && (name == "count" || name == "count_if")
}

/// Số của một ô: cột number là số JSON, cột decimal là chuỗi chữ số. Ô trống hay cột kiểu khác thì None.
fn read_number(cell: Option<&CellValue>, kind: ValueKind) -> Option<f64> {
    match (cell?, kind) {
        (CellValue::Number(value), _) => Some(*value),
        (CellValue::String(text), ValueKind::Decimal) => text.parse().ok(),
        _ => None,
    }
}

fn is_inside(bound: &Bound, value: f64) -> bool {
    let (min, max) = match bound {
        Bound::Ratio => (0.0, 1.0),
        Bound::Range { min, max, .. } => (*min, *max),
        Bound::NotNegative { .. } => (0.0, f64::INFINITY),
    };
    let slack = |edge: f64| TOLERANCE * edge.abs().max(1.0);
    value >= min - slack(min) && value <= max + slack(max)
}

fn build_message(bound: &Bound, name: &str, value: f64) -> String {
    let value = build_number_text(&value.to_string());
    match bound {
        Bound::Ratio => format!(
            "Cột {name} là tỷ lệ nhưng có giá trị {value}, ngoài khoảng 0 đến 1. Kiểm lại tử số và mẫu số của phép \
             chia."
        ),
        Bound::Range {
            column,
            min_text,
            max_text,
            ..
        } => format!(
            "Cột {name} có giá trị {value}, ngoài khoảng {} đến {} của cột {column} trong dữ liệu. Kiểm lại cột và \
             phép tính.",
            build_number_text(min_text),
            build_number_text(max_text)
        ),
        Bound::NotNegative { column } => format!(
            "Cột {name} là tổng của cột {column} nhưng ra số âm {value}, trong khi cột {column} không có giá trị âm. \
             Kiểm lại phép tính."
        ),
    }
}
