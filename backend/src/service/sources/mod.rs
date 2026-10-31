// SPDX-License-Identifier: Apache-2.0
/* service/sources/mod.rs - nguồn dữ liệu là database Postgres của tổ chức (#165, bản tối thiểu, chỉ Postgres).

Cách đọc: chụp bảng thành phiên bản Parquet như nạp file, không truy vấn thẳng database ngoài lúc hỏi. Nhờ vậy hồ sơ
cột, bằng chứng, dòng nguồn và câu hỏi cũ vẫn tái lập được trên đúng bản đã chụp; "Chụp lại" tạo phiên bản mới. Chép
bảng ở postgres.rs (lý do không dùng extension của DuckDB ghi ở đó), mã hóa ở secret.rs, rồi đi tiếp service/ingest.rs
như một file CSV.

Một lần chụp lấy một bảng cùng các bảng nối với nó theo khóa ngoại (relations.rs tìm, snapshot.rs chụp): mỗi bảng thành
một dataset riêng, khóa ngoại giữa chúng ghi thành quan hệ giữa các dataset. Không nối thành bảng phẳng, nên không có
cột lặp lại để cộng nhầm.

Bảo vệ:
- Chỉ admin của workspace làm mọi việc ở đây, kể cả xem cấu hình nguồn; member nhận 403.
- Máy chủ phải nằm trong SOURCE_ALLOWED_HOSTS, để backend không bị dùng dò máy trong mạng nội bộ.
- Thông tin kết nối mã hóa bằng SOURCE_SECRET_KEY trước khi lưu; mật khẩu không bao giờ có trong response, log hay
  prompt. Cột _source_file ghi postgres://máy/database/schema.bảng, không có tài khoản hay mật khẩu. */

mod comments;
mod key_file;
mod pg_types;
mod postgres;
mod relations;
mod secret;
mod snapshot;

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::Member;
use crate::config::Config;
use crate::engine::store::SourceRow;
use crate::{error::AppError, state::AppState};

pub use key_file::apply_key_file;
pub use postgres::SourceTable;
use postgres::{ConnectInfo, SSL_MODES};
pub use relations::{RelationsRequest, RelationsResponse};
use secret::SecretKey;
pub use snapshot::{snapshot, snapshot_dataset, SnapshotRequest, SnapshotResponse};

const KIND_POSTGRES: &str = "postgres";

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateSourceRequest {
    /// Host name or IP; must be listed in SOURCE_ALLOWED_HOSTS.
    pub host: String,
    /// Defaults to 5432.
    pub port: Option<u16>,
    pub database: String,
    pub user: String,
    /// Stored encrypted; never returned.
    pub password: String,
    /// `disable`, `prefer` (default), `require` (TLS, certificate not checked) or `verify-full` (TLS, certificate
    /// signed by a trusted CA and matching the host name).
    pub sslmode: Option<String>,
    /// PEM CA certificate(s) trusted for `verify-full`, besides the system CAs. Only with `verify-full`.
    pub ssl_root_cert: Option<String>,
}

/// A database source without its password.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SourceResponse {
    pub id: Uuid,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub sslmode: String,
    /// True when the source was added with its own CA certificate.
    pub custom_ca: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TestSourceResponse {
    /// Number of tables and views the account can read.
    pub table_count: usize,
}

/// Khóa mã hóa; thiếu hay sai thì tính năng tắt, lời báo nói cách đặt khóa.
fn secret_key(state: &AppState) -> Result<SecretKey, AppError> {
    SecretKey::from_base64(state.config.source_secret_key.expose())
}

pub async fn list(state: &AppState, member: &Member) -> Result<Vec<SourceResponse>, AppError> {
    member.require_admin()?;
    let rows = state.store.list_sources(&member.workspace).await?;
    Ok(rows.iter().map(build_response).collect())
}

