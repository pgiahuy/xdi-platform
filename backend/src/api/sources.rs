// SPDX-License-Identifier: Apache-2.0
/* api/sources.rs - nguồn database của workspace: thêm, thử kết nối, liệt kê bảng và quan hệ khóa ngoại, chụp bảng
thành bảng của dataset (mặc định mọi bảng đọc được), xóa. Chụp lại cả dataset ở api/catalog.rs. Logic ở
service/sources/. Mọi route chỉ cho admin của workspace (member 403), kể cả liệt kê nguồn: cấu hình nguồn (máy chủ, tài
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
    SnapshotResponse, SourceResponse, SourceTableInfo, TestSourceResponse,
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
    summary = "Tables and views the source account can read, for picking what to snapshot (admin only)",
    description = "Every table and view the account can read (its SELECT grants decide the scope), with the row \
                   estimate and size from Postgres statistics, the table comment, foreign keys in and out among \
                   readable tables, `is_referenced` when another table points to it (warn before unticking a lookup \
                   table), `over_limit` when the table alone is over SOURCE_TABLE_MAX_MB (or SOURCE_MAX_ROWS when \
                   set), and what this source already holds for it (`state`).",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    responses(
        (status = 200, body = Vec<SourceTableInfo>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "400 cannot connect, 401, 403 not admin, 404")
    )
)]
async fn tables(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
) -> Result<Json<Vec<SourceTableInfo>>, AppError> {
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
    summary = "Snapshot tables of the source into its database dataset, every readable table by default (admin only)",
    description = "Without `table`, `tables` and `dataset_id`, every table and view the account can read is \
                   snapshotted; `tables` lists the ticked tables instead, and one name snapshots that table alone \
                   (for example one kept as metadata only). All tables are copied in one read-only REPEATABLE READ \
                   transaction, then stored as Parquet versions like uploaded files; a table snapshotted before from \
                   this source gets a new version of that table, and one with unchanged content creates nothing \
                   (`created` false). Every declared foreign key between tables snapshotted from this source is \
                   recorded as a relation; relations are never inferred. Limits are checked on Postgres sizes before \
                   copying: a table over SOURCE_TABLE_MAX_MB, or that would take the snapshot over \
                   SOURCE_SNAPSHOT_MAX_MB (smallest tables first), or over SOURCE_MAX_ROWS when that is set, keeps \
                   columns, types, keys and comments only (`metadata_only`). There is no row limit by default. \
                   `dataset_id` (a table id) snapshots that table again with every table linked to it.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = Uuid, Path, description = "Source id")
    ),
    request_body = SnapshotRequest,
    responses(
        (status = 201, body = SnapshotResponse, description = "At least one new dataset or version created"),
        (status = 200, body = SnapshotResponse, description = "No table created anything (unchanged or metadata only)"),
        (status = "4XX", body = ErrorBody, description = "400 unknown table, cannot read it or cannot connect, \
                                                         401, 403 not admin, 404, 413 a copied view (no size \
                                                         known before copying) is bigger than SOURCE_TABLE_MAX_MB")
    )
)]
async fn snapshot(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, Uuid)>,
    JsonBody(req): JsonBody<SnapshotRequest>,
) -> Result<(StatusCode, Json<SnapshotResponse>), AppError> {
    let result = sources::snapshot(&state, &member, id, req).await?;
    Ok((build_snapshot_status(&result), Json(result)))
}

/// 201 khi có ít nhất một dataset hay phiên bản mới, 200 khi không có gì mới.
fn build_snapshot_status(result: &SnapshotResponse) -> StatusCode {
    if result.datasets.iter().any(|d| d.created) {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}
