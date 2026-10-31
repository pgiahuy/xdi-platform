// SPDX-License-Identifier: Apache-2.0
/* service/lessons/mod.rs - nhật ký lỗi và bài học (#228). Hai việc:
- Ghi nhật ký: luồng hỏi (service/ask/mod.rs, ask/why.rs) gọi record_failures sau bước record với các StepFailure các
  bước đã gom (ask/failures.rs); đánh giá "Chưa đạt" (service/feedback/) gọi record_bad_feedback. Ghi lỗi chỉ ghi log
  máy chủ, không làm câu hỏi hay lần đánh giá lỗi theo: nhật ký là để học, mất một dòng không hại người đang hỏi.
- Admin xem nhật ký, tạo bài học (tay hay từ một dòng nhật ký), bật tắt, xóa. Bật là duyệt: ghi người duyệt và thời
  điểm. Chỉ bài đã bật vào prompt (service/ask/lessons.rs chọn bài liên quan). API ở api/lessons.rs.

Mọi việc ở đây chỉ admin làm được, kể cả xem nhật ký: nhật ký có câu hỏi của mọi thành viên và câu mô hình trả
(docs/PRIVACY.md). */

pub mod contract;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{NewErrorEntry, NewLesson};
use crate::error::AppError;
use crate::service::ask::failures::StepFailure;
use crate::service::lessons::contract::{
    ErrorEntry, ErrorLogResponse, Lesson, LessonRequest, LessonsResponse,
};
use crate::state::AppState;

/// Bài học dài nhất, đếm theo ký tự: mỗi bài vào prompt của nhiều câu hỏi, bài dài làm prompt dài và khó duyệt.
const LESSON_MAX_CHARS: usize = 300;

/// Số dòng nhật ký tối đa một lần xem.
const LOG_LIMIT: i64 = 200;

/// Câu hỏi đang ghi nhật ký: dataset, câu hỏi, và query_id khi câu đã được ghi lịch sử.
pub struct FailedQuestion<'a> {
    pub dataset_id: Uuid,
    pub question: &'a str,
    pub query_id: Option<Uuid>,
}

/// Ghi mỗi StepFailure thành một dòng nhật ký. Lỗi ghi chỉ vào log máy chủ (xem đầu file).
pub async fn record_failures(
    state: &AppState,
    member: &Member,
    asked: &FailedQuestion<'_>,
    failures: &[StepFailure],
) {
    for failure in failures {
        let entry = NewErrorEntry {
            dataset_id: Some(asked.dataset_id),
            query_id: asked.query_id,
            question: asked.question,
            step: failure.step.as_db(),
            error: &failure.error,
            model_output: failure.model_output.as_deref(),
        };
        if let Err(e) = state.store.record_error(&member.workspace, &entry).await {
            tracing::warn!(error = ?e, step = failure.step.as_db(), "could not write the error log");
        }
    }
}

/// Dòng nhật ký cho một lần bấm "Chưa đạt": lý do (mã tiếng Anh) và ghi chú của người hỏi. Ghi chú nằm trong nhật ký
/// cho admin đọc, không vào prompt; admin muốn dùng thì viết thành bài học.
pub async fn record_bad_feedback(
    state: &AppState,
    member: &Member,
    query_id: Uuid,
    reason: Option<&str>,
    note: Option<&str>,
) {
    let mut error = format!("rated bad: {}", reason.unwrap_or("no reason"));
    if let Some(note) = note {
        error.push_str(&format!("; note: {note}"));
    }
    if let Err(e) = state
        .store
        .record_feedback_error(&member.workspace, query_id, &error)
        .await
    {
        tracing::warn!(error = ?e, "could not write the error log for a bad rating");
    }
}

pub async fn list_errors(state: &AppState, member: &Member) -> Result<ErrorLogResponse, AppError> {
    member.require_admin()?;
    let rows = state
        .store
        .list_errors(&member.workspace, LOG_LIMIT)
        .await?;
    Ok(ErrorLogResponse {
        entries: rows.into_iter().filter_map(ErrorEntry::from_row).collect(),
    })
}

pub async fn list_lessons(state: &AppState, member: &Member) -> Result<LessonsResponse, AppError> {
    member.require_admin()?;
    let rows = state.store.list_lessons(&member.workspace).await?;
    Ok(LessonsResponse {
        lessons: rows.into_iter().map(Lesson::from).collect(),
    })
}

/// Tạo bài học dạng nháp: chưa vào prompt cho tới khi admin bật.
pub async fn create_lesson(
    state: &AppState,
    member: &Member,
    request: LessonRequest,
) -> Result<Lesson, AppError> {
    member.require_admin()?;
    let ws = &member.workspace;
    let text = request.text.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("Bài học đang trống".to_string()));
    }
    if text.chars().count() > LESSON_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "Bài học dài quá {LESSON_MAX_CHARS} ký tự; viết một hai câu ngắn"
        )));
    }
    if let Some(id) = request.dataset_id {
        if state.store.get_dataset(ws, id).await?.is_none() {
            return Err(AppError::BadRequest(
                "Không có dataset này trong workspace".to_string(),
            ));
        }
    }
    if let Some(id) = request.error_id {
        if !state.store.error_exists(ws, id).await? {
            return Err(AppError::BadRequest(
                "Không có dòng nhật ký này".to_string(),
            ));
        }
    }
    let new = NewLesson {
        dataset_id: request.dataset_id,
        text,
        error_id: request.error_id,
        created_by: member.user.id,
    };
    let id = state.store.create_lesson(ws, &new).await?;
    load_lesson(state, member, id).await
}

/// Bật (duyệt, ghi người duyệt và thời điểm) hay tắt một bài học.
pub async fn set_enabled(
    state: &AppState,
    member: &Member,
    lesson_id: &str,
    enabled: bool,
) -> Result<Lesson, AppError> {
    member.require_admin()?;
    let id = parse_lesson_id(lesson_id)?;
    let approver = enabled.then_some(member.user.id);
    if !state
        .store
        .set_lesson_enabled(&member.workspace, id, approver)
        .await?
    {
        return Err(lesson_not_found());
    }
    load_lesson(state, member, id).await
}

pub async fn delete_lesson(
    state: &AppState,
    member: &Member,
    lesson_id: &str,
) -> Result<(), AppError> {
    member.require_admin()?;
    let id = parse_lesson_id(lesson_id)?;
    if state.store.delete_lesson(&member.workspace, id).await? {
        Ok(())
    } else {
        Err(lesson_not_found())
    }
}

async fn load_lesson(state: &AppState, member: &Member, id: Uuid) -> Result<Lesson, AppError> {
    let row = state
        .store
        .get_lesson(&member.workspace, id)
        .await?
        .ok_or_else(lesson_not_found)?;
    Ok(Lesson::from(row))
}

fn parse_lesson_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| lesson_not_found())
}

fn lesson_not_found() -> AppError {
    AppError::NotFound("Không có bài học này".to_string())
}
