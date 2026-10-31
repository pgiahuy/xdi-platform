// SPDX-License-Identifier: Apache-2.0
/* service/report/render.rs - dựng lại một mục của báo cáo từ câu hỏi đã lưu (queries). Bảng kết quả không lưu (như mọi
câu hỏi), nên mỗi lần mở báo cáo hay xuất Excel, câu SQL mô hình viết (sql_generated) đi qua guard lại với quyền của
người đang xem, rồi chạy trên đúng các phiên bản câu hỏi đã đọc (read_versions). Cách này giống truy dòng nguồn
(service/drill/): người xem bị lọc dòng theo vai dữ liệu chỉ thấy dòng của vai đó, và nạp phiên bản mới sau khi hỏi
không làm số của báo cáo đổi theo.

Thẻ "Cách tính" và biểu đồ dựng bằng code từ câu vừa chạy (ask/view.rs), các ý "Căn cứ" do code tính lại từ bảng vừa
chạy (service/insight/ với narrate = false: không gọi mô hình). Lời trả lời của lúc hỏi giữ nguyên, kèm kết quả kiểm số
và kiểm nghĩa đã lưu; khi điều kiện lọc dòng của người xem bây giờ không có trong câu đã chạy lúc hỏi (vai dữ liệu đổi
sau đó), lời và phân rã đã lưu có thể nói tới dòng người xem không còn được thấy, nên không hiện. */

use std::path::Path;

use serde::Deserialize;
use uuid::Uuid;

use crate::auth::Member;
use crate::engine::guard::{build_guarded_sql, build_parent_tables, GuardError};
use crate::engine::query::QueryError;
use crate::engine::store::{ReportItemRow, VersionLabel};
use crate::service::ask::build_allowed_table;
use crate::service::ask::context::{build_profiles, ColumnProfile};
use crate::service::ask::contract::{
    AnswerVerdict, AskStatus, Explanation, QuestionType, ResultTable,
};
use crate::service::ask::fanout::build_join_links;
use crate::service::ask::related::{load_group, TableGroup};
use crate::service::ask::repair::build_safe_error;
use crate::service::ask::result::build_result_table;
use crate::service::ask::run::{build_duration_text, run_query_in_turn};
use crate::service::ask::view::{apply_answer_view, build_answer_view, RanQuery};
use crate::service::conversation::contract::SavedAnswer;
use crate::service::explain::breakdown::{QueryEnv, ASK_STOP_HINT};
use crate::service::insight::plan::PlainTable;
use crate::service::insight::{build_insight, InsightInput};
use crate::service::report::contract::{
    NumberCheck, ReportBreakdown, ReportBreakdownGroup, ReportFacts, ReportItem, ReportTableVersion,
};
use crate::{error::AppError, state::AppState};

/// Số nhóm của chiều đầu giữ trong phân rã của một mục: đủ để thấy nhóm nào tạo ra phần lớn chênh lệch, bảng vẫn vừa
/// một trang in.
const BREAKDOWN_GROUPS: usize = 5;

