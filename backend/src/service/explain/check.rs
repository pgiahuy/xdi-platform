// SPDX-License-Identifier: Apache-2.0
/* service/explain/check.rs - kiểm form câu "tại sao" (explain/form.rs) với hồ sơ cột (#22), giá trị cột (column_values)
và metrics, rồi dựng WhyPlan: mọi thứ explain/sql.rs cần để tự viết SQL, không đọc lại form. Hàm thuần, không gọi
Postgres, DuckDB hay mô hình; test ở explain/check_tests.rs.

Mỗi chỗ sai thành một FieldError chỉ đúng trường và nói cách sửa, để mô hình sửa được trong một vòng (explain/run.rs).
Code không đoán thay mô hình: cột lạ, vai trò sai, giá trị không có trong danh sách đều là lỗi, kể cả khi chỉ khác hoa
thường, vì giá trị là điều kiện lọc trên dữ liệu thật và khác một chữ là khác dòng.

Phần độ đo (chỉ số, cột với sum, count) ở explain/measure.rs. */

use sqlparser::ast::Expr;

use crate::engine::store::{MetricDefinition, ProfileColumn, ProfileRole};
use crate::service::ask::values::ValueIndex;
use crate::service::explain::form::{FieldError, WhyForm};
use crate::service::explain::measure::{check_measure, find_metric, parse_expr, PlanMeasure};
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::{build_equals, build_time_expr};

/// Chiều có tới ngần này giá trị khác nhau mới phân rã (#39): nhiều hơn thì bảng đóng góp dài mà mỗi nhóm nhỏ.
pub const DIMENSION_MAX_VALUES: i64 = 50;
/// Số chiều phân rã tối đa (#39): mỗi chiều một câu SQL.
pub const DIMENSION_MAX: usize = 8;
/// Số giá trị đưa vào lời báo lỗi khi giá trị không có trong danh sách, đủ để mô hình thấy cách viết.
const VALUES_IN_ERROR: usize = 20;

/// Những gì kiểm form cần biết về phiên bản dataset.
pub struct WhyProfile<'a> {
    /// Tên bảng logic.
    pub table: &'a str,
    /// Hồ sơ cột của phiên bản (Store::list_columns).
    pub columns: &'a [ProfileColumn],
    /// Giá trị của các cột chiều không nhạy cảm (load_value_index).
    pub values: &'a ValueIndex,
    pub metrics: &'a [MetricDefinition],
    /// Số dòng của phiên bản; để biết cột key có lặp lại không (lượng của #40).
    pub row_count: Option<i64>,
}

/// Form đã kiểm, đủ để dựng SQL.
#[derive(Debug, Clone, PartialEq)]
pub struct WhyPlan {
    pub table: String,
    pub measure: PlanMeasure,
    pub slices: PlanSlices,
    /// Điều kiện của câu hỏi (filters của form) cộng điều kiện của chỉ số; áp cho mọi câu SQL.
    pub filters: Vec<Expr>,
    /// Cột chiều để phân rã, theo thứ tự cột của bảng.
    pub dimensions: Vec<String>,
    /// Cột đếm phân biệt làm lượng (#40); None thì không tách lượng và giá.
    pub volume: Option<String>,
}

/// Hai lát cắt.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanSlices {
    /// So hai kỳ trên một cột thời gian.
    Periods {
        time: PlanTime,
        base: Period,
        compare: Period,
        /// base là kỳ liền trước vì mô hình để trống và người hỏi chưa chọn; giả định để hiện ở thẻ "Đã hiểu là".
        base_by_default: bool,
    },
    /// So hai nhóm của một cột chiều; period là kỳ của câu hỏi khi form có time và compare.
    Groups {
        column: String,
        base: String,
        compare: String,
        period: Option<(PlanTime, Period)>,
    },
}

/// Cột thời gian đã kiểm.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanTime {
    pub column: String,
    /// Biểu thức ngày của cột: chính cột, hoặc strptime(cột, time_format) với cột ngày dạng chữ.
    pub expr: Expr,
    pub grain: Grain,
}

