// SPDX-License-Identifier: Apache-2.0
/* api/data_roles.rs - vai dữ liệu của workspace (#45, #167): vai, luật lọc dòng của từng vai, vai của từng thành viên.
Chỉ admin, trừ data-roles/mine. Logic ở service/data_roles/, test ở api/data_roles_tests.rs. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::engine::store::DataRoleRow;
use crate::error::ErrorBody;
use crate::service::data_roles::contract::{
    DataRoleRequest, DataRoleResponse, FilterColumnResponse, MemberDataRolesRequest,
    SetPoliciesRequest, SetPoliciesResponse,
};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(mine))
        .routes(routes!(update, delete))
        .routes(routes!(set_policies))
        .routes(routes!(set_member_roles))
        .routes(routes!(filter_columns))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/data-roles",
    tag = "data-roles",
    operation_id = "listDataRoles",
    summary = "Data roles of the workspace with their row rules and members (admin only)",
    description = "A member who holds data roles sees only the rows their roles allow. Within one role, rules apply \
                   together (AND); between roles a row is visible when one role allows it (OR), counting only roles \
                   with a rule on the table read or on a table it points to. Admins are never filtered.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<DataRoleResponse>),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member")
    )
)]
async fn list(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<Vec<DataRoleResponse>>, AppError> {
    Ok(Json(service::data_roles::list(&state, &member).await?))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/data-roles/mine",
    tag = "data-roles",
    operation_id = "listMyDataRoles",
    summary = "Data roles of the signed-in member, without their rules",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<DataRoleRow>),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member")
    )
)]
async fn mine(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<Vec<DataRoleRow>>, AppError> {
    Ok(Json(service::data_roles::mine(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/data-roles",
    tag = "data-roles",
    operation_id = "createDataRole",
    summary = "Create a data role without rules (admin only)",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body = DataRoleRequest,
    responses(
        (status = 201, body = DataRoleRow, description = "Created"),
        (status = "4XX", body = ErrorBody, description = "400 empty, too long or taken name; 401; 403 not admin; \
                                                         404 not a member")
    )
)]
async fn create(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
    JsonBody(req): JsonBody<DataRoleRequest>,
) -> Result<(StatusCode, Json<DataRoleRow>), AppError> {
    let role = service::data_roles::create(&state, &member, &req).await?;
    Ok((StatusCode::CREATED, Json(role)))
}

#[utoipa::path(
    patch,
    path = "/workspaces/{ws}/data-roles/{role_id}",
    tag = "data-roles",
    operation_id = "updateDataRole",
    summary = "Rename a data role or change its description (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("role_id" = Uuid, Path, description = "Data role id")
    ),
    request_body = DataRoleRequest,
    responses(
        (status = 200, body = DataRoleRow),
        (status = "4XX", body = ErrorBody, description = "400 empty, too long or taken name; 401; 403 not admin; \
                                                         404 not a member or no such role")
    )
)]
async fn update(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, role_id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<DataRoleRequest>,
) -> Result<Json<DataRoleRow>, AppError> {
    Ok(Json(
        service::data_roles::update(&state, &member, role_id, &req).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/data-roles/{role_id}",
    tag = "data-roles",
    operation_id = "deleteDataRole",
    summary = "Delete a data role and its rules (admin only)",
    description = "Refused while a member holds the role: a member left without roles would see every row.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("role_id" = Uuid, Path, description = "Data role id")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "400 still held by members; 401; 403 not admin; 404 not a \
                                                         member or no such role")
    )
)]
async fn delete(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, role_id)): PathParams<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    service::data_roles::delete(&state, &member, role_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/data-roles/{role_id}/policies",
    tag = "data-roles",
    operation_id = "setDataRolePolicies",
    summary = "Replace the row rules of a data role (admin only)",
    description = "Each rule names a table, a dimension column of its current version and the values the role sees. \
                   Values missing from the column profile are saved with a warning. Takes effect from the next \
                   request of each member.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("role_id" = Uuid, Path, description = "Data role id")
    ),
    request_body = SetPoliciesRequest,
    responses(
        (status = 200, body = SetPoliciesResponse),
        (status = "4XX", body = ErrorBody, description = "400 unknown table or column, not a dimension column, two \
                                                         rules on one column, same rules as another role, or no rules \
                                                         left on a role held by members; 401; 403 not admin; 404 not \
                                                         a member or no such role")
    )
)]
async fn set_policies(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, role_id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<SetPoliciesRequest>,
) -> Result<Json<SetPoliciesResponse>, AppError> {
    Ok(Json(
        service::data_roles::set_policies(&state, &member, role_id, req).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/members/{user_id}/data-roles",
    tag = "data-roles",
    operation_id = "setMemberDataRoles",
    summary = "Replace the data roles of a member (admin only)",
    description = "An empty list removes every role: the member then sees every row. Only roles with at least one \
                   rule can be given.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("user_id" = Uuid, Path, description = "Account id of the member")
    ),
    request_body = MemberDataRolesRequest,
    responses(
        (status = 204, description = "Saved"),
        (status = "4XX", body = ErrorBody, description = "400 a role not in this workspace or without rules; 401; 403 \
                                                         not admin; 404 not a member")
    )
)]
async fn set_member_roles(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, user_id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<MemberDataRolesRequest>,
) -> Result<StatusCode, AppError> {
    service::data_roles::set_member_roles(&state, &member, user_id, &req.role_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/filter-columns",
    tag = "data-roles",
    operation_id = "listFilterColumns",
    summary = "Columns of a table a row rule can use, with their known values (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Table id (`DatasetTable.id`)")
    ),
    responses(
        (status = 200, body = Vec<FilterColumnResponse>),
        (status = "4XX", body = ErrorBody, description = "400 unknown table or no data yet; 401; 403 not admin; 404 \
                                                         not a member")
    )
)]
async fn filter_columns(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
) -> Result<Json<Vec<FilterColumnResponse>>, AppError> {
    Ok(Json(
        service::data_roles::filter_columns(&state, &member, id).await?,
    ))
}
