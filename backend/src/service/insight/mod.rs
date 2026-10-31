// SPDX-License-Identifier: Apache-2.0
/* service/insight/ - phần giải thích của câu thường (#227): kết quả nói lên điều gì, chọn theo hình dạng bảng kết quả
(insight/shape.rs). ask/mod.rs gọi build_insight sau khi có bảng, vai trò cột và đoạn trả lời, trước bước record. Câu
"tại sao" không đi qua đây: nó đã có lời giải thích của phân rã (service/explain/).

Code chọn và tính, mô hình chỉ viết lời:
- Một con số: so với kỳ liền trước khi câu hỏi nêu một kỳ (insight/periods.rs); chênh đáng kể thì phân rã như câu "tại
  sao".
- Chuỗi thời gian: xu hướng chung, kỳ đổi mạnh nhất so với kỳ liền trước, rồi phân rã đúng kỳ đó.
- Theo nhóm: nhóm đứng đầu chiếm bao nhiêu phần tổng, cần mấy nhóm để đạt 80% tổng, nhóm lệch hẳn mức trung bình.
- Top N: N nhóm đầu chiếm bao nhiêu phần tổng (một câu tổng do code dựng từ câu của mô hình, bỏ LIMIT), chênh giữa hạng
  đầu và hạng kế.
- Danh sách dòng: chỉ số dòng và điều kiện lọc.
Câu chữ ở insight/facts.rs, phần so hai kỳ (một con số, chuỗi thời gian) ở insight/period_breakdown.rs, lời của mô hình
và bước kiểm ở insight/narrate.rs.

Phân rã tự động dùng lại nguyên đường của câu "tại sao" (explain/breakdown.rs) với WhyPlan dựng từ câu SQL của câu hỏi
(insight/plan.rs), không gọi mô hình điền form, nên không thêm lần gọi mô hình nào. Trước khi dùng phân rã, code so tổng
hai kỳ do code tính với ô của bảng kết quả: lệch (câu của mô hình nối hay lọc khác cách code hiểu) thì bỏ phân rã, vẫn
giữ phần giải thích từ bảng. Mọi câu do code dựng đi qua guard với đúng bảng và bộ lọc dòng của người hỏi.

Hai trường hợp không gọi mô hình viết lời, dùng câu của code: danh sách dòng (câu chỉ có số dòng và nhãn lọc, mô hình
không thêm được gì), và bảng có cột khóa đọc từ cột đánh dấu nhạy cảm (tên nhóm khi đó là giá trị của cột đó, như email
khách hàng, mà prompt không được chứa; cột khóa và cột chữ tự do cũng vậy, find_unsendable_key).

Phần này không bao giờ làm câu hỏi dừng: lỗi của câu SQL do code dựng chỉ làm mất phân rã (ghi ở bước explain). Lỗi máy
chủ (không mở được DuckDB) vẫn là Err như mọi bước khác. Test ở insight/tests.rs. */

pub mod facts;
pub mod narrate;
mod one_row;
mod period_breakdown;
pub mod periods;
pub mod plan;
pub mod shape;
#[cfg(test)]
mod tests;

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use sqlparser::ast::{
    visit_expressions, DuplicateTreatment, Expr, FunctionArg, FunctionArgExpr, FunctionArguments,
};

use crate::engine::llm::LlmProvider;
use crate::engine::sql_shape::{build_query_shape, QueryShape};
use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::error::AppError;
use crate::service::ask::contract::{
    AskStep, ColumnRole, Explanation, Insight, InsightKind, ResultTable, StepName, Understanding,
};
use crate::service::ask::failures::StepFailure;
use crate::service::ask::fanout::JoinLink;
use crate::service::ask::run::build_step;
use crate::service::ask::summary::build_cell_text;
use crate::service::explain::breakdown::{run_code_query, CodeQuery, QueryEnv};
use crate::service::explain::is_vietnamese_alias;
use crate::service::explain::per_unit::build_alias_label;
use crate::service::insight::facts::{
    build_group_parts, build_rows_parts, build_top_parts, InsightParts, Point,
};
use crate::service::insight::narrate::{build_code_narrated, narrate_insight};
use crate::service::insight::periods::read_series_time;
use crate::service::insight::plan::{read_plain_query, PlainQuery, PlainTable};
use crate::service::insight::shape::{build_cell_decimal, build_result_shape, ResultShape};

