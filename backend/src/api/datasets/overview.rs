// SPDX-License-Identifier: Apache-2.0
/* api/datasets/overview.rs - route tổng quan một dataset (#198): hồ sơ cột, chỉ số và biểu đồ dựng sẵn cho mọi thành
viên. Logic ở service/overview/mod.rs, hợp đồng ở service/overview/contract.rs. */

use axum::extract::State;
use axum::Json;

use super::super::extract::PathParams;
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::overview::{self, contract::DatasetOverviewResponse};
use crate::{error::AppError, state::AppState};

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/overview",
    tag = "datasets",
    operation_id = "getDatasetOverview",
    summary = "What a dataset holds: summary, column profile, metrics and pre-built charts",
    description = "For every member who can ask the dataset. No language model is called. Charts that read the data \
                   go through the SQL guard with the viewer's row filters and are kept per version; row counts by \
                   value come from the column profile. Sensitive columns return no values.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = DatasetOverviewResponse, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member or no such dataset")
    )
)]
pub(super) async fn overview(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<DatasetOverviewResponse>, AppError> {
    Ok(Json(overview::build_overview(&state, &member, &id).await?))
}
