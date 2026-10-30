// SPDX-License-Identifier: Apache-2.0
/* service/sources/mod.rs - nguồn dữ liệu là database Postgres của tổ chức (#165, bản tối thiểu, chỉ Postgres).

Cách đọc: chụp một bảng thành một phiên bản Parquet như nạp file, không truy vấn thẳng database ngoài lúc hỏi. Nhờ
vậy hồ sơ cột, bằng chứng, dòng nguồn và câu hỏi cũ vẫn tái lập được trên đúng bản đã chụp; "Chụp lại" tạo phiên bản
mới. Chép bảng ở postgres.rs (lý do không dùng extension của DuckDB ghi ở đó), mã hóa ở secret.rs, rồi đi tiếp
service/ingest.rs như một file CSV.

Bảo vệ:
- Chỉ admin của workspace làm mọi việc ở đây, kể cả xem cấu hình nguồn; member nhận 403.
- Máy chủ phải nằm trong SOURCE_ALLOWED_HOSTS, để backend không bị dùng dò máy trong mạng nội bộ.
- Thông tin kết nối mã hóa bằng SOURCE_SECRET_KEY trước khi lưu; mật khẩu không bao giờ có trong response, log hay
  prompt. Cột _source_file ghi postgres://máy/database/schema.bảng, không có tài khoản hay mật khẩu. */

mod pg_types;
mod postgres;
mod secret;

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::SourceRow;
use crate::service::ingest::{self, StagedFile, Staging, Upload, UploadResult};
use crate::{error::AppError, state::AppState};

pub use postgres::SourceTable;
use postgres::{ConnectInfo, SSL_MODES};
use secret::SecretKey;

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
    /// `disable` or `prefer` (default). TLS-only servers are not supported yet.
    pub sslmode: Option<String>,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TestSourceResponse {
    /// Number of tables and views the account can read.
    pub table_count: usize,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SnapshotRequest {
    /// `schema.table` from the table list. Not needed with `dataset_id`: the table snapshotted before is used.
    pub table: Option<String>,
    /// Name of the new dataset; defaults to the table name.
    pub name: Option<String>,
    /// Snapshot again into this dataset as a new version.
    pub dataset_id: Option<Uuid>,
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
    check_host_allowed(&state.config.source_allowed_hosts, &info.host)?;
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

/// Chụp một bảng thành dataset mới, hoặc thành phiên bản mới của dataset đã chụp từ nguồn này.
pub async fn snapshot(
    state: &AppState,
    member: &Member,
    id: Uuid,
    req: SnapshotRequest,
) -> Result<UploadResult, AppError> {
    let (row, info) = load(state, member, id).await?;
    let wanted = match (req.dataset_id, req.table.as_deref().map(str::trim)) {
        // Chụp lại: bảng lấy từ lần chụp trước, để không chụp nhầm bảng khác vào cùng dataset.
        (Some(dataset_id), _) => recorded_table(&row, dataset_id)?,
        (None, Some(table)) if !table.is_empty() => table.to_string(),
        (None, _) => return Err(AppError::BadRequest("Chọn một bảng để chụp".to_string())),
    };

    let limit = state.config.source_timeout;
    let staging = Staging::new(&state.config.data_dir)?;
    let path = staging.upload_path(0);
    let max_rows = state.config.source_max_rows;
    let max_bytes = state.config.max_upload_bytes;
    let copy = async {
        let mut conn = postgres::connect(&info, limit).await?;
        // Chỉ chụp bảng có trong danh sách đọc được: tên người dùng gửi không đi thẳng vào câu SQL.
        let table = postgres::list_tables(&mut conn)
            .await?
            .into_iter()
            .find(|t| t.full_name() == wanted)
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Không thấy bảng {wanted} trong database, hoặc tài khoản không có quyền đọc"
                ))
            })?;
        let rows = postgres::count_rows(&mut conn, &table).await?;
        if u64::try_from(rows).unwrap_or(0) > max_rows {
            return Err(AppError::TooLarge(format!(
                "Bảng {wanted} có {rows} dòng, quá giới hạn {max_rows} dòng của một lần chụp (SOURCE_MAX_ROWS)"
            )));
        }
        postgres::copy_to_csv(&mut conn, &table, max_rows, max_bytes, &path).await
    };
    let copied = match tokio::time::timeout(limit, copy).await {
        Ok(result) => result?,
        Err(_) => {
            return Err(AppError::BadRequest(
                "Chụp bảng quá thời gian cho phép (SOURCE_TIMEOUT_SECS): chọn bảng nhỏ hơn hoặc tăng giới hạn"
                    .to_string(),
            ))
        }
    };

    let label = source_file_label(&info, &wanted);
    // Băm cả nhãn nguồn lẫn nội dung: cùng nội dung mà từ bảng khác hay từ một file nạp tay thì vẫn là dữ liệu khác,
    // không bị coi là trùng phiên bản đã có ở dataset khác.
    let sha256 = Sha256::digest(format!("{label}\n{}", copied.sha256).as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let table_name = wanted.rsplit('.').next().unwrap_or(&wanted).to_string();
    let upload = Upload {
        name: Some(req.name.unwrap_or(table_name)),
        dataset_id: req.dataset_id.map(|d| d.to_string()),
        files: vec![StagedFile {
            path,
            name: label,
            sha256,
            column_types: copied.column_types,
        }],
        sheets: Vec::new(),
        database_source: Some(row.id),
    };
    let result = ingest::upload(state, member, staging, upload).await?;
    state
        .store
        .record_source_table(&member.workspace, row.id, result.dataset.id, &wanted)
        .await?;
    // Dataset trả về đọc trước khi ghi bảng vào nguồn; đọc lại để giao diện thấy ngay nguồn và bảng.
    ingest::dataset_detail(state, member, &result.dataset.id.to_string())
        .await
        .map(|dataset| UploadResult { dataset, ..result })
}

/// Nguồn của workspace kèm thông tin kết nối đã giải mã. Chỉ admin.
async fn load(
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
    check_host_allowed(&state.config.source_allowed_hosts, &info.host)?;
    Ok((row, info))
}

async fn get_row(state: &AppState, member: &Member, id: Uuid) -> Result<SourceRow, AppError> {
    state
        .store
        .get_source(&member.workspace, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Không có nguồn này".to_string()))
}

fn recorded_table(row: &SourceRow, dataset_id: Uuid) -> Result<String, AppError> {
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
            "sslmode nhận disable hoặc prefer; máy chủ chỉ nhận TLS thì bản này chưa hỗ trợ"
                .to_string(),
        ));
    }
    Ok(ConnectInfo {
        host,
        port: req.port.unwrap_or(5432),
        database: required(&req.database, "tên database")?,
        user: required(&req.user, "tài khoản")?,
        // Mật khẩu giữ nguyên, kể cả khoảng trắng ở hai đầu; rỗng được (máy chủ dùng trust hay peer).
        password: req.password,
        sslmode,
    })
}

fn check_host_allowed(allowed: &[String], host: &str) -> Result<(), AppError> {
    if allowed.iter().any(|h| h == host) {
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
        created_at: row.created_at,
    }
}

/// Giá trị cột _source_file của mọi dòng chụp từ bảng: chỉ máy chủ, database và bảng.
fn source_file_label(info: &ConnectInfo, table: &str) -> String {
    format!("postgres://{}/{}/{}", info.host, info.database, table)
}
