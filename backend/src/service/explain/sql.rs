// SPDX-License-Identifier: Apache-2.0
/* service/explain/sql.rs - dựng mọi câu SQL của câu "tại sao" từ WhyPlan (explain/check.rs): bảng tổng theo kỳ hay theo
nhóm (bảng kết quả hiển thị), điều kiện của hai lát cắt (ExplanationSlice.filter), câu tổng hai lát cắt kèm lượng (#40)
và câu của từng chiều phân rã (#39). explain/run.rs đưa từng câu qua guard (có lọc dòng theo vai dữ liệu của người hỏi)
rồi mới chạy. Hàm thuần: cùng plan luôn ra cùng chữ SQL; test ở explain/check_tests.rs.

Tên cột và giá trị dựng thành nút của cây cú pháp sqlparser rồi mới in (build_ident, build_string, build_timestamp), không
ghép chuỗi người dùng hay mô hình viết vào SQL: dấu nháy trong giá trị ("Europe' OR 1=1 --") được in thành chuỗi SQL
đúng, không cắt được câu. Phần khung câu (SELECT, CASE WHEN, GROUP BY) là chữ cố định của code.

Độ đo của phân rã tính lại theo từng lát cắt bằng CASE WHEN, như mẫu why (ask/examples.rs): SUM(CASE WHEN <lát> THEN
<giá trị> END). Số thực đổi sang DECIMAL(18, 2) ở từng dòng trước khi cộng, để tổng là phép cộng đúng và các nhóm cộng
lại bằng đúng tổng lát cắt (quy tắc ở Explanation, ask/contract.rs). COALESCE(..., 0) cho nhóm chỉ có ở một lát cắt.
Bảng tổng theo kỳ cộng cùng cách (build_total_aggregate), để số của mỗi kỳ trên bảng bằng tổng các nhóm phân rã.

Độ đo tỷ lệ (PlanMeasure.denominator) thêm hai cột mẫu số bên cạnh hai cột tử số: base_value, compare_value là tử,
base_denominator, compare_denominator là mẫu, cùng cách dựng CASE WHEN. Phép chia không làm trong SQL mà ở
explain/ratio.rs bằng Decimal, vì phân rã cần tử và mẫu của từng nhóm chứ không chỉ thương. */

use sqlparser::ast::{BinaryOperator, DataType, Expr, Ident, TimezoneInfo, TypedString, Value};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::store::ProfileColumn;
use crate::service::explain::check::{PlanSlices, PlanTime, WhyPlan};
use crate::service::explain::measure::Additive;
use crate::service::explain::period::Period;
use crate::service::explain::source::build_from_sql;
use crate::service::profile::is_time_type;

/// Tên cột của câu tổng hai lát cắt và câu từng chiều; explain/run.rs đọc kết quả theo đúng các tên này.
pub const BASE_VALUE: &str = "base_value";
pub const COMPARE_VALUE: &str = "compare_value";
pub const BASE_VOLUME: &str = "base_volume";
pub const COMPARE_VOLUME: &str = "compare_volume";
pub const BASE_ROWS: &str = "base_rows";
pub const COMPARE_ROWS: &str = "compare_rows";
pub const BASE_DENOMINATOR: &str = "base_denominator";
pub const COMPARE_DENOMINATOR: &str = "compare_denominator";

/// Số chữ số thập phân của tỷ lệ trên bảng tổng theo kỳ, bằng RATE_SCALE của explain/ratio.rs để số của mỗi kỳ trên
/// bảng trùng với số của phân rã.
const TOTAL_RATE_SCALE: u32 = crate::service::explain::ratio::RATE_SCALE;

/// SQL của bảng kết quả và điều kiện hai lát cắt, trên tên bảng và tên cột logic, trước guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhySql {
    /// Độ đo theo từng kỳ (hay từng nhóm) của hai lát cắt, đúng công thức của chỉ số: bảng người hỏi thấy.
    pub total_sql: String,
    pub base_filter: String,
    pub compare_filter: String,
}

