// SPDX-License-Identifier: Apache-2.0
/* api/feedback.rs - đánh giá câu trả lời ("Tốt", "Chưa đạt") và câu mẫu (#212). Người hỏi đánh giá câu của mình; admin
xem câu "Tốt", duyệt làm câu mẫu và xuất câu mẫu thành ca ứng viên cho bộ đánh giá. Logic ở service/feedback/. Test ở
api/feedback_tests.rs. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::feedback::contract::{
    EvalCaseCandidate, ExampleCandidatesResponse, FeedbackRequest, QueryFeedback,
};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(rate_answer, clear_rating))
        .routes(routes!(list_examples))
        .routes(routes!(approve_example, withdraw_example))
        .routes(routes!(export_examples))
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/queries/{query_id}/feedback",
    tag = "feedback",
    operation_id = "rateAnswer",
    summary = "Rate an answer of the caller",
    description = "Replaces the earlier rating. Changing a `good` rating to `bad` also withdraws its approval as an \
                   example question.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("query_id" = String, Path, description = "`query_id` of the answer (UUID)")
    ),
    request_body = FeedbackRequest,
    responses(
        (status = 200, body = QueryFeedback, description = "The stored rating"),
        (status = "4XX", body = ErrorBody, description = "400 a reason or note with `good`, or a note over 500 \
                                                         characters; 401; 404 not a member, or no such answer of the \
                                                         caller")
    )
)]
async fn rate_answer(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, query_id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<FeedbackRequest>,
) -> Result<Json<QueryFeedback>, AppError> {
    let feedback = service::feedback::rate(&state, &member, &query_id, request).await?;
    Ok(Json(feedback))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/queries/{query_id}/feedback",
    tag = "feedback",
    operation_id = "clearRating",
    summary = "Remove the rating of an answer of the caller",
    description = "Also withdraws its approval as an example question.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("query_id" = String, Path, description = "`query_id` of the answer (UUID)")
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = "4XX", body = ErrorBody, description = "401; 404 not a member, or no such answer of the caller")
    )
)]
async fn clear_rating(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, query_id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    service::feedback::clear(&state, &member, &query_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/examples",
    tag = "feedback",
    operation_id = "listExamples",
    summary = "Answers rated good, to review as example questions (admin only)",
    description = "Answered questions of every member of the workspace rated `good`, newest rating first, with \
                   whether an admin approved them.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = ExampleCandidatesResponse),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member")
    )
)]
async fn list_examples(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<ExampleCandidatesResponse>, AppError> {
    Ok(Json(
        service::feedback::list_examples(&state, &member).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/workspaces/{ws}/examples/{query_id}/approval",
    tag = "feedback",
    operation_id = "approveExample",
    summary = "Approve an answer rated good as an example question (admin only)",
    description = "Its question and the SQL the model wrote (for a why question, the form) become examples in the \
                   prompt of later questions on the same dataset that share a word with it. Records who approved \
                   and when.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("query_id" = String, Path, description = "`query_id` of the answer (UUID)")
    ),
    responses(
        (status = 204, description = "Approved"),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member, or no answered \
                                                         question rated good with this id")
    )
)]
async fn approve_example(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, query_id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    service::feedback::set_approval(&state, &member, &query_id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/examples/{query_id}/approval",
    tag = "feedback",
    operation_id = "withdrawExample",
    summary = "Withdraw the approval of an example question (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("query_id" = String, Path, description = "`query_id` of the answer (UUID)")
    ),
    responses(
        (status = 204, description = "Withdrawn"),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member, or no answered \
                                                         question rated good with this id")
    )
)]
async fn withdraw_example(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, query_id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    service::feedback::set_approval(&state, &member, &query_id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/datasets/{id}/examples/export",
    tag = "feedback",
    operation_id = "exportExamples",
    summary = "Approved example questions of a dataset as candidate evaluation cases (admin only)",
    description = "A JSON array in the format of the case files in `eval/` (`id`, `question`, `expected_sql`, \
                   `ordered`). The SQL is the one the model wrote; review every case before adding it to a case \
                   file.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("id" = String, Path, description = "Dataset id (UUID)")
    ),
    responses(
        (status = 200, body = Vec<EvalCaseCandidate>),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member or invalid dataset id")
    )
)]
async fn export_examples(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, id)): PathParams<(String, String)>,
) -> Result<Json<Vec<EvalCaseCandidate>>, AppError> {
    Ok(Json(
        service::feedback::export_examples(&state, &member, &id).await?,
    ))
}
