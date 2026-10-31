// SPDX-License-Identifier: Apache-2.0
/* service/explain/breakdown.rs - bước explain của câu "tại sao": chạy các câu SQL do explain/sql.rs dựng (tổng hai lát
cắt, từng chiều), đổi kết quả sang Decimal, gọi phần toán của explain/mod.rs (#39 xếp chiều, #40 tách lượng và giá) và dựng
Explanation của hợp đồng. explain/run.rs gọi sau khi bảng tổng đã chạy, rồi nhờ ask/narrate.rs viết lời.

Mọi câu đi qua guard với đúng bảng và bộ lọc dòng theo vai dữ liệu của người hỏi (AllowedTable::row_filters), như câu
của mô hình: người chỉ được thấy Europe thì phân rã cũng chỉ tính trên dòng Europe, và tổng các chiều vẫn khớp tổng hai
lát cắt vì mọi câu cùng bị lọc như nhau.

Mọi cột ứng viên đều chạy câu từng chiều (run_candidates); explain/dimension_pick.rs bỏ chiều trùng và chiều không so
được gì, rồi phần chấm điểm xếp các chiều còn lại, chiều giải thích tốt nhất trước; không chiều nào bị bỏ vì xếp sau.
Độ đo tỷ lệ đi nhánh riêng ở explain/ratio_breakdown.rs, dùng chung run_candidates và build_slice_labels ở đây.

Một chiều lỗi (guard từ chối, DuckDB báo lỗi, chạm trần dòng, các nhóm không cộng ra đúng tổng) thì bỏ chiều đó, ghi log,
các chiều khác vẫn dùng; không còn chiều nào thì không có phân rã (cảnh báo explanation_unavailable). */

use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;
use tokio::sync::Semaphore;

use crate::engine::guard::{build_guarded_sql, AllowedTable, GuardError};
use crate::engine::query::QueryError;
use crate::error::AppError;
use crate::service::ask::contract::StepName;
use crate::service::ask::contract::{
    AskStep, CellValue, Comparison, DimensionBreakdown, Explanation, ExplanationSlice, MeasureKind,
    ResultTable, ValueKind, VolumePrice,
};
use crate::service::ask::repair::{build_safe_error, build_user_error, mentions_data_files};
use crate::service::ask::result::build_result_table;
use crate::service::ask::run::{build_duration_text, build_step, run_query_in_turn};
use crate::service::explain::annotate::{
    annotate_dimensions, build_percent_change, build_percent_text, build_value_presences, Totals,
};
use crate::service::explain::check::{PlanSlices, WhyPlan};
use crate::service::explain::dimension_pick::{pick_dimensions, Candidate, Presence};
use crate::service::explain::measure::Additive;
use crate::service::explain::ratio_breakdown::build_ratio_breakdown;
use crate::service::explain::sql::{
    build_dimension_sql, BASE_DENOMINATOR, BASE_ROWS, BASE_VALUE, BASE_VOLUME, COMPARE_DENOMINATOR,
    COMPARE_ROWS, COMPARE_VALUE, COMPARE_VOLUME,
};
use crate::service::explain::{
    apply_dimension_order, build_decimal_text, build_dimension_score, build_entity_name,
    build_group_contributions, build_ranking_mode, build_volume_price, GroupValues,
    RankedDimension, PRICE_SCALE,
};

/// Bảng, trần dòng và giới hạn chạy, chung cho mọi câu của một câu hỏi.
pub struct QueryEnv<'a> {
    pub tables: &'a [AllowedTable],
    pub max_rows: u64,
    pub timeout: Duration,
    pub jobs: &'a Arc<Semaphore>,
    /// Lời khuyên nối sau câu báo quá giờ hay quá bộ nhớ. Câu hỏi khuyên hỏi hẹp hơn (ASK_STOP_HINT); trang tổng quan
    /// không có câu hỏi nào nên dùng lời riêng.
    pub stop_hint: &'a str,
}

