// SPDX-License-Identifier: Apache-2.0
/* api/datasets.rs - liệt kê, xem, nạp dataset trong một workspace. Nạp file và lập hồ sơ cột nằm ở service/ingest.rs.
Mọi route nằm dưới /workspaces/{ws}/ và nhận Member (extract.rs): chưa đăng nhập 401, không thuộc workspace 404. */

use axum::{extract::State, routing::get, Json, Router};

use super::extract::PathParams;
use crate::auth::Member;
use crate::engine::store::Dataset;
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/datasets", get(list).post(create))
        .route("/workspaces/{ws}/datasets/{id}", get(detail))
}

async fn list(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<Dataset>>, AppError> {
    Ok(Json(service::ingest::list_datasets(&state, &member).await?))
}

async fn create(State(state): State<AppState>, member: Member) -> Result<(), AppError> {
    service::ingest::create_dataset(&state, &member).await
}

async fn detail(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<Dataset>, AppError> {
    Ok(Json(
        service::ingest::dataset_detail(&state, &member, &id).await?,
    ))
}
