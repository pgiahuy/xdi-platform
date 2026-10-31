// SPDX-License-Identifier: Apache-2.0
/* service/catalog/mod.rs - dataset của workspace (#257): một database đã kết nối hay một file, bảng nằm bên trong
(contract.rs). api/catalog.rs gọi các hàm ở đây. Dựng từ ba phần đã có: dòng sources (engine/store/catalog.rs), bảng
và phiên bản hiện hành (service/ingest.rs, list_datasets), quan hệ giữa các bảng (engine/store/relations.rs).

Việc trên một dataset đi qua phần đã có cho bảng: chụp lại cả dataset ở service/sources/ (mọi bảng trong một giao
dịch), hỏi trên dataset ở service/conversation/ (chọn bảng bắt đầu ở service/ask/scope.rs).

Mọi thành viên đọc được dataset như đọc được bảng; cấu hình kết nối chỉ có ở /sources, cho admin. */

pub mod contract;

use std::collections::HashMap;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{
    CatalogColumnRow, CatalogSourceRow, Dataset, DatasetRelation, MetadataTableRow,
};
use crate::service::ask::contract::{AskResponse, ChosenOption};
use crate::service::catalog::contract::{DatasetKind, DatasetSummary, DatasetTable, TableColumn};
use crate::service::conversation::contract::ConversationListResponse;
use crate::service::conversation::{self, DatasetScope};
use crate::service::ingest;
use crate::service::sources::{self, MetadataOnlyTable, SnapshotResponse};
use crate::{error::AppError, state::AppState};

/// Mọi dataset của workspace kèm bảng của chúng.
pub async fn list(state: &AppState, member: &Member) -> Result<Vec<DatasetSummary>, AppError> {
    let ws = &member.workspace;
    let sources = state.store.list_catalog_sources(ws).await?;
    let tables = ingest::list_datasets(state, member).await?;
    let columns = state.store.list_catalog_columns(ws, None).await?;
    let relations = state.store.list_relations(ws, None).await?;
    let metadata = state.store.list_workspace_metadata_tables(ws).await?;
    Ok(sources
        .into_iter()
        .map(|source| build_summary(source, &tables, &columns, &relations, &metadata))
        .collect())
}

