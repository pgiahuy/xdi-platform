// SPDX-License-Identifier: Apache-2.0
/* service/explain/measure.rs - phần độ đo của bước kiểm form câu "tại sao" (explain/check.rs): chỉ số trong metrics
hoặc một cột với sum, count, đổi thành PlanMeasure để explain/sql.rs dựng SQL. Tách khỏi explain/check.rs vì đọc công
thức chỉ số bằng sqlparser là phần riêng: công thức do tổ chức viết, code phải tách được phần bên trong SUM để đặt điều
kiện lát cắt vào. Test ở explain/check_tests.rs.

Chỉ số chỉ nhận dạng cộng được theo nhóm: SUM(biểu thức) hay COUNT(*), COUNT(cột), không DISTINCT, không cửa sổ. Tỷ lệ,
trung bình, đếm phân biệt không phân rã được bằng phép trừ từng nhóm (ngoài phạm vi của #38), nên báo lỗi rồi câu hỏi
chạy như câu thường. */

use sqlparser::ast::{
    DuplicateTreatment, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, ObjectNamePart,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use crate::engine::store::{MetricDefinition, ProfileRole};
use crate::service::ask::phrases::build_key;
use crate::service::explain::check::{find_column, WhyProfile};
use crate::service::explain::form::{FieldError, FormAgg, WhyForm};
use crate::service::explain::sql::{build_ident, build_total_aggregate};
use crate::service::profile::is_integer_type;

/// Độ đo đã kiểm.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanMeasure {
    /// Tên cho người dùng: tên chỉ số, hoặc "Tổng <cột>", "Số dòng có <cột>".
    pub label: String,
    /// Tên chỉ số khi độ đo là chỉ số trong metrics.
    pub metric: Option<String>,
    /// Biểu thức tổng hợp cho bảng tổng theo kỳ, dựng từ additive như câu phân rã (build_total_aggregate):
    /// SUM(CAST("Order Item Total" AS DECIMAL(18, 2))). Dùng thẳng công thức chỉ số SUM("Order Item Total") thì bảng
    /// cộng số thực đủ chữ số còn phân rã làm tròn từng dòng, hai tổng trên cùng trang lệch ở chữ số lẻ.
    pub aggregate: Expr,
    /// Cách tính lại theo từng lát cắt và từng nhóm, cho phân rã.
    pub additive: Additive,
}

/// Độ đo cộng được, tách thành phần bên trong để đặt điều kiện lát cắt vào: SUM(x) thành SUM(CASE WHEN lát THEN x END).
#[derive(Debug, Clone, PartialEq)]
pub enum Additive {
    /// SUM(value). decimal: đổi value sang DECIMAL(18, 2) ở từng dòng trước khi cộng, vì value là số thực (FLOAT,
    /// DOUBLE) hay biểu thức không rõ kiểu; số nguyên và DECIMAL cộng đúng sẵn.
    Sum { value: Expr, decimal: bool },
    /// COUNT(value); None là COUNT(*).
    Count { value: Option<Expr> },
}

