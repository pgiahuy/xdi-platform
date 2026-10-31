// SPDX-License-Identifier: Apache-2.0
/* api/lessons.rs - nhật ký lỗi và bài học (#228), chỉ admin của workspace. Logic ở service/lessons/. Test ở
api/lessons_tests.rs. */

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::extract::{JsonBody, PathParams};
use crate::auth::Member;
use crate::error::ErrorBody;
use crate::service::lessons::contract::{
    ErrorLogResponse, Lesson, LessonRequest, LessonStateRequest, LessonsResponse,
};
use crate::{error::AppError, service, state::AppState};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_errors))
        .routes(routes!(list_lessons, create_lesson))
        .routes(routes!(set_lesson_state, delete_lesson))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/error-log",
    tag = "lessons",
    operation_id = "listErrorLog",
    summary = "Where the model went wrong, newest first (admin only)",
    description = "Written automatically: guard refusals, SQL that failed to run, blocked double counting over a \
                   join, invalid why forms, text rejected by the number or direction check, and answers rated `bad`. \
                   At most 200 rows.",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = ErrorLogResponse),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member")
    )
)]
async fn list_errors(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<ErrorLogResponse>, AppError> {
    Ok(Json(service::lessons::list_errors(&state, &member).await?))
}

#[utoipa::path(
    get,
    path = "/workspaces/{ws}/lessons",
    tag = "lessons",
    operation_id = "listLessons",
    summary = "Lessons of the workspace, newest first (admin only)",
    params(("ws" = String, Path, description = "Workspace slug")),
    responses(
        (status = 200, body = LessonsResponse),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member")
    )
)]
async fn list_lessons(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
) -> Result<Json<LessonsResponse>, AppError> {
    Ok(Json(service::lessons::list_lessons(&state, &member).await?))
}

#[utoipa::path(
    post,
    path = "/workspaces/{ws}/lessons",
    tag = "lessons",
    operation_id = "createLesson",
    summary = "Write a lesson as a draft (admin only)",
    description = "A draft is not used. Enable it to approve it; then it goes into the prompts of related questions.",
    params(("ws" = String, Path, description = "Workspace slug")),
    request_body = LessonRequest,
    responses(
        (status = 201, body = Lesson, description = "Created as a draft"),
        (status = "4XX", body = ErrorBody, description = "400 empty or over 300 characters, or a dataset or error \
                                                         log row not in this workspace; 401; 403 not admin; 404 not \
                                                         a member")
    )
)]
async fn create_lesson(
    State(state): State<AppState>,
    member: Member,
    PathParams(_ws): PathParams<String>,
    JsonBody(request): JsonBody<LessonRequest>,
) -> Result<(StatusCode, Json<Lesson>), AppError> {
    let lesson = service::lessons::create_lesson(&state, &member, request).await?;
    Ok((StatusCode::CREATED, Json(lesson)))
}

#[utoipa::path(
    patch,
    path = "/workspaces/{ws}/lessons/{lesson_id}",
    tag = "lessons",
    operation_id = "setLessonState",
    summary = "Enable (approve) or disable a lesson (admin only)",
    description = "Enabling records who approved it and when. At most 3 enabled lessons related to a question go \
                   into its prompts: same dataset or the whole workspace, sharing a word with the question or naming \
                   a column it matched.",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("lesson_id" = String, Path, description = "Lesson id (UUID)")
    ),
    request_body = LessonStateRequest,
    responses(
        (status = 200, body = Lesson),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member or no such lesson")
    )
)]
async fn set_lesson_state(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, lesson_id)): PathParams<(String, String)>,
    JsonBody(request): JsonBody<LessonStateRequest>,
) -> Result<Json<Lesson>, AppError> {
    let lesson =
        service::lessons::set_enabled(&state, &member, &lesson_id, request.enabled).await?;
    Ok(Json(lesson))
}

#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/lessons/{lesson_id}",
    tag = "lessons",
    operation_id = "deleteLesson",
    summary = "Delete a lesson (admin only)",
    params(
        ("ws" = String, Path, description = "Workspace slug"),
        ("lesson_id" = String, Path, description = "Lesson id (UUID)")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = "4XX", body = ErrorBody, description = "401; 403 not admin; 404 not a member or no such lesson")
    )
)]
async fn delete_lesson(
    State(state): State<AppState>,
    member: Member,
    PathParams((_ws, lesson_id)): PathParams<(String, String)>,
) -> Result<StatusCode, AppError> {
    service::lessons::delete_lesson(&state, &member, &lesson_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