/// Một dataset của workspace; id của workspace khác trả 404 như id không có.
pub async fn detail(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<DatasetSummary, AppError> {
    let ws = &member.workspace;
    let source = load_source(state, member, id).await?;
    let tables = ingest::list_datasets(state, member).await?;
    let columns = state
        .store
        .list_catalog_columns(ws, Some(source.id))
        .await?;
    let relations = state.store.list_relations(ws, None).await?;
    let metadata = state.store.list_metadata_tables(ws, source.id).await?;
    Ok(build_summary(
        source, &tables, &columns, &relations, &metadata,
    ))
}

/// Chụp lại mọi bảng một dataset là database đang giữ, kể cả bảng chỉ có metadata (chỉ admin, #247), trong một giao
/// dịch đọc.
pub async fn snapshot(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<SnapshotResponse, AppError> {
    member.require_admin()?;
    let source = load_source(state, member, id).await?;
    if source.kind == "local_file" {
        return Err(AppError::BadRequest(
            "Dataset này là một file: nạp lại file để có phiên bản mới".to_string(),
        ));
    }
    sources::snapshot_dataset(state, member, source.id).await
}

/// Hỏi trên cả dataset, hay bắt đầu từ bảng `table_id` của nó.
pub async fn ask(
    state: &AppState,
    member: &Member,
    id: &str,
    question: &str,
    choices: &[ChosenOption],
    conversation_id: Option<Uuid>,
    table_id: Option<Uuid>,
) -> Result<AskResponse, AppError> {
    let source = load_source(state, member, id).await?;
    let tables = load_tables(state, member, source.id).await?;
    let columns = state
        .store
        .list_catalog_columns(&member.workspace, Some(source.id))
        .await?;
    let relations = state.store.list_relations(&member.workspace, None).await?;
    let scope = DatasetScope {
        tables: &tables,
        columns: &columns,
        relations: &relations,
        pinned: table_id,
    };
    conversation::ask_in_dataset(state, member, &scope, question, choices, conversation_id).await
}

/// Cuộc trò chuyện của người gọi trên mọi bảng của dataset.
pub async fn conversations(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<ConversationListResponse, AppError> {
    let source = load_source(state, member, id).await?;
    conversation::list_in_dataset(state, member, source.id).await
}

/// Dòng sources của dataset, hay 404.
async fn load_source(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<CatalogSourceRow, AppError> {
    let id = parse_id(id)?;
    state
        .store
        .get_catalog_source(&member.workspace, id)
        .await?
        .ok_or_else(not_found)
}

/// Bảng của dataset `source_id`, như list_datasets trả (đã có trạng thái hồ sơ, quan hệ).
async fn load_tables(
    state: &AppState,
    member: &Member,
    source_id: Uuid,
) -> Result<Vec<Dataset>, AppError> {
    let mut tables = ingest::list_datasets(state, member).await?;
    tables.retain(|t| t.parent_id == source_id);
    Ok(tables)
}

fn parse_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| not_found())
}

fn not_found() -> AppError {
    AppError::NotFound("Không có dataset này".to_string())
}

fn build_summary(
    source: CatalogSourceRow,
    all_tables: &[Dataset],
    columns: &[CatalogColumnRow],
    relations: &[DatasetRelation],
    metadata: &[MetadataTableRow],
) -> DatasetSummary {
    let mut by_table: HashMap<Uuid, Vec<TableColumn>> = HashMap::new();
    for c in columns {
        by_table
            .entry(c.table_id)
            .or_default()
            .push(build_column(c));
    }
    let mut tables: Vec<DatasetTable> = all_tables
        .iter()
        .filter(|t| t.parent_id == source.id)
        .map(|t| build_table(t, by_table.remove(&t.id).unwrap_or_default()))
        .collect();
    // Bảng xếp theo schema rồi tên, để giao diện nhóm theo schema mà không phải sắp lại.
    tables.sort_by(|a, b| (&a.source_table, &a.name).cmp(&(&b.source_table, &b.name)));
    let ids: Vec<Uuid> = tables.iter().map(|t| t.id).collect();
    let updated_at = tables.iter().filter_map(|t| t.current_version_at).max();
    DatasetSummary {
        id: source.id,
        kind: if source.kind == "local_file" {
            DatasetKind::File
        } else {
            DatasetKind::Database
        },
        name: source.name,
        domain: source.domain,
        relations: relations
            .iter()
            .filter(|r| ids.contains(&r.from_dataset_id) && ids.contains(&r.to_dataset_id))
            .cloned()
            .collect(),
        metadata_only: metadata
            .iter()
            .filter(|m| m.source_id == source.id)
            .map(MetadataOnlyTable::from_stored)
            .collect(),
        tables,
        updated_at,
        created_at: source.created_at,
    }
}

fn build_table(table: &Dataset, columns: Vec<TableColumn>) -> DatasetTable {
    DatasetTable {
        id: table.id,
        name: table.name.clone(),
        logical_table: table.logical_table.clone(),
        schema: table
            .source_table
            .as_deref()
            .and_then(|t| t.split_once('.'))
            .map(|(schema, _)| schema.to_string()),
        source_table: table.source_table.clone(),
        description: table.description.clone(),
        row_count: table.row_count,
        current_version_id: table.current_version_id,
        current_version_at: table.current_version_at,
        current_profile_status: table.current_profile_status.clone(),
        semantic_unavailable: table.semantic_unavailable,
        suggested_questions: table.suggested_questions.clone(),
        columns,
    }
}

fn build_column(c: &CatalogColumnRow) -> TableColumn {
    TableColumn {
        name: c.name.clone(),
        data_type: c.data_type.clone(),
        role: c.role.clone(),
        description: c.description.clone(),
        description_source: c.description_source.clone(),
        is_sensitive: c.is_sensitive,
        aliases: c.aliases.clone(),
    }
}
