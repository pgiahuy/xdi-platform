// SPDX-License-Identifier: Apache-2.0
/* api/query.rs - hỏi dữ liệu (ask) và truy dòng nguồn (drill). Toàn bộ luồng chín bước nằm ở service/ask.rs.
Route nằm dưới /workspaces/{ws}/ và nhận Member: chỉ thành viên workspace hỏi được dữ liệu của nó. */

use axum::{extract::State, routing::post, Router};
use serde::Deserialize;

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/datasets/{id}/ask", post(ask))
        .route("/workspaces/{ws}/datasets/{id}/drill", post(drill))
}

#[derive(Deserialize)]
pub struct AskRequest {
    pub question: String,
}

async fn ask(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(req): JsonBody<AskRequest>,
) -> Result<(), AppError> {
    service::ask::ask(&state, &member, &id, &req.question).await
}

async fn drill(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<(), AppError> {
    service::ask::drill(&state, &member, &id).await
}
