// SPDX-License-Identifier: Apache-2.0
/* service/drill/mod.rs - truy dòng nguồn của một con số (#34): đọc câu hỏi đã lưu (queries) theo workspace và query_id,
dựng câu đọc dòng nguồn từ sql_guarded (drill/sql.rs; tên cột kết quả lấy từ một lần chạy câu gốc với LIMIT 0), cho qua guard với quyền của người đang bấm, chạy trên đúng phiên
bản dữ liệu câu hỏi đã dùng, rồi trả một trang dòng cùng tổng số dòng. Hợp đồng ở drill/contract.rs, route ở
api/query.rs. Test ở drill/tests.rs (không cần Postgres) và api/drill_tests.rs (qua router).

Quyền của người đang bấm, không của người đã hỏi: câu do admin hỏi, thành viên có vai dữ liệu bấm vào thì chỉ thấy dòng
của vai đó. Chạy trên version_id đã lưu: nạp phiên bản mới sau đó không làm dòng nguồn lệch con số đã trả.

Câu nối bảng (JOIN theo khóa ngoại, #256) truy được: trang là dòng của bảng mà độ đo cộng kèm cột khóa và cột lọc của
bảng nối (drill/sources.rs, drill/columns.rs), và mọi độ đo được tính lại trên mọi dòng nguồn (drill/measures.rs) để
người dùng so với con số.

Bấm dòng phân rã của câu "tại sao": đọc queries.explanation (ask/why.rs ghi) lấy câu SQL của chiều được bấm
(DimensionBreakdown.sql, đã qua guard khi hỏi) và điều kiện của lát cắt được bấm, rồi truy như một ô của câu chiều đó
(drill/sql.rs). Request chỉ gửi tên lát cắt, cột và giá trị nhóm; SQL luôn lấy từ chỗ đã lưu. */

mod columns;
pub mod contract;
#[cfg(test)]
mod join_tests;
mod measures;
mod sources;
mod sql;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use duckdb::types::Value;
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::auth::Member;
use crate::engine::guard::{
    build_guarded_sql, build_parent_tables, AllowedTable, GuardError, GuardedSql, JoinLink,
};
use crate::engine::query::{QueryError, QueryOutput};
use crate::engine::store::QueryStatus;
use crate::service::ask::build_allowed_table;
use crate::service::ask::contract::{CellValue, ResultTable};
use crate::service::ask::fanout::build_join_links;
use crate::service::ask::related::load_group;
use crate::service::ask::repair::build_safe_error;
use crate::service::ask::result::build_result_table;
use crate::service::ask::run::{build_duration_text, run_query_in_turn};
use crate::service::drill::columns::PageColumns;
use crate::service::drill::contract::{
    BreakdownKey, DrillKey, DrillMeasure, DrillRequest, DrillResponse, SliceName, MAX_DRILL_LIMIT,
};
use crate::service::drill::sql::{build_drill_plan, PageRequest};
use crate::service::explain::sql::{BASE_VALUE, COMPARE_VALUE};
use crate::service::ingest::{dataset_not_found, parse_dataset_id};
use crate::{error::AppError, state::AppState};