pub fn build_why_sql(plan: &WhyPlan) -> WhySql {
    let (base, compare) = build_slice_conditions(plan);
    let label = build_ident(&plan.measure.label);
    let aggregate = &plan.measure.aggregate;
    let (key, group_by) = match &plan.slices {
        // Kỳ là date_trunc của cột ngày: bảng có một dòng mỗi kỳ, trục thời gian vẽ được đường.
        PlanSlices::Periods { time, .. } => {
            let unit = build_string(time.grain.as_str());
            // Tên cột kỳ hiện ra ở bảng kết quả và tiêu đề biểu đồ, nên dùng danh từ tiếng Việt ("Tháng"), không dùng
            // đơn vị của date_trunc ("month").
            let name = build_ident(time.grain.noun());
            (format!("date_trunc({unit}, {}) AS {name}", time.expr), "1")
        }
        PlanSlices::Groups { column, .. } => (build_ident(column).to_string(), "1"),
    };
    let total_sql = format!(
        "SELECT {key}, {aggregate} AS {label} FROM {} WHERE {} GROUP BY {group_by} ORDER BY {group_by}",
        build_from(plan, &[]),
        build_where(plan)
    );
    WhySql {
        total_sql,
        base_filter: base.to_string(),
        compare_filter: compare.to_string(),
    }
}

/// Tổng độ đo và số dòng của hai lát cắt, cộng lượng (COUNT DISTINCT cột key) khi plan có: một câu cho cả B, C của
/// #39, số dòng để biết lát cắt có dữ liệu, và V0, V1 của #40.
pub fn build_slices_sql(plan: &WhyPlan) -> String {
    let (base, compare) = build_slice_conditions(plan);
    let mut items = build_sliced_items(plan, &base, &compare);
    items.push(format!("COUNT(CASE WHEN {base} THEN 1 END) AS {BASE_ROWS}"));
    items.push(format!(
        "COUNT(CASE WHEN {compare} THEN 1 END) AS {COMPARE_ROWS}"
    ));
    if let Some(volume) = &plan.volume {
        let key = build_ident(volume);
        items.push(format!(
            "COUNT(DISTINCT CASE WHEN {base} THEN {key} END) AS {BASE_VOLUME}"
        ));
        items.push(format!(
            "COUNT(DISTINCT CASE WHEN {compare} THEN {key} END) AS {COMPARE_VOLUME}"
        ));
    }
    format!(
        "SELECT {} FROM {} WHERE {}",
        items.join(", "),
        build_from(plan, &[]),
        build_where(plan)
    )
}

/// Độ đo của hai lát cắt theo từng nhóm của một cột chiều; nhóm null là một dòng riêng.
pub fn build_dimension_sql(plan: &WhyPlan, column: &str) -> String {
    let (base, compare) = build_slice_conditions(plan);
    let from = build_from(plan, &[column]);
    let column = build_ident(column);
    format!(
        "SELECT {column}, {} FROM {from} WHERE {} GROUP BY {column}",
        build_sliced_items(plan, &base, &compare).join(", "),
        build_where(plan)
    )
}

/// Số cặp giá trị khác nhau của hai cột trên đúng tập dòng của câu từng chiều (cùng build_where): explain/dimension_pick.rs
/// so với số nhóm của mỗi cột để biết hai cột có chia dữ liệu y hệt nhau không.
pub fn build_pair_count_sql(plan: &WhyPlan, first: &str, second: &str) -> String {
    format!(
        "SELECT COUNT(*) AS pairs FROM (SELECT DISTINCT {}, {} FROM {} WHERE {}) AS pair_values",
        build_ident(first),
        build_ident(second),
        build_from(plan, &[first, second]),
        build_where(plan)
    )
}

/// Phần FROM: bảng chính, hay câu con nối bảng liên quan khi câu cần cột của chúng (explain/source.rs).
fn build_from(plan: &WhyPlan, columns: &[&str]) -> String {
    build_from_sql(&plan.table, &plan.source, columns)
}

