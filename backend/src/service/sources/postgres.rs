// SPDX-License-Identifier: Apache-2.0
/* service/sources/postgres.rs - đọc một database Postgres bên ngoài bằng sqlx: thử kết nối, liệt kê bảng, đếm dòng,
chép một bảng ra file CSV. service/sources/mod.rs gọi các hàm ở đây, rồi đưa file vào đường nạp sẵn có
(service/ingest.rs).

Chép dữ liệu có hai cách, chọn bằng SOURCE_SNAPSHOT_MODE (snapshot.rs); direct là mặc định vì nhanh hơn và tốn ít đĩa
tạm hơn (docs/PERFORMANCE.md, mục Chụp bảng), copy là đường dự phòng. copy: lệnh COPY ... TO STDOUT (FORMAT csv) ở
đây để Postgres tự ghi mọi kiểu cột thành chữ, DuckDB đọc lại file đó với kiểu từng cột lấy từ Postgres (pg_types.rs),
không đoán như với file CSV nạp tay. direct (direct.rs): DuckDB đọc thẳng Postgres bằng extension postgres và ghi
Parquet một bước. Trước #246 dự án tránh extension này vì INSTALL postgres tải nó từ extensions.duckdb.org lúc chạy:
image chạy bằng user không có thư mục home, máy cài có thể không ra Internet. Nay file extension đúng bản DuckDB được
đóng sẵn lúc build và nạp bằng đường dẫn cục bộ; không nạp được thì quay về COPY ở đây. Danh sách bảng, đếm dòng, kiểu cột
và khóa ngoại luôn đọc bằng sqlx ở đây, cách nào cũng vậy.

Kết nối luôn chỉ đọc: phiên đặt default_transaction_read_only, kèm statement_timeout. Vẫn nên cấp cho xdi một tài
khoản chỉ có quyền SELECT (docs/INSTALL.md).

TLS (#230) qua rustls của sqlx, nghĩa sslmode như libpq: disable không TLS; prefer thử TLS nếu máy chủ nhận, không kiểm
chứng chỉ; require bắt buộc TLS, không kiểm chứng chỉ (chống nghe lén, không chống giả máy chủ); verify-full bắt buộc
TLS, chứng chỉ phải do CA tin cậy cấp và đúng tên máy. CA tin cậy là CA của hệ thống, cộng CA người quản trị dán vào
(ssl_root_cert, dạng PEM) khi máy chủ dùng CA riêng của tổ chức. */

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};
use sqlx::{ConnectOptions, Connection, Executor};
use tokio::io::AsyncWriteExt;

use super::pg_types;
use crate::engine::analytics::FileFormat;
use crate::error::AppError;

/// Thông tin kết nối đầy đủ, kể cả mật khẩu. Chỉ tồn tại trong bộ nhớ lúc kết nối và dưới dạng đã mã hóa trong
/// database (secret.rs). Không derive Debug để không lỡ ghi ra log.
#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectInfo {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub sslmode: String,
    /// Chứng chỉ CA dạng PEM để kiểm máy chủ với verify-full. Nguồn thêm trước khi có TLS không có trường này.
    #[serde(default)]
    pub ssl_root_cert: Option<String>,
}

// Một bảng hay view đọc được.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SourceTable {
    pub schema: String,
    pub name: String,
    /// `table` or `view`.
    pub kind: String,
}

impl SourceTable {
    /// Tên hiển thị và tên gửi lại khi chụp: schema.table.
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    /// Cùng một bảng: so schema và tên, không so kind (bảng đọc từ pg_catalog luôn ghi kind table).
    pub fn same(&self, other: &SourceTable) -> bool {
        self.schema == other.schema && self.name == other.name
    }
}

pub const SSL_MODES: [&str; 4] = ["disable", "prefer", "require", "verify-full"];

