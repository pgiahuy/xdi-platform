// SPDX-License-Identifier: Apache-2.0
/* api/datasets/upload.rs - nạp file thành dataset mới hoặc phiên bản mới (route create). Nhận multipart qua
receive của mod.rs, rồi gọi service/ingest.rs. */

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use utoipa::ToSchema;

use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::ingest::{self, UploadResult};
use crate::{error::AppError, state::AppState};

// Hai struct dưới chỉ để mô tả form multipart trong đặc tả OpenAPI; handler đọc từng trường bằng Multipart (receive),
// không dựng struct này, nên tắt cảnh báo trường không được đọc.
/// Multipart form for uploading a dataset. Total size is limited by MAX_UPLOAD_MB (413 above it).
#[derive(ToSchema)]
#[allow(dead_code)]
struct UploadDatasetRequest {
    /// One or more CSV or Parquet files (repeat the field), or one Excel `.xlsx` file. Files with the same columns are
    /// stacked into one table.
    #[schema(value_type = Vec<String>, format = Binary)]
    file: Vec<Vec<u8>>,
    /// Name of the new dataset; defaults to the first file name.
    name: Option<String>,
    /// Add a new version to this dataset instead of creating a new one.
    dataset_id: Option<String>,
    /// Sheet of the Excel file to upload; repeat to stack several sheets with the same columns. Not needed when the
    /// file has one sheet.
    sheet: Option<Vec<String>>,
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets",
    tag = "datasets",
    operation_id = "uploadDataset",
    summary = "Upload files as a new dataset or a new version of one (admin only)",
    description = "Each upload is a version. Uploading exactly the files of an existing version creates nothing and \
                   returns 200 with `created` false.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body(content = UploadDatasetRequest, content_type = "multipart/form-data"),
    responses(
        (status = 201, body = UploadResult, description = "New dataset or version created"),
        (status = 200, body = UploadResult, description = "Same files as an existing version: nothing created"),
        (status = "4XX", body = ErrorBody, description = "400 unreadable or mismatched files, 401, 403 not admin, \
                                                         404 not a member or unknown dataset_id, 413 too large")
    )
)]
pub(super) async fn create(
    State(state): State<AppState>,
    member: Member,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<(StatusCode, Json<UploadResult>), AppError> {
    // Kiểm vai trước khi nhận file: member không phải chờ gửi xong cả tệp mới biết mình không có quyền.
    member.require_admin()?;
    let (staging, upload) = super::receive(&state, multipart).await?;
    let result = ingest::upload(&state, &member, staging, upload).await?;
    let status = if result.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)))
}
