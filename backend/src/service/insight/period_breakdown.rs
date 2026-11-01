// SPDX-License-Identifier: Apache-2.0
/* service/insight/period_breakdown.rs - phần so hai kỳ của phần giải thích (#227): một con số so với kỳ liền trước, và
kỳ đổi mạnh nhất của chuỗi thời gian. Đọc câu của mô hình (insight/plan.rs) và kỳ (insight/periods.rs), dựng WhyPlan
hai kỳ, chạy tổng hai kỳ rồi phân rã bằng đường của câu "tại sao" (explain/breakdown.rs). Tổng do code tính phải khớp ô
của bảng kết quả, lệch thì bỏ phân rã và giữ phần giải thích từ bảng. insight/mod.rs gọi; test ở insight/tests.rs. */

use rust_decimal::Decimal;

use crate::error::AppError;
use crate::service::ask::contract::Explanation;
use crate::service::explain::breakdown::{
    build_breakdown, read_slice_totals, run_code_query, Breakdown, CodeQuery,
};
use crate::service::explain::check::WhyPlan;
use crate::service::explain::period::Period;
use crate::service::explain::ratio::RATE_SCALE;
use crate::service::explain::run::build_premise_values;
use crate::service::explain::significance::{build_change_weight, ChangeWeight};
use crate::service::explain::source::build_tables_used;
use crate::service::explain::sql::{build_slices_sql, build_why_sql};
use crate::service::explain::{build_template_summary, summary::apply_summary_parts};
use crate::service::insight::facts::{
    build_series_parts, build_single_parts, find_largest_step, InsightParts, MeasureText, Point,
};
use crate::service::insight::periods::{read_cell_period, read_filter_period, read_series_time};
use crate::service::insight::shape::build_cell_decimal;
use crate::service::insight::{build_points, read_query, Found, InsightInput};

pub(super) async fn build_single(
    input: &InsightInput<'_>,
    measure: usize,
    text: MeasureText<'_>,
    found: &mut Found,
) -> Result<InsightParts, AppError> {
    let label = text.label;
    let value = build_cell_decimal(input.result.rows[0][measure].as_ref()).unwrap_or_default();
    let alone = |found: &mut Found, reason: &str| {
        found.detail = Some(reason.to_string());
        let point = Point {
            label: String::new(),
            value,
        };
        build_single_parts(text, &point, None, None)
    };
    let Some(query) = read_query(input, measure, label) else {
        return Ok(alone(found, "Không đọc được câu SQL để so với kỳ trước"));
    };
    let finder = |name: &str| query.find_column(name, input.tables);
    let Some((time, period, used)) = read_filter_period(&query.filters, &finder) else {
        return Ok(alone(found, "Câu hỏi không nêu một kỳ để so với kỳ trước"));
    };
    let filters = query
        .filters
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, f)| f.clone())
        .collect();
    let plan = query.build_period_plan(time, period.previous(), period, filters);
    let ran = match run_period(input, &plan, None, Some(value)).await? {
        Ok(ran) => ran,
        Err(reason) => return Ok(alone(found, &reason)),
    };
    let current = Point {
        label: lower_label(period),
        value,
    };
    let base = Point {
        label: lower_label(period.previous()),
        value: ran.base,
    };
    let notable = build_change_weight(ran.base, value, Some(ran.rows)) == ChangeWeight::Notable;
    let explanation = if notable { ran.explanation } else { None };
    let parts = build_single_parts(
        text,
        &current,
        Some((&base, Some(ran.rows))),
        explanation.as_ref(),
    );
    keep_explanation(found, &plan, explanation);
    Ok(parts)
}

pub(super) async fn build_series(
    input: &InsightInput<'_>,
    measure: usize,
    key: Option<usize>,
    text: MeasureText<'_>,
    found: &mut Found,
) -> Result<InsightParts, AppError> {
    let label = text.label;
    let result = input.result;
    let complete = !result.truncated && !result.limit_reached;
    let mut points = build_points(result, measure, key);
    let query = read_query(input, measure, label);
    let time = query.as_ref().and_then(|q| {
        let expr = q.outputs.get(key?)?.as_ref()?;
        read_series_time(expr, &|name: &str| q.find_column(name, input.tables))
    });
    // Kỳ của từng ô khóa: đọc được hết thì nhãn là "Tháng 11/2017" và sắp theo thời gian, không thì giữ thứ tự của bảng.
    let periods: Option<Vec<Period>> = time.as_ref().and_then(|t| {
        result
            .rows
            .iter()
            .filter(|row| build_cell_decimal(row[measure].as_ref()).is_some())
            .map(|row| read_cell_period(t.grain, row[key?].as_ref()))
            .collect()
    });
    let mut dated: Option<Vec<(Period, Point)>> = periods.map(|periods| {
        let mut pairs: Vec<(Period, Point)> = periods.into_iter().zip(points.clone()).collect();
        pairs.sort_by_key(|(period, _)| period.range());
        pairs
    });
    if let Some(pairs) = dated.as_mut() {
        for (period, point) in pairs.iter_mut() {
            point.label = period.label();
        }
        points = pairs.iter().map(|(_, p)| p.clone()).collect();
    }
    let (Some(query), Some(time), Some(pairs), Some(index)) =
        (query, time, dated, find_largest_step(&points))
    else {
        found.detail = Some("Không đọc được kỳ của chuỗi thời gian để phân rã".to_string());
        return Ok(build_series_parts(text, &points, None, None, complete));
    };
    let (before, after) = (pairs[index - 1].0, pairs[index].0);
    let expected = (pairs[index - 1].1.value, pairs[index].1.value);
    let plan = query.build_period_plan(time, before, after, query.filters.clone());
    let ran = match run_period(input, &plan, Some(expected.0), Some(expected.1)).await? {
        Ok(ran) => ran,
        Err(reason) => {
            found.detail = Some(reason);
            return Ok(build_series_parts(text, &points, None, None, complete));
        }
    };
    let parts = build_series_parts(
        text,
        &points,
        ran.explanation.as_ref(),
        Some(ran.rows),
        complete,
    );
    let explanation = if parts.notable { ran.explanation } else { None };
    keep_explanation(found, &plan, explanation);
    Ok(parts)
}

