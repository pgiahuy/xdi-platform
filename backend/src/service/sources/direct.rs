// SPDX-License-Identifier: Apache-2.0
/* service/sources/direct.rs - chép bảng khi chụp bằng DuckDB đọc thẳng Postgres (SOURCE_SNAPSHOT_MODE=direct, #246):
extension postgres của DuckDB đọc bảng qua giao thức COPY nhị phân của Postgres và ghi Parquet một bước, không qua file
CSV tạm. snapshot.rs gọi copy_tables thay cho postgres::copy_to_csv. Danh sách bảng, đếm dòng, kiểu cột (pg_types.rs),
khóa ngoại và mô tả vẫn đọc bằng sqlx như cách COPY.

Extension không bao giờ tải lúc chạy: file nằm sẵn trong image (backend/Dockerfile) hoặc do make duckdb-extension tải
một lần (scripts/fetch-duckdb-extension.sh ghi phiên bản và checksum), nạp bằng LOAD từ đường dẫn
SOURCE_POSTGRES_EXTENSION. Tự cài, tự nạp extension tắt như ở engine/analytics.rs. File thiếu, lệch bản DuckDB, hay
libpq của extension không kết nối được (bắt tay TLS khác rustls của sqlx) thì trả Unavailable: snapshot.rs ghi log
và chép bằng COPY.

Giữ đúng như cách COPY:
- Chỉ đọc: ATTACH ... READ_ONLY, và phiên Postgres đặt default_transaction_read_only cùng statement_timeout qua options
  của chuỗi kết nối libpq.
- Một ảnh chụp: mọi bảng chép trong một giao dịch DuckDB; extension mở một giao dịch REPEATABLE READ READ ONLY phía
  Postgres cho cả giao dịch đó (ghi rõ ISOLATION_LEVEL khi ATTACH, không dựa vào mặc định của extension), và
  postgres_query chạy trên chính kết nối của giao dịch này.
- Kiểu cột: mỗi cột ép về đúng kiểu DuckDB mà pg_types.rs chọn. Cột đọc thành VARCHAR hay DOUBLE thì Postgres đổi sang
  chữ trước, bằng hàm xuất của kiểu như COPY, nên ra đúng chữ mà COPY ghi: mảng, json, uuid, interval, char(n), float4
  giữ dạng chữ của Postgres thay vì dạng chữ của DuckDB, và float4 0.1 vẫn là 0.1 chứ không thành 0.10000000149011612.
- Trần dòng: LIMIT SOURCE_MAX_ROWS khi admin đặt. Dung lượng: bảng quá trần đã bị loại trước khi chép (plan.rs); file
  Parquet vẫn so với SOURCE_TABLE_MAX_MB sau khi ghi, cho view (không có dung lượng trước).
- TLS theo sslmode của nguồn, cùng tên mode như libpq. Khác rustls ở chỗ CA với verify-full: không có CA dán vào thì
  libpq dùng CA của hệ thống (sslrootcert=system); có CA dán vào thì chỉ tin CA đó, không cộng thêm CA của hệ thống.
- Mật khẩu và CA đi qua file tạm quyền 0600 (libpq.rs), không nằm trong câu SQL của DuckDB. */

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use duckdb::{Connection, InterruptHandle};
use sha2::{Digest, Sha256};

use super::libpq::{build_conninfo, ConnFiles};
use super::postgres::{qualified, quote_ident, ConnectInfo, SourceTable, TableCopy};
use crate::config::Config;
use crate::engine::analytics::FileFormat;
use crate::error::AppError;

/// Tên database Postgres trong DuckDB sau ATTACH.
const ALIAS: &str = "source";

/// Một bảng cần chép: kiểu cột lấy từ pg_types::column_types, file Parquet ghi vào out.
pub struct DirectTable {
    pub table: SourceTable,
    pub column_types: Vec<(String, String)>,
    pub out: PathBuf,
}

pub enum DirectError {
    /// Không dùng được cách đọc thẳng (thiếu file extension, lệch phiên bản, libpq không kết nối được): chép bằng COPY.
    Unavailable(String),
    /// Lỗi trong lúc chép (quyền, quá thời gian, quá dung lượng): báo người dùng như cách COPY.
    Failed(AppError),
}