pub async fn drill(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    req: &DrillRequest,
) -> Result<DrillResponse, AppError> {
    check_request(req)?;
    let ws = &member.workspace;
    let dataset_id = parse_dataset_id(dataset_id)?;
    // Câu hỏi của dataset khác cùng workspace cũng là 404: đường dẫn và query_id phải chỉ cùng một chỗ.
    let query = state
        .store
        .get_query(ws, req.query_id)
        .await?
        .filter(|q| q.dataset_id == Some(dataset_id))
        .ok_or_else(query_not_found)?;
    let sql_guarded = match (query.status.as_str(), query.sql_guarded) {
        (status, Some(sql)) if status == QueryStatus::Ok.as_str() => sql,
        (status, _) if status == QueryStatus::Rejected.as_str() => {
            return Err(no_number("bị guard từ chối"));
        }
        _ => return Err(no_number("chạy lỗi")),
    };
    // check_request đã bảo đảm có đúng một trong cell, breakdown.
    let (sql, keys, slice_filter, only_measure) = match (&req.cell, &req.breakdown) {
        (Some(keys), _) => (sql_guarded, keys.clone(), None, None),
        (None, Some(key)) => {
            let (sql, filter) = find_breakdown(query.explanation, key)?;
            let keys = vec![DrillKey {
                column: key.column.clone(),
                value: key.value.clone(),
            }];
            // Câu chiều có độ đo của cả hai lát cắt; con số được bấm là độ đo của lát cắt đó.
            let measure = match key.slice {
                SliceName::Base => BASE_VALUE,
                SliceName::Compare => COMPARE_VALUE,
            };
            (sql, keys, Some(filter), Some(measure))
        }
        (None, None) => {
            return Err(AppError::Internal(
                "drill: no cell or breakdown".to_string(),
            ))
        }
    };
    let version_id = query.version_id.ok_or_else(version_gone)?;
    let dataset = state
        .store
        .get_dataset(ws, dataset_id)
        .await?
        .ok_or_else(dataset_not_found)?;
    let version = state
        .store
        .get_version(ws, dataset_id, version_id)
        .await?
        .ok_or_else(version_gone)?;
    // Cùng đường dựng bảng với luồng hỏi (ask/related.rs), trên phiên bản câu hỏi đã dùng: hộp lọc trong câu đã lưu có
    // thể nhắc tới bảng khác (điều kiện lan theo khóa ngoại của người đã hỏi, #215), nên các bảng liên quan và bảng cha
    // của điều kiện phải có trong danh sách, mỗi bảng với bộ lọc của người đang bấm. Câu nối bảng cũng cần chúng: mỗi
    // bảng câu đọc nhận bộ lọc của người đang bấm, và hồ sơ cột cho biết cột thuộc bảng nào (drill/sources.rs).
    let columns = state.store.list_columns(ws, version_id).await?;
    let picked = build_allowed_table(state, member, &dataset, &version, &columns).await?;
    let group = load_group(state, member, (&dataset, &version, &columns), picked).await?;
    // Hộp lọc của người đã hỏi có nửa nối theo khóa ngoại (`"customer_id" IN (SELECT "id" FROM "customers" ...)`), nên
    // guard cần quan hệ của nhóm để nhận lại câu đã lưu (luật nối bảng, #256).
    let links = build_join_links(&group);
    let mut tables = group.allowed;
    let parents = build_parent_tables(&tables);
    tables.extend(parents);
    let input = DrillInput {
        sql_guarded: &sql,
        keys: &keys,
        slice_filter: slice_filter.as_deref(),
        only_measure,
        offset: req.offset,
        limit: req.limit,
        tables: &tables,
        links: &links,
        timeout: state.config.query_timeout,
        jobs: &state.query_jobs,
    };
    run_drill(input).await
}

/// Đúng một trong cell, breakdown; cỡ trang trong khoảng cho phép.
fn check_request(req: &DrillRequest) -> Result<(), AppError> {
    if req.cell.is_some() == req.breakdown.is_some() {
        return Err(AppError::BadRequest(
            "Gửi đúng một trong hai: cell (ô của bảng kết quả) hoặc breakdown (dòng phân rã)"
                .to_string(),
        ));
    }
    if req.limit == 0 || req.limit > MAX_DRILL_LIMIT {
        return Err(AppError::BadRequest(format!(
            "limit phải từ 1 tới {MAX_DRILL_LIMIT}"
        )));
    }
    Ok(())
}

/// Phần của queries.explanation (khuôn ở ask/why.rs) mà truy dòng phân rã cần.
#[derive(Deserialize)]
struct StoredWhy {
    explanation: Option<StoredExplanation>,
}

#[derive(Deserialize)]
struct StoredExplanation {
    base: StoredSlice,
    compare: StoredSlice,
    dimensions: Vec<StoredDimension>,
}

#[derive(Deserialize)]
struct StoredSlice {
    filter: String,
}

#[derive(Deserialize)]
struct StoredDimension {
    column: String,
    sql: String,
}

