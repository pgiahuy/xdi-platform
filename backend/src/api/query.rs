// SPDX-License-Identifier: Apache-2.0
/* api/query.rs - hỏi dữ liệu (ask) và truy dòng nguồn (drill). Toàn bộ luồng chín bước nằm ở service/ask.rs. */

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde::Deserialize;

use crate::{error::AppError, service, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/datasets/{id}/ask", post(ask))
        .route("/datasets/{id}/drill", post(drill))
}

#[derive(Deserialize)]
pub struct AskRequest {
    pub question: String,
}

async fn ask(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AskRequest>,
) -> Result<(), AppError> {
    service::ask::ask(&state, &id, &req.question).await
}

async fn drill(State(state): State<AppState>, Path(id): Path<String>) -> Result<(), AppError> {
    service::ask::drill(&state, &id).await
}