/// Một mục đã dựng lại. Lỗi chạy lại câu (bảng đổi tên, quá giờ) không làm cả báo cáo lỗi: mục đó có `error`.
pub(super) async fn render_item(
    state: &AppState,
    member: &Member,
    row: &ReportItemRow,
    labels: &[VersionLabel],
) -> Result<ReportItem, AppError> {
    let read = build_read_versions(row);
    let stored = read_stored_explanation(row.explanation.as_ref());
    let mut item = ReportItem {
        id: row.id,
        query_id: row.query_id,
        question: row.question.clone(),
        asked_at: row.asked_at,
        question_type: if stored.is_why {
            QuestionType::Causal
        } else {
            QuestionType::Quantitative
        },
        tables: build_table_versions(&read, labels),
        answer: None,
        number_check: None,
        meaning_check: None,
        facts: None,
        understanding: None,
        sql: None,
        result: None,
        chart: None,
        breakdown: None,
        notes: Vec::new(),
        error: None,
    };
    if item.tables.iter().any(|t| !t.is_current) {
        item.notes.push(
            "Có bảng đã có phiên bản mới hơn; mục này vẫn tính trên phiên bản lúc hỏi.".to_string(),
        );
    }
    let answer = row
        .answer
        .clone()
        .and_then(|value| serde_json::from_value::<SavedAnswer>(value).ok());
    match rerun(state, member, row, &read).await? {
        Ok(ran) => {
            let within = is_within_filters(row.sql_guarded.as_deref(), &ran.row_filters);
            if within {
                apply_answer(&mut item, answer);
                if stored.is_why {
                    item.breakdown = stored.breakdown;
                }
            } else {
                item.notes.push(
                    "Quyền xem dòng của bạn đã đổi từ lúc hỏi: lời trả lời và phân rã đã lưu không hiện, số dưới đây \
                     tính lại theo quyền hiện tại."
                        .to_string(),
                );
            }
            if !stored.is_why {
                item.breakdown = ran.breakdown;
            }
            if let (Some(before), Some(result)) = (row.row_count, &ran.result) {
                if u64::try_from(before).ok() != Some(result.total_rows) {
                    item.notes.push(format!(
                        "Lúc hỏi kết quả có {before} dòng, chạy lại bây giờ có {} dòng.",
                        result.total_rows
                    ));
                }
            }
            item.facts = ran.facts;
            item.understanding = ran.understanding;
            item.sql = Some(ran.sql);
            item.result = ran.result;
            item.chart = ran.chart;
        }
        Err(message) => {
            apply_answer(&mut item, answer);
            item.sql = row.sql_guarded.clone();
            item.error = Some(message);
        }
    }
    Ok(item)
}

/// Lời đã lưu và hai kết quả kiểm của nó.
fn apply_answer(item: &mut ReportItem, answer: Option<SavedAnswer>) {
    let Some(answer) = answer else {
        return;
    };
    item.number_check = Some(build_number_check(&answer));
    item.meaning_check = answer.verdict;
    item.answer = Some(answer);
}

/// Kiểm số: lời của mô hình hiện ra nghĩa là đã qua; lời của code mà kết luận là mismatch nghĩa là lời của mô hình bị
/// thay; còn lại là code viết từ số, không có lời nào để kiểm.
pub(super) fn build_number_check(answer: &SavedAnswer) -> NumberCheck {
    if answer.by_model {
        NumberCheck::Passed
    } else if answer.verdict == Some(AnswerVerdict::Mismatch) {
        NumberCheck::Replaced
    } else {
        NumberCheck::NotNeeded
    }
}

/// Mọi điều kiện lọc dòng của người xem bây giờ đều có trong câu đã chạy lúc hỏi: lời và phân rã đã lưu được tính trên
/// tập dòng không rộng hơn tập người xem được thấy. Guard in điều kiện đúng như trong câu (GuardedSql.row_filters).
pub(super) fn is_within_filters(stored_sql: Option<&str>, filters: &[String]) -> bool {
    match stored_sql {
        Some(sql) => filters.iter().all(|f| sql.contains(f.as_str())),
        None => filters.is_empty(),
    }
}

/// Phần của lần chạy lại mà mục cần.
struct Rerun {
    sql: String,
    row_filters: Vec<String>,
    result: Option<ResultTable>,
    understanding: Option<crate::service::ask::contract::Understanding>,
    chart: Option<crate::service::ask::contract::ChartSuggestion>,
    facts: Option<ReportFacts>,
    breakdown: Option<ReportBreakdown>,
}