/// Câu SQL của chiều được bấm và điều kiện của lát cắt được bấm. 400 khi câu hỏi không có phân rã hay không có chiều đó.
fn find_breakdown(
    stored: Option<serde_json::Value>,
    key: &BreakdownKey,
) -> Result<(String, String), AppError> {
    let no_breakdown = || {
        AppError::BadRequest(
            "Câu hỏi này không có bảng phân rã nên không có dòng phân rã để truy".to_string(),
        )
    };
    let stored: StoredWhy = match stored {
        Some(json) => serde_json::from_value(json)
            .map_err(|e| AppError::Internal(format!("drill: stored explanation: {e}")))?,
        None => return Err(no_breakdown()),
    };
    let explanation = stored.explanation.ok_or_else(no_breakdown)?;
    let slice = match key.slice {
        SliceName::Base => explanation.base,
        SliceName::Compare => explanation.compare,
    };
    let dimension = explanation
        .dimensions
        .into_iter()
        .find(|d| d.column == key.column)
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "Bảng phân rã của câu hỏi này không có chiều {}",
                key.column
            ))
        })?;
    Ok((dimension.sql, slice.filter))
}

/// Đầu vào của run_drill, đã đọc xong từ Postgres; test dựng trực tiếp, không cần Postgres.
pub(super) struct DrillInput<'a> {
    /// Câu đã chạy được truy: sql_guarded của câu hỏi, hay câu của chiều phân rã được bấm.
    pub sql_guarded: &'a str,
    pub keys: &'a [DrillKey],
    /// Điều kiện lát cắt khi bấm dòng phân rã (drill/sql.rs).
    pub slice_filter: Option<&'a str>,
    /// Tên cột độ đo của lát cắt được bấm, khi bấm dòng phân rã; None là mọi độ đo của ô.
    pub only_measure: Option<&'a str>,
    pub offset: u64,
    pub limit: u32,
    /// Bảng của dataset, row_filters theo người đang bấm.
    pub tables: &'a [AllowedTable],
    /// Quan hệ của nhóm bảng (ask/fanout.rs, build_join_links), cho luật nối bảng của guard.
    pub links: &'a [JoinLink],
    pub timeout: Duration,
    pub jobs: &'a Arc<Semaphore>,
}

pub(super) async fn run_drill(input: DrillInput<'_>) -> Result<DrillResponse, AppError> {
    let plan = build_drill_plan(input.sql_guarded)?;
    // Trần của guard bằng cỡ trang lớn nhất: LIMIT của trang nằm dưới trần nên guard giữ nguyên.
    let max_rows = u64::from(MAX_DRILL_LIMIT);
    // Câu gốc với LIMIT 0 chỉ để biết tên cột kết quả, như giao diện đã nhận; DuckDB không đọc dòng nào.
    let names_sql = guard(&plan.build_names_sql(), &input, max_rows)?;
    let names: Vec<String> = run(&input, names_sql.runnable)
        .await?
        .columns
        .into_iter()
        .map(|c| c.name)
        .collect();
    let built = plan.build_sql(&PageRequest {
        names: &names,
        keys: input.keys,
        slice_filter: input.slice_filter,
        only_measure: input.only_measure,
        offset: input.offset,
        limit: input.limit,
        tables: input.tables,
        links: input.links,
    })?;
    let page = guard(&built.page, &input, max_rows)?;
    let count = guard(&built.count, &input, max_rows)?;
    let rows = run(&input, page.runnable).await?;
    let (total_rows, values) = read_totals(run(&input, count.runnable).await?)?;
    // Trang đã gửi đủ dòng (cỡ trang tối đa bằng số dòng ResultTable gửi) và không bị guard cắt, nên trần đưa vào là
    // u64::MAX: limit_reached luôn false, tổng thật nằm ở DrillResponse.total_rows.
    let result = build_result_table(rows, u64::MAX);
    let (joined_columns, measures) = build_column_notes(&built.columns, &result, values);
    Ok(DrillResponse {
        sql: page.shown,
        result,
        total_rows,
        source_table: built.columns.source_table,
        joined_columns,
        measures,
    })
}