/// Lời khuyên khi câu SQL của một câu hỏi bị dừng vì quá giờ hay quá bộ nhớ.
pub const ASK_STOP_HINT: &str = "Thử hỏi hẹp hơn, ví dụ thêm bộ lọc";

/// Kết quả một câu SQL do code dựng.
pub enum CodeQuery {
    Ran {
        /// Bản đã qua guard, trên tên logic: hiện cho người dùng.
        shown: String,
        result: ResultTable,
        /// Điều kiện phân quyền guard đã thêm (GuardedSql::row_filters), để thẻ "Đã hiểu là" gắn nhãn phân quyền.
        row_filters: Vec<String>,
    },
    /// Guard từ chối; lý do tiếng Việt.
    Refused(String),
    /// DuckDB lỗi, quá giờ hay quá bộ nhớ; lời tiếng Việt đã bỏ đường dẫn file.
    Failed(String),
}

/// Guard rồi chạy một câu. steps có thì ghi bước guard và run (bảng tổng); câu của bước explain thì không ghi riêng.
/// Err cho lỗi máy chủ (không mở được DuckDB, file Parquet hỏng), như run_with_repair.
pub async fn run_code_query(
    env: &QueryEnv<'_>,
    sql: &str,
    mut steps: Option<&mut Vec<AskStep>>,
) -> Result<CodeQuery, AppError> {
    let started = std::time::Instant::now();
    let guarded = match build_guarded_sql(sql, env.tables, env.max_rows) {
        Ok(guarded) => guarded,
        Err(GuardError::Internal(e)) => return Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => {
            if let Some(steps) = steps.as_deref_mut() {
                steps.push(build_step(StepName::Guard, started, false, Some(&reason)));
            }
            return Ok(CodeQuery::Refused(reason));
        }
    };
    if let Some(steps) = steps.as_deref_mut() {
        let note = guarded.limit_note.as_deref();
        steps.push(build_step(StepName::Guard, started, true, note));
    }
    let started = std::time::Instant::now();
    let outcome = match run_query_in_turn(env.jobs, guarded.runnable, env.timeout).await {
        Ok(output) => CodeQuery::Ran {
            shown: guarded.shown,
            result: build_result_table(output, env.max_rows),
            row_filters: guarded.row_filters,
        },
        Err(QueryError::Sql(raw)) if mentions_data_files(&raw, env.tables) => {
            return Err(AppError::Internal(format!("read dataset files: {raw}")));
        }
        Err(QueryError::Sql(raw)) => {
            CodeQuery::Failed(build_user_error(&build_safe_error(&raw, env.tables)))
        }
        Err(QueryError::Timeout) => CodeQuery::Failed(format!(
            "Câu SQL chạy quá {} nên đã bị dừng. {}",
            build_duration_text(env.timeout),
            env.stop_hint
        )),
        Err(QueryError::OutOfMemory) => CodeQuery::Failed(format!(
            "Câu SQL cần nhiều bộ nhớ hơn mức cho phép nên đã bị dừng. {}",
            env.stop_hint
        )),
        Err(QueryError::Internal(e)) => return Err(e),
    };
    if let Some(steps) = steps {
        let detail = match &outcome {
            CodeQuery::Failed(message) => Some(message.as_str()),
            _ => None,
        };
        let ok = matches!(outcome, CodeQuery::Ran { .. });
        steps.push(build_step(StepName::Run, started, ok, detail));
    }
    Ok(outcome)
}

/// Tổng của hai lát cắt, đọc từ câu của build_slices_sql.
pub struct SliceTotals {
    /// Câu đã chạy, sau guard: cũng là VolumePrice.sql.
    pub sql: String,
    pub base: Decimal,
    pub compare: Decimal,
    pub base_rows: Decimal,
    pub compare_rows: Decimal,
    /// V0, V1 khi plan có lượng.
    pub volume: Option<(Decimal, Decimal)>,
    /// Mẫu số của hai lát cắt khi độ đo là tỷ lệ; base, compare ở trên khi đó là tử số.
    pub denominators: Option<(Decimal, Decimal)>,
}

