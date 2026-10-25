// SPDX-License-Identifier: Apache-2.0
/* service/drill.rs - truy dòng nguồn của một con số (#34): đọc câu hỏi đã lưu (queries) theo workspace và query_id,
dựng câu đọc dòng nguồn từ sql_guarded (drill_sql.rs; tên cột kết quả lấy từ một lần chạy câu gốc với LIMIT 0), cho qua guard với quyền của người đang bấm, chạy trên đúng phiên
bản dữ liệu câu hỏi đã dùng, rồi trả một trang dòng cùng tổng số dòng. Hợp đồng ở drill_contract.rs, route ở
api/query.rs. Test ở drill_tests.rs (không cần Postgres) và api/drill_tests.rs (qua router).

Quyền của người đang bấm, không của người đã hỏi: câu do admin hỏi, thành viên có vai dữ liệu bấm vào thì chỉ thấy dòng
của vai đó. Chạy trên version_id đã lưu: nạp phiên bản mới sau đó không làm dòng nguồn lệch con số đã trả.

Bấm dòng phân rã cần điều kiện hai lát cắt và bộ lọc của form lưu cùng câu hỏi. Cột queries.explanation đã có (migration
0006) nhưng chỉ được ghi khi ask gọi run_why cho câu "tại sao" (#38), nên tới đó trả 400. */

use std::sync::Arc;
use std::time::Duration;

use duckdb::types::Value;
use tokio::sync::Semaphore;

use super::ask_repair::build_safe_error;
use super::ask_result::build_result_table;
use super::ask_run::{build_duration_text, run_query_in_turn};
use super::drill_contract::{DrillKey, DrillRequest, DrillResponse, MAX_DRILL_LIMIT};
use super::drill_sql::build_drill_plan;
use super::ingest::{dataset_not_found, parse_dataset_id};
use crate::auth::{Member, Role};
use crate::engine::guard::{build_guarded_sql, AllowedTable, GuardError, GuardedSql};
use crate::engine::query::{QueryError, QueryOutput};
use crate::engine::store::QueryStatus;
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
    let Some(keys) = &req.cell else {
        return Err(AppError::BadRequest(
            "Chưa truy được dòng nguồn của dòng phân rã: lát cắt của câu \"tại sao\" chưa được lưu cùng câu hỏi"
                .to_string(),
        ));
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
    // Phiên bản nạp từ file luôn có đường dẫn; nguồn database (sau này) thì không.
    let relative = version
        .parquet_path
        .ok_or_else(|| AppError::Internal("version has no parquet path".to_string()))?;
    let columns = state.store.list_columns(ws, version_id).await?;
    // Admin không bị lọc: họ tự sửa được chính sách, lọc với họ không bảo vệ được gì (#45).
    let row_filters = match member.role {
        Role::Admin => Vec::new(),
        Role::Member => {
            state
                .store
                .list_row_filters(ws, dataset_id, member.user.id)
                .await?
        }
    };
    let tables = [AllowedTable {
        name: dataset.logical_table,
        columns: columns.into_iter().map(|c| c.name).collect(),
        parquet_dir: state.config.data_dir.join(relative),
        row_filters,
    }];
    let input = DrillInput {
        sql_guarded: &sql_guarded,
        keys,
        offset: req.offset,
        limit: req.limit,
        tables: &tables,
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

/// Đầu vào của run_drill, đã đọc xong từ Postgres; test dựng trực tiếp, không cần Postgres.
pub(super) struct DrillInput<'a> {
    pub sql_guarded: &'a str,
    pub keys: &'a [DrillKey],
    pub offset: u64,
    pub limit: u32,
    /// Bảng của dataset, row_filters theo người đang bấm.
    pub tables: &'a [AllowedTable],
    pub timeout: Duration,
    pub jobs: &'a Arc<Semaphore>,
}

pub(super) async fn run_drill(input: DrillInput<'_>) -> Result<DrillResponse, AppError> {
    let plan = build_drill_plan(input.sql_guarded)?;
    // Trần của guard bằng cỡ trang lớn nhất: LIMIT của trang nằm dưới trần nên guard giữ nguyên.
    let max_rows = u64::from(MAX_DRILL_LIMIT);
    // Câu gốc với LIMIT 0 chỉ để biết tên cột kết quả, như giao diện đã nhận; DuckDB không đọc dòng nào.
    let names_sql = guard(&plan.build_names_sql(), input.tables, max_rows)?;
    let names: Vec<String> = run(&input, names_sql.runnable)
        .await?
        .columns
        .into_iter()
        .map(|c| c.name)
        .collect();
    let built = plan.build_sql(&names, input.keys, input.offset, input.limit)?;
    let page = guard(&built.page, input.tables, max_rows)?;
    let count = guard(&built.count, input.tables, max_rows)?;
    let rows = run(&input, page.runnable).await?;
    let total_rows = read_count(run(&input, count.runnable).await?)?;
    // Trang đã gửi đủ dòng (cỡ trang tối đa bằng số dòng ResultTable gửi) và không bị guard cắt, nên trần đưa vào là
    // u64::MAX: limit_reached luôn false, tổng thật nằm ở DrillResponse.total_rows.
    let result = build_result_table(rows, u64::MAX);
    Ok(DrillResponse {
        sql: page.shown,
        result,
        total_rows,
    })
}

/// Câu do code dựng từ một câu đã qua guard, nên guard từ chối chỉ khi hồ sơ đổi từ lúc hỏi (bảng đổi tên, cột không
/// còn): báo như câu chưa truy được.
fn guard(sql: &str, tables: &[AllowedTable], max_rows: u64) -> Result<GuardedSql, AppError> {
    match build_guarded_sql(sql, tables, max_rows) {
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

/// count(*) của DuckDB là BIGINT, một dòng một cột.
fn read_count(output: QueryOutput) -> Result<u64, AppError> {
    match output.rows.first().and_then(|row| row.first()) {
        Some(Value::BigInt(n)) => {
            u64::try_from(*n).map_err(|_| AppError::Internal(format!("drill: negative count {n}")))
        }
        other => Err(AppError::Internal(format!(
            "drill: unexpected count {other:?}"
        ))),
    }
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
