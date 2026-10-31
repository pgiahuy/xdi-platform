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
- Một ảnh chụp: mọi bảng chép trong một giao dịch DuckDB; extension giữ một giao dịch REPEATABLE READ phía Postgres cho
  cả giao dịch đó.
- Kiểu cột: mỗi cột ép về đúng kiểu DuckDB mà pg_types.rs chọn. Cột đọc thành VARCHAR hay DOUBLE thì Postgres đổi sang
  chữ trước, bằng hàm xuất của kiểu như COPY, nên ra đúng chữ mà COPY ghi: mảng, json, uuid, interval, float4 giữ dạng chữ của Postgres thay vì
  dạng chữ của DuckDB, và float4 0.1 vẫn là 0.1 chứ không thành 0.10000000149011612.
- Trần dòng: LIMIT SOURCE_MAX_ROWS. Dung lượng: file Parquet so với MAX_UPLOAD_MB sau khi ghi.
- TLS theo sslmode của nguồn, cùng tên mode như libpq. Khác rustls ở chỗ CA với verify-full: không có CA dán vào thì
  libpq dùng CA của hệ thống (sslrootcert=system); có CA dán vào thì chỉ tin CA đó, không cộng thêm CA của hệ thống.
- Mật khẩu và CA ghi ra file tạm quyền 0600 (passfile và sslrootcert của libpq), xóa khi lần chụp xong hay lỗi. Mật
  khẩu không nằm trong câu SQL của DuckDB, nên lỗi của extension (có chép chuỗi kết nối) không chứa mật khẩu. */

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use duckdb::{Connection, InterruptHandle};
use sha2::{Digest, Sha256};
use uuid::Uuid;

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
    let max_bytes = config.max_upload_bytes;
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
    max_rows: u64,
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
        "ATTACH {} AS {ALIAS} (TYPE postgres, READ_ONLY)",
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
fn build_copy_sql(t: &DirectTable, max_rows: u64) -> String {
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
    let remote_sql = format!(
        "SELECT {} FROM {} LIMIT {max_rows}",
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
        return Err(DirectError::Failed(AppError::TooLarge(format!(
            "Bảng {} lớn hơn giới hạn {} MB của một lần nạp (MAX_UPLOAD_MB)",
            t.table.full_name(),
            max_bytes / 1024 / 1024
        ))));
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

/// Chuỗi kết nối libpq dạng key='value'. Không có mật khẩu: libpq đọc nó từ passfile.
fn build_conninfo(info: &ConnectInfo, timeout: Duration, files: &ConnFiles) -> String {
    let mut pairs = vec![
        ("host", info.host.clone()),
        ("port", info.port.to_string()),
        ("dbname", info.database.clone()),
        ("user", info.user.clone()),
        ("passfile", files.passfile.to_string_lossy().into_owned()),
        ("sslmode", info.sslmode.clone()),
        ("application_name", "xdi-platform".to_string()),
        ("connect_timeout", timeout.as_secs().max(2).to_string()),
        (
            "options",
            format!(
                "-c default_transaction_read_only=on -c statement_timeout={}",
                timeout.as_millis()
            ),
        ),
    ];
    if info.sslmode == "verify-full" {
        let root = match &files.root_cert {
            Some(path) => path.to_string_lossy().into_owned(),
            None => "system".to_string(),
        };
        pairs.push(("sslrootcert", root));
    }
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", conninfo_value(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Giá trị trong chuỗi kết nối libpq: bọc nháy đơn, thoát nháy đơn và dấu gạch ngược bằng dấu gạch ngược.
fn conninfo_value(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Chuỗi SQL của DuckDB: bọc nháy đơn, nhân đôi nháy đơn bên trong.
fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Thư mục tạm của một lần chụp, quyền 0700, chứa passfile và CA dán vào (nếu có), mỗi file quyền 0600. Nằm dưới
/// DATA_DIR/staging, nên backend chết giữa chừng thì lần khởi động sau xóa (ingest::clear_staging). Drop xóa cả thư mục.
struct ConnFiles {
    dir: PathBuf,
    passfile: PathBuf,
    root_cert: Option<PathBuf>,
}

impl ConnFiles {
    fn write(data_dir: &Path, info: &ConnectInfo) -> Result<Self, DirectError> {
        // passfile của libpq đọc từng dòng: mật khẩu có xuống dòng thì không ghi vào đó được, chép bằng COPY.
        if info.password.contains(['\n', '\r']) {
            return Err(DirectError::Unavailable(
                "password contains a line break, which a libpq passfile cannot hold".to_string(),
            ));
        }
        let dir = data_dir
            .join("staging")
            .join(format!("libpq-{}", Uuid::new_v4()));
        let internal = |e: std::io::Error| {
            DirectError::Failed(AppError::Internal(format!(
                "write libpq files in {}: {e}",
                dir.display()
            )))
        };
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(internal)?;
        // Tạo struct ngay khi có thư mục, để lỗi ở bước ghi file sau vẫn xóa thư mục qua Drop.
        let mut files = ConnFiles {
            passfile: dir.join("passfile"),
            root_cert: None,
            dir: dir.clone(),
        };
        // Dòng passfile host:port:database:user:password, * khớp mọi giá trị; ':' và '\' trong mật khẩu thoát bằng '\'.
        let password = info.password.replace('\\', "\\\\").replace(':', "\\:");
        write_private(&files.passfile, format!("*:*:*:*:{password}\n").as_bytes())
            .map_err(internal)?;
        if let Some(pem) = &info.ssl_root_cert {
            let path = dir.join("root.crt");
            write_private(&path, pem.as_bytes()).map_err(internal)?;
            files.root_cert = Some(path);
        }
        Ok(files)
    }
}

impl Drop for ConnFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Ghi file chỉ chủ sở hữu đọc được (0600) ngay từ lúc tạo; libpq bỏ qua passfile mà người khác đọc được.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
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

    fn info(password: &str, sslmode: &str, ca: Option<&str>) -> ConnectInfo {
        ConnectInfo {
            host: "db.example.com".to_string(),
            port: 5432,
            database: "shop".to_string(),
            user: "reader".to_string(),
            password: password.to_string(),
            sslmode: sslmode.to_string(),
            ssl_root_cert: ca.map(str::to_string),
        }
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xdi-direct-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Mật khẩu chỉ nằm trong passfile quyền 0600, không trong chuỗi kết nối; thư mục tạm bị xóa khi xong.
    #[test]
    fn password_goes_to_a_private_passfile_only() {
        use std::os::unix::fs::PermissionsExt;
        let data = temp_dir();
        let source = info("pa:ss\\'word", "verify-full", Some("PEM"));
        let files = ConnFiles::write(&data, &source).ok().unwrap();
        let conninfo = build_conninfo(&source, Duration::from_secs(30), &files);
        assert!(!conninfo.contains("pa:ss"), "{conninfo}");
        assert!(conninfo.contains("sslmode='verify-full'"), "{conninfo}");
        assert!(conninfo.contains("statement_timeout=30000"), "{conninfo}");
        let passfile = fs::read_to_string(&files.passfile).unwrap();
        assert_eq!(passfile, "*:*:*:*:pa\\:ss\\\\'word\n");
        let mode = fs::metadata(&files.passfile).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let ca = files.root_cert.clone().unwrap();
        assert!(conninfo.contains(&format!("sslrootcert='{}'", ca.display())));
        let dir = files.dir.clone();
        drop(files);
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(&data);
    }

    /// verify-full không có CA dán vào dùng CA của hệ thống; mode khác không đặt sslrootcert.
    #[test]
    fn root_certificate_follows_the_mode() {
        let data = temp_dir();
        let full = info("pw", "verify-full", None);
        let files = ConnFiles::write(&data, &full).ok().unwrap();
        assert!(
            build_conninfo(&full, Duration::from_secs(5), &files).contains("sslrootcert='system'")
        );
        let require = info("pw", "require", None);
        assert!(!build_conninfo(&require, Duration::from_secs(5), &files).contains("sslrootcert"));
        drop(files);
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn line_break_in_password_falls_back() {
        let data = temp_dir();
        let result = ConnFiles::write(&data, &info("a\nb", "disable", None));
        assert!(matches!(result, Err(DirectError::Unavailable(_))));
        let _ = fs::remove_dir_all(&data);
    }

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
        let sql = build_copy_sql(&t, 10);
        assert!(sql.contains("CAST(\"id\" AS INTEGER) AS \"id\""), "{sql}");
        assert!(
            sql.contains("CAST(\"ratio\" AS DOUBLE) AS \"ratio\""),
            "{sql}"
        );
        assert!(sql.contains("'SELECT \"id\", CASE WHEN"), "{sql}");
        assert!(sql.contains("concat(\"note\") END AS \"note\""), "{sql}");
        assert!(sql.contains("FROM \"sales\".\"it''s\" LIMIT 10'"), "{sql}");
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
