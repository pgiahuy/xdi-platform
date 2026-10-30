// SPDX-License-Identifier: Apache-2.0
/* api/sources.rs - nguồn database của workspace: thêm, thử kết nối, liệt kê bảng và quan hệ khóa ngoại, chụp một nhóm
bảng thành dataset (mỗi bảng một dataset), xóa. Logic ở service/sources/. Mọi route chỉ cho admin của workspace (member 403), kể cả liệt kê nguồn: cấu hình nguồn (máy chủ, tài
khoản) cũng là thông tin cần giữ. Không route nào trả mật khẩu. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::sources::{
    self, CreateSourceRequest, RelationsRequest, RelationsResponse, SnapshotRequest,
    SnapshotResponse, SourceResponse, SourceTable, TestSourceResponse,
};
use crate::{error::AppError, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(remove))
        .routes(routes!(test))
        .routes(routes!(tables))
        .routes(routes!(relations))
        .routes(routes!(snapshot))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/sources",
    tag = "sources",
    operation_id = "listSources",
    summary = "Database sources of the workspace, without passwords (admin only)",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<SourceResponse>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 403 not admin, 404 not a member")
    )
)]
async fn list(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<SourceResponse>>, AppError> {
    Ok(Json(sources::list(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/sources",
    tag = "sources",
    operation_id = "createSource",
    summary = "Add a Postgres source (admin only)",
    description = "The connection is tested before it is saved. The host must be listed in SOURCE_ALLOWED_HOSTS. \
                   Connection details are stored encrypted with SOURCE_SECRET_KEY.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body = CreateSourceRequest,
    responses(
        (status = 201, body = SourceResponse, description = "Created"),
        (status = "4XX", body = ErrorBody, description = "400 invalid form, host not allowed or cannot connect, \
                                                         401, 403 not admin, 404 not a member"),
        (status = 503, body = ErrorBody, description = "SOURCE_SECRET_KEY not set: database sources are off")
    )
)]
async fn create(
    State(state): State<AppState>,
    member: Member,
    JsonBody(req): JsonBody<CreateSourceRequest>,
) -> Result<(StatusCode, Json<SourceResponse>), AppError> {
    let source = sources::create(&state, &member, req).await?;
    Ok((StatusCode::CREATED, Json(source)))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/sources/{id}",
    tag = "sources",
    operation_id = "deleteSource",
    summary = "Delete a source that has no datasets left (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "400 datasets still use it, 401, 403 not admin, 404")
    )
)]
async fn remove(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    sources::delete(&state, &member, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/sources/{id}/test",
    tag = "sources",
    operation_id = "testSource",
    summary = "Connect to the source and count readable tables (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    responses(
        (status = 200, body = TestSourceResponse, description = "Connected"),
        (status = "4XX", body = ErrorBody, description = "400 cannot connect, with how to fix it, 401, 403, 404")
    )
)]
async fn test(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
) -> Result<Json<TestSourceResponse>, AppError> {
    Ok(Json(sources::test(&state, &member, id).await?))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/sources/{id}/tables",
    tag = "sources",
    operation_id = "listSourceTables",
    summary = "Tables and views the source account can read (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    responses(
        (status = 200, body = Vec<SourceTable>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "400 cannot connect, 401, 403 not admin, 404")
    )
)]
async fn tables(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
) -> Result<Json<Vec<SourceTable>>, AppError> {
    Ok(Json(sources::tables(&state, &member, id).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/sources/{id}/relations",
    tag = "sources",
    operation_id = "listSourceRelations",
    summary = "Tables linked to a table by foreign keys, in both directions (admin only)",
    description = "Read from pg_catalog, one hop around the table. `many_to_one` lists the tables its foreign keys \
                   point to, `one_to_many` the tables whose foreign keys point to it. A table is addable to the \
                   snapshot when the account can read it and the referenced columns are the primary key or a UNIQUE \
                   constraint; otherwise `reason` says why.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    request_body = RelationsRequest,
    responses(
        (status = 200, body = RelationsResponse, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "400 unknown table or cannot connect, 401, 403 not admin, 404")
    )
)]
async fn relations(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<RelationsRequest>,
) -> Result<Json<RelationsResponse>, AppError> {
    Ok(Json(sources::relations(&state, &member, id, req).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/sources/{id}/snapshot",
    tag = "sources",
    operation_id = "snapshotSource",
    summary = "Snapshot a table and related tables, one dataset per table (admin only)",
    description = "Every table is copied in one read-only REPEATABLE READ transaction, then stored as a Parquet \
                   version like an uploaded file. A table snapshotted before from this source gets a new version of \
                   its dataset. Foreign keys among the tables are recorded as dataset relations. A snapshot again \
                   (`dataset_id`) takes every dataset of this source linked to it. At most 10 tables; \
                   SOURCE_MAX_ROWS and MAX_UPLOAD_MB apply to each table. A table with the same content as an \
                   existing version creates nothing (`created` false); the status is 200 when no table created \
                   anything.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    request_body = SnapshotRequest,
    responses(
        (status = 201, body = SnapshotResponse, description = "At least one new dataset or version created"),
        (status = 200, body = SnapshotResponse, description = "Every table matches an existing version: nothing created"),
        (status = "4XX", body = ErrorBody, description = "400 unknown table, cannot read it, more than 10 tables, \
                                                         401, 403 not admin, 404, 413 a table has more rows than \
                                                         SOURCE_MAX_ROWS or is bigger than MAX_UPLOAD_MB")
    )
)]
async fn snapshot(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<SnapshotRequest>,
) -> Result<(StatusCode, Json<SnapshotResponse>), AppError> {
    let result = sources::snapshot(&state, &member, id, req).await?;
    let status = if result.datasets.iter().any(|d| d.created) {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)))
}