/// Đầu vào của build_insight: câu đã chạy và bảng kết quả đã có vai trò cột.
pub struct InsightInput<'a> {
    pub question: &'a str,
    pub result: &'a ResultTable,
    /// Câu của mô hình, trên tên bảng logic, trước guard: phân rã tự động đọc độ đo, vế lọc và khóa thời gian từ đây.
    pub sql_generated: &'a str,
    /// Câu đã chạy: để biết LIMIT có phải của mô hình không (top N).
    pub sql_guarded: &'a str,
    pub limit_kept: bool,
    pub understanding: Option<&'a Understanding>,
    /// Các bảng của nhóm (ask/related.rs) kèm hồ sơ cột, bảng đang hỏi ở đầu.
    pub tables: &'a [PlainTable<'a>],
    pub links: &'a [JoinLink],
    pub env: QueryEnv<'a>,
    /// Config::insight_text.
    pub narrate: bool,
    pub narrate_timeout: Duration,
    /// Bài học đã duyệt liên quan câu hỏi (ask/lessons.rs), thêm vào prompt viết lời.
    pub lessons: &'a [String],
    /// Cụm của câu hỏi khớp nhiều cột mà ngữ cảnh tự chọn một (ask/context.rs), kèm tên tiếng Việt của cột đã chọn: phần
    /// giải thích nói rõ đã tính theo cột nào, người hỏi không phải mở thẻ "Cách tính" mới biết.
    pub readings: &'a [ColumnReading],
}

/// Một cụm và cột ngữ cảnh đã chọn cho nó, bằng lời.
pub struct ColumnReading {
    pub phrase: String,
    /// Tên tiếng Việt của cột (bí danh đầu của hồ sơ, không có thì mô tả, cuối cùng mới là tên cột).
    pub label: String,
}

/// Phần giải thích, phân rã tự động khi có, các bước (explain, narrate) và bảng phân rã đã đọc thêm.
pub struct InsightOutcome {
    pub insight: Insight,
    pub explanation: Option<Explanation>,
    pub steps: Vec<AskStep>,
    /// Tên bảng logic phân rã đã đọc (bảng chính và bảng liên quan), để ghi phiên bản vào lịch sử.
    pub tables_read: Vec<String>,
    /// Bước kiểm chặn lời của mô hình, cho nhật ký lỗi (#228).
    pub failure: Option<StepFailure>,
}

