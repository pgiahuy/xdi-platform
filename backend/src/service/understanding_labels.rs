// SPDX-License-Identifier: Apache-2.0
/* service/understanding_labels.rs - nhãn tiếng Việt cho từng mục của thẻ "Đã hiểu là" (service/understanding.rs), đọc
từ cây sqlparser của một biểu thức trong engine/sql_shape.rs và hồ sơ cột. Nhãn chỉ để người dùng đọc; đoạn SQL đi kèm
mỗi mục mới là thứ đối chiếu được, nên dạng nào không theo khuôn thì nhãn là chính đoạn SQL, không đoán nghĩa.

Tên cột: mô tả (#23) kèm tên trong ngoặc, thiếu mô tả thì chỉ tên, để người dùng thấy cả nghĩa lẫn đúng cột trong dữ
liệu của mình. */

use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, DataType, DateTimeField, DuplicateTreatment, Expr, Function, FunctionArg,
    FunctionArgExpr, FunctionArguments, Value, Visit, Visitor,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use super::ask_context::ColumnProfile;
use crate::engine::store::MetricDefinition;

/// Hàm lấy một phần của thời gian, và chữ cho phần đó. Đọc ở nhãn khóa nhóm ("Quý của ngày đặt hàng") và để nhận biểu
/// thức thời gian. Cũng là bảng tên phần của EXTRACT(YEAR FROM x), date_part('year', x) và date_trunc('month', x).
const TIME_PARTS: &[(&str, &str)] = &[
    ("year", "Năm"),
    ("quarter", "Quý"),
    ("month", "Tháng"),
    ("week", "Tuần"),
    ("weekofyear", "Tuần"),
    ("day", "Ngày"),
    ("dayofmonth", "Ngày"),
    ("hour", "Giờ"),
];

/// Hàm đọc chữ thành thời gian: nhãn đọc xuyên qua chúng tới cột bên trong.
const TIME_PARSERS: &[&str] = &["strptime", "to_timestamp"];

/// Hàm nhận tên phần thời gian ở tham số đầu: date_trunc('month', x) và date_part('year', x).
const TIME_PART_FUNCTIONS: &[&str] = &["date_trunc", "datetrunc", "date_part", "datepart"];

pub fn build_column_label(name: &str, profile: &[ColumnProfile], mid_sentence: bool) -> String {
    let column = profile.iter().find(|c| c.name.eq_ignore_ascii_case(name));
    match column.and_then(|c| c.description.as_deref()) {
        Some(description) if !description.trim().is_empty() => {
            let description = if mid_sentence {
                lower_first(description.trim())
            } else {
                description.trim().to_string()
            };
            format!("{description} ({name})")
        }
        _ => name.to_string(),
    }
}

/// Nhãn của thứ được đo, nhóm hay lọc: cột, phần thời gian của cột, hoặc đoạn SQL khi không theo khuôn.
pub fn build_subject_label(expr: &Expr, profile: &[ColumnProfile], mid_sentence: bool) -> String {
    match expr {
        Expr::Identifier(ident) => build_column_label(&ident.value, profile, mid_sentence),
        Expr::CompoundIdentifier(parts) => match parts.last() {
            Some(ident) => build_column_label(&ident.value, profile, mid_sentence),
            None => expr.to_string(),
        },
        Expr::Nested(inner) => build_subject_label(inner, profile, mid_sentence),
        Expr::Cast {
            expr: inner,
            data_type: DataType::Date,
            ..
        } => build_part_label("Ngày", inner, profile, mid_sentence),
        Expr::Cast { expr: inner, .. } => build_subject_label(inner, profile, mid_sentence),
        Expr::Extract {
            field, expr: inner, ..
        } => match find_time_part(&build_field_name(field)) {
            Some(part) => build_part_label(part, inner, profile, mid_sentence),
            None => expr.to_string(),
        },
        Expr::Function(function) => build_function_subject(expr, function, profile, mid_sentence),
        _ => expr.to_string(),
    }
}

