// SPDX-License-Identifier: Apache-2.0
/* api/datasets/sheets.rs - liệt kê sheet của một file Excel để người dùng chọn trước khi nạp (route sheets).
File đọc xong bỏ, không lưu. */

use axum::extract::{Multipart, State};
use axum::Json;
use utoipa::ToSchema;

use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::excel::{self, SheetSummary};
use crate::{error::AppError, state::AppState};

/// Multipart form for listing the sheets of an Excel file. The file is not stored.
#[derive(ToSchema)]
#[allow(dead_code)]
struct ListSheetsRequest {
    /// One Excel `.xlsx` file.
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

// Liệt kê sheet của một file Excel để người dùng chọn trước khi nạp. File không được lưu lại.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/sheets",
    tag = "datasets",
    operation_id = "listExcelSheets",
    summary = "Sheets of an Excel file, to choose which to upload (admin only)",
    description = "The file is read and discarded.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body(content = ListSheetsRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, body = Vec<SheetSummary>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "400 not a readable Excel file, 401, 403 not admin, \
                                                         404 not a member, 413 too large")
    )
)]
pub(super) async fn sheets(
    State(state): State<AppState>,
    member: Member,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<Json<Vec<SheetSummary>>, AppError> {
    member.require_admin()?;
    let (staging, upload) = super::receive(&state, multipart).await?;
    Ok(Json(excel::list_sheets(&state, staging, upload).await?))
}