/// None khi bảng không có dòng nào hay mọi ô trống (như đoạn trả lời, ask/summary.rs).
pub async fn build_insight(
    llm: &dyn LlmProvider,
    input: &InsightInput<'_>,
) -> Result<Option<InsightOutcome>, AppError> {
    let shape = match build_query_shape(input.sql_guarded) {
        Ok(shape) => shape,
        Err(error) => {
            tracing::warn!(
                ?error,
                "cannot read the guarded sql, answering without an insight"
            );
            return Ok(None);
        }
    };
    let Some(mut result_shape) = build_result_shape(input.result, &shape, input.limit_kept) else {
        return Ok(None);
    };
    // Khóa nhóm là kỳ viết thành chữ (strftime(ngày, '%Y-%m')) thì vai trò cột là dimension, nhưng bảng vẫn là chuỗi
    // thời gian.
    if result_shape.kind == InsightKind::Groups && is_time_key(input, result_shape) {
        result_shape.kind = InsightKind::TimeSeries;
    }
    let started = Instant::now();
    let mut found = Found::default();
    // Một dòng ngắn của danh sách dòng được kể bằng lời (insight/one_row.rs), thay cho câu đếm dòng.
    let one_row = (result_shape.kind == InsightKind::Rows)
        .then(|| one_row::build_one_row(input, &shape))
        .flatten();
    let (mut parts, one_row_code_only) = match one_row {
        Some(row) => (row.parts, Some(row.code_only)),
        None => {
            let additive = result_shape.measure.is_some_and(|m| is_additive(&shape, m));
            (
                build_parts(input, result_shape, additive, &mut found).await?,
                None,
            )
        }
    };
    for reading in input.readings {
        parts.add_reading(&reading.phrase, &reading.label);
    }
    parts.apply_readable_numbers();
    let mut steps = vec![build_step(
        StepName::Explain,
        started,
        true,
        found.detail.as_deref(),
    )];
    let code_only = match one_row_code_only {
        Some(reason) => reason,
        None => build_code_only_reason(input, &shape, result_shape),
    };
    let narrated = match code_only {
        Some(detail) => build_code_narrated(&parts, Instant::now(), detail),
        None => {
            narrate_insight(
                llm,
                input.question,
                &parts,
                input.narrate,
                input.narrate_timeout,
                input.lessons,
            )
            .await
        }
    };
    steps.push(narrated.step);
    Ok(Some(InsightOutcome {
        insight: Insight {
            kind: result_shape.kind,
            title: parts.title,
            points: parts.points,
            summary: narrated.summary,
            summary_by_model: narrated.by_model,
            summary_warnings: narrated.warnings,
            summary_verdict: narrated.verdict,
            notable: parts.notable,
        },
        explanation: found.explanation,
        steps,
        tables_read: found.tables_read,
        failure: narrated.failure,
    }))
}

/// Lý do dùng câu của code mà không gọi mô hình (chi tiết bước narrate), hay None khi được gọi.
fn build_code_only_reason(
    input: &InsightInput<'_>,
    shape: &QueryShape,
    result_shape: ResultShape,
) -> Option<String> {
    if result_shape.kind == InsightKind::Rows {
        return Some("Danh sách dòng: dùng câu do code viết, không gọi mô hình".to_string());
    }
    let key = result_shape.key?;
    let column = find_unsendable_key(input.tables, shape, key)?;
    Some(format!(
        "Cột khóa đọc từ {column}, cột nhạy cảm hay không phải cột phân loại, nên không gửi tên nhóm cho mô hình; \
         dùng câu do code viết"
    ))
}

