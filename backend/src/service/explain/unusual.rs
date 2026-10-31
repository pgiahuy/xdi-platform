// SPDX-License-Identifier: Apache-2.0
/* service/explain/unusual.rs - câu "tại sao" hỏi kỳ hay nhóm bất thường (#244): "Tháng nào giảm bất thường năm
2017?", "Nhóm nào đang kéo doanh thu xuống?". Mô hình chỉ ghi find trong form (explain/form.rs); code tìm, rồi phân rã
như câu so hai kỳ thường. explain/check.rs gọi check_find khi kiểm form; explain/run.rs gọi apply_find sau khi kiểm,
trước khi chạy bảng tổng. Mọi câu SQL ở đây do explain/sql.rs dựng và đi qua guard như các câu khác của phân rã.

- Kỳ (kind period): độ đo theo từng kỳ trên mọi dòng của bộ lọc (build_series_sql), chọn kỳ đổi nhiều nhất so với kỳ
  liền trước nó trong chuỗi, như phần giải thích câu thường chọn kỳ đổi mạnh nhất (#227). Câu nói hướng ("giảm", "tăng",
  explain/premise.rs) thì chỉ xét bước đổi theo hướng đó.
- Nhóm (kind group): trên hai kỳ của form, phần chênh lệch của từng nhóm của cột (build_dimension_sql), chọn nhóm góp
  nhiều nhất theo hướng câu hỏi; rồi lọc đúng nhóm đó và phân rã chênh lệch hai kỳ của nhóm theo các cột còn lại. Với
  tỷ lệ R = N / D, phần của nhóm i là N1_i / D1 - N0_i / D0: cộng mọi nhóm ra đúng R1 - R0.
Test ở explain/frames_tests.rs (kiểm form, chọn bước) và explain/frames_run_tests.rs (trên DuckDB). */

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::engine::store::ProfileRole;
use crate::error::AppError;
use crate::service::ask::contract::{CellValue, ResultTable};
use crate::service::explain::breakdown::{run_code_query, CodeQuery, QueryEnv};
use crate::service::explain::build_value_text;
use crate::service::explain::check::{find_column, wrong_role, PlanSlices, WhyPlan, WhyProfile};
use crate::service::explain::form::{FieldError, FindKind, WhyForm};
use crate::service::explain::period::Period;
use crate::service::explain::premise::AskedDirection;
use crate::service::explain::sql::{build_dimension_sql, build_equals, build_series_sql};

/// Kiểm find của form: tìm kỳ hay nhóm đều cần so hai kỳ (có time, không groups); tìm nhóm cần cột chiều.
pub(super) fn check_find(form: &WhyForm, profile: &WhyProfile, errors: &mut Vec<FieldError>) {
    let Some(find) = &form.find else {
        return;
    };
    if form.time.is_none() || form.groups.is_some() {
        errors.push(FieldError::new(
            "find",
            "\"find\" works with two periods: set \"time\" with a grain and leave \"groups\" out",
        ));
    }
    if find.kind != FindKind::Group {
        return;
    }
    let Some(name) = find.column.as_deref().filter(|c| !c.trim().is_empty()) else {
        errors.push(FieldError::new(
            "find.column",
            "set \"column\" to the column of role dimension whose groups are searched",
        ));
        return;
    };
    if let Some(column) = find_column(profile, name, "find.column", errors) {
        if column.role != ProfileRole::Dimension.as_str() {
            errors.push(wrong_role(column, "find.column", "dimension"));
        }
    }
}

/// Kết quả của bước tìm: chữ cho tab Các bước, hay lý do không tìm được (lời cho cảnh báo).
pub enum Found {
    /// Plan đã đổi theo kỳ hay nhóm tìm được.
    Applied(String),
    NotFound(String),
}

/// Tìm kỳ hay nhóm theo find của form rồi sửa plan: kỳ thì thay hai lát cắt, nhóm thì thêm bộ lọc nhóm đó.
pub async fn apply_find(
    env: &QueryEnv<'_>,
    form: &WhyForm,
    plan: &mut WhyPlan,
    direction: Option<AskedDirection>,
) -> Result<Option<Found>, AppError> {
    let Some(find) = &form.find else {
        return Ok(None);
    };
    let found = match find.kind {
        FindKind::Period => apply_found_period(env, plan, direction).await?,
        FindKind::Group => {
            let column = find.column.clone().unwrap_or_default();
            apply_found_group(env, plan, &column, direction).await?
        }
    };
    Ok(Some(found))
}

async fn apply_found_period(
    env: &QueryEnv<'_>,
    plan: &mut WhyPlan,
    direction: Option<AskedDirection>,
) -> Result<Found, AppError> {
    let PlanSlices::Periods { time, .. } = &plan.slices else {
        return Ok(Found::NotFound(
            "Câu hỏi tìm kỳ cần so theo thời gian".to_string(),
        ));
    };
    let time = time.clone();
    let Some(result) = run_rows(env, &build_series_sql(plan, &time)).await? else {
        return Ok(Found::NotFound(
            "Không tính được số theo từng kỳ".to_string(),
        ));
    };
    let points: Vec<(Period, Decimal)> = result
        .rows
        .iter()
        .filter_map(|row| {
            let period = read_period(time.grain, row.first()?.as_ref()?)?;
            Some((period, read_number(row.get(1)?.as_ref())?))
        })
        .collect();
    let values: Vec<Decimal> = points.iter().map(|(_, v)| *v).collect();
    let Some(i) = pick_step(&values, direction) else {
        return Ok(Found::NotFound(
            "Không có hai kỳ liền nhau nào đổi theo hướng câu hỏi".to_string(),
        ));
    };
    let (base, compare) = (points[i - 1].0, points[i].0);
    plan.slices = PlanSlices::Periods {
        time,
        base,
        compare,
        base_by_default: false,
    };
    Ok(Found::Applied(format!(
        "Kỳ đổi nhiều nhất so với kỳ liền trước trong {} kỳ: {} so với {}",
        points.len(),
        compare.label(),
        base.label()
    )))
}

