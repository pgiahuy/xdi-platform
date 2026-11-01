// SPDX-License-Identifier: Apache-2.0
/* service/sources/catalog.rs - đọc catalog của database nguồn cho màn kết nối và cho bảng chỉ lưu metadata (#247):
- read_stats: số dòng ước lượng (pg_class.reltuples), dung lượng (pg_table_size), comment bảng, số cột đọc được của
  mọi bảng đọc được, trong một câu.
- build_table_list: danh sách bảng của GET /sources/{id}/tables, kèm khóa ngoại vào, ra giữa các bảng đọc được và cờ
  "bảng khác đang trỏ tới", để giao diện cảnh báo khi admin bỏ tick một bảng danh mục.
- read_metadata_columns: cột, kiểu, khóa chính, mô tả của một bảng không chép dữ liệu (plan.rs quyết bảng nào).
- read_key_columns: cột khóa chính và cột khóa ngoại của một bảng đã chép, cho vai trò key (service/column_roles.rs).

Chỉ đọc catalog, không đọc dòng dữ liệu nào. Số dòng ước lượng là thống kê của Postgres (lần ANALYZE gần nhất), có thể
lệch số thật; dung lượng là số thật của file. Lúc chụp chỉ đếm dòng thật khi admin đặt SOURCE_MAX_ROWS
(plan::needs_exact_count). */

use std::collections::HashMap;

use serde::Serialize;
use sqlx::postgres::PgConnection;
use uuid::Uuid;

use super::comments::build_clean_comment;
use super::plan::{build_table_cause, Limits, SkipCause};
use super::postgres::{query_error, SourceTable};
use super::relations::ForeignKey;
use crate::engine::store::MetadataColumn;
use crate::error::AppError;
use crate::service::column_roles::SourceKeys;

/// Thống kê của một bảng đọc từ pg_class.
#[derive(Debug, Clone, Default)]
pub struct TableStats {
    /// None khi Postgres chưa có thống kê (bảng chưa ANALYZE) hay với view.
    pub estimated_rows: Option<i64>,
    /// None với view.
    pub size_bytes: Option<i64>,
    pub comment: Option<String>,
    pub column_count: i64,
}

/// What a source holds for one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableState {
    /// Snapshotted into a dataset (`dataset_id`).
    Snapshotted,
    /// Columns, types, keys and comments only, no rows (`reason` says why).
    MetadataOnly,
}

/// A foreign key between two readable tables, as declared in the source database.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TableLink {
    pub constraint: String,
    /// Table holding the foreign key (the many side) and its key columns.
    pub from_table: String,
    pub from_columns: Vec<String>,
    /// Table the foreign key points to (the one side) and its key columns.
    pub to_table: String,
    pub to_columns: Vec<String>,
}

/// A table or view the source account can read, with what the connect screen needs to pick tables.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SourceTableInfo {
    pub schema: String,
    pub name: String,
    /// `table` or `view`.
    pub kind: String,
    /// `schema.table`, the name to send in `SnapshotRequest.tables`.
    pub table: String,
    /// Row estimate from Postgres statistics (last ANALYZE); null when Postgres has none yet, and for views.
    #[schema(required)]
    pub estimated_rows: Option<i64>,
    /// Size reported by Postgres (`pg_table_size`, without indexes); null for views.
    #[schema(required)]
    pub size_bytes: Option<i64>,
    /// `COMMENT ON TABLE`, cleaned; null when none.
    #[schema(required)]
    pub comment: Option<String>,
    /// Columns the account can read.
    pub column_count: i64,
    /// Foreign keys of this table pointing to other readable tables.
    pub references: Vec<TableLink>,
    /// Foreign keys of other readable tables pointing to this table.
    pub referenced_by: Vec<TableLink>,
    /// True when another readable table points to this one (a lookup table): unticking it leaves those tables without
    /// the relation.
    pub is_referenced: bool,
    /// Limit this table alone is over, from Postgres statistics: `size_limit` above SOURCE_TABLE_MAX_MB, `row_limit`
    /// above SOURCE_MAX_ROWS when set. A snapshot keeps its metadata only. Null when it fits (the SOURCE_SNAPSHOT_MAX_MB
    /// total depends on the other ticked tables and is not checked here).
    #[schema(required)]
    pub over_limit: Option<SkipCause>,
    /// What this source already holds for the table; null when never snapshotted.
    #[schema(required)]
    pub state: Option<TableState>,
    /// Dataset of the table when `state` is `snapshotted`.
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
    /// Why the table has metadata only; null otherwise.
    #[schema(required)]
    pub reason: Option<String>,
}

/// Bảng này nguồn đang giữ thế nào, để ghi vào danh sách: đã chụp (dataset) hay chỉ metadata (lý do).
pub enum Held {
    Dataset(Uuid),
    Metadata(String),
}

/// Thống kê của mọi bảng, view tài khoản thấy được, theo schema.bảng.
pub async fn read_stats(c: &mut PgConnection) -> Result<HashMap<String, TableStats>, AppError> {
    type Row = (
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<String>,
        i64,
    );
    // reltuples là -1 khi chưa có thống kê (Postgres 14 trở lên). pg_table_size của view là 0, nên bỏ.
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT n.nspname::text, c.relname::text,
                CASE WHEN c.relkind IN ('r', 'm', 'p') AND c.reltuples >= 0 THEN c.reltuples::bigint END,
                CASE WHEN c.relkind IN ('r', 'm', 'p') THEN pg_table_size(c.oid) END,
                obj_description(c.oid, 'pg_class'),
                (SELECT count(*) FROM pg_attribute a
                 WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
                   AND has_column_privilege(a.attrelid, a.attnum, 'SELECT'))
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'v', 'm', 'p', 'f')
           AND n.nspname NOT IN ('pg_catalog', 'information_schema')",
    )
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    Ok(rows
        .into_iter()
        .map(|(schema, name, rows, size, comment, columns)| {
            let stats = TableStats {
                estimated_rows: rows,
                size_bytes: size,
                comment: comment.as_deref().and_then(build_clean_comment),
                column_count: columns,
            };
            (format!("{schema}.{name}"), stats)
        })
        .collect())
}

