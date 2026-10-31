// SPDX-License-Identifier: Apache-2.0
/* service/sources/snapshot.rs - chụp một nhóm bảng của nguồn Postgres: mỗi bảng thành một dataset riêng (một phiên bản
Parquet như nạp file), cùng các quan hệ khóa ngoại giữa chúng (engine/store/relations.rs). api/sources.rs gọi
`snapshot`; chép bảng ở postgres.rs, tìm khóa ngoại ở relations.rs, nạp ở service/ingest.rs.

Các bước:
1. Danh sách bảng: bảng bắt đầu và các bảng chọn kèm, tối đa MAX_SNAPSHOT_TABLES. Chụp lại (dataset_id) lấy cả nhóm
   dataset nối với nhau qua quan hệ, cùng nguồn.
2. Chép: một kết nối, một giao dịch REPEATABLE READ. Đếm dòng mọi bảng trước (giới hạn SOURCE_MAX_ROWS), rồi chép mỗi
   bảng ra file riêng (giới hạn MAX_UPLOAD_MB), kèm kiểu cột Postgres: COPY ra CSV trong chính giao dịch đó, hoặc với
   SOURCE_SNAPSHOT_MODE=direct thì DuckDB đọc thẳng ra Parquet trong một giao dịch REPEATABLE READ riêng của nó
   (direct.rs). Cách nào thì mọi bảng cũng là ảnh của một thời điểm. Cả bước chung một giới hạn SOURCE_TIMEOUT_SECS.
3. Nạp từng bảng. Bảng đã từng chụp từ nguồn này thì thêm phiên bản vào đúng dataset đó (một bảng nguồn, một dataset),
   nên bảng chiều dùng chung bởi hai bảng sự kiện không bị nhân đôi.
4. Ghi quan hệ giữa các bảng trong nhóm, trong một giao dịch.

Lỗi giữa bước 3: các bảng đã nạp trước đó giữ phiên bản mới, quan hệ của lần này chưa ghi. Chụp lại là đủ: bảng không
đổi có cùng hash nội dung nên không tạo phiên bản trùng, chỉ bảng lỗi và quan hệ được ghi. */

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::direct::{self, DirectError, DirectTable};
use super::postgres::{self, ConnectInfo, SourceTable, TableCopy};
use super::relations::{self, ForeignKey};
use super::{find_table_in, load, pg_types, recorded_table, source_file_label};
use crate::auth::Member;
use crate::config::SnapshotMode;
use crate::engine::store::{DatasetRelation, NewRelation, SourceRow};
use crate::service::ingest::{self, StagedFile, Staging, Upload, UploadResult};
use crate::{error::AppError, state::AppState};