/// Ok(Err(lời)) khi câu không chạy lại được vì lý do người xem đọc được; Err chỉ cho lỗi máy chủ.
async fn rerun(
    state: &AppState,
    member: &Member,
    row: &ReportItemRow,
    read: &[(Uuid, Uuid)],
) -> Result<Result<Rerun, String>, AppError> {
    let ws = &member.workspace;
    let (Some(dataset_id), Some(version_id), Some(sql)) =
        (row.dataset_id, row.version_id, row.sql_generated.as_deref())
    else {
        return Ok(Err(
            "Câu hỏi này không có câu SQL đã lưu để chạy lại.".to_string()
        ));
    };
    let Some(dataset) = state.store.get_dataset(ws, dataset_id).await? else {
        return Ok(Err("Bảng của câu hỏi này không còn.".to_string()));
    };
    let Some(version) = state.store.get_version(ws, dataset_id, version_id).await? else {
        return Ok(Err(
            "Phiên bản dữ liệu của câu hỏi này không còn.".to_string()
        ));
    };
    let columns = state.store.list_columns(ws, version_id).await?;
    let picked = build_allowed_table(state, member, &dataset, &version, &columns).await?;
    let mut group = load_group(state, member, (&dataset, &version, &columns), picked).await?;
    apply_stored_versions(state, member, &mut group, read).await?;
    let links = build_join_links(&group);
    let mut tables = group.allowed.clone();
    tables.extend(build_parent_tables(&group.allowed));
    let max_rows = state.config.ask_max_rows;
    let guarded = match build_guarded_sql(sql, &tables, &links, max_rows) {
        Ok(guarded) => guarded,
        Err(GuardError::Internal(e)) => return Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => {
            return Ok(Err(format!(
                "Câu SQL đã lưu không qua guard lần nữa: {reason}"
            )))
        }
    };
    let timeout = state.config.query_timeout;
    let output = match run_query_in_turn(&state.query_jobs, guarded.runnable.clone(), timeout).await
    {
        Ok(output) => output,
        Err(QueryError::Sql(raw)) => {
            return Ok(Err(format!(
                "Câu SQL đã lưu không chạy lại được: {}",
                build_safe_error(&raw, &tables)
            )))
        }
        Err(QueryError::Timeout) => {
            return Ok(Err(format!(
                "Chạy lại câu SQL quá {} nên đã bị dừng.",
                build_duration_text(timeout)
            )))
        }
        Err(QueryError::OutOfMemory) => {
            return Ok(Err(
                "Chạy lại câu SQL cần nhiều bộ nhớ hơn mức cho phép nên đã bị dừng.".to_string(),
            ))
        }
        Err(QueryError::Internal(e)) => return Err(e),
    };
    let mut result = build_result_table(output, max_rows);
    let profiles = build_view_profiles(&group);
    let metrics = state.store.list_metrics(ws, dataset.id).await?;
    let limit_kept = guarded.limit_note.is_none();
    let ran = RanQuery {
        status: AskStatus::Answered,
        sql_guarded: Some(&guarded.shown),
        result: Some(&result),
        limit_kept,
        row_filters: &guarded.row_filters,
        plan: None,
    };
    let view = build_answer_view(&ran, &profiles, &metrics, &[]);
    let (understanding, chart) = apply_answer_view(view, Some(&mut result));
    let plain = PlainInput {
        state,
        group: &group,
        links: &links,
        question: &row.question,
        sql_generated: sql,
        sql_guarded: &guarded.shown,
        limit_kept,
    };
    let (facts, breakdown) = build_plain_facts(&plain, &result, understanding.as_ref()).await?;
    Ok(Ok(Rerun {
        sql: guarded.shown,
        row_filters: guarded.row_filters,
        result: Some(result),
        understanding,
        chart,
        facts,
        breakdown,
    }))
}

struct PlainInput<'a> {
    state: &'a AppState,
    group: &'a TableGroup,
    links: &'a [crate::engine::guard::JoinLink],
    question: &'a str,
    sql_generated: &'a str,
    sql_guarded: &'a str,
    limit_kept: bool,
}