// Nhãn kỳ giữa câu: "tháng 11/2017".
fn lower_label(period: Period) -> String {
    crate::service::explain::lower_first(&period.label())
}

/// Ghi phân rã và các bảng nó đã đọc.
fn keep_explanation(found: &mut Found, plan: &WhyPlan, explanation: Option<Explanation>) {
    if let Some(explanation) = &explanation {
        let columns: Vec<&str> = explanation
            .dimensions
            .iter()
            .map(|d| d.column.as_str())
            .collect();
        found.tables_read = std::iter::once(plan.table.clone())
            .chain(build_tables_used(&plan.source, &columns))
            .collect();
        found.detail = Some(format!(
            "Phân rã {} so với {} theo {} chiều",
            explanation.compare.label,
            explanation.base.label,
            explanation.dimensions.len()
        ));
    }
    found.explanation = explanation;
}

/// Kết quả chạy hai kỳ: giá trị kỳ gốc (đã so với bảng), số dòng hai kỳ, phân rã khi có.
struct PeriodRun {
    base: Decimal,
    rows: (i64, i64),
    explanation: Option<Explanation>,
}

/// Kết quả của run_period; Err là lý do không dùng được hai kỳ do code tính (ghi vào bước explain).
type PeriodResult = Result<PeriodRun, String>;

/// Tổng hai kỳ và phân rã. expected là ô của bảng kết quả cho kỳ gốc và kỳ so sánh khi bảng có: tổng do code tính phải
/// khớp, không thì code đã hiểu câu hỏi khác câu đã chạy và phân rã không được dùng.
async fn run_period(
    input: &InsightInput<'_>,
    plan: &WhyPlan,
    base_cell: Option<Decimal>,
    compare_cell: Option<Decimal>,
) -> Result<PeriodResult, AppError> {
    let totals = match run_code_query(&input.env, &build_slices_sql(plan), None).await? {
        CodeQuery::Ran { shown, result, .. } => read_slice_totals(shown, &result),
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(%reason, "insight slice totals query failed");
            None
        }
    };
    let Some(totals) = totals else {
        return Ok(Err("Không tính được tổng hai kỳ".to_string()));
    };
    let Some((base, compare)) = build_premise_values(&totals) else {
        return Ok(Err("Kỳ trước không có giá trị để so".to_string()));
    };
    let unit = if totals.denominators.is_some() {
        Decimal::new(1, RATE_SCALE)
    } else {
        Decimal::new(1, 2)
    };
    let matches = |cell: Option<Decimal>, value: Decimal| {
        cell.is_none_or(|cell| (cell - value).abs() <= (cell.abs() * Decimal::new(1, 4)).max(unit))
    };
    if !matches(base_cell, base) || !matches(compare_cell, compare) {
        return Ok(Err(
            "Tổng hai kỳ do code tính không khớp bảng kết quả nên không phân rã".to_string(),
        ));
    }
    let rows = (
        i64::try_from(totals.base_rows).unwrap_or(0),
        i64::try_from(totals.compare_rows).unwrap_or(0),
    );
    if rows.0 == 0 || rows.1 == 0 {
        return Ok(Err("Một trong hai kỳ không có dòng dữ liệu nào".to_string()));
    }
    let sql = build_why_sql(plan);
    let explanation = match build_breakdown(
        &input.env,
        plan,
        &totals,
        (sql.base_filter, sql.compare_filter),
    )
    .await?
    {
        Breakdown::Ready(explanation) => {
            let mut explanation = *explanation;
            apply_summary_parts(&mut explanation);
            explanation.summary = build_template_summary(&explanation);
            Some(explanation)
        }
        Breakdown::Unavailable(reason) => {
            tracing::info!(%reason, "insight breakdown unavailable");
            None
        }
    };
    Ok(Ok(PeriodRun {
        base,
        rows,
        explanation,
    }))
}