/// None khi kết quả không có đúng một dòng hay một ô không đọc được thành số (số quá 28 chữ số của Decimal).
pub fn read_slice_totals(sql: String, result: &ResultTable) -> Option<SliceTotals> {
    let row = match result.rows.as_slice() {
        [row] => row,
        _ => return None,
    };
    let cell = |name: &str| -> Option<Decimal> {
        let i = result.columns.iter().position(|c| c.name == name)?;
        read_decimal(row.get(i)?.as_ref())
    };
    let volume = match (cell(BASE_VOLUME), cell(COMPARE_VOLUME)) {
        (Some(base), Some(compare)) => Some((base, compare)),
        _ => None,
    };
    let denominators = match (cell(BASE_DENOMINATOR), cell(COMPARE_DENOMINATOR)) {
        (Some(base), Some(compare)) => Some((base, compare)),
        _ => None,
    };
    Some(SliceTotals {
        denominators,
        base: cell(BASE_VALUE)?,
        compare: cell(COMPARE_VALUE)?,
        base_rows: cell(BASE_ROWS)?,
        compare_rows: cell(COMPARE_ROWS)?,
        volume,
        sql,
    })
}

/// Ô của cột tổng hợp: DECIMAL, HUGEINT, BIGINT đều tới đây dạng chuỗi chữ số (ValueKind::Decimal), đổi sang Decimal
/// không qua f64. Ô trống là 0, như COALESCE trong SQL.
pub(super) fn read_decimal(cell: Option<&CellValue>) -> Option<Decimal> {
    match cell {
        None => Some(Decimal::ZERO),
        Some(CellValue::String(text)) => text.parse().ok(),
        Some(_) => None,
    }
}

/// Kết quả bước explain: phân rã, hoặc lý do không có (lời cho cảnh báo explanation_unavailable).
// Box: Explanation lớn hơn chuỗi lý do nhiều lần, enum không bọc thì biến thể nhỏ cũng chiếm chỗ của biến thể lớn.
pub enum Breakdown {
    Ready(Box<Explanation>),
    Unavailable(String),
}