fn build_function_subject(
    expr: &Expr,
    function: &Function,
    profile: &[ColumnProfile],
    mid_sentence: bool,
) -> String {
    let name = function.name.to_string().to_lowercase();
    let args = build_plain_args(function);
    if TIME_PARSERS.contains(&name.as_str()) {
        if let Some(inner) = args.first() {
            return build_subject_label(inner, profile, mid_sentence);
        }
    }
    let part = match (name.as_str(), args.as_slice()) {
        (unit_function, [unit, inner]) if TIME_PART_FUNCTIONS.contains(&unit_function) => {
            build_literal_text(unit)
                .and_then(|unit| find_time_part(&unit.to_lowercase()))
                .map(|part| (part, *inner))
        }
        (_, [inner]) => find_time_part(&name).map(|part| (part, *inner)),
        _ => None,
    };
    match part {
        Some((part, inner)) => build_part_label(part, inner, profile, mid_sentence),
        None => expr.to_string(),
    }
}

/// Tên phần của EXTRACT, chữ thường: YEAR thành year. DuckDB cũng nhận EXTRACT('year' FROM x), sqlparser giữ nó là
/// Custom kèm nháy nên đọc thẳng chữ bên trong.
fn build_field_name(field: &DateTimeField) -> String {
    match field {
        DateTimeField::Custom(ident) => ident.value.to_lowercase(),
        other => other.to_string().to_lowercase(),
    }
}

fn find_time_part(name: &str) -> Option<&'static str> {
    TIME_PARTS
        .iter()
        .find(|(function, _)| *function == name)
        .map(|(_, label)| *label)
}

fn build_part_label(part: &str, inner: &Expr, profile: &[ColumnProfile], mid: bool) -> String {
    let part = if mid {
        lower_first(part)
    } else {
        part.to_string()
    };
    format!("{part} của {}", build_subject_label(inner, profile, true))
}

/// Tham số thường của hàm (không tên, không `*`), theo thứ tự.
fn build_plain_args(function: &Function) -> Vec<&Expr> {
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

/// Giá trị hằng như người dùng đọc: chuỗi bỏ nháy, số, TIMESTAMP '2017-07-01' thành 2017-07-01. None khi không phải
/// hằng.
pub fn build_literal_text(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(value) => match &value.value {
            Value::SingleQuotedString(text) => Some(text.clone()),
            Value::Number(number, _) => Some(number.clone()),
            Value::Boolean(flag) => Some(flag.to_string()),
            _ => None,
        },
        Expr::TypedString(typed) => typed.value.clone().into_string(),
        Expr::Cast { expr: inner, .. } | Expr::Nested(inner) => build_literal_text(inner),
        _ => None,
    }
}

/// Biểu thức có đọc thời gian: dùng cột vai trò time của hồ sơ, gọi hàm thời gian, hay đổi sang DATE, TIMESTAMP.
pub fn is_time_expr(expr: &Expr, profile: &[ColumnProfile]) -> bool {
    expr.visit(&mut TimeFinder { profile }).is_break()
}

struct TimeFinder<'a> {
    profile: &'a [ColumnProfile],
}

impl TimeFinder<'_> {
    fn is_time_column(&self, name: &str) -> bool {
        self.profile
            .iter()
            .any(|c| c.role == "time" && c.name.eq_ignore_ascii_case(name))
    }
}