/// Chép các bảng sang Parquet trong một giao dịch, trả SHA-256 và kiểu cột của từng file, theo thứ tự của tables.
pub async fn copy_tables(
    config: &Config,
    info: &ConnectInfo,
    tables: Vec<DirectTable>,
) -> Result<Vec<TableCopy>, DirectError> {
    let extension = config.source_postgres_extension.clone();
    if !extension.is_file() {
        return Err(DirectError::Unavailable(format!(
            "extension file {} not found (make duckdb-extension, docs/INSTALL.md)",
            extension.display()
        )));
    }
    // files giữ passfile và CA tới cuối hàm; ra khỏi hàm (kể cả khi lỗi hay bị hủy vì quá thời gian) thì bị xóa.
    let files = ConnFiles::write(&config.data_dir, info)?;
    let conninfo = build_conninfo(info, config.source_timeout, &files);
    let conn = Connection::open_in_memory()
        .map_err(|e| DirectError::Failed(AppError::Internal(format!("open duckdb: {e}"))))?;
    let mut interrupt = InterruptOnDrop {
        handle: conn.interrupt_handle(),
        armed: true,
    };
    let max_rows = config.source_max_rows;
    let max_bytes = config.source_table_max_bytes;
    // DuckDB đồng bộ và tốn CPU: chạy ngoài luồng async, như engine/analytics.rs. conn chuyển hẳn vào luồng đó.
    let task = tokio::task::spawn_blocking(move || {
        run(&conn, &extension, &conninfo, &tables, max_rows)?;
        tables
            .iter()
            .map(|t| build_table_copy(t, max_bytes))
            .collect::<Result<Vec<_>, _>>()
    });
    let result = task.await.map_err(|e| {
        DirectError::Failed(AppError::Internal(format!("direct snapshot task: {e}")))
    })?;
    interrupt.armed = false;
    drop(files);
    result
}

/// Phần chạy trong DuckDB: nạp extension, ATTACH chỉ đọc, chép từng bảng trong một giao dịch.
fn run(
    conn: &Connection,
    extension: &Path,
    conninfo: &str,
    tables: &[DirectTable],
    max_rows: Option<u64>,
) -> Result<(), DirectError> {
    let internal =
        |e: duckdb::Error| DirectError::Failed(AppError::Internal(format!("duckdb: {e}")));
    conn.execute_batch(
        "SET autoinstall_known_extensions = false; SET autoload_known_extensions = false;",
    )
    .map_err(internal)?;
    // LOAD kiểm chữ ký của file và bản DuckDB mà file được build cho; lệch bản thì lỗi ở đây, không chạy code lạ.
    conn.execute_batch(&format!(
        "LOAD {}",
        sql_string(&extension.to_string_lossy())
    ))
    .map_err(|e| DirectError::Unavailable(format!("load {}: {e}", extension.display())))?;
    conn.execute_batch(&format!(
        "ATTACH {} AS {ALIAS} (TYPE postgres, READ_ONLY, ISOLATION_LEVEL 'REPEATABLE READ')",
        sql_string(conninfo)
    ))
    .map_err(|e| DirectError::Unavailable(format!("attach: {e}")))?;
    conn.execute_batch("BEGIN TRANSACTION").map_err(internal)?;
    // Câu thử đầu tiên mở kết nối libpq và giao dịch phía Postgres. Lỗi ở đây (TLS, pg_hba) là lỗi của libpq: sqlx vừa
    // kết nối được cùng máy chủ, nên quay về COPY thay vì báo người dùng.
    conn.execute_batch(&format!(
        "SELECT * FROM postgres_query({}, 'SELECT 1')",
        sql_string(ALIAS)
    ))
    .map_err(|e| DirectError::Unavailable(format!("connect: {e}")))?;
    for t in tables {
        conn.execute_batch(&build_copy_sql(t, max_rows))
            .map_err(|e| DirectError::Failed(copy_error(&t.table, &e)))?;
    }
    conn.execute_batch("COMMIT").map_err(internal)?;
    Ok(())
}

/// Lệnh DuckDB chép một bảng. Câu chạy trên Postgres chọn từng cột, cột VARCHAR hay DOUBLE đổi sang chữ ngay trên
/// Postgres (lý do ở đầu file); phía DuckDB ép mọi cột về đúng kiểu của pg_types.rs.
fn build_copy_sql(t: &DirectTable, max_rows: Option<u64>) -> String {
    let mut remote = Vec::new();
    let mut casts = Vec::new();
    for (name, duckdb_type) in &t.column_types {
        let column = quote_ident(name);
        // concat đổi giá trị sang chữ bằng đúng hàm xuất của kiểu, như COPY. Không dùng ::text: char(n)::text cắt dấu
        // cách cuối ('ab  ' thành 'ab'), inet::text thêm /32, nên lệch với bản COPY. concat(NULL) là chuỗi rỗng, nên
        // giữ NULL bằng CASE.
        if duckdb_type == "VARCHAR" || duckdb_type == "DOUBLE" {
            remote.push(format!(
                "CASE WHEN {column} IS NULL THEN NULL ELSE concat({column}) END AS {column}"
            ));
        } else {
            remote.push(column.clone());
        }
        // duckdb_type do pg_types.rs viết (INTEGER, DECIMAL(10,2)...), không phải chữ người dùng gửi.
        casts.push(format!("CAST({column} AS {duckdb_type}) AS {column}"));
    }
    let limit = max_rows.map(|n| format!(" LIMIT {n}")).unwrap_or_default();
    let remote_sql = format!(
        "SELECT {} FROM {}{limit}",
        remote.join(", "),
        qualified(&t.table)
    );
    format!(
        "COPY (SELECT {} FROM postgres_query({}, {})) TO {} (FORMAT parquet)",
        casts.join(", "),
        sql_string(ALIAS),
        sql_string(&remote_sql),
        sql_string(&t.out.to_string_lossy())
    )
}