/// Cột mà cột khóa của bảng kết quả đọc từ đó (một cột có trong biểu thức của cột khóa) và giá trị của nó không được
/// gửi cho mô hình, hay None. Chỉ gửi được giá trị của cột phân loại (dimension) hay cột thời gian không đánh dấu nhạy
/// cảm, như danh sách giá trị của prompt SQL (ask/prompt.rs); cột nhạy cảm (email khách hàng), cột khóa (mã đơn), cột chữ
/// tự do thì không. Biểu thức không đọc được (cột `*`) thì coi là không gửi được: thà mất lời của mô hình còn hơn gửi
/// giá trị không được phép.
pub(crate) fn find_unsendable_key(
    tables: &[PlainTable],
    shape: &QueryShape,
    key: usize,
) -> Option<String> {
    let Some(tree) = shape.outputs.get(key).and_then(|o| o.tree.as_ref()) else {
        return Some("*".to_string());
    };
    let sendable = |c: &ProfileColumn| {
        !c.is_sensitive
            && (c.role == ProfileRole::Dimension.as_str() || c.role == ProfileRole::Time.as_str())
    };
    let mut found = None;
    let _ = visit_expressions(tree, |e| {
        let name = match e {
            Expr::Identifier(ident) => Some(&ident.value),
            Expr::CompoundIdentifier(parts) => parts.last().map(|p| &p.value),
            _ => None,
        };
        let column = name.and_then(|n| {
            tables
                .iter()
                .flat_map(|t| t.columns.iter())
                .find(|c| c.name.eq_ignore_ascii_case(n))
        });
        match column {
            Some(column) if !sendable(column) => {
                found = Some(column.name.clone());
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    });
    found
}

/// Những gì phân rã tự động tìm được, và chi tiết cho bước explain.
#[derive(Default)]
pub(super) struct Found {
    pub(super) explanation: Option<Explanation>,
    pub(super) tables_read: Vec<String>,
    pub(super) detail: Option<String>,
}

/// Độ đo cộng được giữa các nhóm: SUM hay COUNT không DISTINCT ở ngoài cùng (bỏ qua CAST, ngoặc). Cộng các tỷ lệ, trung
/// bình hay số đếm không trùng của từng nhóm không ra số có nghĩa ("chiếm 35,6% tổng tỷ lệ đạt"), nên với các độ đo đó
/// phần giải thích không nói tỷ trọng trên tổng hay mức tập trung.
fn is_additive(shape: &QueryShape, measure: usize) -> bool {
    let mut expr = match shape.outputs.get(measure).and_then(|o| o.tree.as_ref()) {
        Some(expr) => expr,
        None => return false,
    };
    loop {
        match expr {
            Expr::Nested(inner) => expr = inner,
            Expr::Cast { expr: inner, .. } => expr = inner,
            Expr::Function(function) => {
                let name = function.name.to_string().to_lowercase();
                let FunctionArguments::List(list) = &function.args else {
                    return false;
                };
                // ROUND(SUM(x), 2), COALESCE(SUM(x), 0) vẫn là tổng: xét đối số đầu.
                if name == "round" || name == "coalesce" {
                    match list.args.first() {
                        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(inner))) => {
                            expr = inner;
                            continue;
                        }
                        _ => return false,
                    }
                }
                let distinct =
                    matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
                return function.over.is_none()
                    && (name == "sum" || (name == "count" && !distinct));
            }
            _ => return false,
        }
    }
}

async fn build_parts(
    input: &InsightInput<'_>,
    shape: ResultShape,
    additive: bool,
    found: &mut Found,
) -> Result<InsightParts, AppError> {
    let result = input.result;
    let (Some(measure), key) = (shape.measure, shape.key) else {
        return Ok(build_rows_parts(
            result.total_rows,
            result.limit_reached,
            &build_filter_labels(input.understanding),
        ));
    };
    let label = build_measure_label(input, measure);
    let parts = match shape.kind {
        InsightKind::SingleValue => {
            period_breakdown::build_single(input, measure, &label, found).await?
        }
        InsightKind::TimeSeries => {
            period_breakdown::build_series(input, measure, key, &label, found).await?
        }
        InsightKind::Groups => {
            let points = build_points(result, measure, key);
            let complete = !result.truncated && !result.limit_reached && additive;
            let total = complete.then(|| points.iter().map(|p| p.value).sum());
            let column = build_key_label(input, key);
            build_group_parts(&label, &column, &points, total)
        }
        InsightKind::TopN => {
            let points = build_points(result, measure, key);
            let all = match additive {
                true => run_total(input, measure).await?,
                false => None,
            };
            build_top_parts(&label, &points, all)
        }
        InsightKind::Rows => build_rows_parts(
            result.total_rows,
            result.limit_reached,
            &build_filter_labels(input.understanding),
        ),
    };
    Ok(parts)
}

