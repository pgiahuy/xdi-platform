// SPDX-License-Identifier: Apache-2.0
/* api/datasets/mod.rs - liệt kê, xem, nạp dataset trong một workspace. Logic nạp ở service/ingest.rs.
Mọi route nằm dưới /workspaces/{ws}/ và nhận Member (extract.rs): chưa đăng nhập 401, không thuộc workspace 404.

Nạp file: multipart/form-data, các trường
- file: một hoặc nhiều file CSV hoặc Parquet (lặp lại trường file), hoặc một file Excel .xlsx,
- name: tên dataset mới (không bắt buộc, mặc định lấy tên file đầu),
- dataset_id: có thì thêm phiên bản cho dataset này thay vì tạo dataset mới,
- sheet: sheet cần nạp của file Excel (lặp lại để chọn nhiều sheet cùng cột; file chỉ một sheet thì bỏ trống được).
Liệt kê sheet: gửi một file Excel (trường file) tới /datasets/sheets, nhận tên sheet, cột, số dòng; file không lưu lại.
File ghi thẳng xuống đĩa từng đoạn trong lúc nhận, không giữ cả file trong bộ nhớ; SHA-256 tính cùng lúc. */
use axum::extract::{multipart::MultipartError, DefaultBodyLimit, Multipart};
use axum::http::StatusCode;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::service::ingest::{StagedFile, Staging, Upload};
use crate::{error::AppError, state::AppState};

mod read;
mod sheets;
mod upload;

#[cfg(test)]
mod tests;

/// Trần số trường multipart trong một request: chặn kiểu gửi hàng vạn trường rỗng để giữ kết nối.
const MAX_FIELDS: usize = 1000;
/// Tên file dài hơn thì từ chối: tên đi vào cột _source_file của mọi dòng.
const MAX_FILE_NAME: usize = 255;

/// max_upload_bytes: MAX_UPLOAD_MB. Giới hạn body cả request đặt rộng hơn một chút để chứa phần đầu và ranh giới của
/// multipart; nội dung các trường thì handler đếm chính xác và trả 413 JSON. Giới hạn đặt cho cả nhóm route này (layer
/// của OpenApiRouter áp cho mọi route đã thêm); hai route GET không có body nên không bị ảnh hưởng.
pub fn routes(max_upload_bytes: u64) -> OpenApiRouter<AppState> {
    let body_limit = usize::try_from(max_upload_bytes)
        .unwrap_or(usize::MAX)
        .saturating_add(1024 * 1024);
    OpenApiRouter::new()
        .routes(routes!(read::list, upload::create))
        .routes(routes!(sheets::sheets))
        .routes(routes!(read::detail))
        .layer(DefaultBodyLimit::max(body_limit))
}
/// Nhận multipart: ghi từng file xuống thư mục tạm, đọc các trường chữ, đếm tổng dung lượng.
async fn receive(
    state: &AppState,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<(Staging, Upload), AppError> {
    let mut multipart = multipart.map_err(|_| {
        AppError::BadRequest("Cần gửi file dạng multipart/form-data, trường file".to_string())
    })?;
    let staging = Staging::new(&state.config.data_dir)?;
    let mut upload = Upload {
        name: None,
        dataset_id: None,
        files: Vec::new(),
        sheets: Vec::new(),
    };
    let limit = state.config.max_upload_bytes;
    let mut received: u64 = 0;

    let mut fields = 0;
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        fields += 1;
        if fields > MAX_FIELDS {
            return Err(AppError::BadRequest(format!(
                "Quá {MAX_FIELDS} trường trong một lần nạp"
            )));
        }
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "file" => {
                let name = file_name_only(field.file_name().unwrap_or_default());
                if name.is_empty() {
                    return Err(AppError::BadRequest(
                        "Trường file thiếu tên file".to_string(),
                    ));
                }
                if name.chars().count() > MAX_FILE_NAME {
                    return Err(AppError::BadRequest(format!(
                        "Tên file dài tối đa {MAX_FILE_NAME} ký tự"
                    )));
                }
                let path = staging.upload_path(upload.files.len());
                let mut out = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| AppError::Internal(format!("create {}: {e}", path.display())))?;
                let mut hasher = Sha256::new();
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                    hasher.update(&chunk);
                    out.write_all(&chunk).await.map_err(|e| {
                        AppError::Internal(format!("write {}: {e}", path.display()))
                    })?;
                }
                out.flush()
                    .await
                    .map_err(|e| AppError::Internal(format!("flush {}: {e}", path.display())))?;
                let sha256 = hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                upload.files.push(StagedFile { path, name, sha256 });
            }
            "name" | "dataset_id" | "sheet" => {
                // Trường chữ cũng đếm vào giới hạn, để không ai gửi một "tên" dài hàng GB.
                let mut bytes = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                    bytes.extend_from_slice(&chunk);
                }
                let text = String::from_utf8(bytes).map_err(|_| {
                    AppError::BadRequest(format!("Trường {field_name} không phải chữ UTF-8"))
                })?;
                match field_name.as_str() {
                    "name" => upload.name = Some(text),
                    "dataset_id" => upload.dataset_id = Some(text),
                    _ => upload.sheets.push(text),
                }
            }
            // Trường lạ: bỏ qua nhưng vẫn đọc hết, vẫn đếm dung lượng.
            _ => {
                while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                    received += chunk.len() as u64;
                    check_limit(received, limit)?;
                }
            }
        }
    }

    Ok((staging, upload))
}

fn check_limit(received: u64, limit: u64) -> Result<(), AppError> {
    if received > limit {
        return Err(AppError::TooLarge(format!(
            "Tổng dung lượng vượt giới hạn {} MB của một lần nạp (MAX_UPLOAD_MB)",
            limit / 1024 / 1024
        )));
    }
    Ok(())
}

/// Bỏ phần thư mục nếu trình duyệt hay công cụ gửi kèm ("../../a.csv" hay đường dẫn Windows thành "a.csv"). Tên này chỉ
/// để hiển thị và ghi vào _source_file; đường dẫn trên đĩa do máy chủ đặt.
fn file_name_only(raw: &str) -> String {
    raw.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn multipart_error(e: MultipartError) -> AppError {
    tracing::debug!(error = %e, "multipart rejected");
    // Vượt giới hạn body cả request (DefaultBodyLimit) cũng đi qua đây.
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return AppError::TooLarge(
            "Tổng dung lượng vượt giới hạn của một lần nạp (MAX_UPLOAD_MB)".to_string(),
        );
    }
    AppError::BadRequest("Dữ liệu multipart bị hỏng hoặc gửi không trọn".to_string())
}