/// Phân rã theo từng chiều và tách lượng, giá. filters là điều kiện hai lát cắt (base, compare) của build_why_sql.
/// summary để trống, bước narrate điền sau.
pub async fn build_breakdown(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    totals: &SliceTotals,
    filters: (String, String),
) -> Result<Breakdown, AppError> {
    if let Some(denominators) = totals.denominators {
        return build_ratio_breakdown(env, plan, totals, denominators, filters).await;
    }
    let Some(change) = totals.compare.checked_sub(totals.base) else {
        return Ok(Breakdown::Unavailable(
            "Số quá lớn để tính chênh lệch".to_string(),
        ));
    };
    if change.is_zero() {
        return Ok(Breakdown::Unavailable(format!(
            "{} như nhau ở hai lát cắt nên không có chênh lệch để phân rã",
            plan.measure.label
        )));
    }
    let candidates = run_candidates(env, plan, 2).await?;
    let presence = Presence {
        base: 0,
        compare: 1,
    };
    let picked = pick_dimensions(env, plan, candidates, presence).await?;
    let dimensions: Vec<_> = picked
        .kept
        .into_iter()
        .filter_map(|candidate| build_dimension(plan, candidate))
        .collect();
    let groups: Vec<Vec<GroupValues>> = dimensions.iter().map(|(_, g)| g.clone()).collect();
    let mode = build_ranking_mode(&groups, totals.base, totals.compare);
    let mut ranked = Vec::new();
    for (breakdown, groups) in dimensions {
        match build_dimension_score(&groups, totals.base, totals.compare, mode) {
            Ok(score) => ranked.push(RankedDimension { breakdown, score }),
            Err(error) => {
                tracing::warn!(column = %breakdown.column, ?error, "dimension dropped from the breakdown");
            }
        }
    }
    if ranked.is_empty() {
        return Ok(Breakdown::Unavailable(
            "Không có cột chiều nào phân rã được chênh lệch".to_string(),
        ));
    }
    apply_dimension_order(&mut ranked, mode);
    let dimensions: Vec<DimensionBreakdown> = ranked.into_iter().map(|d| d.breakdown).collect();
    let presences: Vec<_> = dimensions.iter().map(build_value_presences).collect();
    let slices = Totals {
        base: totals.base,
        compare: totals.compare,
        change,
    };
    let mut notes = annotate_dimensions(&dimensions, &presences, slices);
    // Chiều dimension_pick đã bỏ không còn trong dimensions để annotate thấy; thêm vào để lời tóm tắt vẫn nói vì sao bỏ.
    notes.dropped.extend(picked.dropped);
    let (base_label, compare_label) = build_slice_labels(plan);
    Ok(Breakdown::Ready(Box::new(Explanation {
        measure: plan.measure.label.clone(),
        measure_kind: MeasureKind::Total,
        comparison: build_comparison(plan),
        counts_rows: matches!(plan.measure.additive, Additive::Count { .. }),
        slice_column: build_slice_column(plan),
        slice_column_label: build_slice_column(plan).and_then(|c| plan.labels.get(&c).cloned()),
        base: ExplanationSlice {
            label: base_label,
            filter: filters.0,
            value: build_decimal_text(totals.base, totals.base.scale()),
            rows: build_decimal_text(totals.base_rows, 0),
        },
        compare: ExplanationSlice {
            label: compare_label,
            filter: filters.1,
            value: build_decimal_text(totals.compare, totals.compare.scale()),
            rows: build_decimal_text(totals.compare_rows, 0),
        },
        change: build_decimal_text(change, totals.base.scale().max(totals.compare.scale())),
        percent_change: build_percent_change(slices).map(build_percent_text),
        dimensions,
        volume_price: build_volume(plan, totals),
        summary: String::new(),
        summary_title: String::new(),
        summary_points: Vec::new(),
        summary_by_model: false,
        summary_warnings: Vec::new(),
        summary_verdict: None,
        notes,
    })))
}

/// Câu từng chiều của mọi cột ứng viên (WhyPlan::dimensions), lần lượt; chiều lỗi thì bỏ (run_dimension_rows ghi log).
/// numbers: số ô số sau giá trị nhóm, 2 với độ đo cộng được, 4 với độ đo tỷ lệ.
pub(super) async fn run_candidates(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    numbers: usize,
) -> Result<Vec<Candidate>, AppError> {
    let mut candidates = Vec::new();
    for column in &plan.dimensions {
        if let Some(rows) = run_dimension_rows(env, plan, column, numbers).await? {
            candidates.push(Candidate {
                column: column.clone(),
                rows,
            });
        }
    }
    Ok(candidates)
}

/// Một chiều của độ đo cộng được: đọc các nhóm, tính đóng góp. None (kèm log) khi chiều phải bỏ.
fn build_dimension(
    plan: &WhyPlan,
    candidate: Candidate,
) -> Option<(DimensionBreakdown, Vec<GroupValues>)> {
    let Candidate { column, rows } = candidate;
    let groups: Vec<GroupValues> = rows
        .groups
        .into_iter()
        .map(|(value, numbers)| GroupValues {
            value,
            base_value: numbers[0],
            compare_value: numbers[1],
        })
        .collect();
    let Some(contributions) = build_group_contributions(&groups) else {
        tracing::warn!(%column, "dimension contributions overflow");
        return None;
    };
    let breakdown = DimensionBreakdown {
        label: plan.labels.get(&column).cloned(),
        column,
        kind: rows.kind,
        sql: rows.shown,
        groups: contributions,
        mix_effect: None,
        rate_effect: None,
    };
    Some((breakdown, groups))
}