pub async fn connect(conn: &ConnectInfo, timeout: Duration) -> Result<PgConnection, AppError> {
    let ssl_mode = match conn.sslmode.as_str() {
        "disable" => PgSslMode::Disable,
        "require" => PgSslMode::Require,
        "verify-full" => PgSslMode::VerifyFull,
        _ => PgSslMode::Prefer,
    };
    let mut options = PgConnectOptions::new()
        .host(&conn.host)
        .port(conn.port)
        .database(&conn.database)
        .username(&conn.user)
        .password(&conn.password)
        .ssl_mode(ssl_mode)
        .application_name("xdi-platform")
        // Chỉ đọc cho mọi giao dịch của phiên, và mỗi câu không chạy quá timeout ở phía máy chủ Postgres.
        .options([
            ("default_transaction_read_only", "on".to_string()),
            ("statement_timeout", format!("{}", timeout.as_millis())),
        ])
        // Câu SQL của lần chụp không ghi vào log của xdi: chúng chỉ có tên bảng, nhưng log vẫn nên gọn.
        .disable_statement_logging();
    // CA dán vào được thêm vào cạnh CA của hệ thống, không thay chúng.
    if let Some(pem) = &conn.ssl_root_cert {
        options = options.ssl_root_cert_from_pem(pem.as_bytes().to_vec());
    }
    let connecting = PgConnection::connect_with(&options);
    match tokio::time::timeout(timeout, connecting).await {
        Ok(Ok(c)) => Ok(c),
        Ok(Err(e)) => Err(connect_error(conn, &e)),
        Err(_) => Err(unreachable(conn)),
    }
}

/// Bảng và view tài khoản này thấy được (information_schema chỉ liệt kê bảng có quyền), trừ schema hệ thống.
pub async fn list_tables(c: &mut PgConnection) -> Result<Vec<SourceTable>, AppError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT table_schema::text, table_name::text,
                CASE WHEN table_type = 'VIEW' THEN 'view' ELSE 'table' END
         FROM information_schema.tables
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
         ORDER BY 1, 2",
    )
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    Ok(rows
        .into_iter()
        .map(|(schema, name, kind)| SourceTable { schema, name, kind })
        .collect())
}

/// Định danh SQL trong ngoặc kép, nhân đôi ngoặc kép bên trong. Tên bảng còn được đối chiếu với list_tables trước
/// khi tới đây, nên không có tên nào người dùng gõ tự do đi thẳng vào câu SQL.
pub(super) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(super) fn qualified(table: &SourceTable) -> String {
    format!(
        "{}.{}",
        quote_ident(&table.schema),
        quote_ident(&table.name)
    )
}

/// Mở giao dịch REPEATABLE READ chỉ đọc: mọi lần đếm và chép bảng sau đó cùng thấy một ảnh chụp MVCC của database, nên
/// các bảng của một nhóm khớp nhau (không có dòng hàng trỏ tới đơn hàng chưa có trong bản chụp đơn hàng), kể cả khi
/// database đang được ghi. Kết thúc bằng commit_snapshot; kết nối bị đóng giữa chừng thì Postgres tự hủy giao dịch (chỉ
/// đọc, không có gì để hoàn tác).
pub async fn begin_snapshot(c: &mut PgConnection) -> Result<(), AppError> {
    c.execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(|e| query_error(&e))?;
    Ok(())
}

pub async fn commit_snapshot(c: &mut PgConnection) -> Result<(), AppError> {
    c.execute("COMMIT").await.map_err(|e| query_error(&e))?;
    Ok(())
}

pub async fn count_rows(c: &mut PgConnection, table: &SourceTable) -> Result<i64, AppError> {
    sqlx::query_scalar(&format!("SELECT count(*) FROM {}", qualified(table)))
        .fetch_one(&mut *c)
        .await
        .map_err(|e| query_error(&e))
}

/// File vừa chép một bảng: SHA-256 của nội dung, kiểu DuckDB của từng cột (pg_types.rs) và định dạng file (CSV của
/// COPY ở đây, Parquet của DuckDB ở direct.rs).
pub struct TableCopy {
    pub sha256: String,
    pub column_types: Vec<(String, String)>,
    pub format: FileFormat,
}

/// Câu SELECT chép cả bảng; pg_types::column_types đọc kiểu cột từ đúng câu này.
pub(super) fn select_all(table: &SourceTable) -> String {
    format!("SELECT * FROM {}", qualified(table))
}

