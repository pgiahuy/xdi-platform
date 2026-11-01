// SPDX-License-Identifier: Apache-2.0
/* service/report/auto_run.rs - chạy báo cáo tự động cho một dataset: dựng kế hoạch (auto.rs), cho từng câu SQL do code
dựng qua guard với bảng và bộ lọc dòng của người bấm, chạy trên phiên bản hiện hành, rồi để service/insight/ tính các
ý và phân rã, mô hình viết lời qua đúng hai cổng kiểm của câu hỏi (kiểm số bằng code, rồi một lần gọi riêng kiểm
nghĩa). Mỗi phần được ghi thành một dòng queries như một câu đã trả lời (không thuộc cuộc trò chuyện nào), nên mở báo
cáo, xuất Excel và truy dòng nguồn đi chung một đường với báo cáo tự gom.

Số lần gọi mô hình: mỗi phần tối đa một lần viết lời và một lần kiểm nghĩa, thêm một lần viết lại và một lần kiểm nữa
khi lời đầu không qua (service/insight/narrate.rs); ba phần nên thường 6 lần, nhiều nhất 12. INSIGHT_TEXT=false thì
không gọi lần nào: lời là câu của code. Số thật đếm bằng CallLog và lưu ở reports.model_calls. */

use std::time::Instant;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::guard::{
    build_guarded_sql, build_parent_tables, AllowedTable, GuardError, JoinLink,
};
use crate::engine::llm::meter::CallLog;
use crate::engine::query::QueryError;
use crate::engine::store::{Dataset, DatasetVersion, NewQuery, NewReport, QueryStatus};
use crate::service::ask::build_allowed_table;
use crate::service::ask::context::check_profile_ready;
use crate::service::ask::contract::{AskStatus, AskStep, StepName};
use crate::service::ask::fanout::build_join_links;
use crate::service::ask::related::{load_group, TableGroup};
use crate::service::ask::repair::build_safe_error;
use crate::service::ask::result::build_result_table;
use crate::service::ask::run::{build_step, run_query_in_turn};
use crate::service::ask::summary::{build_answer, SummaryInput};
use crate::service::ask::view::{apply_answer_view, build_answer_view, RanQuery};
use crate::service::conversation::contract::SavedAnswer;
use crate::service::conversation::history::build_understood_text;
use crate::service::explain::breakdown::{QueryEnv, ASK_STOP_HINT};
use crate::service::insight::plan::PlainTable;
use crate::service::insight::{build_insight, InsightInput};
use crate::service::lessons::{record_failures, FailedQuestion};
use crate::service::report::auto::{build_auto_plan, AutoSection, AutoTable};
use crate::service::report::render::build_view_profiles;
use crate::{error::AppError, state::AppState};

/// Báo cáo tự động vừa ghi: id, số lần gọi mô hình và phần không dựng được.
pub(super) struct BuiltReport {
    pub id: Uuid,
    pub skipped: Vec<String>,
}

pub(super) async fn build_auto_report(
    state: &AppState,
    member: &Member,
    source_id: Uuid,
) -> Result<BuiltReport, AppError> {
    let ws = &member.workspace;
    let source = state
        .store
        .get_catalog_source(ws, source_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không có dataset này".to_string()))?;
    let loaded = load_tables(state, member, source_id).await?;
    let metrics = load_metrics(state, member, &loaded).await?;
    let auto_tables: Vec<AutoTable> = loaded
        .iter()
        .zip(&metrics)
        .map(|(t, m)| AutoTable {
            id: t.dataset.id,
            logical_table: &t.dataset.logical_table,
            columns: &t.columns,
            metrics: m,
        })
        .collect();
    let relations = state.store.list_relations(ws, None).await?;
    let plan = build_auto_plan(&auto_tables, &relations);
    let Some(main) = plan
        .table_id
        .and_then(|id| loaded.iter().find(|t| t.dataset.id == id))
    else {
        return Err(AppError::BadRequest(plan.skipped.join(" ")));
    };

    // Mọi lần gọi mô hình của các phần đi qua state_for_report, để đếm đủ (như ask/mod.rs).
    let log = CallLog::new();
    let state_for_report = state.with_call_log(&log);
    let picked =
        build_allowed_table(state, member, &main.dataset, &main.version, &main.columns).await?;
    let group = load_group(
        state,
        member,
        (&main.dataset, &main.version, &main.columns),
        picked,
    )
    .await?;
    let mut skipped = plan.skipped.clone();
    let mut query_ids: Vec<Uuid> = Vec::new();
    for section in &plan.sections {
        match run_section(&state_for_report, member, &group, section).await? {
            Ok(id) => query_ids.push(id),
            Err(reason) => skipped.push(format!("{}: {reason}", section.question)),
        }
    }
    if query_ids.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Chưa tạo được phần nào của báo cáo. {}",
            skipped.join(" ")
        )));
    }
    let calls = log.calls().len();
    tracing::info!(
        model_calls = calls,
        sections = query_ids.len(),
        "automatic report built"
    );
    let title = format!("Báo cáo tự động: {}", source.name);
    let intro = format!(
        "Báo cáo do hệ thống tạo cho dataset {}: {} theo thời gian và theo nhóm, trên dữ liệu bạn được xem. Câu SQL \
         của từng phần do code dựng; lời diễn giải do mô hình viết thì đã qua kiểm số và kiểm nghĩa, không qua thì là \
         câu do hệ thống viết từ số.",
        source.name, plan.measure
    );
    let report = NewReport {
        created_by: member.user.id,
        title: &title,
        intro: &intro,
        source_id: Some(source_id),
        model_calls: Some(i32::try_from(calls).unwrap_or(i32::MAX)),
    };
    let id = state.store.create_report(ws, &report).await?;
    for query_id in query_ids {
        state.store.add_report_item(ws, id, query_id).await?;
    }
    Ok(BuiltReport { id, skipped })
}