pub async fn create(
    state: &AppState,
    member: &Member,
    req: CreateSourceRequest,
) -> Result<SourceResponse, AppError> {
    member.require_admin()?;
    let key = secret_key(state)?;
    let info = build_connect_info(req)?;
    check_host_allowed(&state.config, &info.host, info.port)?;
    // Thử kết nối trước khi lưu: nguồn sai mật khẩu hay không tới được thì báo ngay, không để lại dòng hỏng.
    let mut conn = postgres::connect(&info, state.config.source_timeout).await?;
    postgres::list_tables(&mut conn).await?;
    let plain = serde_json::to_vec(&info)
        .map_err(|e| AppError::Internal(format!("encode source secret: {e}")))?;
    let sealed = key.encrypt(member.workspace.as_uuid(), &plain)?;
    let row = state
        .store
        .create_source(
            &member.workspace,
            KIND_POSTGRES,
            // Tên dataset của database là tên database; giao diện ghi kèm máy chủ khi cần phân biệt.
            &info.database,
            &public_config(&info),
            &sealed,
        )
        .await?;
    tracing::info!(source_id = %row.id, host = %info.host, "database source created");
    Ok(build_response(&row))
}

pub async fn test(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<TestSourceResponse, AppError> {
    let tables = tables(state, member, id).await?;
    Ok(TestSourceResponse {
        table_count: tables.len(),
    })
}

pub async fn tables(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<Vec<SourceTable>, AppError> {
    let (_, info) = load(state, member, id).await?;
    let mut conn = postgres::connect(&info, state.config.source_timeout).await?;
    postgres::list_tables(&mut conn).await
}

/// Bảng nối với một bảng theo khóa ngoại, cả hai chiều, kèm lý do khi một bảng không chụp kèm được.
pub async fn relations(
    state: &AppState,
    member: &Member,
    id: Uuid,
    req: RelationsRequest,
) -> Result<RelationsResponse, AppError> {
    let (_, info) = load(state, member, id).await?;
    let mut conn = postgres::connect(&info, state.config.source_timeout).await?;
    let readable = postgres::list_tables(&mut conn).await?;
    let table = find_table_in(&readable, req.table.trim())?;
    relations::discover(&mut conn, &table).await
}

pub async fn delete(state: &AppState, member: &Member, id: Uuid) -> Result<(), AppError> {
    member.require_admin()?;
    get_row(state, member, id).await?;
    if !state.store.delete_source(&member.workspace, id).await? {
        return Err(AppError::BadRequest(
            "Nguồn còn dataset chụp từ nó: xóa các dataset đó trước rồi xóa nguồn".to_string(),
        ));
    }
    Ok(())
}

/// Bảng có trong danh sách đọc được của tài khoản: tên người dùng gửi không đi thẳng vào câu SQL.
pub(super) fn find_table_in(
    readable: &[SourceTable],
    wanted: &str,
) -> Result<SourceTable, AppError> {
    readable
        .iter()
        .find(|t| t.full_name() == wanted)
        .cloned()
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "Không thấy bảng {wanted} trong database, hoặc tài khoản không có quyền đọc"
            ))
        })
}

