// SPDX-License-Identifier: Apache-2.0
/* service/sources/snapshot.rs - chụp bảng của nguồn Postgres: mỗi bảng thành một dataset riêng (một phiên bản Parquet
như nạp file), cùng các quan hệ khóa ngoại giữa chúng (engine/store/relations.rs). api/sources.rs gọi `snapshot` và
`resnapshot`; lịch chụp lại (schedule.rs) gọi `resnapshot`. Chép bảng ở copy.rs, nạp ở service/ingest.rs.

Các bước:
1. Bảng nào (#247): mặc định mọi bảng, view tài khoản đọc được, nên phạm vi dữ liệu là quyền SELECT của tài khoản; admin
   gửi `tables` khi bỏ tick bớt. Không giới hạn số bảng; trần là SOURCE_MAX_ROWS từng bảng và SOURCE_SNAPSHOT_MAX_MB
   cả lần (plan.rs). Chụp lại cả nguồn lấy mọi bảng nguồn đang giữ: bảng đã thành dataset và bảng chỉ có metadata.
   Chụp lại một nhóm (dataset_id) lấy cả nhóm dataset nối với nhau qua quan hệ, cùng nguồn.
2. Chép trong một giao dịch (copy.rs). Bảng quá trần không chép dòng nào: cột, kiểu, khóa, mô tả ghi vào
   source_metadata_tables (migration 0018), để câu hỏi cần bảng đó được báo đúng tên bảng.
3. Nạp từng bảng. Bảng đã từng chụp từ nguồn này thì thêm phiên bản vào đúng dataset đó (một bảng nguồn, một dataset),
   nên bảng chiều dùng chung bởi hai bảng sự kiện không bị nhân đôi. Bảng không đổi có cùng hash nên không tạo phiên
   bản mới.
4. Ghi quan hệ: mọi khóa ngoại khai báo ở database nguồn giữa hai bảng nguồn đang giữ thành dataset, ít nhất một đầu
   là bảng vừa chụp; trong một giao dịch. Không đoán quan hệ từ tên cột.

Lỗi giữa bước 3: các bảng đã nạp trước đó giữ phiên bản mới, quan hệ của lần này chưa ghi. Chụp lại là đủ: bảng không
đổi không tạo phiên bản trùng, chỉ bảng lỗi và quan hệ được ghi. */

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::copy::{copy_group, CopiedTable, Selection, SkippedTable, Wanted};
use super::plan::SkipCause;
use super::postgres::ConnectInfo;
use super::relations::{self, ForeignKey};
use super::{load, recorded_table, source_file_label};
use crate::auth::Member;
use crate::engine::store::{
    DatasetRelation, MetadataColumn, NewMetadataTable, NewRelation, SourceRow,
};
use crate::service::ingest::{self, SourceDescriptions, StagedFile, Upload, UploadResult};
use crate::{error::AppError, state::AppState};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SnapshotRequest {
    /// `schema.table` named `name` when it is new. Optional: without `table`, `tables` and `dataset_id`, every table
    /// and view the account can read is snapshotted.
    pub table: Option<String>,
    /// `schema.table` names to snapshot (the ticked tables), each into its own dataset. Empty with no `table` and no
    /// `dataset_id`: every readable table. One name snapshots that one table, for example a table kept as metadata
    /// only. Not allowed with `dataset_id`.
    #[serde(default)]
    pub tables: Vec<String>,
    /// Name of the dataset of `table` when it is new; defaults to the table name. Other tables use their table name.
    pub name: Option<String>,
    /// Snapshot this dataset again, with every dataset of the same source linked to it by relations.
    pub dataset_id: Option<Uuid>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SnapshotResponse {
    /// One entry per table copied. `created` is false for a table whose content did not change.
    pub datasets: Vec<UploadResult>,
    /// Foreign-key relations recorded for these datasets (both ends snapshotted from this source).
    pub relations: Vec<DatasetRelation>,
    /// Tables over SOURCE_MAX_ROWS or the SOURCE_SNAPSHOT_MAX_MB total: no rows copied, metadata kept.
    pub metadata_only: Vec<MetadataOnlyTable>,
    /// Tables this source held that the account can no longer read (re-snapshot only); their datasets keep their
    /// last version.
    pub unreadable: Vec<String>,
}

/// A table of the source with columns, types, keys and comments but no rows.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MetadataOnlyTable {
    /// `schema.table`.
    pub table: String,
    pub cause: SkipCause,
    /// Why, in Vietnamese, with the setting to change.
    pub reason: String,
    /// Rows counted, or estimated by Postgres when far over SOURCE_MAX_ROWS.
    #[schema(required)]
    pub row_count: Option<i64>,
    /// Size reported by Postgres; null for views.
    #[schema(required)]
    pub size_bytes: Option<i64>,
    /// `COMMENT ON TABLE`, cleaned; null when none.
    #[schema(required)]
    pub description: Option<String>,
    pub columns: Vec<MetadataColumn>,
    /// Dataset of an earlier snapshot of this table, which keeps its last version; null when never snapshotted.
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
}