/// Độ đo của hai lát cắt: tử base, tử compare, rồi mẫu base, mẫu compare khi độ đo là tỷ lệ. explain/breakdown.rs đọc
/// các cột của câu từng chiều theo đúng thứ tự này.
fn build_sliced_items(plan: &WhyPlan, base: &Expr, compare: &Expr) -> Vec<String> {
    let measure = &plan.measure;
    let mut items = vec![
        format!("{} AS {BASE_VALUE}", build_sliced(&measure.additive, base)),
        format!(
            "{} AS {COMPARE_VALUE}",
            build_sliced(&measure.additive, compare)
        ),
    ];
    if let Some(denominator) = &measure.denominator {
        items.push(format!(
            "{} AS {BASE_DENOMINATOR}",
            build_sliced(denominator, base)
        ));
        items.push(format!(
            "{} AS {COMPARE_DENOMINATOR}",
            build_sliced(denominator, compare)
        ));
    }
    items
}

/// Điều kiện của lát cắt base và compare.
fn build_slice_conditions(plan: &WhyPlan) -> (Expr, Expr) {
    match &plan.slices {
        PlanSlices::Periods {
            time,
            base,
            compare,
            ..
        } => (
            build_period_condition(time, *base),
            build_period_condition(time, *compare),
        ),
        PlanSlices::Groups {
            column,
            base,
            compare,
            ..
        } => (
            base.build_condition(column, compare),
            build_equals(column, compare),
        ),
    }
}

/// `ngày >= TIMESTAMP 'đầu' AND ngày < TIMESTAMP 'cuối'`: so khoảng [đầu, cuối) đúng cả với cột có giờ phút, và
/// DuckDB dùng được thống kê min, max của Parquet để bỏ qua nhóm dòng ngoài kỳ, điều quarter(...) = 3 không cho.
fn build_period_condition(time: &PlanTime, period: Period) -> Expr {
    // range() chỉ None với năm ngoài 1 tới 9999, mà Period::parse đã chặn; ngày mặc định khi đó cho điều kiện rỗng.
    let (start, end) = period.range().unwrap_or_default();
    build_binary(
        build_binary(
            time.expr.clone(),
            BinaryOperator::GtEq,
            build_timestamp(start),
        ),
        BinaryOperator::And,
        build_binary(time.expr.clone(), BinaryOperator::Lt, build_timestamp(end)),
    )
}

/// Các vế WHERE chung của mọi câu, theo thứ tự trong câu. build_where ghép chúng bằng AND; thẻ "Đã hiểu là"
/// (understanding/why.rs) đọc từng vế để mỗi mục có đoạn SQL nguyên văn của câu đã chạy.
pub struct WhyConditions {
    /// Bộ lọc của câu hỏi và của chỉ số; vế có OR đã bọc ngoặc.
    pub filters: Vec<Expr>,
    /// Kỳ của câu hỏi khi so hai nhóm.
    pub period: Option<Expr>,
    /// Dòng thuộc một trong hai lát cắt: `(<base> OR <compare>)`. Đọc ít dòng hơn, kết quả không đổi vì dòng ngoài
    /// hai lát cắt không góp vào số nào.
    pub slices: Expr,
}

pub fn build_why_conditions(plan: &WhyPlan) -> WhyConditions {
    let (base, compare) = build_slice_conditions(plan);
    let period = match &plan.slices {
        PlanSlices::Groups {
            period: Some((time, period)),
            ..
        } => Some(build_period_condition(time, *period)),
        _ => None,
    };
    WhyConditions {
        filters: plan.filters.iter().cloned().map(build_grouped).collect(),
        period,
        slices: Expr::Nested(Box::new(build_binary(base, BinaryOperator::Or, compare))),
    }
}

/// Điều kiện WHERE chung của mọi câu: bộ lọc của câu hỏi và của chỉ số, kỳ của câu hỏi khi so hai nhóm, và dòng thuộc
/// một trong hai lát cắt.
fn build_where(plan: &WhyPlan) -> String {
    let conditions = build_why_conditions(plan);
    let texts: Vec<String> = conditions
        .filters
        .iter()
        .chain(conditions.period.iter())
        .chain([&conditions.slices])
        .map(Expr::to_string)
        .collect();
    texts.join(" AND ")
}

// Bọc điều kiện có OR trong ngoặc: ghép bằng AND mà không bọc thì "a OR b AND c" đổi nghĩa.
fn build_grouped(condition: Expr) -> Expr {
    match condition {
        Expr::BinaryOp {
            op: BinaryOperator::Or,
            ..
        } => Expr::Nested(Box::new(condition)),
        other => other,
    }
}

