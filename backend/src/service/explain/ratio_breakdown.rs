// SPDX-License-Identifier: Apache-2.0
/* service/explain/ratio_breakdown.rs - bước explain của câu "tại sao" khi độ đo là tỷ lệ (PlanMeasure.denominator, xem
explain/measure.rs). explain/breakdown.rs gọi build_ratio_breakdown thay cho nhánh cộng được khi câu tổng hai lát cắt có
mẫu số. Chạy câu từng chiều (có thêm hai cột mẫu số, explain/sql.rs) qua guard như nhánh cộng được, gọi phần toán ở
explain/ratio.rs, rồi dựng Explanation với measure_kind ratio. Không có tách lượng và giá: tỷ lệ không phải lượng nhân
giá.

Các chiều xếp theo cách Effect của Adtributor trên đóng góp của từng nhóm, không theo độ bất ngờ S: đóng góp của tỷ lệ
có thể âm (nghịch lý Simpson cho cả nhóm âm lẫn dương), và tỷ lệ của các nhóm không phải một phân phối nên S không có
nghĩa. Một chiều lỗi thì bỏ chiều đó và ghi log, như nhánh cộng được. */

use rust_decimal::Decimal;

use crate::error::AppError;
use crate::service::ask::contract::{
    DimensionBreakdown, Explanation, ExplanationSlice, MeasureKind,
};
use crate::service::explain::annotate::{annotate_dimensions, build_percent_text, Totals};
use crate::service::explain::breakdown::{
    build_comparison, build_slice_column, build_slice_labels, run_dimension_rows, Breakdown,
    QueryEnv, SliceTotals,
};
use crate::service::explain::check::WhyPlan;
use crate::service::explain::ratio::{
    build_rate_text, build_ratio_change, build_ratio_split, RatioGroup, RatioParts,
};
use crate::service::explain::{
    apply_dimension_order, build_decimal_text, build_dimension_score, RankedDimension, RankingMode,
};

/// Phân rã của độ đo tỷ lệ: tỷ lệ hai lát cắt, rồi tách cơ cấu và hiệu suất theo từng chiều (explain/ratio.rs).
pub(super) async fn build_ratio_breakdown(
    env: &QueryEnv<'_>,
    plan: &WhyPlan,
    totals: &SliceTotals,
    (base_denominator, compare_denominator): (Decimal, Decimal),
    filters: (String, String),
) -> Result<Breakdown, AppError> {
    let base = RatioParts {
        numerator: totals.base,
        denominator: base_denominator,
    };
    let compare = RatioParts {
        numerator: totals.compare,
        denominator: compare_denominator,
    };
    let Some(change) = build_ratio_change(base, compare) else {
        return Ok(Breakdown::Unavailable(format!(
            "Mẫu số của {} bằng 0 ở một lát cắt nên không có tỷ lệ để so",
            plan.measure.label
        )));
    };
    if change.change.is_zero() {
        return Ok(Breakdown::Unavailable(format!(
            "{} như nhau ở hai lát cắt nên không có chênh lệch để phân rã",
            plan.measure.label
        )));
    }
    let mut ranked = Vec::new();
    for column in &plan.dimensions {
        let Some(rows) = run_dimension_rows(env, plan, column, 4).await? else {
            continue;
        };
        // Bốn ô số theo thứ tự của câu từng chiều (explain/sql.rs): tử base, tử compare, mẫu base, mẫu compare.
        let groups: Vec<RatioGroup> = rows
            .groups
            .into_iter()
            .map(|(value, n)| RatioGroup {
                value,
                base: RatioParts {
                    numerator: n[0],
                    denominator: n[2],
                },
                compare: RatioParts {
                    numerator: n[1],
                    denominator: n[3],
                },
            })
            .collect();
        // Đóng góp của tỷ lệ có thể âm và không có phân phối p_i, q_i: luôn xếp theo Effect (tổng base 0 làm
        // build_dimension_score từ chối Surprise nếu gọi nhầm).
        let scored = build_ratio_split(&groups, base, compare).and_then(|split| {
            let score = build_dimension_score(
                &split.scored,
                Decimal::ZERO,
                change.change,
                RankingMode::Effect,
            )?;
            Ok((split, score))
        });
        match scored {
            Ok((split, score)) => ranked.push(RankedDimension {
                breakdown: DimensionBreakdown {
                    column: column.to_string(),
                    label: None,
                    has_outliers: false,
                    kind: rows.kind,
                    sql: rows.shown,
                    groups: split.groups,
                    mix_effect: Some(build_rate_text(split.mix_effect)),
                    rate_effect: Some(build_rate_text(split.rate_effect)),
                },
                score,
            }),
            Err(error) => {
                tracing::warn!(%column, ?error, "dimension dropped from the ratio breakdown");
            }
        }
    }
    if ranked.is_empty() {
        return Ok(Breakdown::Unavailable(
            "Không có cột chiều nào phân rã được chênh lệch".to_string(),
        ));
    }
    apply_dimension_order(&mut ranked, RankingMode::Effect);
    // % thay đổi của tỷ lệ tính trên tỷ lệ của hai lát cắt, như % thay đổi của từng nhóm (tỷ lệ của nhóm).
    let annotated = annotate_dimensions(
        ranked.into_iter().map(|d| d.breakdown).collect(),
        Totals {
            base: change.base_rate,
            compare: change.compare_rate,
            change: change.change,
        },
        &plan.labels,
    );
    let (base_label, compare_label) = build_slice_labels(plan);
    Ok(Breakdown::Ready(Box::new(Explanation {
        measure: plan.measure.label.clone(),
        measure_kind: MeasureKind::Ratio,
        comparison: build_comparison(plan),
        counts_rows: false,
        slice_column: build_slice_column(plan),
        slice_column_label: build_slice_column(plan).and_then(|c| plan.labels.get(&c).cloned()),
        base: ExplanationSlice {
            label: base_label,
            filter: filters.0,
            value: build_rate_text(change.base_rate),
            rows: build_decimal_text(totals.base_rows, 0),
        },
        compare: ExplanationSlice {
            label: compare_label,
            filter: filters.1,
            value: build_rate_text(change.compare_rate),
            rows: build_decimal_text(totals.compare_rows, 0),
        },
        change: build_rate_text(change.change),
        percent_change: annotated.percent_change.map(build_percent_text),
        dimensions: annotated.dimensions,
        dropped_dimensions: annotated.dropped,
        volume_price: None,
        summary: String::new(),
        summary_title: String::new(),
        summary_points: Vec::new(),
        summary_by_model: false,
    })))
}