async fn apply_found_group(
    env: &QueryEnv<'_>,
    plan: &mut WhyPlan,
    column: &str,
    direction: Option<AskedDirection>,
) -> Result<Found, AppError> {
    let Some(result) = run_rows(env, &build_dimension_sql(plan, column)).await? else {
        return Ok(Found::NotFound(format!("Không tính được số theo {column}")));
    };
    let groups: Vec<(Option<CellValue>, Vec<Decimal>)> = result
        .rows
        .iter()
        .filter_map(|row| {
            let numbers: Option<Vec<Decimal>> = row
                .iter()
                .skip(1)
                .map(|c| read_number(c.as_ref()))
                .collect();
            Some((row.first()?.clone(), numbers?))
        })
        .collect();
    let parts: Vec<Decimal> = build_group_parts(&groups);
    let Some(i) = pick_largest(&parts, direction) else {
        return Ok(Found::NotFound(format!(
            "Không nhóm nào của {column} đổi theo hướng câu hỏi"
        )));
    };
    // Nhóm trống không lọc được bằng "=": báo không tìm được thay vì lọc sai nhóm.
    let Some(value) = groups[i].0.as_ref().map(build_value_text) else {
        return Ok(Found::NotFound(format!(
            "Nhóm đổi nhiều nhất của {column} là nhóm trống"
        )));
    };
    plan.filters.push(build_equals(column, &value));
    plan.dimensions.retain(|d| d != column);
    Ok(Found::Applied(format!(
        "Nhóm góp nhiều nhất vào chênh lệch theo {column}: {value}"
    )))
}

/// Phần chênh lệch của từng nhóm: compare - base với độ đo cộng được (hai số mỗi nhóm), N1/D1 - N0/D0 với tỷ lệ (bốn số:
/// tử base, tử compare, mẫu base, mẫu compare như build_sliced_items). Mẫu cả lát cắt là tổng mẫu các nhóm.
pub(super) fn build_group_parts(groups: &[(Option<CellValue>, Vec<Decimal>)]) -> Vec<Decimal> {
    let total = |k: usize| -> Decimal { groups.iter().filter_map(|(_, n)| n.get(k)).sum() };
    let (d0, d1) = (total(2), total(3));
    groups
        .iter()
        .map(|(_, n)| match n.as_slice() {
            [b, c] => c - b,
            [n0, n1, _, _] => {
                let share = |n: &Decimal, d: Decimal| n.checked_div(d).unwrap_or_default();
                share(n1, d1) - share(n0, d0)
            }
            _ => Decimal::ZERO,
        })
        .collect()
}

/// Vị trí i (từ 1) của bước values[i] - values[i - 1] chọn được: theo hướng câu hỏi thì bước âm nhỏ nhất hay dương lớn
/// nhất, không hướng thì trị tuyệt đối lớn nhất; bằng nhau lấy bước sớm hơn. None khi không bước nào hợp.
pub(super) fn pick_step(values: &[Decimal], direction: Option<AskedDirection>) -> Option<usize> {
    let steps: Vec<Decimal> = values.windows(2).map(|w| w[1] - w[0]).collect();
    pick_largest(&steps, direction).map(|i| i + 1)
}

/// Vị trí của số lớn nhất theo hướng: Down là số âm nhỏ nhất, Up là số dương lớn nhất, None là trị tuyệt đối lớn nhất.
fn pick_largest(values: &[Decimal], direction: Option<AskedDirection>) -> Option<usize> {
    let score = |v: Decimal| match direction {
        Some(AskedDirection::Down) => -v,
        Some(AskedDirection::Up) => v,
        None => v.abs(),
    };
    let mut best: Option<usize> = None;
    for (i, value) in values.iter().enumerate() {
        if score(*value) <= Decimal::ZERO {
            continue;
        }
        if best.is_none_or(|b| score(*value) > score(values[b])) {
            best = Some(i);
        }
    }
    best
}

/// Kết quả của một câu qua guard; None (kèm log) khi bị từ chối, lỗi, hay chạm trần dòng.
async fn run_rows(env: &QueryEnv<'_>, sql: &str) -> Result<Option<ResultTable>, AppError> {
    match run_code_query(env, sql, None).await? {
        CodeQuery::Ran { result, .. } if !result.limit_reached && !result.truncated => {
            Ok(Some(result))
        }
        CodeQuery::Ran { .. } => {
            tracing::warn!("unusual search query reached the row cap");
            Ok(None)
        }
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(%reason, "unusual search query failed");
            Ok(None)
        }
    }
}

// Ô số: DECIMAL, BIGINT tới dạng chữ (như read_decimal của explain/breakdown.rs), tỷ lệ ROUND trên số thực tới dạng số.
fn read_number(cell: Option<&CellValue>) -> Option<Decimal> {
    match cell {
        None => Some(Decimal::ZERO),
        Some(CellValue::String(text)) => text.parse().ok(),
        Some(CellValue::Number(number)) => Decimal::from_f64(*number),
        Some(_) => None,
    }
}

// Kỳ in dạng "2017-11-01" (build_series_sql).
fn read_period(grain: crate::service::explain::period::Grain, cell: &CellValue) -> Option<Period> {
    let CellValue::String(text) = cell else {
        return None;
    };
    let date = chrono::NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()?;
    Some(Period::containing(grain, date))
}