/// Chép bảng ra file CSV có dòng tiêu đề. SHA-256 của nội dung tính trong lúc ghi. Quá max_bytes (SOURCE_TABLE_MAX_MB)
/// thì dừng ngay và báo lỗi, không ghi tiếp. max_rows (SOURCE_MAX_ROWS khi đặt) chặn thêm một lần nữa phòng bảng tăng
/// dòng sau bước count_rows.
pub async fn copy_to_csv(
    c: &mut PgConnection,
    table: &SourceTable,
    max_rows: Option<u64>,
    max_bytes: u64,
    out_path: &Path,
) -> Result<TableCopy, AppError> {
    let select = select_all(table);
    // Kiểu cột đọc từ chính câu SELECT được chép, nên khớp đúng cột và thứ tự cột của file CSV.
    let column_types = pg_types::column_types(c, &select).await?;
    let limit = max_rows.map(|n| format!(" LIMIT {n}")).unwrap_or_default();
    let sql = format!("COPY ({select}{limit}) TO STDOUT WITH (FORMAT csv, HEADER true)");
    let mut out = tokio::fs::File::create(out_path)
        .await
        .map_err(|e| AppError::Internal(format!("create {}: {e}", out_path.display())))?;
    let mut hasher = Sha256::new();
    let mut written: u64 = 0;
    // copy_out_raw mượn kết nối cho tới khi đọc hết luồng; khối riêng để trả kết nối trước khi đóng file.
    {
        let mut stream = c.copy_out_raw(&sql).await.map_err(|e| query_error(&e))?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| query_error(&e))?;
            written += chunk.len() as u64;
            if written > max_bytes {
                return Err(AppError::TooLarge(build_too_large(table, max_bytes)));
            }
            hasher.update(&chunk);
            out.write_all(&chunk)
                .await
                .map_err(|e| AppError::Internal(format!("write {}: {e}", out_path.display())))?;
        }
    }
    out.flush()
        .await
        .map_err(|e| AppError::Internal(format!("flush {}: {e}", out_path.display())))?;
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(TableCopy {
        sha256,
        column_types,
        format: FileFormat::DatabaseCsv,
    })
}

/// Lời báo khi file chép ra của một bảng quá SOURCE_TABLE_MAX_MB. Bảng có dung lượng đã bị loại trước khi chép
/// (plan.rs), nên lỗi này gặp với view, hay với file CSV của cách copy lớn hơn bảng trong Postgres.
pub fn build_too_large(table: &SourceTable, max_bytes: u64) -> String {
    format!(
        "Bảng {} chép ra lớn hơn giới hạn {} MB của một bảng (SOURCE_TABLE_MAX_MB): bỏ tick bảng này hoặc tăng giới hạn",
        table.full_name(),
        max_bytes / 1024 / 1024
    )
}

/// Lỗi lúc kết nối thành lời báo cách sửa. Không chép nguyên văn lỗi của sqlx vào lời báo, chỉ ghi vào log ở mức debug:
/// lời báo đi tới trình duyệt, còn lỗi gốc có thể chứa chi tiết mạng nội bộ.
fn connect_error(conn: &ConnectInfo, e: &sqlx::Error) -> AppError {
    tracing::debug!(host = %conn.host, port = conn.port, error = %e, "source connect failed");
    match e.as_database_error().and_then(|d| d.code()).as_deref() {
        // invalid_password, invalid_authorization_specification
        Some("28P01") | Some("28000") => AppError::BadRequest(format!(
            "Postgres từ chối đăng nhập của tài khoản {}: kiểm tên tài khoản, mật khẩu và pg_hba.conf của máy chủ",
            conn.user
        )),
        // invalid_catalog_name
        Some("3D000") => AppError::BadRequest(format!(
            "Máy chủ không có database {}: kiểm lại tên database",
            conn.database
        )),
        // too_many_connections, cannot_connect_now: máy chủ đầy kết nối hoặc đang khởi động.
        Some("53300") | Some("57P03") => AppError::BadRequest(
            "Máy chủ Postgres đang hết chỗ kết nối hoặc chưa sẵn sàng: thử lại sau ít phút".to_string(),
        ),
        // Mã SQLSTATE không chứa thông tin kết nối, ghi kèm để người quản trị tra cứu.
        Some(code) => AppError::BadRequest(format!(
            "Postgres từ chối kết nối (mã {code}): kiểm quyền CONNECT của tài khoản trên database này"
        )),
        None if is_tls_error(e) => tls_error(conn),
        None => unreachable(conn),
    }
}

