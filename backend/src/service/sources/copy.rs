// SPDX-License-Identifier: Apache-2.0
/* service/sources/copy.rs - bước chép của một lần chụp (snapshot.rs gọi copy_group): một kết nối, một giao dịch
REPEATABLE READ READ ONLY cho mọi bảng, nên mọi bảng là ảnh của cùng một thời điểm.

Trong giao dịch đó, theo thứ tự:
1. Thống kê mọi bảng (catalog.rs), đếm dòng thật từng bảng chọn (trừ bảng ước lượng đã quá xa trần, plan.rs).
2. plan.rs chia bảng chép dữ liệu và bảng chỉ lưu metadata (quá SOURCE_MAX_ROWS hay trần SOURCE_SNAPSHOT_MAX_MB).
3. Chép bảng chép được ra file riêng (giới hạn MAX_UPLOAD_MB từng bảng): COPY ra CSV ngay trên kết nối này, hoặc với
   SOURCE_SNAPSHOT_MODE=direct thì DuckDB đọc thẳng ra Parquet trong một giao dịch REPEATABLE READ riêng của nó
   (direct.rs). Comment bảng, cột (comments.rs) đọc bằng sqlx ở đây, cách chép nào cũng vậy.
4. Cột, kiểu, khóa, comment của bảng chỉ lưu metadata; mọi khóa ngoại của database (relations.rs).
Cả bước chung một giới hạn SOURCE_TIMEOUT_SECS. */

use std::path::PathBuf;

use uuid::Uuid;

use super::catalog;
use super::comments;
use super::direct::{self, DirectError, DirectTable};
use super::find_table_in;
use super::pg_types;
use super::plan::{self, Candidate, RowCount, Skipped};
use super::postgres::{self, ConnectInfo, SourceTable, TableCopy};
use super::relations::{self, ForeignKey};
use crate::config::SnapshotMode;
use crate::engine::store::MetadataColumn;
use crate::service::ingest::{SourceDescriptions, Staging};
use crate::{error::AppError, state::AppState};

/// Một bảng cần chụp; dataset đã biết khi chụp lại.
#[derive(Debug, Clone)]
pub struct Wanted {
    pub table: String,
    pub dataset_id: Option<Uuid>,
}

/// Bảng nào cần chụp.
pub enum Selection {
    /// Mọi bảng, view tài khoản đọc được lúc chụp.
    All,
    /// Đúng các bảng này. skip_unreadable: bảng không còn đọc được (đã xóa, mất quyền) thì bỏ qua và báo trong
    /// unreadable, thay vì từ chối cả lần chụp; dùng khi chụp lại cả nguồn, để một bảng đã xóa ở nguồn không chặn mãi
    /// các bảng còn lại.
    Tables {
        wanted: Vec<Wanted>,
        skip_unreadable: bool,
    },
}

/// Một bảng đã chép ra file tạm, chờ nạp. Staging giữ thư mục tạm tới khi ingest::upload nhận nó.
pub struct CopiedTable {
    pub table: SourceTable,
    pub staging: Staging,
    pub path: PathBuf,
    pub copy: TableCopy,
    pub descriptions: SourceDescriptions,
    pub dataset_id: Option<Uuid>,
}

/// Một bảng chỉ lưu metadata.
pub struct SkippedTable {
    pub skipped: Skipped,
    pub description: Option<String>,
    pub columns: Vec<MetadataColumn>,
    pub dataset_id: Option<Uuid>,
}

pub struct GroupCopy {
    pub copied: Vec<CopiedTable>,
    pub skipped: Vec<SkippedTable>,
    /// Mọi khóa ngoại của database nguồn, đọc trong cùng giao dịch.
    pub keys: Vec<ForeignKey>,
    /// Bảng xin chụp mà tài khoản không còn đọc được (chỉ với skip_unreadable).
    pub unreadable: Vec<String>,
}