/// Kiểm form. base_choice là phương án người hỏi đã chọn cho lựa chọn kỳ so sánh (explain/choices.rs, id why:base), đè
/// lên base của form. Err gồm mọi chỗ sai tìm được, không dừng ở chỗ đầu, để mô hình sửa hết trong một vòng.
pub fn build_why_check(
    form: &WhyForm,
    profile: &WhyProfile,
    base_choice: Option<&str>,
) -> Result<WhyPlan, Vec<FieldError>> {
    let mut errors = Vec::new();
    let measure = check_measure(form, profile, &mut errors);
    let slices = check_slices(form, profile, base_choice, &mut errors);
    let mut filters = check_filters(form, profile, &mut errors);
    let (Some(measure), Some(slices)) = (measure, slices) else {
        return Err(errors);
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    let metric_filter = measure
        .metric
        .as_deref()
        .and_then(|name| find_metric(profile.metrics, name))
        .and_then(|m| m.filter.as_deref())
        .map(parse_expr)
        .transpose()
        .map_err(|e| vec![FieldError::new("metric", e)])?;
    filters.extend(metric_filter);
    Ok(WhyPlan {
        table: profile.table.to_string(),
        dimensions: build_dimensions(form, profile, &slices),
        volume: build_volume(profile),
        measure,
        slices,
        filters,
    })
}

fn check_slices(
    form: &WhyForm,
    profile: &WhyProfile,
    base_choice: Option<&str>,
    errors: &mut Vec<FieldError>,
) -> Option<PlanSlices> {
    if let Some(groups) = &form.groups {
        let column = find_column(profile, &groups.column, "groups.column", errors)?;
        if column.role != ProfileRole::Dimension.as_str() {
            errors.push(wrong_role(column, "groups.column", "dimension"));
            return None;
        }
        let compare = check_value(profile, column, &groups.compare, "groups.compare", errors);
        let base = check_value(profile, column, &groups.base, "groups.base", errors);
        let (compare, base) = (compare?, base?);
        if compare == base {
            errors.push(FieldError::new(
                "groups.base",
                "the two groups are the same value; compare two different values",
            ));
            return None;
        }
        // Có kỳ thì kỳ đó là khoảng thời gian của câu hỏi ("quý 3/2017, vì sao Europe thấp hơn LATAM"); base của form
        // không dùng vì hai lát cắt đã là hai nhóm.
        let period = match (&form.time, non_empty(&form.compare)) {
            (Some(_), Some(text)) => {
                let time = check_time(form, profile, errors)?;
                let period = check_period(&time, profile, text, "compare", errors)?;
                Some((time, period))
            }
            _ => None,
        };
        return Some(PlanSlices::Groups {
            column: column.name.clone(),
            base,
            compare,
            period,
        });
    }
    if form.time.is_none() {
        errors.push(FieldError::new(
            "time",
            "set \"time\" to compare two periods, or \"groups\" to compare two groups",
        ));
        return None;
    }
    let time = check_time(form, profile, errors)?;
    let Some(compare_text) = non_empty(&form.compare) else {
        errors.push(FieldError::new(
            "compare",
            "write the period to explain, such as 2017-Q3; the question must name it",
        ));
        return None;
    };
    let compare = check_period(&time, profile, compare_text, "compare", errors)?;
    let (base, base_by_default) = match build_chosen_base(compare, base_choice) {
        Some(base) => (base, false),
        None => match non_empty(&form.base) {
            Some(text) => (check_period(&time, profile, text, "base", errors)?, false),
            None => (compare.previous(), true),
        },
    };
    if base == compare {
        errors.push(FieldError::new(
            "base",
            "base is the same period as compare; leave base out to use the period just before",
        ));
        return None;
    }
    // Kỳ liền trước do code chọn cũng phải có dữ liệu: quý đầu tiên của dữ liệu không có quý trước để so.
    if base_by_default {
        check_period_in_data(&time, profile, base, "base", errors)?;
    }
    Some(PlanSlices::Periods {
        time,
        base,
        compare,
        base_by_default,
    })
}

/// Phương án người hỏi chọn cho kỳ so sánh: previous_<grain>, same_<grain>_last_year, hoặc một kỳ viết như trong form.
/// None khi không có lựa chọn hay phương án không đọc được (bỏ qua, dùng base của form).
pub fn build_chosen_base(compare: Period, choice: Option<&str>) -> Option<Period> {
    let choice = choice?;
    let grain = compare.grain.as_str();
    if choice == format!("previous_{grain}") {
        return Some(compare.previous());
    }
    if choice == format!("same_{grain}_last_year") {
        return Some(compare.same_last_year());
    }
    Period::parse(compare.grain, choice).ok()
}

fn check_time(
    form: &WhyForm,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Option<PlanTime> {
    let time = form.time.as_ref()?;
    let column = find_column(profile, &time.column, "time.column", errors)?;
    let expr = (column.role == ProfileRole::Time.as_str())
        .then(|| build_time_expr(column))
        .flatten();
    let Some(expr) = expr else {
        errors.push(wrong_role(column, "time.column", "time"));
        return None;
    };
    Some(PlanTime {
        column: column.name.clone(),
        expr,
        grain: time.grain,
    })
}

fn check_period(
    time: &PlanTime,
    profile: &WhyProfile,
    text: &str,
    field: &str,
    errors: &mut Vec<FieldError>,
) -> Option<Period> {
    let period = match Period::parse(time.grain, text) {
        Ok(period) => period,
        Err(e) => {
            errors.push(FieldError::new(field, e));
            return None;
        }
    };
    check_period_in_data(time, profile, period, field, errors)?;
    Some(period)
}

/// Kỳ phải chạm khoảng [min, max] của cột thời gian trong hồ sơ. Kỳ nằm trong khoảng mà không có dòng nào (dữ liệu
/// thủng, hay bộ lọc bỏ hết) thì explain/run.rs bắt khi chạy câu tổng hai lát cắt.
fn check_period_in_data(
    time: &PlanTime,
    profile: &WhyProfile,
    period: Period,
    field: &str,
    errors: &mut Vec<FieldError>,
) -> Option<()> {
    let column = profile.columns.iter().find(|c| c.name == time.column)?;
    let (Some(min), Some(max)) = (
        build_profile_date(column.min_value.as_deref()),
        build_profile_date(column.max_value.as_deref()),
    ) else {
        // Hồ sơ không có min, max đọc được: để bước chạy bảng tổng kiểm.
        return Some(());
    };
    let (start, end) = period.range()?;
    if end <= min || start > max {
        errors.push(FieldError::new(
            field,
            format!(
                "{} has no rows: \"{}\" has data from {min} to {max}",
                period.text(),
                time.column
            ),
        ));
        return None;
    }
    Some(())
}

// min, max của cột thời gian trong hồ sơ là chữ ISO ("2015-01-01 00:00:00" hay "2015-01-01"); mười ký tự đầu là ngày.
fn build_profile_date(text: Option<&str>) -> Option<chrono::NaiveDate> {
    let day = text?.get(..10)?;
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()
}

fn check_filters(form: &WhyForm, profile: &WhyProfile, errors: &mut Vec<FieldError>) -> Vec<Expr> {
    let mut filters = Vec::new();
    let slice_column = form.groups.as_ref().map(|g| g.column.as_str());
    for (i, filter) in form.filters.iter().enumerate() {
        let field = format!("filters[{i}]");
        if filter.op != "=" {
            errors.push(FieldError::new(
                format!("{field}.op"),
                "only \"=\" is supported",
            ));
            continue;
        }
        let column_field = format!("{field}.column");
        let Some(column) = find_column(profile, &filter.column, &column_field, errors) else {
            continue;
        };
        if column.role != ProfileRole::Dimension.as_str() {
            errors.push(wrong_role(column, &column_field, "dimension"));
            continue;
        }
        if slice_column == Some(column.name.as_str()) {
            errors.push(FieldError::new(
                column_field,
                "this column already holds the two groups; remove the filter",
            ));
            continue;
        }
        let value_field = format!("{field}.value");
        if let Some(value) = check_value(profile, column, &filter.value, &value_field, errors) {
            filters.push(build_equals(&column.name, &value));
        }
    }
    filters
}

/// Giá trị phải có nguyên văn trong column_values của cột. Cột chiều nhạy cảm, hay giá trị chưa tải được, không có
/// danh sách để đối chiếu nên cũng là lỗi: lọc theo giá trị không kiểm được thì không đi đường form.
fn check_value(
    profile: &WhyProfile,
    column: &ProfileColumn,
    value: &str,
    field: &str,
    errors: &mut Vec<FieldError>,
) -> Option<String> {
    let listed = profile
        .values
        .columns
        .iter()
        .find(|v| v.column_id == column.id);
    let Some(listed) = listed else {
        errors.push(FieldError::new(
            field,
            format!(
                "the values of \"{}\" are not available; leave this out",
                column.name
            ),
        ));
        return None;
    };
    if listed.values.iter().any(|v| v.value == value) {
        return Some(value.to_string());
    }
    let some: Vec<String> = listed
        .values
        .iter()
        .take(VALUES_IN_ERROR)
        .map(|v| format!("\"{}\"", v.value))
        .collect();
    errors.push(FieldError::new(
        field,
        format!(
            "\"{value}\" is not a value of \"{}\"; write a value exactly as listed, such as {}",
            column.name,
            some.join(", ")
        ),
    ));
    None
}

/// Cột chiều để phân rã (#39): vai trò dimension, không nhạy cảm (giá trị nhóm hiện ra và gửi cho mô hình viết lời),
/// tới DIMENSION_MAX_VALUES giá trị, trừ cột làm lát cắt và cột đã lọc bằng một giá trị (chỉ còn một nhóm).
fn build_dimensions(form: &WhyForm, profile: &WhyProfile, slices: &PlanSlices) -> Vec<String> {
    let slice_column = match slices {
        PlanSlices::Groups { column, .. } => Some(column.as_str()),
        PlanSlices::Periods { .. } => None,
    };
    profile
        .columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .filter(|c| c.distinct_count.is_some_and(|n| n <= DIMENSION_MAX_VALUES))
        .filter(|c| slice_column != Some(c.name.as_str()))
        .filter(|c| !form.filters.iter().any(|f| f.column == c.name))
        .take(DIMENSION_MAX)
        .map(|c| c.name.clone())
        .collect()
}

/// Lượng của #40: đúng một cột vai trò key và cột đó lặp lại (ít giá trị khác nhau hơn số dòng, như Order Id khi một
/// đơn có nhiều dòng). Nhiều cột key thì không đoán cột nào là đơn. Cột key không lặp thì đếm nó chỉ là đếm dòng.
/// metrics chưa có trường đánh dấu chỉ số nào là lượng, nên chưa lấy lượng từ metrics.
fn build_volume(profile: &WhyProfile) -> Option<String> {
    let keys: Vec<&ProfileColumn> = profile
        .columns
        .iter()
        .filter(|c| c.role == ProfileRole::Key.as_str())
        .collect();
    let [key] = keys.as_slice() else {
        return None;
    };
    let repeats =
        matches!((key.distinct_count, profile.row_count), (Some(d), Some(rows)) if d < rows);
    repeats.then(|| key.name.clone())
}

pub(super) fn find_column<'a>(
    profile: &'a WhyProfile,
    name: &str,
    field: &str,
    errors: &mut Vec<FieldError>,
) -> Option<&'a ProfileColumn> {
    let found = profile.columns.iter().find(|c| c.name == name);
    if found.is_none() {
        errors.push(FieldError::new(
            field,
            format!(
                "\"{name}\" is not a column of {}; use a column name exactly as listed",
                profile.table
            ),
        ));
    }
    found
}

fn wrong_role(column: &ProfileColumn, field: &str, wanted: &str) -> FieldError {
    FieldError::new(
        field,
        format!(
            "\"{}\" has role {}; this field needs a column of role {wanted}",
            column.name, column.role
        ),
    )
}

fn non_empty(text: &Option<String>) -> Option<&str> {
    text.as_deref().map(str::trim).filter(|t| !t.is_empty())
}