/// Danh sách bảng của màn kết nối. Chỉ ghi khóa ngoại có hai đầu đều đọc được: bảng không đọc được không chụp được.
pub fn build_table_list(
    readable: &[SourceTable],
    stats: &HashMap<String, TableStats>,
    keys: &[ForeignKey],
    held: &HashMap<String, Held>,
    limits: &Limits,
) -> Vec<SourceTableInfo> {
    let is_readable = |t: &SourceTable| readable.iter().any(|r| r.same(t));
    let links: Vec<(&ForeignKey, TableLink)> = keys
        .iter()
        .filter(|k| is_readable(&k.from) && is_readable(&k.to))
        .map(|k| (k, build_link(k)))
        .collect();
    readable
        .iter()
        .map(|t| {
            let full = t.full_name();
            let st = stats.get(&full).cloned().unwrap_or_default();
            let references: Vec<TableLink> = links
                .iter()
                .filter(|(k, _)| k.from.same(t))
                .map(|(_, l)| l.clone())
                .collect();
            let referenced_by: Vec<TableLink> = links
                .iter()
                .filter(|(k, _)| k.to.same(t))
                .map(|(_, l)| l.clone())
                .collect();
            // Khóa trỏ về chính bảng (quản lý trỏ tới nhân viên) không làm bảng thành bảng danh mục của bảng khác.
            let is_referenced = links.iter().any(|(k, _)| k.to.same(t) && !k.from.same(t));
            let over_limit = build_table_cause(st.estimated_rows, st.size_bytes, limits);
            let (state, dataset_id, reason) = match held.get(&full) {
                Some(Held::Dataset(id)) => (Some(TableState::Snapshotted), Some(*id), None),
                Some(Held::Metadata(why)) => {
                    (Some(TableState::MetadataOnly), None, Some(why.clone()))
                }
                None => (None, None, None),
            };
            SourceTableInfo {
                schema: t.schema.clone(),
                name: t.name.clone(),
                kind: t.kind.clone(),
                table: full,
                estimated_rows: st.estimated_rows,
                size_bytes: st.size_bytes,
                comment: st.comment,
                column_count: st.column_count,
                references,
                referenced_by,
                is_referenced,
                over_limit,
                state,
                dataset_id,
                reason,
            }
        })
        .collect()
}

fn build_link(k: &ForeignKey) -> TableLink {
    TableLink {
        constraint: k.constraint.clone(),
        from_table: k.from.full_name(),
        from_columns: k.from_columns.clone(),
        to_table: k.to.full_name(),
        to_columns: k.to_columns.clone(),
    }
}

/// Cột đọc được của một bảng không chép dữ liệu: tên, kiểu Postgres, có nhận null, thuộc khóa chính, comment.
pub async fn read_metadata_columns(
    c: &mut PgConnection,
    table: &SourceTable,
) -> Result<Vec<MetadataColumn>, AppError> {
    let rows: Vec<(String, String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT a.attname::text, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull,
                EXISTS (SELECT 1 FROM pg_constraint p
                        WHERE p.conrelid = c.oid AND p.contype = 'p' AND a.attnum = ANY (p.conkey)),
                col_description(c.oid, a.attnum)
         FROM pg_attribute a
         JOIN pg_class c ON c.oid = a.attrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped
           AND has_column_privilege(a.attrelid, a.attnum, 'SELECT')
         ORDER BY a.attnum",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    Ok(rows
        .into_iter()
        .map(
            |(name, data_type, nullable, primary_key, comment)| MetadataColumn {
                name,
                data_type,
                nullable,
                primary_key,
                description: comment.as_deref().and_then(build_clean_comment),
            },
        )
        .collect())
}

/// Cột khóa chính (theo thứ tự của khóa) và cột thuộc khóa ngoại của một bảng, từ pg_constraint. Chỉ đọc catalog.
pub async fn read_key_columns(
    c: &mut PgConnection,
    table: &SourceTable,
) -> Result<SourceKeys, AppError> {
    // contype là kiểu "char" của Postgres, đổi sang text để sqlx đọc thành String. Vị trí trong khóa (k.ord) giữ thứ tự
    // cột của khóa chính nhiều cột.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT a.attname::text, p.contype::text
         FROM pg_constraint p
         JOIN pg_class c ON c.oid = p.conrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         CROSS JOIN LATERAL unnest(p.conkey) WITH ORDINALITY AS k(attnum, ord)
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum
         WHERE n.nspname = $1 AND c.relname = $2 AND p.contype IN ('p', 'f')
         ORDER BY p.contype, p.conname, k.ord",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    let mut keys = SourceKeys::default();
    for (column, kind) in rows {
        let list = if kind == "p" {
            &mut keys.primary_key
        } else {
            &mut keys.foreign_key_columns
        };
        if !list.contains(&column) {
            list.push(column);
        }
    }
    Ok(keys)
}
