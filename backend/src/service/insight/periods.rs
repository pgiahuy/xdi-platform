// SPDX-License-Identifier: Apache-2.0
/* service/insight/periods.rs - đọc kỳ của câu thường cho phân rã tự động (#227): cột thời gian và độ hạt (tháng, quý,
năm) của khóa nhóm chuỗi thời gian, kỳ của từng ô khóa, và kỳ của câu hỏi một con số từ các vế lọc thời gian. Biểu thức
đã viết lại theo nguồn của plan (insight/plan.rs). Hàm thuần, test ở insight/tests.rs.

Chỉ nhận vài dạng hay gặp, đọc chắc được: date_trunc('month', t), strftime(t, '%Y-%m'), strftime(t, '%Y'), year(t) cho
khóa nhóm; year(t) = 2017 (kèm month(t) hay quarter(t)), strftime(t, ...) = '...', date_trunc(...) = '...' và khoảng
t >= ... AND t < ... (hay BETWEEN) đúng bằng một tháng, một quý hay một năm cho vế lọc. Dạng khác thì không có phân rã
tự động: câu vẫn có phần giải thích từ bảng kết quả, chỉ thiếu phần so kỳ trước. Kỳ đọc sai cũng không lọt ra ngoài:
insight/mod.rs so tổng của kỳ do code tính với ô của bảng kết quả, lệch thì bỏ phân rã. */

use chrono::{Duration, NaiveDate};
use sqlparser::ast::{BinaryOperator, Expr};

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::ask::contract::CellValue;
use crate::service::explain::check::PlanTime;
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::build_time_expr;
use crate::service::understanding::labels::{
    build_comparison, build_literal_text, build_plain_args,
};

/// Cột thời gian và độ hạt của khóa nhóm chuỗi thời gian. column trả hồ sơ của một tên cột trong nguồn (tên trơn của
/// bảng chính, "bảng.cột" của bảng liên quan), đã đổi tên thành tên trong nguồn.
pub fn read_series_time(
    key: &Expr,
    column: &dyn Fn(&str) -> Option<ProfileColumn>,
) -> Option<PlanTime> {
    let (name, args) = read_call(key)?;
    let grain = match (name.as_str(), args.as_slice()) {
        ("date_trunc", [unit, _]) => read_grain(&build_literal_text(unit)?)?,
        ("strftime", [_, format]) => read_format_grain(&build_literal_text(format)?)?,
        ("year", [_]) => Grain::Year,
        _ => return None,
    };
    let value = match (name.as_str(), args.as_slice()) {
        ("date_trunc", [_, value]) | ("strftime", [value, _]) | ("year", [value]) => *value,
        _ => return None,
    };
    build_plan_time(value, grain, column)
}

/// Kỳ của một ô khóa: ngày (date_trunc) thành kỳ chứa ngày đó, chữ "2017-05" hay "2017" (strftime), số 2017 (year).
pub fn read_cell_period(grain: Grain, cell: Option<&CellValue>) -> Option<Period> {
    match cell? {
        CellValue::String(text) => match read_date(text) {
            Some(date) => Some(Period::containing(grain, date)),
            None => Period::parse(grain, text).ok(),
        },
        CellValue::Number(number) if grain == Grain::Year && number.fract() == 0.0 => {
            Period::parse(grain, &format!("{number:.0}")).ok()
        }
        _ => None,
    }
}

/// Kỳ của câu hỏi một con số, đọc từ các vế lọc: (cột thời gian, kỳ, vị trí các vế thời gian để bỏ khi so kỳ trước).
/// None khi không có vế thời gian, vế không đọc được, hay các vế không ghép thành đúng một kỳ.
pub fn read_filter_period(
    filters: &[Expr],
    column: &dyn Fn(&str) -> Option<ProfileColumn>,
) -> Option<(PlanTime, Period, Vec<usize>)> {
    let mut parts = PeriodParts::default();
    let mut used = Vec::new();
    let mut time_name: Option<String> = None;
    for (index, filter) in filters.iter().enumerate() {
        let Some(subject) = find_time_name(filter, column) else {
            continue;
        };
        if time_name.as_ref().is_some_and(|name| *name != subject) {
            return None;
        }
        time_name = Some(subject);
        // Vế có cột thời gian mà không đọc được thì không biết kỳ: không đoán.
        apply_filter(&mut parts, filter)?;
        used.push(index);
    }
    let name = time_name?;
    let period = parts.build_period()?;
    let profile = column(&name)?;
    let time = PlanTime {
        expr: build_time_expr(&profile)?,
        column: name,
        grain: period.grain,
    };
    Some((time, period, used))
}

/// Những gì các vế lọc nói về kỳ.
#[derive(Default)]
struct PeriodParts {
    year: Option<i32>,
    month: Option<u32>,
    quarter: Option<u32>,
    /// Kỳ đọc thẳng từ một vế (strftime, date_trunc).
    period: Option<Period>,
    /// Khoảng [start, end).
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
}

impl PeriodParts {
    fn build_period(&self) -> Option<Period> {
        if let Some(period) = self.period {
            return Some(period);
        }
        if let Some(year) = self.year {
            return match (self.month, self.quarter) {
                (Some(month), None) => {
                    Period::parse(Grain::Month, &format!("{year:04}-{month:02}")).ok()
                }
                (None, Some(quarter)) => {
                    Period::parse(Grain::Quarter, &format!("{year:04}-Q{quarter}")).ok()
                }
                (None, None) => Period::parse(Grain::Year, &format!("{year:04}")).ok(),
                _ => None,
            };
        }
        let (start, end) = (self.start?, self.end?);
        [Grain::Month, Grain::Quarter, Grain::Year]
            .into_iter()
            .map(|grain| Period::containing(grain, start))
            .find(|period| period.range() == Some((start, end)))
    }
}

