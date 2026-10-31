// SPDX-License-Identifier: Apache-2.0
/* service/catalog/contract.rs - hợp đồng của dataset (#257): một dataset là một database đã kết nối hay một file, bảng
nằm bên trong. Giao diện đọc ở GET /workspaces/{ws}/catalog (api/catalog.rs). Đây là hợp đồng chung giữa backend và
frontend (AGENTS.md mục 2): đổi tên trường thì báo cả đội.

Tên trong code giữ như cũ để diff dễ đọc: kiểu Dataset của engine/store/ và route /datasets/{id}/... vẫn là một bảng
(id bảng), dòng sources là một dataset. Ở đây, và trong mọi tài liệu mới, "dataset" là cấp trên, "bảng" (table) là cấp
dưới. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::engine::store::DatasetRelation;
use crate::service::sources::MetadataOnlyTable;

/// What a dataset is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatasetKind {
    /// A connected database; its tables are snapshots of tables of that database.
    Database,
    /// One uploaded file (CSV, Parquet, Excel); it has one table.
    File,
}

/// A dataset: one connected database or one uploaded file, with its tables.
// Trường Option luôn có trong JSON (null khi trống), nên đánh dấu required như Dataset ở engine/store/datasets.rs.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DatasetSummary {
    /// Dataset id. For a database, the same id as its source in `/sources/{id}` (admin only).
    pub id: Uuid,
    pub kind: DatasetKind,
    /// Database name for a database, the name given at upload for a file.
    pub name: String,
    /// Domain pack bound to the dataset (`key` of `GET .../domains`); null when none.
    #[schema(required)]
    pub domain: Option<String>,
    /// Tables, ordered by `source_table` (schema, then table) and then by name. A file has one table; a database
    /// connected but not snapshotted yet has none.
    pub tables: Vec<DatasetTable>,
    /// Foreign-key relations between tables of this dataset, copied from the source database. Empty for a file.
    pub relations: Vec<DatasetRelation>,
    /// Tables of the database kept with columns, types, keys and comments but no rows (over SOURCE_MAX_ROWS or the
    /// SOURCE_SNAPSHOT_MAX_MB total), ordered by `table`. An admin snapshots one with `POST /sources/{id}/snapshot`
    /// and `tables: [table]`. Empty for a file.
    pub metadata_only: Vec<MetadataOnlyTable>,
    /// Latest `current_version_at` of its tables: the last snapshot or upload. Null while no table has data.
    #[schema(required)]
    pub updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// A table of a dataset. Its id is the id the table routes take: `/datasets/{id}/overview`, `/metrics`, `/drill`,
/// `/conversations/{conversation_id}`.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DatasetTable {
    pub id: Uuid,
    pub name: String,
    /// Table name used in SQL; unique within the dataset only, so two databases can both have `orders`.
    pub logical_table: String,
    /// Schema in the source database (`sales` for `sales.orders`); null for a file.
    #[schema(required)]
    pub schema: Option<String>,
    /// `schema.table` in the source database; null for a file.
    #[schema(required)]
    pub source_table: Option<String>,
    /// Table comment from the source database (`COMMENT ON TABLE`) at the current version; null when none.
    #[schema(required)]
    pub description: Option<String>,
    /// Rows in the current version; null while the table has no data.
    #[schema(required)]
    pub row_count: Option<i64>,
    #[schema(required)]
    pub current_version_id: Option<Uuid>,
    /// When the current version was created: upload time, or the last snapshot.
    #[schema(required)]
    pub current_version_at: Option<DateTime<Utc>>,
    /// `pending`, `structure`, `semantic` or `failed`; null without a current version. Asking works from
    /// `structure` on.
    #[schema(required)]
    pub current_profile_status: Option<String>,
    /// Same meaning as `Dataset.semantic_unavailable`.
    pub semantic_unavailable: bool,
    /// Suggested questions of the current version; empty until the semantic profile is done.
    pub suggested_questions: Vec<String>,
    /// Columns of the current version, in table order; empty without a current version.
    pub columns: Vec<TableColumn>,
}

/// A column of the current version of a table.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TableColumn {
    pub name: String,
    /// DuckDB type, for example `BIGINT`, `VARCHAR`, `DATE`.
    pub data_type: String,
    /// `dimension`, `measure`, `time`, `key`, `text` or `ignore`.
    pub role: String,
    #[schema(required)]
    pub description: Option<String>,
    /// `database` (comment in the source database) or `model` (written by the semantic profile); null when unknown.
    #[schema(required)]
    pub description_source: Option<String>,
    /// Sensitive column: its values never go to the language model.
    pub is_sensitive: bool,
    /// Other names of the column (semantic profile), used to match question words.
    pub aliases: Vec<String>,
}