impl Visitor for TimeFinder<'_> {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        let found = match expr {
            Expr::Identifier(ident) => self.is_time_column(&ident.value),
            Expr::CompoundIdentifier(parts) => {
                parts.last().is_some_and(|p| self.is_time_column(&p.value))
            }
            Expr::Function(function) => {
                let name = function.name.to_string().to_lowercase();
                TIME_PARSERS.contains(&name.as_str())
                    || find_time_part(&name).is_some()
                    || TIME_PART_FUNCTIONS.contains(&name.as_str())
            }
            Expr::Extract { .. } => true,
            Expr::Cast { data_type, .. } => matches!(
                data_type,
                DataType::Date | DataType::Timestamp(..) | DataType::Datetime(..)
            ),
            _ => false,
        };
        if found {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

/// Một phép so giữa một biểu thức và một hằng, hằng đã chuyển về bên phải: `2017 = year(x)` thành `year(x) = 2017`.
pub struct Comparison<'a> {
    pub subject: &'a Expr,
    pub op: BinaryOperator,
    pub value: String,
}

pub fn build_comparison(expr: &Expr) -> Option<Comparison<'_>> {
    let Expr::BinaryOp { left, op, right } = expr else {
        return None;
    };
    let flipped = match op {
        BinaryOperator::Eq | BinaryOperator::NotEq => op.clone(),
        BinaryOperator::Gt => BinaryOperator::Lt,
        BinaryOperator::GtEq => BinaryOperator::LtEq,
        BinaryOperator::Lt => BinaryOperator::Gt,
        BinaryOperator::LtEq => BinaryOperator::GtEq,
        _ => return None,
    };
    match (build_literal_text(left), build_literal_text(right)) {
        (None, Some(value)) => Some(Comparison {
            subject: left,
            op: op.clone(),
            value,
        }),
        (Some(value), None) => Some(Comparison {
            subject: right,
            op: flipped,
            value,
        }),
        _ => None,
    }
}

/// Nhãn của một vế lọc không phải khoảng thời gian.
pub fn build_filter_label(expr: &Expr, profile: &[ColumnProfile]) -> String {
    if let Some(comparison) = build_comparison(expr) {
        let subject = build_subject_label(comparison.subject, profile, false);
        let value = comparison.value;
        let phrase = match comparison.op {
            BinaryOperator::Eq => format!("là {value}"),
            BinaryOperator::NotEq => format!("khác {value}"),
            BinaryOperator::Gt => format!("lớn hơn {value}"),
            BinaryOperator::GtEq => format!("từ {value} trở lên"),
            BinaryOperator::Lt => format!("nhỏ hơn {value}"),
            _ => format!("không quá {value}"),
        };
        return format!("{subject} {phrase}");
    }
    match expr {
        Expr::InList {
            expr: subject,
            list,
            negated,
        } => match build_literal_list(list) {
            Some(values) => {
                let word = if *negated {
                    "không thuộc"
                } else {
                    "là một trong"
                };
                let subject = build_subject_label(subject, profile, false);
                format!("{subject} {word} {values}")
            }
            None => expr.to_string(),
        },
        Expr::Between {
            expr: subject,
            negated: false,
            low,
            high,
        } => match (build_literal_text(low), build_literal_text(high)) {
            (Some(low), Some(high)) => {
                let subject = build_subject_label(subject, profile, false);
                format!("{subject} từ {low} đến {high}")
            }
            _ => expr.to_string(),
        },
        Expr::IsNull(subject) => format!("{} trống", build_subject_label(subject, profile, false)),
        Expr::IsNotNull(subject) => {
            format!(
                "{} có giá trị",
                build_subject_label(subject, profile, false)
            )
        }
        _ => expr.to_string(),
    }
}

fn build_literal_list(list: &[Expr]) -> Option<String> {
    let values: Option<Vec<String>> = list.iter().map(build_literal_text).collect();
    values.map(|values| values.join(", "))
}