/// Tên độ đo: bí danh tiếng Việt câu SQL đặt cho cột (is_vietnamese_alias), không thì nhãn của thẻ "Cách tính" (cùng
/// thứ tự với các cột tổng hợp), không có thẻ thì tên cột.
pub(super) fn build_measure_label(input: &InsightInput<'_>, measure: usize) -> String {
    let result = input.result;
    let position = result.columns[..measure]
        .iter()
        .filter(|c| c.role == ColumnRole::Measure)
        .count();
    let name = &result.columns[measure].name;
    match input.understanding.and_then(|u| u.measures.get(position)) {
        Some(item) if !is_vietnamese_alias(name, &item.sql) => item.label.clone(),
        // Bí danh "Lợi nhuận mỗi đơn" đặt cho một SUM là tổng (#260): gọi "Tổng lợi nhuận".
        Some(item) => build_alias_label(name, &item.sql),
        None => name.clone(),
    }
}

fn build_key_label(input: &InsightInput<'_>, key: Option<usize>) -> String {
    let Some(key) = key else {
        return String::new();
    };
    input
        .understanding
        .and_then(|u| u.group_by.first())
        .map(|g| g.label.clone())
        .unwrap_or_else(|| input.result.columns[key].name.clone())
}

fn build_filter_labels(understanding: Option<&Understanding>) -> Vec<String> {
    let Some(u) = understanding else {
        return Vec::new();
    };
    u.time_range
        .iter()
        .chain(&u.filters)
        .map(|item| item.label.clone())
        .collect()
}

/// Điểm của bảng theo nhóm hay top N: nhãn là chữ của ô khóa như bảng hiện, bỏ dòng có ô độ đo trống.
pub(super) fn build_points(result: &ResultTable, measure: usize, key: Option<usize>) -> Vec<Point> {
    result
        .rows
        .iter()
        .filter_map(|row| {
            let value = build_cell_decimal(row[measure].as_ref())?;
            let label = match key.and_then(|k| row[k].as_ref()) {
                Some(cell) => build_cell_text(cell, result.columns[key?].kind),
                None => "trống".to_string(),
            };
            Some(Point { label, value })
        })
        .collect()
}

/// Tổng của mọi nhóm và số nhóm cho top N: câu của mô hình bỏ ORDER BY, LIMIT, bọc trong SUM. Chỉ với độ đo SUM hay
/// COUNT (cộng được qua các nhóm); None khi không tính được.
async fn run_total(
    input: &InsightInput<'_>,
    measure: usize,
) -> Result<Option<(Decimal, i64)>, AppError> {
    let Some(sql) = plan::build_total_sql(input.sql_generated, measure) else {
        return Ok(None);
    };
    let result = match run_code_query(&input.env, &sql, None).await? {
        CodeQuery::Ran { result, .. } => result,
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => {
            tracing::warn!(%reason, "top n total query failed");
            return Ok(None);
        }
    };
    let row = result.rows.first();
    let total = row.and_then(|r| build_cell_decimal(r.first()?.as_ref()));
    let groups = row
        .and_then(|r| build_cell_decimal(r.get(1)?.as_ref()))
        .and_then(|n| i64::try_from(n).ok());
    Ok(total.zip(groups))
}

/// Khóa của bảng theo nhóm đọc được thành cột thời gian và độ hạt (insight/periods.rs).
fn is_time_key(input: &InsightInput<'_>, shape: ResultShape) -> bool {
    let (Some(measure), Some(key)) = (shape.measure, shape.key) else {
        return false;
    };
    let label = &input.result.columns[measure].name;
    let Some(query) = read_query(input, measure, label) else {
        return false;
    };
    let Some(Some(expr)) = query.outputs.get(key) else {
        return false;
    };
    read_series_time(expr, &|name: &str| query.find_column(name, input.tables)).is_some()
}

/// Câu của mô hình đã đọc thành PlainQuery (insight/plan.rs).
pub(super) fn read_query(
    input: &InsightInput<'_>,
    measure: usize,
    label: &str,
) -> Option<PlainQuery> {
    read_plain_query(
        input.sql_generated,
        measure,
        label,
        input.tables,
        input.links,
    )
}