/// Lỗi thuộc bước TLS. sqlx báo máy chủ không bật TLS bằng Error::Tls, còn chứng chỉ không qua kiểm (CA lạ, sai tên
/// máy) thì rustls báo trong lúc bắt tay, tới đây dưới dạng Error::Io bọc một rustls::Error.
fn is_tls_error(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Tls(_) => true,
        sqlx::Error::Io(io) => io
            .get_ref()
            .is_some_and(|inner| inner.downcast_ref::<rustls::Error>().is_some()),
        _ => false,
    }
}

/// Lỗi TLS: máy chủ không bật TLS (với require, verify-full), chứng chỉ không qua kiểm (verify-full), hay bắt tay TLS lỗi.
/// Với prefer: sqlx thử TLS khi máy chủ nhận TLS, và khác libpq, bắt tay lỗi thì không thử lại kết nối thường; thường do
/// máy chủ chỉ có TLS cũ (1.0, 1.1) hay chứng chỉ quá yếu, nên lời báo không khuyên chọn lại prefer.
pub(super) fn tls_error(conn: &ConnectInfo) -> AppError {
    if conn.sslmode == "prefer" {
        return AppError::BadRequest(format!(
            "Máy chủ {}:{} nhận TLS nhưng bắt tay TLS lỗi (sslmode prefer không quay về kết nối thường): máy chủ có thể \
             chỉ hỗ trợ TLS 1.0, 1.1 hay dùng chứng chỉ quá yếu. Nâng cấu hình TLS của máy chủ (TLS 1.2 trở lên), hoặc \
             chọn disable nếu mạng giữa hai máy là mạng tin cậy",
            conn.host, conn.port
        ));
    }
    AppError::BadRequest(format!(
        "Không lập được kết nối TLS tới {}:{} (sslmode {}): máy chủ phải bật TLS; với verify-full, chứng chỉ của máy chủ \
         phải do một CA tin cậy cấp (CA của hệ thống hoặc CA dán vào ô chứng chỉ CA) và ghi đúng tên máy {}. Máy chủ \
         không bật TLS thì chọn prefer hoặc disable",
        conn.host, conn.port, conn.sslmode, conn.host
    ))
}

fn unreachable(conn: &ConnectInfo) -> AppError {
    AppError::BadRequest(format!(
        "Không kết nối được tới {}:{}: kiểm tên máy chủ, cổng, tường lửa, và máy chủ có nhận kết nối từ backend xdi \
         không (listen_addresses, pg_hba.conf). Máy chủ chỉ nhận TLS thì chọn sslmode require hoặc verify-full",
        conn.host, conn.port
    ))
}

pub(super) fn query_error(e: &sqlx::Error) -> AppError {
    tracing::debug!(error = %e, "source query failed");
    match e.as_database_error().and_then(|d| d.code()).as_deref() {
        // insufficient_privilege
        Some("42501") => AppError::BadRequest(
            "Tài khoản không có quyền SELECT trên bảng này: cấp quyền (GRANT SELECT) rồi thử lại".to_string(),
        ),
        // query_canceled: statement_timeout
        Some("57014") => AppError::BadRequest(
            "Đọc bảng quá thời gian cho phép (SOURCE_TIMEOUT_SECS): chọn bảng nhỏ hơn hoặc tăng giới hạn".to_string(),
        ),
        _ => AppError::BadRequest(
            "Đọc bảng từ Postgres lỗi: thử kết nối lại, kiểm bảng còn tồn tại và tài khoản còn quyền đọc".to_string(),
        ),
    }
}
