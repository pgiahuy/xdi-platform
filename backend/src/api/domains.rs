// SPDX-License-Identifier: Apache-2.0
/* api/domains.rs - gói khái niệm ngành của workspace (#229): mọi thành viên xem, admin thêm, sửa, tắt khái niệm. Logic
và kiểm khái niệm ở service/domains/. Lỗi của một trường trả 400 kèm `field` trong body. Test ở api/domains_tests.rs. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::domains::contract::{Concept, ConceptRequest, ConceptsResponse};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_concepts))
        .routes(routes!(create_concept))
        .routes(routes!(update_concept))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/domains",
    tag = "domains",
    operation_id = "listConcepts",
    summary = "Industry packs of the workspace with all their concepts",
    description = "The first call in a workspace copies the packs of `data/domains/` into it; later calls add only \
                   packs and concepts it does not have yet and never overwrite an edit.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = ConceptsResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member")
    )
)]
async fn list_concepts(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<ConceptsResponse>, AppError> {
    Ok(Json(service::domains::list(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/domains/{pack}/concepts",
    tag = "domains",
    operation_id = "createConcept",
    summary = "Add a concept to a pack (admin only)",
    description = "The concept goes last in the pack and is used by the next question on a dataset with this pack.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("pack" = String, Path, description = "Pack key, for example `retail`")
    ),
    request_body = ConceptRequest,
    responses(
        (status = 201, body = Concept, description = "Created"),
        (status = "4XX", body = ErrorBody,
         description = "400 invalid concept, `field` names the field; 401; 403 not admin; 404 not a member or no such pack")
    )
)]
async fn create_concept(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, pack)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<ConceptRequest>,
) -> Result<(StatusCode, Json<Concept>), AppError> {
    let concept = service::domains::create(&state, &member, &pack, request).await?;
    Ok((StatusCode::CREATED, Json(concept)))
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/domains/{pack}/concepts/{concept_id}",
    tag = "domains",
    operation_id = "updateConcept",
    summary = "Replace a concept (admin only)",
    description = "Checked like a new concept. Send `enabled: false` to leave the concept out of the prompt.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("pack" = String, Path, description = "Pack key, for example `retail`"),
        ("concept_id" = String, Path, description = "Concept id (UUID)")
    ),
    request_body = ConceptRequest,
    responses(
        (status = 200, body = Concept, description = "Saved"),
        (status = "4XX", body = ErrorBody,
         description = "400 invalid concept, `field` names the field; 401; 403 not admin; 404 no such pack or concept")
    )
)]
async fn update_concept(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, pack, concept_id)): PathParams<(String, String, String)>,
    JsonBody(request): JsonBody<ConceptRequest>,
) -> Result<Json<Concept>, AppError> {
    let concept = service::domains::update(&state, &member, &pack, &concept_id, request).await?;
    Ok(Json(concept))
}
