// SPDX-License-Identifier: Apache-2.0
/* api/datasets/read.rs - liệt kê, xem và xóa một dataset của workspace (route list, detail, remove). */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use super::super::extract::PathParams;
use crate::auth::Member;
use crate::engine::store::Dataset;
use crate::error::ErrorBody;
use crate::service::ingest;
use crate::{error::AppError, state::AppState};

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets",
    tag = "datasets",
    operation_id = "listDatasets",
    summary = "Tables of the workspace, every dataset mixed (deprecated: use listCatalog)",
    description = "Each item is one table; `parent_id` is its dataset in `GET .../catalog`. Kept for the current UI.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<Dataset>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member")
    )
)]
// Giao diện cũ còn gọi; #257 chuyển sang GET .../catalog. deprecated đi vào đặc tả (utoipa đọc thuộc tính này).
#[deprecated(note = "use GET /workspaces/{ws}/catalog")]
pub(super) async fn list(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<Dataset>>, AppError> {
    Ok(Json(ingest::list_datasets(&state, &member).await?))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}",
    tag = "datasets",
    operation_id = "getDataset",
    summary = "One dataset of the workspace",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = Dataset, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member or no such dataset")
    )
)]
pub(super) async fn detail(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<Dataset>, AppError> {
    Ok(Json(ingest::dataset_detail(&state, &member, &id).await?))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/datasets/{id}",
    tag = "datasets",
    operation_id = "deleteDataset",
    summary = "Delete a dataset with all its versions, files and question history (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "401, 403 not admin, 404 not a member or no such dataset")
    )
)]
pub(super) async fn remove(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    ingest::delete_dataset(&state, &member, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