/// Đọc lại file Parquet vừa ghi: kiểm dung lượng và tính SHA-256 (chống trùng phiên bản ở service/ingest.rs).
fn build_table_copy(t: &DirectTable, max_bytes: u64) -> Result<TableCopy, DirectError> {
    let io = |e: std::io::Error| {
        DirectError::Failed(AppError::Internal(format!("read {}: {e}", t.out.display())))
    };
    let size = fs::metadata(&t.out).map_err(io)?.len();
    if size > max_bytes {
        return Err(DirectError::Failed(AppError::TooLarge(
            super::postgres::build_too_large(&t.table, max_bytes),
        )));
    }
    let mut file = fs::File::open(&t.out).map_err(io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buffer).map_err(io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(TableCopy {
        sha256,
        column_types: t.column_types.clone(),
        format: FileFormat::DatabaseParquet,
    })
}

/// Lỗi khi chép thành lời báo như postgres::query_error. Lỗi gốc của DuckDB (có lời của Postgres) chỉ vào log.
fn copy_error(table: &SourceTable, e: &duckdb::Error) -> AppError {
    let text = e.to_string();
    tracing::warn!(table = %table.full_name(), error = %text, "direct snapshot failed");
    if text.contains("statement timeout") || text.contains("INTERRUPT") {
        return AppError::BadRequest(
            "Đọc bảng quá thời gian cho phép (SOURCE_TIMEOUT_SECS): chọn bảng nhỏ hơn hoặc tăng giới hạn".to_string(),
        );
    }
    if text.contains("permission denied") {
        return AppError::BadRequest(
            "Tài khoản không có quyền SELECT trên bảng này: cấp quyền (GRANT SELECT) rồi thử lại"
                .to_string(),
        );
    }
    if text.contains("IO Error") && !text.contains("Postgres") {
        return AppError::Internal(format!("direct snapshot {}: {text}", table.full_name()));
    }
    AppError::BadRequest(format!(
        "Đọc bảng {} từ Postgres lỗi: thử chụp lại, kiểm bảng còn tồn tại và tài khoản còn quyền đọc",
        table.full_name()
    ))
}

/// Chuỗi SQL của DuckDB: bọc nháy đơn, nhân đôi nháy đơn bên trong.
fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Hủy câu DuckDB đang chạy khi lần chụp bị bỏ giữa chừng: quá SOURCE_TIMEOUT_SECS thì snapshot.rs bỏ future của
/// copy_tables, nhưng luồng spawn_blocking không tự dừng theo và sẽ đọc Postgres tới hết bảng dù không ai chờ.
struct InterruptOnDrop {
    handle: Arc<InterruptHandle>,
    armed: bool,
}

impl Drop for InterruptOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.handle.interrupt();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cột VARCHAR, DOUBLE lấy chữ của Postgres; cột khác đọc nhị phân; mọi cột ép về kiểu của pg_types.rs.
    #[test]
    fn copy_sql_casts_every_column() {
        let t = DirectTable {
            table: SourceTable {
                schema: "sales".to_string(),
                name: "it's".to_string(),
                kind: "table".to_string(),
            },
            column_types: vec![
                ("id".to_string(), "INTEGER".to_string()),
                ("note".to_string(), "VARCHAR".to_string()),
                ("ratio".to_string(), "DOUBLE".to_string()),
            ],
            out: PathBuf::from("/tmp/out"),
        };
        let sql = build_copy_sql(&t, Some(10));
        assert!(sql.contains("CAST(\"id\" AS INTEGER) AS \"id\""), "{sql}");
        assert!(
            sql.contains("CAST(\"ratio\" AS DOUBLE) AS \"ratio\""),
            "{sql}"
        );
        assert!(sql.contains("'SELECT \"id\", CASE WHEN"), "{sql}");
        assert!(sql.contains("concat(\"note\") END AS \"note\""), "{sql}");
        assert!(sql.contains("FROM \"sales\".\"it''s\" LIMIT 10'"), "{sql}");
        // Không có trần dòng: không LIMIT.
        let sql = build_copy_sql(&t, None);
        assert!(sql.contains("FROM \"sales\".\"it''s\"'"), "{sql}");
    }

    /// Bản DuckDB trong binary phải đúng bản mà scripts/fetch-duckdb-extension.sh tải extension cho: nâng crate duckdb mà
    /// quên sửa script thì LOAD sẽ lỗi ở mọi nơi cài, nên chặn ở test.
    #[test]
    fn duckdb_version_matches_the_extension_pin() {
        let script = include_str!("../../../../scripts/fetch-duckdb-extension.sh");
        let pinned = script
            .lines()
            .find_map(|line| line.strip_prefix("DUCKDB_VERSION="))
            .expect("DUCKDB_VERSION line in fetch-duckdb-extension.sh");
        let conn = Connection::open_in_memory().unwrap();
        let version: String = conn
            .query_row("SELECT version()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, pinned);
    }
}