pub async fn copy_group(
    state: &AppState,
    info: &ConnectInfo,
    selection: Selection,
) -> Result<GroupCopy, AppError> {
    let limit = state.config.source_timeout;
    let copy = async {
        let mut conn = postgres::connect(info, limit).await?;
        let readable = postgres::list_tables(&mut conn).await?;
        let (wanted, unreadable) = resolve(&readable, selection)?;
        postgres::begin_snapshot(&mut conn).await?;
        let stats = catalog::read_stats(&mut conn).await?;
        let candidates = count_tables(state, &mut conn, &wanted, &stats).await?;
        let (taken, skipped) = plan::build_plan(
            candidates,
            state.config.source_max_rows,
            state.config.source_snapshot_max_bytes,
        );
        let dataset_of = |t: &SourceTable| {
            wanted
                .iter()
                .find(|(w, _)| w.same(t))
                .and_then(|(_, id)| *id)
        };
        let tables: Vec<SourceTable> = taken.into_iter().map(|c| c.table).collect();
        let mut copied = if tables.is_empty() {
            Vec::new()
        } else {
            copy_tables(state, info, &mut conn, &tables).await?
        };
        // Comment đọc bằng sqlx trong giao dịch của conn, cách chép nào (COPY hay DuckDB) cũng vậy.
        for c in &mut copied {
            c.descriptions = comments::read_descriptions(&mut conn, &c.table).await?;
            c.dataset_id = dataset_of(&c.table);
        }
        let mut skipped_tables = Vec::new();
        for s in skipped {
            let table = &s.candidate.table;
            skipped_tables.push(SkippedTable {
                columns: catalog::read_metadata_columns(&mut conn, table).await?,
                description: stats
                    .get(&table.full_name())
                    .and_then(|st| st.comment.clone()),
                dataset_id: dataset_of(table),
                skipped: s,
            });
        }
        let keys = relations::list_all_foreign_keys(&mut conn).await?;
        postgres::commit_snapshot(&mut conn).await?;
        Ok::<_, AppError>(GroupCopy {
            copied,
            skipped: skipped_tables,
            keys,
            unreadable,
        })
    };
    match tokio::time::timeout(limit, copy).await {
        Ok(result) => result,
        Err(_) => Err(AppError::BadRequest(
            "Chụp bảng quá thời gian cho phép (SOURCE_TIMEOUT_SECS): chọn ít bảng hơn, bảng nhỏ hơn, hoặc tăng giới hạn"
                .to_string(),
        )),
    }
}

/// Bảng đọc được cần chụp, kèm dataset đã biết của nó.
type ResolvedTable = (SourceTable, Option<Uuid>);

/// Bảng cần chụp đối chiếu với danh sách đọc được: tên người dùng gửi không đi thẳng vào câu SQL. Không trùng.
fn resolve(
    readable: &[SourceTable],
    selection: Selection,
) -> Result<(Vec<ResolvedTable>, Vec<String>), AppError> {
    let (wanted, skip_unreadable) = match selection {
        Selection::All => {
            if readable.is_empty() {
                return Err(AppError::BadRequest(
                    "Tài khoản không đọc được bảng nào: cấp GRANT SELECT trên các bảng cần hỏi rồi chụp lại"
                        .to_string(),
                ));
            }
            return Ok((
                readable.iter().map(|t| (t.clone(), None)).collect(),
                Vec::new(),
            ));
        }
        Selection::Tables {
            wanted,
            skip_unreadable,
        } => (wanted, skip_unreadable),
    };
    let mut tables: Vec<ResolvedTable> = Vec::new();
    let mut unreadable = Vec::new();
    for w in wanted {
        match find_table_in(readable, &w.table) {
            Ok(t) if !tables.iter().any(|(seen, _)| seen.same(&t)) => {
                tables.push((t, w.dataset_id))
            }
            Ok(_) => {}
            Err(_) if skip_unreadable => {
                tracing::warn!(table = %w.table, "source table is no longer readable, skipping it");
                unreadable.push(w.table);
            }
            Err(e) => return Err(e),
        }
    }
    Ok((tables, unreadable))
}

/// Số dòng của từng bảng: đếm thật, trừ bảng mà ước lượng đã quá xa trần (plan::needs_exact_count).
async fn count_tables(
    state: &AppState,
    conn: &mut sqlx::PgConnection,
    wanted: &[ResolvedTable],
    stats: &std::collections::HashMap<String, catalog::TableStats>,
) -> Result<Vec<Candidate>, AppError> {
    let max_rows = state.config.source_max_rows;
    let mut candidates = Vec::new();
    for (table, _) in wanted {
        let st = stats.get(&table.full_name()).cloned().unwrap_or_default();
        let rows = match st.estimated_rows {
            Some(n) if !plan::needs_exact_count(Some(n), max_rows) => RowCount::Estimate(n),
            _ => RowCount::Exact(postgres::count_rows(conn, table).await?),
        };
        candidates.push(Candidate {
            table: table.clone(),
            rows,
            size_bytes: st.size_bytes,
        });
    }
    Ok(candidates)
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
            descriptions: SourceDescriptions::default(),
            dataset_id: None,
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
            descriptions: SourceDescriptions::default(),
            dataset_id: None,
        })
        .collect())
}
