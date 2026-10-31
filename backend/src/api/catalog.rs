// SPDX-License-Identifier: Apache-2.0
/* api/catalog.rs - dataset của workspace (#257): một database đã kết nối hay một file, bảng nằm bên trong. Liệt kê và
xem dataset kèm bảng, chụp lại cả dataset, hỏi trên dataset, cuộc trò chuyện trên dataset. Logic ở service/catalog/,
kiểu response ở service/catalog/contract.rs.

Đường dẫn là /catalog chứ không phải /datasets vì /datasets/{id} đã là route của một bảng (id bảng) từ trước, giao diện
cũ còn dùng; hai nghĩa trên một đường dẫn thì không khai được kiểu response. Route của bảng (tổng quan, chỉ số, truy dòng
nguồn, mở một cuộc) vẫn ở /datasets/{id} với id là DatasetTable.id. Mọi thành viên đọc và hỏi được; chụp lại chỉ admin. */

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::ask::contract::{AskResponse, ChosenOption};
use crate::service::catalog::contract::DatasetSummary;
use crate::service::conversation::contract::ConversationListResponse;
use crate::service::sources::SnapshotResponse;
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(detail))
        .routes(routes!(snapshot))
        .routes(routes!(ask))
        .routes(routes!(conversations))
}

// Cùng các trường như AskRequest của api/query.rs, thêm table_id. Không gộp bằng serde(flatten): kiểu TypeScript sinh
// ra từ allOf khó đọc hơn một object phẳng.
/// A question on a whole dataset, or starting from one of its tables.
#[derive(Deserialize, ToSchema)]
pub struct DatasetAskRequest {
    pub question: String,
    /// Same as `AskRequest.choices`.
    #[serde(default)]
    pub choices: Vec<ChosenOption>,
    /// Conversation to continue, from `AskResponse.conversation_id`. A conversation on any table of this dataset can
    /// be continued; one of another user or another dataset is 404.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    /// Table to start from (`DatasetTable.id`). Leave out to let the backend pick it from the question; the answer
    /// tells which one in `AskResponse.table_id`. Either way the question also reads tables linked to it by relations
    /// (two steps, eight tables at most).
    #[serde(default)]
    pub table_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/catalog",
    tag = "catalog",
    operation_id = "listCatalog",
    summary = "Datasets of the workspace with their tables",
    description = "A dataset is one connected database or one uploaded file. Oldest first. Each dataset lists its \
                   tables (with columns of the current version) and the relations between them.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = Vec<DatasetSummary>, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member")
    )
)]
async fn list(
    State(state): State<AppState>,
    member: Member,
) -> Result<Json<Vec<DatasetSummary>>, AppError> {
    Ok(Json(service::catalog::list(&state, &member).await?))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/catalog/{id}",
    tag = "catalog",
    operation_id = "getCatalogDataset",
    summary = "One dataset with its tables",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = DatasetSummary, description = "OK"),
        (status = "4XX", body = ErrorBody, description = "401, 404 not a member or no such dataset")
    )
)]
async fn detail(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<DatasetSummary>, AppError> {
    Ok(Json(service::catalog::detail(&state, &member, &id).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/catalog/{id}/snapshot",
    tag = "catalog",
    operation_id = "snapshotCatalogDataset",
    summary = "Snapshot every table of a database dataset again (admin only)",
    description = "Copies every table snapshotted before from this database, in one read-only transaction so all \
                   tables show the same moment, then replaces the relations between them. A table whose content did \
                   not change gets no new version (`created: false`); the status is 200 when no table changed. At \
                   most 30 tables; SOURCE_MAX_ROWS and MAX_UPLOAD_MB apply to each table.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 201, body = SnapshotResponse, description = "At least one table got a new version"),
        (status = 200, body = SnapshotResponse, description = "No table changed: nothing created"),
        (status = "4XX", body = ErrorBody, description = "400 a file dataset, no table yet, too many tables, or the \
                                                         database refused; 401; 403 not admin; 404 not a member or \
                                                         no such dataset; 413 a table over SOURCE_MAX_ROWS")
    )
)]
async fn snapshot(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<(StatusCode, Json<SnapshotResponse>), AppError> {
    let result = service::catalog::snapshot(&state, &member, &id).await?;
    let status = if result.datasets.iter().any(|d| d.created) {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/catalog/{id}/ask",
    tag = "catalog",
    operation_id = "askCatalogDataset",
    summary = "Ask a question about a dataset",
    description = "Same response as `ask`. Without `table_id` the backend picks the table to start from: the one \
                   whose name, column names or column aliases match most words of the question, then the table of \
                   the conversation being continued, then the table holding most foreign keys.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    request_body = DatasetAskRequest,
    responses(
        (status = 200, body = AskResponse, description = "Answer, or why there is none"),
        (status = "4XX", body = ErrorBody, description = "400 empty question, a dataset with no table or no data \
                                                         yet; 401; 404 not a member, no such dataset, a `table_id` \
                                                         outside the dataset, or a conversation that is not the \
                                                         caller's"),
        (status = 502, body = ErrorBody, description = "The language model service failed before any SQL was \
                                                       written"),
        (status = 503, body = ErrorBody, description = "The language model is not configured on the server")
    )
)]
async fn ask(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
    JsonBody(req): JsonBody<DatasetAskRequest>,
) -> Result<Json<AskResponse>, AppError> {
    let response = service::catalog::ask(
        &state,
        &member,
        &id,
        &req.question,
        &req.choices,
        req.conversation_id,
        req.table_id,
    )
    .await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/catalog/{id}/conversations",
    tag = "catalog",
    operation_id = "listCatalogConversations",
    summary = "Conversations of the caller on any table of a dataset",
    description = "Newest first. Conversations with no recorded question are left out. Open one with \
                   `GET .../datasets/{table_id}/conversations/{id}`.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = ConversationListResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member or no such dataset")
    )
)]
async fn conversations(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<ConversationListResponse>, AppError> {
    Ok(Json(
        service::catalog::conversations(&state, &member, &id).await?,
    ))
}