/// Một bảng của dataset đã lập hồ sơ xong, kèm phiên bản hiện hành và hồ sơ cột.
struct LoadedTable {
    dataset: Dataset,
    version: DatasetVersion,
    columns: Vec<crate::engine::store::ProfileColumn>,
}

async fn load_tables(
    state: &AppState,
    member: &Member,
    source_id: Uuid,
) -> Result<Vec<LoadedTable>, AppError> {
    let ws = &member.workspace;
    let mut loaded = Vec::new();
    for dataset in state.store.list_datasets(ws).await? {
        if dataset.parent_id != source_id {
            continue;
        }
        let Some(version) = state.store.get_current_version(ws, dataset.id).await? else {
            continue;
        };
        if check_profile_ready(&version).is_err() || version.parquet_path.is_none() {
            continue;
        }
        let columns = state.store.list_columns(ws, version.id).await?;
        loaded.push(LoadedTable {
            dataset,
            version,
            columns,
        });
    }
    Ok(loaded)
}

async fn load_metrics(
    state: &AppState,
    member: &Member,
    tables: &[LoadedTable],
) -> Result<Vec<Vec<crate::engine::store::MetricDefinition>>, AppError> {
    let mut all = Vec::new();
    for table in tables {
        all.push(
            state
                .store
                .list_metrics(&member.workspace, table.dataset.id)
                .await?,
        );
    }
    Ok(all)
}