/// Các ý do code tính và phân rã tự động của bảng vừa chạy (service/insight/), không gọi mô hình viết lời. Câu "tại
/// sao" cũng đi qua đây: các ý của nó tính trên bảng hai lát cắt, phân rã của nó lấy từ phần đã lưu (render_item).
async fn build_plain_facts(
    input: &PlainInput<'_>,
    result: &ResultTable,
    understanding: Option<&crate::service::ask::contract::Understanding>,
) -> Result<(Option<ReportFacts>, Option<ReportBreakdown>), AppError> {
    let state = input.state;
    let tables: Vec<PlainTable> = input
        .group
        .tables
        .iter()
        .map(|t| PlainTable {
            table: &t.dataset.logical_table,
            columns: &t.columns,
            row_count: t.version.row_count,
        })
        .collect();
    let insight_input = InsightInput {
        question: input.question,
        result,
        sql_generated: input.sql_generated,
        sql_guarded: input.sql_guarded,
        limit_kept: input.limit_kept,
        understanding,
        tables: &tables,
        links: input.links,
        env: QueryEnv {
            tables: &input.group.allowed,
            links: input.links,
            max_rows: state.config.ask_max_rows,
            timeout: state.config.query_timeout,
            jobs: &state.query_jobs,
            stop_hint: ASK_STOP_HINT,
        },
        narrate: false,
        narrate_timeout: state.config.gemini_timeout,
        lessons: &[],
        readings: &[],
    };
    let Some(found) = build_insight(state.llm.as_ref(), &insight_input).await? else {
        return Ok((None, None));
    };
    let facts = ReportFacts {
        title: found.insight.title.clone(),
        points: found
            .insight
            .points
            .iter()
            .map(|p| format!("{}: {}", p.lead, p.text))
            .collect(),
    };
    let breakdown = found.explanation.as_ref().and_then(build_breakdown_from);
    Ok((Some(facts), breakdown))
}

/// Hồ sơ cột cho nhãn của thẻ "Cách tính": bảng đầu, rồi cột chưa trùng tên của các bảng liên quan (như ask/mod.rs).
pub(super) fn build_view_profiles(group: &TableGroup) -> Vec<ColumnProfile> {
    let mut all: Vec<ColumnProfile> = Vec::new();
    for table in &group.tables {
        for column in build_profiles(table.columns.clone()) {
            if !all
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(&column.name))
            {
                all.push(column);
            }
        }
    }
    all
}

/// Bảng liên quan đọc ở phiên bản câu hỏi đã đọc, không phải phiên bản hiện hành load_group chọn. Phiên bản đó đã bị
/// xóa thì giữ bản hiện hành (bảng đầu thì câu hỏi đã mất theo, migration 0011). Điều kiện lọc dòng lan theo khóa ngoại
/// (RowFilter::Parent) vẫn đọc bảng cha ở phiên bản hiện hành: điều kiện đó là quyền của người xem bây giờ.
async fn apply_stored_versions(
    state: &AppState,
    member: &Member,
    group: &mut TableGroup,
    read: &[(Uuid, Uuid)],
) -> Result<(), AppError> {
    let ws = &member.workspace;
    for (index, table) in group.tables.iter_mut().enumerate().skip(1) {
        let Some((_, wanted)) = read.iter().find(|(d, _)| *d == table.dataset.id) else {
            continue;
        };
        if *wanted == table.version.id {
            continue;
        }
        let Some(version) = state
            .store
            .get_version(ws, table.dataset.id, *wanted)
            .await?
        else {
            continue;
        };
        let Some(relative) = version.parquet_path.clone() else {
            continue;
        };
        let columns = state.store.list_columns(ws, version.id).await?;
        let allowed = &mut group.allowed[index];
        allowed.parquet_dir = Path::new(&state.config.data_dir).join(relative);
        allowed.columns = columns.iter().map(|c| c.name.clone()).collect();
        table.version = version;
        table.columns = columns;
    }
    Ok(())
}

/// Các cặp (bảng, phiên bản) câu hỏi đã đọc, bảng đầu trước; câu ghi trước khi có read_versions thì chỉ có bảng đầu.
fn build_read_versions(row: &ReportItemRow) -> Vec<(Uuid, Uuid)> {
    #[derive(Deserialize)]
    struct Pair {
        dataset_id: Uuid,
        version_id: Uuid,
    }
    let mut pairs: Vec<(Uuid, Uuid)> = Vec::new();
    if let (Some(d), Some(v)) = (row.dataset_id, row.version_id) {
        pairs.push((d, v));
    }
    let stored: Vec<Pair> = row
        .read_versions
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    for pair in stored {
        if !pairs.iter().any(|(d, _)| *d == pair.dataset_id) {
            pairs.push((pair.dataset_id, pair.version_id));
        }
    }
    pairs
}