/// Một độ đo cộng được (tử hay mẫu) chỉ tính trên các dòng thỏa condition.
fn build_sliced(additive: &Additive, condition: &Expr) -> String {
    match additive {
        Additive::Sum { value, decimal } => {
            let value = build_summand(value, *decimal);
            format!("COALESCE(SUM(CASE WHEN {condition} THEN {value} END), 0)")
        }
        Additive::Count { value: None } => format!("COUNT(CASE WHEN {condition} THEN 1 END)"),
        Additive::Count { value: Some(value) } => {
            format!("COUNT(CASE WHEN {condition} THEN {value} END)")
        }
    }
}

/// Độ đo trên mọi dòng của câu, cho bảng tổng theo kỳ hay theo nhóm (PlanMeasure.aggregate). Không COALESCE như
/// build_sliced: mỗi dòng của bảng là một nhóm có dòng. Tỷ lệ là tử chia mẫu, làm tròn như phân rã; NULLIF cho kỳ có
/// mẫu bằng 0 ra ô trống thay vì lỗi chia cho 0.
pub fn build_total_aggregate(additive: &Additive, denominator: Option<&Additive>) -> String {
    let numerator = build_additive_aggregate(additive);
    match denominator {
        Some(denominator) => format!(
            "ROUND({numerator} / NULLIF({}, 0), {TOTAL_RATE_SCALE})",
            build_additive_aggregate(denominator)
        ),
        None => numerator,
    }
}

fn build_additive_aggregate(additive: &Additive) -> String {
    match additive {
        Additive::Sum { value, decimal } => format!("SUM({})", build_summand(value, *decimal)),
        Additive::Count { value: None } => "COUNT(*)".to_string(),
        Additive::Count { value: Some(value) } => format!("COUNT({value})"),
    }
}

// Một chỗ duy nhất chọn kiểu DECIMAL cho cả bảng tổng và phân rã: hai nơi chọn khác nhau thì hai con số trên trang lệch.
fn build_summand(value: &Expr, decimal: bool) -> String {
    if decimal {
        format!("CAST({value} AS DECIMAL(18, 2))")
    } else {
        value.to_string()
    }
}

/// Biểu thức ngày của một cột thời gian: cột ngày dạng chữ đọc qua strptime theo time_format của hồ sơ; cột DATE,
/// TIMESTAMP dùng thẳng. None khi cột không đọc được thành ngày.
pub fn build_time_expr(column: &ProfileColumn) -> Option<Expr> {
    let ident = build_ident(&column.name);
    if let Some(format) = &column.time_format {
        let text = format!("strptime({ident}, {})", build_string(format));
        return Parser::new(&DuckDbDialect {})
            .try_with_sql(&text)
            .and_then(|mut parser| parser.parse_expr())
            .ok();
    }
    is_time_type(&column.data_type).then_some(ident)
}

/// Tên cột hay bảng trong dấu nháy kép; sqlparser nhân đôi dấu nháy kép bên trong khi in.
pub fn build_ident(name: &str) -> Expr {
    Expr::Identifier(Ident::with_quote('"', name))
}

/// Chuỗi SQL; sqlparser nhân đôi dấu nháy đơn bên trong khi in.
pub fn build_string(value: &str) -> Expr {
    Expr::Value(Value::SingleQuotedString(value.to_string()).with_empty_span())
}

/// `"cột" = 'giá trị'`: bộ lọc của form và lát cắt hai nhóm.
pub fn build_equals(column: &str, value: &str) -> Expr {
    build_binary(build_ident(column), BinaryOperator::Eq, build_string(value))
}

fn build_timestamp(date: chrono::NaiveDate) -> Expr {
    Expr::TypedString(TypedString {
        data_type: DataType::Timestamp(None, TimezoneInfo::None),
        value: Value::SingleQuotedString(date.format("%Y-%m-%d").to_string()).with_empty_span(),
        uses_odbc_syntax: false,
    })
}

fn build_binary(left: Expr, op: BinaryOperator, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    }
}