/// Tên cột bảng nối và độ đo trên trang. Cột thêm nằm cuối trang theo thứ tự của PageColumns, nên tên của chúng đọc
/// theo vị trí: tên cuối cùng có thể khác tên đã đặt khi trùng cột của bảng (build_unique_names thêm hậu tố).
fn build_column_notes(
    columns: &PageColumns,
    result: &ResultTable,
    values: Vec<Option<CellValue>>,
) -> (Vec<String>, Vec<DrillMeasure>) {
    let names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    let row_columns = columns.measures.iter().filter(|m| m.has_row_column).count();
    let measure_start = names.len().saturating_sub(row_columns);
    let joined_start = measure_start.saturating_sub(columns.joined_added);
    let mut joined: Vec<String> = columns
        .joined_named
        .iter()
        .filter(|name| names.contains(&name.as_str()))
        .cloned()
        .collect();
    joined.extend(
        names[joined_start..measure_start]
            .iter()
            .map(|n| n.to_string()),
    );
    let mut row_names = names[measure_start..].iter();
    let mut values = values.into_iter();
    let measures = columns
        .measures
        .iter()
        .map(|measure| DrillMeasure {
            column: measure.column.clone(),
            rule: measure.rule,
            value: values.next().flatten(),
            row_column: measure
                .has_row_column
                .then(|| row_names.next().map(|n| n.to_string()))
                .flatten(),
        })
        .collect();
    (joined, measures)
}

/// Câu do code dựng từ một câu đã qua guard, nên guard từ chối chỉ khi hồ sơ đổi từ lúc hỏi (bảng đổi tên, cột không
/// còn): báo như câu chưa truy được.
fn guard(sql: &str, input: &DrillInput<'_>, max_rows: u64) -> Result<GuardedSql, AppError> {
    match build_guarded_sql(sql, input.tables, input.links, max_rows) {
        Ok(guarded) => Ok(guarded),
        Err(GuardError::Internal(e)) => Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => Err(AppError::BadRequest(format!(
            "Câu hỏi này chưa truy được dòng nguồn: {reason}"
        ))),
    }
}

async fn run(input: &DrillInput<'_>, runnable: String) -> Result<QueryOutput, AppError> {
    match run_query_in_turn(input.jobs, runnable, input.timeout).await {
        Ok(output) => Ok(output),
        // Lỗi của DuckDB đã bỏ đường dẫn file trên máy chủ, như ở luồng hỏi.
        Err(QueryError::Sql(raw)) => Err(AppError::BadRequest(format!(
            "Không đọc được dòng nguồn: {}",
            build_safe_error(&raw, input.tables)
        ))),
        Err(QueryError::Timeout) => Err(AppError::BadRequest(format!(
            "Đọc dòng nguồn quá {} nên đã bị dừng",
            build_duration_text(input.timeout)
        ))),
        Err(QueryError::OutOfMemory) => Err(AppError::BadRequest(
            "Đọc dòng nguồn cần nhiều bộ nhớ hơn mức cho phép nên đã bị dừng".to_string(),
        )),
        Err(QueryError::Internal(e)) => Err(e),
    }
}

/// Một dòng: count(*) của DuckDB (BIGINT) rồi giá trị các độ đo, đổi sang ô như bảng kết quả.
fn read_totals(output: QueryOutput) -> Result<(u64, Vec<Option<CellValue>>), AppError> {
    let count = match output.rows.first().and_then(|row| row.first()) {
        Some(Value::BigInt(n)) => u64::try_from(*n)
            .map_err(|_| AppError::Internal(format!("drill: negative count {n}")))?,
        other => {
            return Err(AppError::Internal(format!(
                "drill: unexpected count {other:?}"
            )))
        }
    };
    let table = build_result_table(output, u64::MAX);
    let values = table
        .rows
        .into_iter()
        .next()
        .map(|row| row.into_iter().skip(1).collect())
        .unwrap_or_default();
    Ok((count, values))
}

fn query_not_found() -> AppError {
    AppError::NotFound("Không có câu hỏi này trong dataset".to_string())
}

fn version_gone() -> AppError {
    AppError::BadRequest(
        "Phiên bản dữ liệu của câu hỏi này đã bị xóa nên không truy được dòng nguồn".to_string(),
    )
}

fn no_number(reason: &str) -> AppError {
    AppError::BadRequest(format!(
        "Câu hỏi này {reason} nên không có con số để truy dòng nguồn"
    ))
}