/// Số bảng tối đa của một lần chụp: mỗi bảng giữ một file tạm tới MAX_UPLOAD_MB trên đĩa cho tới khi nạp xong.
pub const MAX_SNAPSHOT_TABLES: usize = 10;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SnapshotRequest {
    /// `schema.table` to start from. Not needed with `dataset_id`.
    pub table: Option<String>,
    /// More `schema.table` names snapshotted with `table`, usually picked from its relation list. Each table becomes
    /// its own dataset. Not allowed with `dataset_id`.
    #[serde(default)]
    pub tables: Vec<String>,
    /// Name of the dataset of `table` when it is new; defaults to the table name. Other tables use their table name.
    pub name: Option<String>,
    /// Snapshot this dataset again, with every dataset of the same source linked to it by relations.
    pub dataset_id: Option<Uuid>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SnapshotResponse {
    /// One entry per table, the starting table first. `created` is false for a table whose content did not change.
    pub datasets: Vec<UploadResult>,
    /// Foreign-key relations among these datasets, as recorded.
    pub relations: Vec<DatasetRelation>,
}

/// Một bảng cần chụp; dataset đã biết khi chụp lại cả nhóm.
struct Wanted {
    table: String,
    dataset_id: Option<Uuid>,
}

/// Một bảng đã chép ra file tạm, chờ nạp. Staging giữ thư mục tạm tới khi ingest::upload nhận nó.
struct CopiedTable {
    table: SourceTable,
    staging: Staging,
    path: PathBuf,
    copy: TableCopy,
}

pub async fn snapshot(
    state: &AppState,
    member: &Member,
    id: Uuid,
    req: SnapshotRequest,
) -> Result<SnapshotResponse, AppError> {
    let (row, info) = load(state, member, id).await?;
    let wanted = build_wanted(state, member, &row, &req).await?;
    let (copied, keys) = copy_group(state, &info, &wanted).await?;

    let mut results = Vec::new();
    let mut datasets: Vec<(SourceTable, Uuid)> = Vec::new();
    for (index, (copied, want)) in copied.into_iter().zip(&wanted).enumerate() {
        let name = if index == 0 { req.name.clone() } else { None };
        let table = copied.table.clone();
        let result =
            ingest_table(state, member, &row, &info, copied, name, want.dataset_id).await?;
        datasets.push((table, result.dataset.id));
        results.push(result);
    }
    let ids: Vec<Uuid> = datasets.iter().map(|(_, id)| *id).collect();
    let new_relations = build_new_relations(&keys, &datasets);
    let ws = &member.workspace;
    state
        .store
        .replace_relations(ws, &ids, &new_relations)
        .await?;

    // Dataset trả về đọc trước khi ghi bảng nguồn và quan hệ; đọc lại để giao diện thấy ngay bảng và quan hệ.
    let mut out = Vec::new();
    for result in results {
        let dataset = ingest::dataset_detail(state, member, &result.dataset.id.to_string()).await?;
        out.push(UploadResult { dataset, ..result });
    }
    let relations = state
        .store
        .list_relations(ws, None)
        .await?
        .into_iter()
        .filter(|r| ids.contains(&r.from_dataset_id) && ids.contains(&r.to_dataset_id))
        .collect();
    Ok(SnapshotResponse {
        datasets: out,
        relations,
    })
}

/// Bảng cần chụp, bảng bắt đầu đứng đầu, không trùng.
async fn build_wanted(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    req: &SnapshotRequest,
) -> Result<Vec<Wanted>, AppError> {
    let wanted = match req.dataset_id {
        // Chụp lại: bảng lấy từ lần chụp trước, để không chụp nhầm bảng khác vào cùng dataset.
        Some(dataset_id) => {
            if !req.tables.is_empty() {
                return Err(AppError::BadRequest(
                    "Chụp lại dùng đúng nhóm bảng đã chụp; muốn thêm bảng thì chụp từ bảng đó và chọn bảng kèm"
                        .to_string(),
                ));
            }
            build_group(state, member, row, dataset_id).await?
        }
        None => {
            let start = req.table.as_deref().map(str::trim).unwrap_or_default();
            if start.is_empty() {
                return Err(AppError::BadRequest("Chọn một bảng để chụp".to_string()));
            }
            let mut names: Vec<String> = vec![start.to_string()];
            for t in &req.tables {
                let t = t.trim();
                if !t.is_empty() && !names.iter().any(|n| n == t) {
                    names.push(t.to_string());
                }
            }
            names
                .into_iter()
                .map(|table| Wanted {
                    table,
                    dataset_id: None,
                })
                .collect()
        }
    };
    if wanted.len() > MAX_SNAPSHOT_TABLES {
        return Err(AppError::BadRequest(format!(
            "Một lần chụp tối đa {MAX_SNAPSHOT_TABLES} bảng, đang chọn {}: bỏ bớt bảng rồi chụp lại",
            wanted.len()
        )));
    }
    Ok(wanted)
}

/// Dataset `start` và mọi dataset cùng nguồn nối với nó qua quan hệ (đi hết các bước), kèm bảng nguồn của từng cái.
async fn build_group(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    start: Uuid,
) -> Result<Vec<Wanted>, AppError> {
    let mut group = vec![Wanted {
        table: recorded_table(row, start)?,
        dataset_id: Some(start),
    }];
    let mut queue = vec![start];
    while let Some(id) = queue.pop() {
        for r in state
            .store
            .list_relations(&member.workspace, Some(id))
            .await?
        {
            for other in [r.from_dataset_id, r.to_dataset_id] {
                let known = group.iter().any(|w| w.dataset_id == Some(other));
                // Dataset của nguồn khác (hay file nạp) không có bảng ghi trong nguồn này: không chụp lại nó ở đây.
                if let (false, Ok(table)) = (known, recorded_table(row, other)) {
                    group.push(Wanted {
                        table,
                        dataset_id: Some(other),
                    });
                    queue.push(other);
                }
            }
        }
    }
    Ok(group)
}

/// Bước 2: chép mọi bảng trong một giao dịch, rồi đọc khóa ngoại giữa chúng trong cùng giao dịch đó.
async fn copy_group(
    state: &AppState,
    info: &ConnectInfo,
    wanted: &[Wanted],
) -> Result<(Vec<CopiedTable>, Vec<ForeignKey>), AppError> {
    let limit = state.config.source_timeout;
    let max_rows = state.config.source_max_rows;
    let copy = async {
        let mut conn = postgres::connect(info, limit).await?;
        let readable = postgres::list_tables(&mut conn).await?;
        let mut tables = Vec::new();
        for w in wanted {
            tables.push(find_table_in(&readable, &w.table)?);
        }
        postgres::begin_snapshot(&mut conn).await?;
        // Đếm hết các bảng trước khi chép bảng nào: một bảng quá trần thì từ chối ngay, không chép phí các bảng khác.
        for table in &tables {
            let rows = postgres::count_rows(&mut conn, table).await?;
            check_row_count(table, rows, max_rows)?;
        }
        let copied = copy_tables(state, info, &mut conn, &tables).await?;
        let mut keys = Vec::new();
        for table in &tables {
            keys.extend(relations::list_foreign_keys(&mut conn, table).await?);
        }
        postgres::commit_snapshot(&mut conn).await?;
        Ok::<_, AppError>((copied, relations::build_group_keys(keys, &tables)))
    };
    match tokio::time::timeout(limit, copy).await {
        Ok(result) => result,
        Err(_) => Err(AppError::BadRequest(
            "Chụp bảng quá thời gian cho phép (SOURCE_TIMEOUT_SECS): chọn ít bảng hơn, bảng nhỏ hơn, hoặc tăng giới hạn"
                .to_string(),
        )),
    }
}

/// Chép dữ liệu các bảng theo SOURCE_SNAPSHOT_MODE. direct không dùng được (thiếu file extension, lệch phiên bản,
/// libpq không kết nối được) thì ghi log lý do và chép bằng COPY, trong cùng giao dịch của conn.
async fn copy_tables(
    state: &AppState,
    info: &ConnectInfo,
    conn: &mut sqlx::PgConnection,
    tables: &[SourceTable],
) -> Result<Vec<CopiedTable>, AppError> {
    if state.config.source_snapshot_mode == SnapshotMode::Direct {
        match copy_direct(state, info, conn, tables).await {
            Ok(copied) => return Ok(copied),
            Err(DirectError::Failed(e)) => return Err(e),
            Err(DirectError::Unavailable(reason)) => tracing::warn!(
                reason = %reason,
                "SOURCE_SNAPSHOT_MODE=direct is unavailable, copying tables with COPY instead"
            ),
        }
    }
    let mut copied = Vec::new();
    for table in tables {
        let staging = Staging::new(&state.config.data_dir)?;
        let path = staging.upload_path(0);
        let copy = postgres::copy_to_csv(
            conn,
            table,
            state.config.source_max_rows,
            state.config.max_upload_bytes,
            &path,
        )
        .await?;
        copied.push(CopiedTable {
            table: table.clone(),
            staging,
            path,
            copy,
        });
    }
    Ok(copied)
}

/// Cách direct (direct.rs): kiểu cột vẫn đọc bằng sqlx từ catalog, DuckDB chép dữ liệu.
async fn copy_direct(
    state: &AppState,
    info: &ConnectInfo,
    conn: &mut sqlx::PgConnection,
    tables: &[SourceTable],
) -> Result<Vec<CopiedTable>, DirectError> {
    let mut stagings = Vec::new();
    let mut wanted = Vec::new();
    for table in tables {
        let column_types = pg_types::column_types(conn, &postgres::select_all(table))
            .await
            .map_err(DirectError::Failed)?;
        let staging = Staging::new(&state.config.data_dir).map_err(DirectError::Failed)?;
        wanted.push(DirectTable {
            table: table.clone(),
            column_types,
            out: staging.upload_path(0),
        });
        stagings.push(staging);
    }
    let copies = direct::copy_tables(&state.config, info, wanted).await?;
    Ok(tables
        .iter()
        .zip(stagings)
        .zip(copies)
        .map(|((table, staging), copy)| CopiedTable {
            table: table.clone(),
            path: staging.upload_path(0),
            staging,
            copy,
        })
        .collect())
}

/// Bước 3 cho một bảng: nạp file đã chép thành phiên bản mới của dataset của bảng (hay dataset mới), rồi ghi bảng nguồn
/// của dataset vào nguồn.
async fn ingest_table(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    info: &ConnectInfo,
    copied: CopiedTable,
    name: Option<String>,
    dataset_id: Option<Uuid>,
) -> Result<UploadResult, AppError> {
    let full_name = copied.table.full_name();
    let dataset_id = match dataset_id {
        Some(id) => Some(id),
        None => {
            state
                .store
                .find_source_dataset(&member.workspace, row.id, &full_name)
                .await?
        }
    };
    let label = source_file_label(info, &full_name);
    // Băm cả nhãn nguồn lẫn nội dung: cùng nội dung mà từ bảng khác hay từ một file nạp tay thì vẫn là dữ liệu khác,
    // không bị coi là trùng phiên bản đã có ở dataset khác.
    let sha256 = Sha256::digest(format!("{label}\n{}", copied.copy.sha256).as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let upload = Upload {
        name: Some(name.unwrap_or_else(|| copied.table.name.clone())),
        dataset_id: dataset_id.map(|d| d.to_string()),
        files: vec![StagedFile {
            path: copied.path,
            name: label,
            sha256,
            column_types: copied.copy.column_types,
        }],
        sheets: Vec::new(),
        database_source: Some(row.id),
        format: Some(copied.copy.format),
    };
    let result = ingest::upload(state, member, copied.staging, upload).await?;
    state
        .store
        .record_source_table(&member.workspace, row.id, result.dataset.id, &full_name)
        .await?;
    Ok(result)
}

/// Khóa ngoại giữa các bảng của nhóm, đổi bảng thành dataset vừa nạp.
pub(super) fn build_new_relations(
    keys: &[ForeignKey],
    datasets: &[(SourceTable, Uuid)],
) -> Vec<NewRelation> {
    let dataset_of = |t: &SourceTable| datasets.iter().find(|(d, _)| d.same(t)).map(|(_, id)| *id);
    keys.iter()
        .filter_map(|k| {
            Some(NewRelation {
                from_dataset_id: dataset_of(&k.from)?,
                from_columns: k.from_columns.clone(),
                to_dataset_id: dataset_of(&k.to)?,
                to_columns: k.to_columns.clone(),
                constraint_name: k.constraint.clone(),
                cardinality: k.cardinality(),
            })
        })
        .collect()
}

/// Bảng quá SOURCE_MAX_ROWS thì từ chối cả lần chụp, trước khi chép: chép thiếu dòng thì mọi phép cộng, đếm sai mà
/// không ai thấy.
pub(super) fn check_row_count(
    table: &SourceTable,
    rows: i64,
    max_rows: u64,
) -> Result<(), AppError> {
    if u64::try_from(rows).unwrap_or(0) > max_rows {
        return Err(AppError::TooLarge(format!(
            "Bảng {} có {rows} dòng, quá giới hạn {max_rows} dòng của một bảng trong một lần chụp (SOURCE_MAX_ROWS)",
            table.full_name()
        )));
    }
    Ok(())
}