/// Các dòng của câu từng chiều: giá trị nhóm và `numbers` ô số đi sau nó, theo thứ tự của build_dimension_sql.
pub(super) struct DimensionRows {
    pub shown: String,
    pub kind: ValueKind,
    pub groups: Vec<(Option<CellValue>, Vec<Decimal>)>,
}

pub(super) async fn run_dimension_rows(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    column: &str,
    numbers: usize,
) -> Result<Option<DimensionRows>, AppError> {
    let (shown, result) =
        match run_code_query(env, &build_dimension_sql(plan, column), None).await? {
            CodeQuery::Ran { shown, result, .. } => (shown, result),
            CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
                tracing::warn!(%column, %reason, "dimension query failed");
                return Ok(None);
            }
        };
    // Trần dòng của guard cắt bớt nhóm thì các nhóm không cộng ra tổng; trần nhỏ hơn DIMENSION_MAX_VALUES mới gặp.
    if result.limit_reached || result.truncated {
        tracing::warn!(%column, "dimension query reached the row cap");
        return Ok(None);
    }
    let mut groups = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        let cells: Option<Vec<Decimal>> = (1..=numbers)
            .map(|i| row.get(i).and_then(|c| read_decimal(c.as_ref())))
            .collect();
        let Some(cells) = cells else {
            tracing::warn!(%column, "dimension query returned a value that is not a decimal");
            return Ok(None);
        };
        groups.push((row.first().cloned().flatten(), cells));
    }
    Ok(Some(DimensionRows {
        shown,
        kind: result.columns[0].kind,
        groups,
    }))
}

/// Nhãn hai lát cắt cho người dùng: "Quý 2/2017" và "Quý 3/2017", hay hai giá trị nhóm.
/// Hai kỳ hay hai nhóm, theo lát cắt của plan.
pub(super) fn build_comparison(plan: &WhyPlan) -> Comparison {
    match &plan.slices {
        PlanSlices::Periods { .. } => Comparison::Periods,
        PlanSlices::Groups { .. } => Comparison::Groups,
    }
}

/// Cột có hai giá trị đang so, khi so hai nhóm.
pub(super) fn build_slice_column(plan: &WhyPlan) -> Option<String> {
    match &plan.slices {
        PlanSlices::Groups { column, .. } => Some(column.clone()),
        PlanSlices::Periods { .. } => None,
    }
}

pub(super) fn build_slice_labels(plan: &WhyPlan) -> (String, String) {
    match &plan.slices {
        PlanSlices::Periods { base, compare, .. } => (base.label(), compare.label()),
        PlanSlices::Groups { base, compare, .. } => (base.clone(), compare.clone()),
    }
}

/// VolumePrice khi plan có lượng và cả hai lượng khác 0 (build_volume_price trả None khi một lượng bằng 0). Nhãn do
/// code ghép (#40) từ tên tiếng Việt của cột lượng khi hồ sơ có: "Số đơn hàng", "Doanh thu trên mỗi đơn hàng"
/// (build_entity_name); chưa có thì từ tên cột.
fn build_volume(plan: &WhyPlan, totals: &SliceTotals) -> Option<VolumePrice> {
    let column = plan.volume.as_ref()?;
    let (base_volume, compare_volume) = totals.volume?;
    let numbers = build_volume_price(
        base_volume,
        compare_volume,
        totals.base,
        totals.compare,
        PRICE_SCALE,
    )?;
    let entity = build_entity_name(column, plan.labels.get(column).map(String::as_str));
    Some(VolumePrice {
        volume_label: format!("Số {entity}"),
        price_label: format!("{} trên mỗi {entity}", plan.measure.label),
        sql: totals.sql.clone(),
        base_volume: numbers.base_volume,
        compare_volume: numbers.compare_volume,
        base_price: numbers.base_price,
        compare_price: numbers.compare_price,
        volume_effect: numbers.volume_effect,
        price_effect: numbers.price_effect,
        if_volume_unchanged: numbers.if_volume_unchanged,
    })
}