fn build_table_versions(read: &[(Uuid, Uuid)], labels: &[VersionLabel]) -> Vec<ReportTableVersion> {
    read.iter()
        .filter_map(|(_, version_id)| labels.iter().find(|l| l.version_id == *version_id))
        .map(|label| ReportTableVersion {
            table_id: label.dataset_id,
            name: label.table_name.clone(),
            version: label.version,
            captured_at: label.created_at,
            is_current: label.current_version_id == Some(label.version_id),
        })
        .collect()
}

/// Mọi phiên bản các mục đã đọc, để đọc nhãn một lần cho cả báo cáo.
pub(super) fn collect_version_ids(rows: &[ReportItemRow]) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = Vec::new();
    for row in rows {
        for (_, version) in build_read_versions(row) {
            if !ids.contains(&version) {
                ids.push(version);
            }
        }
    }
    ids
}

/// Phần của queries.explanation mục cần: có form (câu "tại sao") không, và phân rã đã lưu.
struct StoredExplanation {
    is_why: bool,
    breakdown: Option<ReportBreakdown>,
}

fn read_stored_explanation(stored: Option<&serde_json::Value>) -> StoredExplanation {
    let Some(stored) = stored else {
        return StoredExplanation {
            is_why: false,
            breakdown: None,
        };
    };
    let is_why = stored.get("form").is_some_and(|form| !form.is_null());
    let breakdown = stored
        .get("explanation")
        .and_then(|e| serde_json::from_value::<BreakdownJson>(e.clone()).ok())
        .and_then(build_breakdown);
    StoredExplanation { is_why, breakdown }
}

/// Phân rã vừa tính (Explanation) về cùng dạng với phân rã đã lưu, để một hàm dựng ReportBreakdown cho cả hai.
fn build_breakdown_from(explanation: &Explanation) -> Option<ReportBreakdown> {
    let json = serde_json::to_value(explanation).ok()?;
    serde_json::from_value::<BreakdownJson>(json)
        .ok()
        .and_then(build_breakdown)
}

/// Các trường của Explanation (ask/contract.rs) mà mục cần, đọc từ JSON: phần đã lưu có thể ghi từ bản cũ hơn của
/// kiểu đó, nên chỉ đọc đúng các trường dưới đây.
#[derive(Deserialize)]
struct BreakdownJson {
    measure: String,
    base: SliceJson,
    compare: SliceJson,
    change: String,
    dimensions: Vec<DimensionJson>,
}

#[derive(Deserialize)]
struct SliceJson {
    label: String,
    value: String,
}

#[derive(Deserialize)]
struct DimensionJson {
    column: String,
    label: Option<String>,
    groups: Vec<GroupJson>,
}

#[derive(Deserialize)]
struct GroupJson {
    value: Option<serde_json::Value>,
    base_value: String,
    compare_value: String,
    contribution: String,
    share_of_change: Option<String>,
}

fn build_breakdown(json: BreakdownJson) -> Option<ReportBreakdown> {
    let dimension = json.dimensions.into_iter().next()?;
    let groups = dimension
        .groups
        .into_iter()
        .take(BREAKDOWN_GROUPS)
        .map(|g| ReportBreakdownGroup {
            name: build_cell_text(g.value.as_ref()),
            base_value: g.base_value,
            compare_value: g.compare_value,
            contribution: g.contribution,
            share_of_change: g.share_of_change,
        })
        .collect();
    Some(ReportBreakdown {
        measure: json.measure,
        base_label: json.base.label,
        compare_label: json.compare.label,
        base_value: json.base.value,
        compare_value: json.compare.value,
        change: json.change,
        dimension: dimension.label.unwrap_or(dimension.column),
        groups,
    })
}

/// Giá trị nhóm thành chữ: chuỗi giữ nguyên, số và đúng sai in như JSON, trống thành chuỗi rỗng.
fn build_cell_text(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}