/// Nguồn của workspace kèm thông tin kết nối đã giải mã. Chỉ admin.
pub(super) async fn load(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<(SourceRow, ConnectInfo), AppError> {
    member.require_admin()?;
    let key = secret_key(state)?;
    let row = get_row(state, member, id).await?;
    let sealed = state.store.source_secret(&member.workspace, id).await?;
    let plain = key.decrypt(member.workspace.as_uuid(), &sealed)?;
    let info: ConnectInfo = serde_json::from_slice(&plain)
        .map_err(|e| AppError::Internal(format!("decode source secret: {e}")))?;
    // Danh sách máy chủ được phép có thể đã hẹp lại sau khi thêm nguồn: kiểm lại mỗi lần kết nối.
    check_host_allowed(&state.config, &info.host, info.port)?;
    Ok((row, info))
}

async fn get_row(state: &AppState, member: &Member, id: Uuid) -> Result<SourceRow, AppError> {
    state
        .store
        .get_source(&member.workspace, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không có nguồn này".to_string()))
}

pub(super) fn recorded_table(row: &SourceRow, dataset_id: Uuid) -> Result<String, AppError> {
    row.config["tables"][dataset_id.to_string()]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("Dataset này không được chụp từ nguồn này".to_string()))
}

fn build_connect_info(req: CreateSourceRequest) -> Result<ConnectInfo, AppError> {
    let host = req.host.trim().to_lowercase();
    // Tên máy hay IP, không nhận URL hay chuỗi có khoảng trắng: phần kiểm danh sách cho phép so đúng chuỗi này.
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
    {
        return Err(AppError::BadRequest(
            "Máy chủ là tên máy hoặc địa chỉ IP, ví dụ db.example.com".to_string(),
        ));
    }
    let required = |value: &str, what: &str| {
        let value = value.trim();
        if value.is_empty() {
            Err(AppError::BadRequest(format!("Thiếu {what}")))
        } else {
            Ok(value.to_string())
        }
    };
    let sslmode = req.sslmode.unwrap_or_else(|| "prefer".to_string());
    if !SSL_MODES.contains(&sslmode.as_str()) {
        return Err(AppError::BadRequest(
            "sslmode nhận disable, prefer, require hoặc verify-full".to_string(),
        ));
    }
    let ssl_root_cert = build_root_cert(req.ssl_root_cert, &sslmode)?;
    Ok(ConnectInfo {
        host,
        port: req.port.unwrap_or(5432),
        database: required(&req.database, "tên database")?,
        user: required(&req.user, "tài khoản")?,
        // Mật khẩu giữ nguyên, kể cả khoảng trắng ở hai đầu; rỗng được (máy chủ dùng trust hay peer).
        password: req.password,
        sslmode,
        ssl_root_cert,
    })
}

/// Chứng chỉ CA dán vào: bỏ trống thì không có. Chỉ nhận với verify-full, vì prefer và require không kiểm chứng chỉ
/// nên CA dán vào sẽ bị bỏ qua mà người dùng tưởng là có kiểm. Kiểm đọc được ít nhất một chứng chỉ PEM ngay lúc thêm,
/// để lỗi dán thiếu dòng không hiện ra thành lỗi TLS khó hiểu lúc kết nối.
fn build_root_cert(pem: Option<String>, sslmode: &str) -> Result<Option<String>, AppError> {
    use rustls::pki_types::{pem::PemObject, CertificateDer};

    let Some(pem) = pem.filter(|p| !p.trim().is_empty()) else {
        return Ok(None);
    };
    if sslmode != "verify-full" {
        return Err(AppError::BadRequest(
            "Chứng chỉ CA chỉ dùng với sslmode verify-full: chọn verify-full hoặc bỏ trống ô chứng chỉ".to_string(),
        ));
    }
    let certs: Result<Vec<_>, _> = CertificateDer::pem_slice_iter(pem.as_bytes()).collect();
    match certs {
        Ok(certs) if !certs.is_empty() => Ok(Some(pem)),
        _ => Err(AppError::BadRequest(
            "Không đọc được chứng chỉ CA: dán nguyên văn file PEM, từ dòng -----BEGIN CERTIFICATE----- tới dòng \
             -----END CERTIFICATE-----"
                .to_string(),
        )),
    }
}

fn check_host_allowed(config: &Config, host: &str, port: u16) -> Result<(), AppError> {
    if config.source_host_allowed(host, port) {
        return Ok(());
    }
    Err(AppError::BadRequest(format!(
        "Máy chủ {host} không nằm trong danh sách được phép: người quản trị cài đặt thêm nó vào SOURCE_ALLOWED_HOSTS"
    )))
}

/// Phần lưu dạng chữ thường trong config: đủ để hiển thị, không có mật khẩu.
fn public_config(info: &ConnectInfo) -> Value {
    json!({
        "host": info.host,
        "port": info.port,
        "database": info.database,
        "user": info.user,
        "sslmode": info.sslmode,
        "custom_ca": info.ssl_root_cert.is_some(),
        "tables": {},
    })
}

fn build_response(row: &SourceRow) -> SourceResponse {
    let text = |key: &str| row.config[key].as_str().unwrap_or_default().to_string();
    SourceResponse {
        id: row.id,
        kind: row.kind.clone(),
        host: text("host"),
        port: row.config["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(5432),
        database: text("database"),
        user: text("user"),
        sslmode: text("sslmode"),
        custom_ca: row.config["custom_ca"].as_bool().unwrap_or(false),
        created_at: row.created_at,
    }
}

/// Giá trị cột _source_file của mọi dòng chụp từ bảng: chỉ máy chủ, database và bảng.
pub(super) fn source_file_label(info: &ConnectInfo, table: &str) -> String {
    format!("postgres://{}/{}/{}", info.host, info.database, table)
}
