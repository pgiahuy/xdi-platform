// SPDX-License-Identifier: Apache-2.0
/* service/explain/check.rs - kiểm form câu "tại sao" (explain/form.rs) với hồ sơ cột (#22), giá trị cột (column_values)
và metrics, rồi dựng WhyPlan: mọi thứ explain/sql.rs cần để tự viết SQL, không đọc lại form. Hàm thuần, không gọi
Postgres, DuckDB hay mô hình; test ở explain/check_tests.rs.

Mỗi chỗ sai thành một FieldError chỉ đúng trường và nói cách sửa, để mô hình sửa được trong một vòng (explain/run.rs).
Code không đoán thay mô hình: cột lạ, vai trò sai, giá trị không có trong danh sách đều là lỗi, kể cả khi chỉ khác hoa
thường, vì giá trị là điều kiện lọc trên dữ liệu thật và khác một chữ là khác dòng.

Phần độ đo (chỉ số, cột với sum, count) ở explain/measure.rs, phần bộ lọc ở explain/filter.rs. */

use std::collections::HashMap;

use sqlparser::ast::Expr;

use crate::engine::store::{MetricDefinition, ProfileColumn, ProfileRole};
use crate::service::ask::values::ValueIndex;
use crate::service::explain::filter::{build_single_value_columns, check_filters};
use crate::service::explain::form::{CompareWith, FieldError, FormAgg, WhyForm};
use crate::service::explain::group_base::{check_rest_measure, GroupBase};
use crate::service::explain::measure::{
    check_measure, find_metric, parse_expr, Additive, PlanMeasure,
};
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::source::{PlanSource, RelatedTables};
use crate::service::explain::sql::build_time_expr;
use crate::service::explain::unusual::check_find;

/// Chiều có tới ngần này giá trị khác nhau mới phân rã (#39): nhiều hơn thì bảng đóng góp dài mà mỗi nhóm nhỏ.
pub const DIMENSION_MAX_VALUES: i64 = 50;
/// Số cột ứng viên tối đa chạy cho một câu hỏi: mỗi ứng viên một câu SQL, chạy lần lượt. Chọn ở đây: DataCo có 13 ứng
/// viên (12 khi so hai phân khúc, vì cột đang so không là ứng viên), và phân rã 12 ứng viên đo một lần mất 199 ms (bản
/// release, 180 nghìn dòng), nên 30 ứng viên ước còn dưới một giây trên cỡ dữ liệu đó. Bảng rộng hơn thì các cột sau ứng
/// viên thứ 30 theo thứ tự trong file không được xét.
pub const DIMENSION_CANDIDATES_MAX: usize = 30;
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
    /// Tên tiếng Việt của cột (bí danh đầu tiên trong hồ sơ ngữ nghĩa), để lời tóm tắt ghi "Type (cách thanh toán)".
    /// Cột chưa có bí danh thì không có mục nào.
    pub labels: HashMap<String, String>,
    /// Bảng liên quan nối nhiều-một vào table (explain/source.rs, #227); rỗng với dataset nạp từ file. Chiều tên
    /// "bảng.cột" trong dimensions đọc qua đây.
    pub source: PlanSource,
}

/// Hai lát cắt.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanSlices {
    /// So hai kỳ trên một cột thời gian.
    Periods {
        time: PlanTime,
        base: Period,
        compare: Period,
        /// base là kỳ liền trước vì mô hình để trống và người hỏi chưa chọn; giả định để hiện ở thẻ "Cách tính".
        base_by_default: bool,
    },
    /// So hai nhóm của một cột chiều: một nhóm với một nhóm khác, hay với mọi nhóm còn lại (GroupBase); period là kỳ
    /// của câu hỏi khi form có time và compare.
    Groups {
        column: String,
        base: GroupBase,
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
    check_find(form, profile, &mut errors);
    check_rest_measure(form, &mut errors);
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
        // Đếm phân biệt (#244) đã là một số đếm như lượng: tách "số khách trên mỗi đơn" không có nghĩa.
        volume: match measure.additive {
            Additive::CountDistinct { .. } => None,
            _ => build_volume(profile),
        },
        labels: build_labels(profile),
        measure,
        slices,
        filters,
        source: PlanSource::default(),
    })
}

