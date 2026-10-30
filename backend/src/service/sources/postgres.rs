// SPDX-License-Identifier: Apache-2.0
/* service/sources/postgres.rs - đọc một database Postgres bên ngoài: thử kết nối, liệt kê bảng, chép một bảng ra file
CSV. service/sources/mod.rs gọi các hàm ở đây, rồi đưa file CSV vào đường nạp sẵn có (service/ingest.rs).

Vì sao không dùng extension postgres của DuckDB: INSTALL postgres tải extension từ extensions.duckdb.org lúc chạy,
đúng bản DuckDB đã biên dịch kèm. Image Docker chạy bằng user không có thư mục home (nơi DuckDB lưu extension) và máy
cài đặt có thể không ra được Internet, nên tính năng sẽ hỏng tùy nơi cài. sqlx đã có trong dự án, nói chuyện trực tiếp
với Postgres; lệnh COPY ... TO STDOUT (FORMAT csv) để Postgres tự ghi mọi kiểu cột thành chữ, DuckDB đọc lại như một
file CSV nạp tay.

Kết nối luôn chỉ đọc: phiên đặt default_transaction_read_only, kèm statement_timeout. Vẫn nên cấp cho xdi một tài
khoản chỉ có quyền SELECT (docs/INSTALL.md). Bản này chưa bật TLS (sqlx biên dịch không kèm bộ TLS), nên sslmode chỉ
nhận disable hoặc prefer. */

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};
use sqlx::{ConnectOptions, Connection, Executor};
use tokio::io::AsyncWriteExt;

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

pub const SSL_MODES: [&str; 2] = ["disable", "prefer"];

pub async fn connect(conn: &ConnectInfo, timeout: Duration) -> Result<PgConnection, AppError> {
    let ssl_mode = match conn.sslmode.as_str() {
        "disable" => PgSslMode::Disable,
        _ => PgSslMode::Prefer,
    };
    let options = PgConnectOptions::new()
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

/// Định danh SQL trong ngoặc kép, nhân đôi ngoặc kép bên trong. Tên bảng, tên cột tới đây đều đọc từ danh sách bảng
/// hay pg_catalog (relations.rs), nên không có tên nào người dùng gõ tự do đi thẳng vào câu SQL.
pub(super) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Mở giao dịch REPEATABLE READ chỉ đọc: đếm dòng và chép bảng sau đó cùng thấy một ảnh chụp MVCC của database, nên số
/// dòng đã kiểm là số dòng được chép, kể cả khi database đang được ghi. Kết thúc bằng commit_snapshot; kết nối bị
/// đóng giữa chừng thì Postgres tự hủy giao dịch (chỉ đọc, không có gì để hoàn tác).
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

/// Số dòng của bảng chính và của câu SELECT đã nối, trong một câu lệnh. Nối nhiều-một thì hai số bằng nhau; khác nhau
/// là lần nối nhân dòng (mod.rs từ chối). Không nối gì thì chỉ đếm một lần.
pub async fn count_rows(
    c: &mut PgConnection,
    fact: &str,
    select: Option<&str>,
) -> Result<(i64, i64), AppError> {
    let sql = match select {
        Some(select) => {
            format!("SELECT (SELECT count(*) FROM {fact}), (SELECT count(*) FROM ({select}) AS j)")
        }
        None => format!("SELECT count(*), count(*) FROM {fact}"),
    };
    sqlx::query_as(&sql)
        .fetch_one(&mut *c)
        .await
        .map_err(|e| query_error(&e))
}

/// Chép kết quả của câu SELECT ra file CSV có dòng tiêu đề. COPY (SELECT ...) là một câu lệnh, nên mọi bảng trong câu
/// được đọc tại cùng một thời điểm, kể cả ngoài giao dịch của begin_snapshot. Trả SHA-256 của nội dung, tính trong lúc ghi. Quá max_bytes thì dừng và báo
/// lỗi, như nạp file quá MAX_UPLOAD_MB. max_rows chặn thêm một lần nữa phòng bảng tăng dòng sau bước count_rows.
pub async fn copy_to_csv(
    c: &mut PgConnection,
    select: &str,
    max_rows: u64,
    max_bytes: u64,
    out_path: &Path,
) -> Result<String, AppError> {
    let sql = format!("COPY ({select} LIMIT {max_rows}) TO STDOUT WITH (FORMAT csv, HEADER true)");
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
                return Err(AppError::TooLarge(format!(
                    "Bảng lớn hơn giới hạn {} MB của một lần nạp (MAX_UPLOAD_MB)",
                    max_bytes / 1024 / 1024
                )));
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
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
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
        None => unreachable(conn),
    }
}

fn unreachable(conn: &ConnectInfo) -> AppError {
    AppError::BadRequest(format!(
        "Không kết nối được tới {}:{}: kiểm tên máy chủ, cổng, tường lửa, và máy chủ có nhận kết nối từ backend xdi \
         không (listen_addresses, pg_hba.conf). Máy chủ chỉ nhận TLS thì bản này chưa hỗ trợ",
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