/// Ok(Err(lý do)) khi phần không dựng được (guard từ chối, chạy lỗi, không có dòng); Err chỉ cho lỗi máy chủ.
async fn run_section(
    state: &AppState,
    member: &Member,
    group: &TableGroup,
    section: &AutoSection,
) -> Result<Result<Uuid, String>, AppError> {
    let ws = &member.workspace;
    let main = &group.tables[0];
    let links = build_join_links(group);
    let mut tables: Vec<AllowedTable> = group.allowed.clone();
    tables.extend(build_parent_tables(&group.allowed));
    let max_rows = state.config.ask_max_rows;
    // Lần gọi mô hình của riêng phần này, ghi vào queries.timings như một câu hỏi (#184): log đếm cho cả báo cáo.
    let calls_before = state.call_log.as_ref().map_or(0, |log| log.calls().len());
    let mut steps: Vec<AskStep> = Vec::new();
    let started = Instant::now();
    let guarded = match build_guarded_sql(&section.sql, &tables, &links, max_rows) {
        Ok(guarded) => guarded,
        Err(GuardError::Internal(e)) => return Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => return Ok(Err(format!("guard không nhận câu ({reason})"))),
    };
    steps.push(build_step(StepName::Guard, started, true, None));
    let started = Instant::now();
    let timeout = state.config.query_timeout;
    let output = match run_query_in_turn(&state.query_jobs, guarded.runnable.clone(), timeout).await
    {
        Ok(output) => output,
        Err(QueryError::Sql(raw)) => {
            return Ok(Err(format!(
                "câu chạy lỗi ({})",
                build_safe_error(&raw, &tables)
            )))
        }
        Err(QueryError::Timeout | QueryError::OutOfMemory) => {
            return Ok(Err("câu chạy quá giờ hay quá bộ nhớ".to_string()))
        }
        Err(QueryError::Internal(e)) => return Err(e),
    };
    steps.push(build_step(StepName::Run, started, true, None));
    let mut result = build_result_table(output, max_rows);
    if result.rows.is_empty() {
        return Ok(Err("không có dòng nào".to_string()));
    }
    let profiles = build_view_profiles(group);
    let metrics = state.store.list_metrics(ws, main.dataset.id).await?;
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
    let (understanding, _) = apply_answer_view(view, Some(&mut result));
    let sections = SectionRun {
        state,
        group,
        links: &links,
        question: &section.question,
        sql: &section.sql,
        sql_guarded: &guarded.shown,
        limit_kept,
    };
    let insight = run_section_insight(&sections, &result, understanding.as_ref()).await?;
    let mut failures = Vec::new();
    let mut read_names = guarded.tables_read.clone();
    let (insight, explanation) = match insight {
        Some(found) => {
            steps.extend(found.steps);
            failures.extend(found.failure);
            for name in found.tables_read {
                if !read_names.contains(&name) {
                    read_names.push(name);
                }
            }
            (Some(found.insight), found.explanation)
        }
        None => (None, None),
    };
    let summary = build_answer(&SummaryInput {
        understanding: understanding.as_ref(),
        result: &result,
        insight: insight.as_ref(),
        enabled: state.config.answer_text,
    });
    let answer = summary.map(|s| SavedAnswer {
        by_model: s.by_model,
        verdict: insight.as_ref().and_then(|i| i.summary_verdict),
        failed_check: insight.as_ref().and_then(|i| i.summary_failed_check),
        warnings: match &insight {
            Some(i) if s.by_model => i.summary_warnings.clone(),
            _ => Vec::new(),
        },
        text: s.text,
    });
    let explanation_json = match &explanation {
        Some(e) => Some(serde_json::json!({
            "form": null,
            "explanation": serde_json::to_value(e)
                .map_err(|e| AppError::Internal(format!("serialize report breakdown: {e}")))?,
        })),
        None => None,
    };
    let tables_read = group.build_tables_read(&read_names);
    let calls: Vec<_> = state
        .call_log
        .as_ref()
        .map(|log| log.calls().into_iter().skip(calls_before).collect())
        .unwrap_or_default();
    let query = NewQuery {
        target: Some((main.dataset.id, main.version.id)),
        user_id: Some(member.user.id),
        question: &section.question,
        sql_generated: Some(&section.sql),
        sql_guarded: Some(&guarded.shown),
        status: QueryStatus::Ok,
        error: None,
        timings: serde_json::json!({"steps": steps, "model_calls": calls}),
        row_count: Some(i64::try_from(result.total_rows).unwrap_or(i64::MAX)),
        explanation: explanation_json,
        read_versions: group.build_read_versions(&tables_read),
    };
    let query_id = state.store.record_query(ws, &query).await?;
    let understood = understanding.as_ref().and_then(build_understood_text);
    let answer_json = answer.and_then(|a| serde_json::to_value(a).ok());
    state
        .store
        .set_query_answer(ws, query_id, understood.as_deref(), answer_json.as_ref())
        .await?;
    let asked = FailedQuestion {
        dataset_id: main.dataset.id,
        question: &section.question,
        query_id: Some(query_id),
    };
    record_failures(state, member, &asked, &failures).await;
    Ok(Ok(query_id))
}

struct SectionRun<'a> {
    state: &'a AppState,
    group: &'a TableGroup,
    links: &'a [JoinLink],
    question: &'a str,
    sql: &'a str,
    sql_guarded: &'a str,
    limit_kept: bool,
}

/// Phần giải thích của một phần, như của câu thường (ask/plain_insight.rs): code tính, mô hình viết lời khi
/// INSIGHT_TEXT bật, lời qua hai cổng kiểm.
async fn run_section_insight(
    input: &SectionRun<'_>,
    result: &crate::service::ask::contract::ResultTable,
    understanding: Option<&crate::service::ask::contract::Understanding>,
) -> Result<Option<crate::service::insight::InsightOutcome>, AppError> {
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
    let narrate_timeout = match state.config.llm_provider.as_str() {
        "ollama" => state.config.ollama_timeout,
        _ => state.config.gemini_timeout,
    };
    let insight_input = InsightInput {
        question: input.question,
        result,
        sql_generated: input.sql,
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
        narrate: state.config.insight_text,
        narrate_timeout,
        lessons: &[],
        readings: &[],
    };
    build_insight(state.llm.as_ref(), &insight_input).await
}