pub(super) fn check_measure(
    form: &WhyForm,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Option<PlanMeasure> {
    // Có cả hai thì dùng chỉ số: công thức của tổ chức đáng tin hơn cột mô hình tự chọn.
    if let Some(name) = form.metric.as_deref().filter(|n| !n.trim().is_empty()) {
        return check_metric(name, profile, errors);
    }
    let Some(measure) = &form.measure else {
        errors.push(FieldError::new(
            "metric",
            "set \"metric\" to a metric from the list, or \"measure\" to a column",
        ));
        return None;
    };
    let field = "measure.column";
    let column = find_column(profile, &measure.column, field, errors)?;
    let ident = build_ident(&column.name);
    match measure.agg {
        FormAgg::Sum => {
            let numeric = is_integer_type(&column.data_type)
                || column.data_type.starts_with("DECIMAL")
                || matches!(column.data_type.as_str(), "FLOAT" | "DOUBLE");
            if column.role != ProfileRole::Measure.as_str() || !numeric {
                errors.push(FieldError::new(
                    field,
                    format!(
                        "\"{}\" is a {} column of type {}; \"sum\" needs a number column of role measure",
                        column.name, column.role, column.data_type
                    ),
                ));
                return None;
            }
            let additive = Additive::Sum {
                decimal: !is_exact_type(&column.data_type),
                value: ident,
            };
            build_plan_measure(format!("Tổng {}", column.name), None, additive)
        }
        FormAgg::Count => build_plan_measure(
            format!("Số dòng có {}", column.name),
            None,
            Additive::Count { value: Some(ident) },
        ),
    }
}

fn build_plan_measure(
    label: String,
    metric: Option<String>,
    additive: Additive,
) -> Option<PlanMeasure> {
    Some(PlanMeasure {
        label,
        metric,
        aggregate: parse_expr(&build_total_aggregate(&additive)).ok()?,
        additive,
    })
}

fn check_metric(
    name: &str,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Option<PlanMeasure> {
    let Some(metric) = find_metric(profile.metrics, name) else {
        let names: Vec<String> = profile
            .metrics
            .iter()
            .map(|m| format!("\"{}\"", m.name))
            .collect();
        let hint = if names.is_empty() {
            "there are no metrics; set \"measure\" instead".to_string()
        } else {
            format!("use one of {}, or set \"measure\"", names.join(", "))
        };
        errors.push(FieldError::new(
            "metric",
            format!("\"{name}\" is not a metric; {hint}"),
        ));
        return None;
    };
    let not_additive = || {
        FieldError::new(
            "metric",
            format!(
                "the metric \"{}\" is not a plain SUM or COUNT, so its change cannot be broken down; set \"measure\" \
                 to a column instead",
                metric.name
            ),
        )
    };
    let aggregate = match parse_expr(&metric.expression) {
        Ok(expr) => expr,
        Err(e) => {
            errors.push(FieldError::new("metric", e));
            return None;
        }
    };
    let Some(additive) = build_additive(&aggregate, profile) else {
        errors.push(not_additive());
        return None;
    };
    build_plan_measure(
        build_capitalized(&metric.name),
        Some(metric.name.clone()),
        additive,
    )
}

/// SUM(x), COUNT(*), COUNT(x) không DISTINCT, không FILTER, không OVER; dạng khác trả None.
fn build_additive(aggregate: &Expr, profile: &WhyProfile) -> Option<Additive> {
    let Expr::Function(function) = aggregate else {
        return None;
    };
    let [ObjectNamePart::Identifier(name)] = function.name.0.as_slice() else {
        return None;
    };
    let plain = matches!(function.parameters, FunctionArguments::None)
        && function.filter.is_none()
        && function.over.is_none()
        && function.null_treatment.is_none()
        && function.within_group.is_empty();
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
    if !plain || distinct || !list.clauses.is_empty() || list.args.len() != 1 {
        return None;
    }
    let FunctionArg::Unnamed(arg) = &list.args[0] else {
        return None;
    };
    match (name.value.to_lowercase().as_str(), arg) {
        ("sum", FunctionArgExpr::Expr(value)) => Some(Additive::Sum {
            decimal: !is_exact_expr(value, profile),
            value: value.clone(),
        }),
        ("count", FunctionArgExpr::Wildcard) => Some(Additive::Count { value: None }),
        ("count", FunctionArgExpr::Expr(value)) => Some(Additive::Count {
            value: Some(value.clone()),
        }),
        _ => None,
    }
}

// Biểu thức cộng đúng sẵn khi là đúng một cột số nguyên hay DECIMAL; biểu thức khác không biết kiểu nên đổi sang DECIMAL.
fn is_exact_expr(value: &Expr, profile: &WhyProfile) -> bool {
    let Expr::Identifier(ident) = value else {
        return false;
    };
    profile
        .columns
        .iter()
        .find(|c| c.name == ident.value)
        .is_some_and(|c| is_exact_type(&c.data_type))
}

fn is_exact_type(data_type: &str) -> bool {
    is_integer_type(data_type) || data_type.starts_with("DECIMAL")
}

/// Tên chỉ số như mô hình viết, so không phân biệt hoa thường và cách đặt dấu thanh, như find_metric của ask/choices.rs.
pub fn find_metric<'a>(
    metrics: &'a [MetricDefinition],
    name: &str,
) -> Option<&'a MetricDefinition> {
    let key = build_key(name);
    metrics.iter().find(|m| build_key(&m.name) == key)
}

/// Đọc một biểu thức SQL (công thức, điều kiện của chỉ số). Err là lời tiếng Anh cho log và mô hình. Chữ thừa sau
/// biểu thức là lỗi: parse_expr dừng ở cuối biểu thức đầu tiên, bỏ qua phần sau sẽ chạy một công thức khác với định nghĩa.
pub(super) fn parse_expr(text: &str) -> Result<Expr, String> {
    let fail = |e: String| format!("cannot read the metric definition \"{text}\": {e}");
    let mut parser = Parser::new(&DuckDbDialect {})
        .try_with_sql(text)
        .map_err(|e| fail(e.to_string()))?;
    let expr = parser.parse_expr().map_err(|e| fail(e.to_string()))?;
    if parser.peek_token().token != Token::EOF {
        return Err(fail("text after the expression".to_string()));
    }
    Ok(expr)
}

// "doanh thu" thành "Doanh thu" cho nhãn độ đo.
fn build_capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