/// Thêm chiều của bảng liên quan (explain/source.rs) sau chiều của bảng chính, đến khi đủ DIMENSION_CANDIDATES_MAX.
/// excluded là cột đã lọc bằng một giá trị: chỉ một nhóm, không giải thích gì. Không có bảng liên quan thì plan giữ
/// nguyên.
pub fn apply_related(plan: &mut WhyPlan, related: &RelatedTables, excluded: &[String]) {
    if related.source.joins.is_empty() {
        return;
    }
    plan.source = related.source.clone();
    for dimension in &related.dimensions {
        if plan.dimensions.len() >= DIMENSION_CANDIDATES_MAX {
            break;
        }
        if excluded.contains(&dimension.name) || plan.dimensions.contains(&dimension.name) {
            continue;
        }
        plan.dimensions.push(dimension.name.clone());
        if let Some(label) = &dimension.label {
            plan.labels.insert(dimension.name.clone(), label.clone());
        }
    }
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
        // base vắng hay rỗng: so với mọi nhóm còn lại của cột (#244).
        let base = match non_empty(&groups.base) {
            Some(text) => {
                check_value(profile, column, text, "groups.base", errors).map(GroupBase::Value)
            }
            None => Some(GroupBase::Rest),
        };
        let (compare, base) = (compare?, base?);
        if base == GroupBase::Value(compare.clone()) {
            errors.push(FieldError::new(
                "groups.base",
                "the two groups are the same value; compare two different values, or leave base out to compare with \
                 all other groups",
            ));
            return None;
        }
        // Có kỳ thì kỳ đó là khoảng thời gian của câu hỏi ("quý 3/2017, vì sao Europe thấp hơn LATAM"); base của form
        // không dùng vì hai lát cắt đã là hai nhóm.
        let period = match (&form.time, non_empty(&form.compare)) {
            (Some(time), Some(text)) => {
                // Kỳ là giá trị của một cột chiều (kỳ học "2014J"): mô hình hay đặt vào time. Với hai nhóm, kỳ đó là một
                // bộ lọc; lời chỉ đúng bộ lọc cần thêm, nếu không vòng sửa lại đổi groups và câu rơi về câu thường.
                if let Some(column) = profile
                    .columns
                    .iter()
                    .find(|c| c.name == time.column && c.role == ProfileRole::Dimension.as_str())
                {
                    errors.push(FieldError::new(
                        "time.column",
                        format!(
                            "\"{}\" has role dimension, so \"{text}\" is a value, not a period: keep \"groups\" as it is, \
                             leave \"time\" out, write \"compare\" as an empty string and add the filter \
                             {{\"column\": \"{}\", \"op\": \"=\", \"value\": \"{text}\"}}",
                            column.name, column.name
                        ),
                    ));
                    return None;
                }
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
        None => match (non_empty(&form.base), form.compare_with) {
            (Some(text), _) => (check_period(&time, profile, text, "base", errors)?, false),
            // Cùng kỳ năm trước do mô hình chọn theo câu hỏi: không là giả định, nhưng vẫn phải có dữ liệu.
            (None, Some(CompareWith::SamePeriodLastYear)) => {
                let base = compare.same_last_year();
                check_period_in_data(&time, profile, base, "compare_with", errors)?;
                (base, false)
            }
            (None, _) => (compare.previous(), true),
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

/// Giá trị phải có nguyên văn trong column_values của cột. Cột chiều nhạy cảm, hay giá trị chưa tải được, không có
/// danh sách để đối chiếu nên cũng là lỗi: lọc theo giá trị không kiểm được thì không đi đường form.
pub(super) fn check_value(
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

/// Cột chiều ứng viên để phân rã (#39): vai trò dimension, không nhạy cảm (giá trị nhóm hiện ra và gửi cho mô hình viết
/// lời), từ 2 tới DIMENSION_MAX_VALUES giá trị (một giá trị thì chỉ có một nhóm), trừ cột làm lát cắt và cột đã lọc bằng
/// một giá trị. Chỉ cắt ở DIMENSION_CANDIDATES_MAX: explain/dimension_pick.rs bỏ chiều trùng, chiều không so được gì,
/// rồi phần chấm điểm xếp các chiều còn lại. Cắt sớm hơn theo thứ tự cột ở đây thì cột đứng sau trong file không bao giờ
/// được xét, dù giải thích tốt hơn.
fn build_dimensions(form: &WhyForm, profile: &WhyProfile, slices: &PlanSlices) -> Vec<String> {
    let single = build_single_value_columns(form);
    let mut excluded: Vec<&str> = single.iter().map(String::as_str).collect();
    // Cột của độ đo share: chia theo chính cột đó thì nhóm có giá trị đang đếm giải thích trọn chênh lệch, không nói gì.
    if let Some(measure) = form.measure.as_ref().filter(|m| m.agg == FormAgg::Share) {
        excluded.push(&measure.column);
    }
    if let PlanSlices::Groups { column, .. } = slices {
        excluded.push(column);
    }
    build_dimension_candidates(profile.columns, &excluded)
}

/// Cột chiều ứng viên của bảng chính theo luật trên, trừ các cột trong excluded (cột làm lát cắt, cột đã lọc bằng một
/// giá trị). Phân rã tự động của phần giải thích (service/insight/) dùng cùng luật.
pub(crate) fn build_dimension_candidates(
    columns: &[ProfileColumn],
    excluded: &[&str],
) -> Vec<String> {
    columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .filter(|c| {
            c.distinct_count
                .is_some_and(|n| (2..=DIMENSION_MAX_VALUES).contains(&n))
        })
        .filter(|c| !excluded.contains(&c.name.as_str()))
        .take(DIMENSION_CANDIDATES_MAX)
        .map(|c| c.name.clone())
        .collect()
}

/// Lượng của #40: trong các cột vai trò key có lặp lại (ít giá trị khác nhau hơn số dòng, như Order Id khi một đơn có
/// nhiều dòng), cột có nhiều giá trị khác nhau nhất. Đó là đơn vị nhỏ nhất mà nhiều dòng cùng thuộc về, như đơn hàng
/// với các dòng hàng trong đơn; mã khách hay mã danh mục cũng lặp nhưng ít giá trị hơn. Cột key không lặp thì đếm nó
/// chỉ là đếm dòng, không có nghĩa kinh doanh, nên bỏ. Hồ sơ ngữ nghĩa đổi mọi cột mã sang key
/// (backend/prompts/semantic_profile.txt), nên thường có nhiều cột key. metrics chưa có trường đánh dấu chỉ số nào là
/// lượng, nên chưa lấy lượng từ metrics. Phần giải thích của câu thường (insight/plan.rs) dùng cùng hàm này.
pub(crate) fn build_volume(profile: &WhyProfile) -> Option<String> {
    let rows = profile.row_count?;
    profile
        .columns
        .iter()
        .filter(|c| c.role == ProfileRole::Key.as_str())
        .filter_map(|c| Some((c, c.distinct_count?)))
        .filter(|(_, distinct)| *distinct < rows)
        .max_by_key(|(_, distinct)| *distinct)
        .map(|(c, _)| c.name.clone())
}

// Bí danh của hồ sơ ngữ nghĩa là tiếng Việt (luật của prompt lập hồ sơ); bí danh đầu là tên mô hình thấy hợp nhất.
// Bí danh trùng tên cột (khác hoa thường) thì không thêm gì cho người đọc, nên bỏ.
pub(crate) fn build_labels(profile: &WhyProfile) -> HashMap<String, String> {
    profile
        .columns
        .iter()
        .filter_map(|c| {
            let alias = c.aliases.iter().map(|a| a.trim()).find(|a| !a.is_empty())?;
            (alias.to_lowercase() != c.name.to_lowercase())
                .then(|| (c.name.clone(), alias.to_string()))
        })
        .collect()
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

pub(super) fn wrong_role(column: &ProfileColumn, field: &str, wanted: &str) -> FieldError {
    // Cột kỳ học dạng chữ ("2013J", "2014J") có vai trò chiều: mô hình hay đặt nó vào "time", sửa lần hai thì bỏ "time"
    // mà quên "groups", form sai hai lần và câu rơi về câu thường. Chỉ luôn đường sửa đúng trong lời báo lỗi.
    let hint = if field.starts_with("time") && column.role == "dimension" {
        format!(
            "; to compare two values of \"{}\", leave \"time\" out and set \"groups\" to this column with the two \
             values: the one the question asks about as \"compare\", the other as \"base\"",
            column.name
        )
    } else {
        String::new()
    };
    FieldError::new(
        field,
        format!(
            "\"{}\" has role {}; this field needs a column of role {wanted}{hint}",
            column.name, column.role
        ),
    )
}

fn non_empty(text: &Option<String>) -> Option<&str> {
    text.as_deref().map(str::trim).filter(|t| !t.is_empty())
}