// Một vế lọc thời gian; None khi vế không thuộc dạng nào đọc được.
fn apply_filter(parts: &mut PeriodParts, filter: &Expr) -> Option<()> {
    if let Expr::Between {
        negated: false,
        low,
        high,
        ..
    } = filter
    {
        parts.start = Some(read_date(&build_literal_text(low)?)?);
        parts.end = Some(read_inclusive_end(&build_literal_text(high)?)?);
        return Some(());
    }
    let comparison = build_comparison(filter)?;
    let value = comparison.value.as_str();
    if let Some((name, args)) = read_call(comparison.subject) {
        if comparison.op != BinaryOperator::Eq {
            return None;
        }
        let number = || value.parse::<u32>().ok();
        match (name.as_str(), args.as_slice()) {
            ("year", [_]) => parts.year = Some(i32::try_from(number()?).ok()?),
            ("month", [_]) => parts.month = Some(number()?),
            ("quarter", [_]) => parts.quarter = Some(number()?),
            ("strftime", [_, format]) => {
                let grain = read_format_grain(&build_literal_text(format)?)?;
                parts.period = Some(Period::parse(grain, value).ok()?);
            }
            ("date_trunc", [unit, _]) => {
                let grain = read_grain(&build_literal_text(unit)?)?;
                let date = read_date(value)?;
                let period = Period::containing(grain, date);
                // Ngày phải là ngày đầu kỳ, như date_trunc trả; ngày khác thì vế không khớp dòng nào.
                (period.range()?.0 == date).then_some(())?;
                parts.period = Some(period);
            }
            // strptime, CAST: chính là cột thời gian, so khoảng như dưới.
            ("strptime", _) => apply_bound(parts, &comparison.op, value)?,
            _ => return None,
        }
        return Some(());
    }
    apply_bound(parts, &comparison.op, value)
}

fn apply_bound(parts: &mut PeriodParts, op: &BinaryOperator, value: &str) -> Option<()> {
    match op {
        BinaryOperator::GtEq => parts.start = Some(read_date(value)?),
        BinaryOperator::Gt => parts.start = Some(read_date(value)? + Duration::days(1)),
        BinaryOperator::Lt => parts.end = Some(read_date(value)?),
        BinaryOperator::LtEq => parts.end = Some(read_inclusive_end(value)?),
        _ => return None,
    }
    Some(())
}

// Mốc cuối tính cả mốc đó ("<= 2017-11-30", "BETWEEN ... AND 2017-11-30"): ngày sau đó là mốc cuối không tính. Mốc có
// giờ khác 00:00 ("2017-11-30 23:59:59") vẫn là trọn ngày 30.
fn read_inclusive_end(value: &str) -> Option<NaiveDate> {
    Some(read_date(value)? + Duration::days(1))
}

/// Mười ký tự đầu là ngày ISO: "2017-11-01", "2017-11-01 00:00:00", "2017-11-01T00:00:00".
fn read_date(text: &str) -> Option<NaiveDate> {
    let day = text.get(..10)?;
    NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()
}

fn read_grain(unit: &str) -> Option<Grain> {
    match unit.to_lowercase().as_str() {
        "month" => Some(Grain::Month),
        "quarter" => Some(Grain::Quarter),
        "year" => Some(Grain::Year),
        _ => None,
    }
}

fn read_format_grain(format: &str) -> Option<Grain> {
    match format {
        "%Y-%m" => Some(Grain::Month),
        "%Y" => Some(Grain::Year),
        _ => None,
    }
}

/// Tên hàm (viết thường) và các đối số trơn của một lời gọi hàm; bỏ CAST và ngoặc bao ngoài.
fn read_call(expr: &Expr) -> Option<(String, Vec<&Expr>)> {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => read_call(inner),
        Expr::Function(function) => Some((
            function.name.to_string().to_lowercase(),
            build_plain_args(function),
        )),
        _ => None,
    }
}

/// PlanTime của biểu thức chứa đúng một cột thời gian (cột trơn, strptime(cột, ...), CAST(cột AS DATE)).
fn build_plan_time(
    value: &Expr,
    grain: Grain,
    column: &dyn Fn(&str) -> Option<ProfileColumn>,
) -> Option<PlanTime> {
    let name = find_time_name(value, column)?;
    let profile = column(&name)?;
    Some(PlanTime {
        expr: build_time_expr(&profile)?,
        column: name,
        grain,
    })
}

// Tên cột thời gian của một biểu thức: cột vai trò time đầu tiên xuất hiện trong đó.
fn find_time_name(expr: &Expr, column: &dyn Fn(&str) -> Option<ProfileColumn>) -> Option<String> {
    let mut found = None;
    let _ = sqlparser::ast::visit_expressions(expr, |e| {
        if let Expr::Identifier(ident) = e {
            if column(&ident.value).is_some_and(|c| c.role == ProfileRole::Time.as_str()) {
                found = Some(ident.value.clone());
                return std::ops::ControlFlow::Break(());
            }
        }
        std::ops::ControlFlow::Continue(())
    });
    found
}
