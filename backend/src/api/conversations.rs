// SPDX-License-Identifier: Apache-2.0
/* api/conversations.rs - đọc cuộc trò chuyện trên một dataset (#164): danh sách cuộc của người gọi và các lượt của một
cuộc. Cuộc được tạo và nối thêm lượt qua route hỏi (api/query.rs, AskRequest.conversation_id); logic ở
service/conversation/. Chỉ người tạo đọc được một cuộc, cuộc của người khác trả 404. Test ở api/conversation_tests.rs. */

use axum::{extract::State, Json};
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::PathParams;
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::conversation::contract::{ConversationListResponse, ConversationResponse};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_conversations))
        .routes(routes!(get_conversation))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/conversations",
    tag = "query",
    operation_id = "listConversations",
    summary = "Conversations of the caller on a dataset",
    description = "Newest first. Conversations with no recorded question are left out.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = ConversationListResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member or invalid dataset id")
    )
)]
async fn list_conversations(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<ConversationListResponse>, AppError> {
    let response = service::conversation::list(&state, &member, &id).await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/conversations/{conversation_id}",
    tag = "query",
    operation_id = "getConversation",
    summary = "One conversation of the caller with its turns",
    description = "Turns oldest first, each with its question, status, SQL and the short text of what was understood. \
                   Results are not stored: ask a turn again to see its numbers.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)"),
        ("conversation_id" = String, Path, description = "Conversation id (UUID)")
    ),
    responses(
        (status = 200, body = ConversationResponse),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, or no such conversation of the \
                                                         caller on this dataset")
    )
)]
async fn get_conversation(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id, conversation_id)): PathParams<(String, String, String)>,
) -> Result<Json<ConversationResponse>, AppError> {
    let response = service::conversation::open(&state, &member, &id, &conversation_id).await?;
    Ok(Json(response))
}