pub async fn snapshot(
    state: &AppState,
    member: &Member,
    id: Uuid,
    req: SnapshotRequest,
) -> Result<SnapshotResponse, AppError> {
    let (row, info) = load(state, member, id).await?;
    let selection = build_selection(state, member, &row, &req).await?;
    // Tên tự đặt chỉ cho bảng `table`, không cho bảng đầu tiên chép được: bảng đó có thể chỉ lưu metadata.
    let named = match (req.table.as_deref().map(str::trim), req.name) {
        (Some(table), Some(name)) if !table.is_empty() => Some((table.to_string(), name)),
        _ => None,
    };
    run_snapshot(state, member, &row, &info, selection, named).await
}

/// Chụp lại mọi bảng nguồn đang giữ (#247): bảng đã thành dataset và bảng chỉ có metadata, trong một giao dịch.
pub async fn resnapshot(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<SnapshotResponse, AppError> {
    let (row, info) = load(state, member, id).await?;
    let ws = &member.workspace;
    let mut wanted: Vec<Wanted> = state
        .store
        .list_source_datasets(ws, row.id)
        .await?
        .into_iter()
        .map(|(dataset_id, table)| Wanted {
            table,
            dataset_id: Some(dataset_id),
        })
        .collect();
    for m in state.store.list_metadata_tables(ws, row.id).await? {
        if !wanted.iter().any(|w| w.table == m.table_name) {
            wanted.push(Wanted {
                table: m.table_name,
                dataset_id: None,
            });
        }
    }
    if wanted.is_empty() {
        return Err(AppError::BadRequest(
            "Nguồn này chưa có bảng nào để chụp lại: chụp các bảng trước".to_string(),
        ));
    }
    let selection = Selection::Tables {
        wanted,
        skip_unreadable: true,
    };
    run_snapshot(state, member, &row, &info, selection, None).await
}

/// Bước 2 đến 4. named: (bảng, tên dataset) khi người gọi đặt tên cho một bảng mới.
async fn run_snapshot(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    info: &ConnectInfo,
    selection: Selection,
    named: Option<(String, String)>,
) -> Result<SnapshotResponse, AppError> {
    let group = copy_group(state, info, selection).await?;
    let ws = &member.workspace;

    let mut results = Vec::new();
    let mut run_tables = Vec::new();
    for copied in group.copied {
        let full_name = copied.table.full_name();
        let name = named
            .as_ref()
            .filter(|(table, _)| *table == full_name)
            .map(|(_, name)| name.clone());
        let result = ingest_table(state, member, row, info, copied, name).await?;
        // Bảng từng chỉ có metadata nay đã có dữ liệu.
        state
            .store
            .delete_metadata_table(ws, row.id, &full_name)
            .await?;
        run_tables.push(full_name);
        results.push(result);
    }
    let metadata_only = record_metadata_only(state, member, row, group.skipped).await?;
    let ids: Vec<Uuid> = results.iter().map(|r| r.dataset.id).collect();
    let held = state.store.list_source_datasets(ws, row.id).await?;
    let new_relations = build_run_relations(group.keys, &held, &run_tables);
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
        .filter(|r| ids.contains(&r.from_dataset_id) || ids.contains(&r.to_dataset_id))
        .collect();
    Ok(SnapshotResponse {
        datasets: out,
        relations,
        metadata_only,
        unreadable: group.unreadable,
    })
}

/// Bảng nào cần chụp, theo request.
async fn build_selection(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    req: &SnapshotRequest,
) -> Result<Selection, AppError> {
    if let Some(dataset_id) = req.dataset_id {
        // Chụp lại một nhóm: bảng lấy từ lần chụp trước, để không chụp nhầm bảng khác vào cùng dataset.
        if !req.tables.is_empty() {
            return Err(AppError::BadRequest(
                "Chụp lại dùng đúng nhóm bảng đã chụp; muốn thêm bảng thì gửi danh sách bảng, không kèm dataset_id"
                    .to_string(),
            ));
        }
        let wanted = build_group(state, member, row, dataset_id).await?;
        return Ok(Selection::Tables {
            wanted,
            skip_unreadable: false,
        });
    }
    let mut names: Vec<String> = Vec::new();
    for t in req.table.iter().chain(&req.tables) {
        let t = t.trim();
        if !t.is_empty() && !names.iter().any(|n| n == t) {
            names.push(t.to_string());
        }
    }
    if names.is_empty() {
        return Ok(Selection::All);
    }
    let wanted = names
        .into_iter()
        .map(|table| Wanted {
            table,
            dataset_id: None,
        })
        .collect();
    Ok(Selection::Tables {
        wanted,
        skip_unreadable: false,
    })
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

/// Ghi metadata của bảng không chép được. Bảng đã có dataset từ lần chụp trước thì không ghi: dataset giữ phiên bản
/// cũ cùng hồ sơ cột của nó, câu hỏi vẫn trả lời trên phiên bản đó.
async fn record_metadata_only(
    state: &AppState,
    member: &Member,
    row: &SourceRow,
    skipped: Vec<SkippedTable>,
) -> Result<Vec<MetadataOnlyTable>, AppError> {
    let ws = &member.workspace;
    let mut out = Vec::new();
    let mut rows = Vec::new();
    for s in skipped {
        let table = s.skipped.candidate.table.full_name();
        let dataset_id = match s.dataset_id {
            Some(id) => Some(id),
            None => state.store.find_source_dataset(ws, row.id, &table).await?,
        };
        let mut reason = s.skipped.reason.clone();
        if dataset_id.is_some() {
            reason.push_str(". Dataset của bảng giữ phiên bản đã chụp lần trước");
        }
        let item = MetadataOnlyTable {
            table: table.clone(),
            cause: s.skipped.cause,
            reason,
            row_count: Some(s.skipped.candidate.rows.value()),
            size_bytes: s.skipped.candidate.size_bytes,
            description: s.description,
            columns: s.columns,
            dataset_id,
        };
        if dataset_id.is_none() {
            rows.push(NewMetadataTable {
                table_name: table,
                cause: item.cause.as_str(),
                reason: item.reason.clone(),
                row_count: item.row_count,
                size_bytes: item.size_bytes,
                description: item.description.clone(),
                columns: item.columns.clone(),
            });
        }
        out.push(item);
    }
    state
        .store
        .upsert_metadata_tables(ws, row.id, &rows)
        .await?;
    Ok(out)
}

/// Quan hệ cần ghi sau lần chụp: khóa ngoại giữa hai bảng nguồn đang giữ, ít nhất một đầu vừa chụp (relations.rs).
fn build_run_relations(
    keys: Vec<ForeignKey>,
    held: &[(Uuid, String)],
    run_tables: &[String],
) -> Vec<NewRelation> {
    let held_names: Vec<String> = held.iter().map(|(_, t)| t.clone()).collect();
    let kept = relations::build_group_keys(keys, &held_names, run_tables);
    build_new_relations(&kept, held)
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
) -> Result<UploadResult, AppError> {
    let full_name = copied.table.full_name();
    let dataset_id = match copied.dataset_id {
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
    // không bị coi là trùng phiên bản đã có ở dataset khác. Comment cũng vào hash (build_comment_part).
    let hashed = format!(
        "{label}\n{}{}",
        copied.copy.sha256,
        build_comment_part(&copied.descriptions)
    );
    let sha256 = Sha256::digest(hashed.as_bytes())
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
        descriptions: copied.descriptions,
    };
    let result = ingest::upload(state, member, copied.staging, upload).await?;
    state
        .store
        .record_source_table(&member.workspace, row.id, result.dataset.id, &full_name)
        .await?;
    Ok(result)
}

/// Phần comment trong hash của bản chụp: chỉ đổi comment mà dữ liệu giữ nguyên thì chụp lại vẫn ra phiên bản mới mang
/// comment mới (comment nằm trên phiên bản). Bảng không có comment nào thì chuỗi rỗng, nên hash giữ như trước #245 và
/// chụp lại bảng không đổi không tạo phiên bản thừa.
pub(super) fn build_comment_part(descriptions: &SourceDescriptions) -> String {
    if descriptions.table.is_none() && descriptions.columns.is_empty() {
        return String::new();
    }
    let mut part = format!("\ncomments:{:?}", descriptions.table);
    for (name, text) in &descriptions.columns {
        part.push_str(&format!("\n{name:?}={text:?}"));
    }
    part
}

/// Khóa ngoại đổi bảng thành dataset: datasets là (dataset, schema.bảng) của các bảng nguồn đang giữ.
pub(super) fn build_new_relations(
    keys: &[ForeignKey],
    datasets: &[(Uuid, String)],
) -> Vec<NewRelation> {
    let dataset_of = |t: &super::postgres::SourceTable| {
        let full = t.full_name();
        datasets.iter().find(|(_, d)| *d == full).map(|(id, _)| *id)
    };
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