/// Nhãn của một điều kiện phân quyền guard thêm (GuardedSql::row_filters). Guard viết nó dạng `"cột" IN ('a', 'b')`,
/// hoặc FALSE khi vai dữ liệu không được thấy giá trị nào (engine/guard/mod.rs, build_column_condition).
pub fn build_row_filter_label(sql: &str, profile: &[ColumnProfile]) -> String {
    let parsed = Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_expr());
    let described = match &parsed {
        Ok(Expr::InList {
            expr,
            list,
            negated: false,
        }) => build_literal_list(list).map(|values| {
            let column = build_subject_label(expr, profile, true);
            format!("chỉ thấy {column} là {values}")
        }),
        Ok(Expr::Value(value)) if value.value == Value::Boolean(false) => {
            Some("không được thấy dòng nào".to_string())
        }
        _ => None,
    };
    format!(
        "Phân quyền: {}",
        described.unwrap_or_else(|| sql.to_string())
    )
}

/// Nhãn ngắn (để ghép vào mục sắp xếp) và nhãn đủ của một cột tổng hợp. Trùng biểu thức của một chỉ số thì "<tên chỉ
/// số>: <mô tả>"; hàm tổng hợp quen thuộc trên một cột thì câu như "Tổng doanh thu"; còn lại là tên cột kết quả.
pub fn build_measure_labels(
    tree: &Expr,
    name: &str,
    metrics: &[MetricDefinition],
    profile: &[ColumnProfile],
) -> (String, String) {
    if let Some(metric) = find_metric(tree, metrics) {
        let full = if metric.description.trim().is_empty() {
            metric.name.clone()
        } else {
            format!("{}: {}", metric.name, metric.description.trim())
        };
        return (metric.name.clone(), full);
    }
    let label = build_aggregate_label(tree, profile).unwrap_or_else(|| name.to_string());
    (label.clone(), label)
}

// Chỉ số có filter thì biểu thức trùng chưa đủ để nói câu đã tính đúng chỉ số đó (phải có cả điều kiện lọc), nên bỏ
// qua. So bằng chữ in lại từ cây, không phân biệt hoa thường: SUM và sum, "Order Item Total" và "order item total"
// đều là một với DuckDB.
fn find_metric<'a>(tree: &Expr, metrics: &'a [MetricDefinition]) -> Option<&'a MetricDefinition> {
    let text = tree.to_string();
    metrics.iter().find(|metric| {
        metric.filter.is_none()
            && Parser::new(&DuckDbDialect {})
                .try_with_sql(&metric.expression)
                .and_then(|mut parser| parser.parse_expr())
                .is_ok_and(|expr| expr.to_string().eq_ignore_ascii_case(&text))
    })
}

fn build_aggregate_label(tree: &Expr, profile: &[ColumnProfile]) -> Option<String> {
    let Expr::Function(function) = tree else {
        return None;
    };
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    if function.filter.is_some() || function.over.is_some() {
        return None;
    }
    let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
    let name = function.name.to_string().to_lowercase();
    let arg = match list.args.as_slice() {
        [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)] if name == "count" && !distinct => {
            return Some("Số dòng".to_string());
        }
        [FunctionArg::Unnamed(FunctionArgExpr::Expr(arg))] => arg,
        _ => return None,
    };
    let subject = build_subject_label(arg, profile, true);
    let label = match (name.as_str(), distinct) {
        ("count", true) => format!("Số {subject} khác nhau"),
        ("count", false) => format!("Số dòng có {subject}"),
        ("sum", false) => format!("Tổng {subject}"),
        ("avg" | "mean", false) => format!("Trung bình {subject}"),
        ("min", _) => format!("Nhỏ nhất của {subject}"),
        ("max", _) => format!("Lớn nhất của {subject}"),
        ("median", false) => format!("Trung vị của {subject}"),
        _ => return None,
    };
    Some(label)
}

/// Hạ chữ đầu để ghép giữa câu ("Quý của ngày đặt hàng"). Từ đầu viết hoa cả (SKU, EU) thì giữ nguyên.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.clone().next()) {
        (Some(first), second) if !second.is_some_and(char::is_uppercase) => {
            first.to_lowercase().chain(chars).collect()
        }
        _ => text.to_string(),
    }
}
