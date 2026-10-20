// SPDX-License-Identifier: Apache-2.0
/* api/datasets.rs - tạo, liệt kê, xem dataset. Nạp file và lập hồ sơ cột nằm ở service/ingest.rs. */

use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};

use crate::{error::AppError, service, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/datasets", get(list).post(create))
        .route("/datasets/{id}", get(detail))
}

async fn list(State(state): State<AppState>) -> Result<(), AppError> {
    service::ingest::list_datasets(&state).await
}

async fn create(State(state): State<AppState>) -> Result<(), AppError> {
    service::ingest::create_dataset(&state).await
}

async fn detail(State(state): State<AppState>, Path(id): Path<String>) -> Result<(), AppError> {
    service::ingest::dataset_detail(&state, &id).await
}
