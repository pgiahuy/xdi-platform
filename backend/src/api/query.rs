// SPDX-License-Identifier: Apache-2.0
/* api/query.rs - hỏi dữ liệu (ask) và truy dòng nguồn (drill). Toàn bộ luồng chín bước nằm ở service/ask.rs.
Route nằm dưới /workspaces/{ws}/ và nhận Member: chỉ thành viên workspace hỏi được dữ liệu của nó. */

use axum::extract::State;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ask))
        .routes(routes!(drill))
}

#[derive(Deserialize, ToSchema)]
pub struct AskRequest {
    pub question: String,
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/{id}/ask",
    tag = "query",
    operation_id = "ask",
    summary = "Ask a question about a dataset (not implemented yet)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = AskRequest,
    responses(
        (status = 501, body = ErrorBody, description = "Not implemented yet"),
        (status = "4XX", body = ErrorBody, description = "400 invalid JSON body, 401, 404 not a member")
    )
)]
async fn ask(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(req): JsonBody<AskRequest>,
) -> Result<(), AppError> {
    service::ask::ask(&state, &member, &id, &req.question).await
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/datasets/{id}/drill",
    tag = "query",
    operation_id = "drill",
    summary = "Source rows behind a number (not implemented yet)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 501, body = ErrorBody, description = "Not implemented yet"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member")
    )
)]
async fn drill(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<(), AppError> {
    service::ask::drill(&state, &member, &id).await
}
