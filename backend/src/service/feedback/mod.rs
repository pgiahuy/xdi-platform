// SPDX-License-Identifier: Apache-2.0
/* service/feedback/mod.rs - đánh giá câu trả lời và câu mẫu (#212). Người hỏi bấm "Tốt" hay "Chưa đạt" trên câu trả
lời của mình; admin xem các câu "Tốt", duyệt câu nào thành câu mẫu, và xuất câu mẫu thành ca ứng viên cho bộ đánh
giá (eval/, #202). API ở api/feedback.rs, ghi xuống Postgres ở engine/store/feedback.rs.

Chỉ câu đã duyệt vào prompt làm ví dụ (Store::list_approved_examples, service/ask/prompt.rs): đánh giá "Tốt" là ý của
một người về một câu trả lời, chưa phải bảo đảm câu SQL đúng; câu SQL sai vào ví dụ thì mô hình chép cái sai sang các
câu sau. Câu của người khác trả 404 như câu không có, như cuộc trò chuyện (service/conversation/mod.rs): câu hỏi của
một người là việc riêng của họ. Riêng câu đã bấm "Tốt" thì admin của workspace đọc được câu hỏi và SQL của nó trong
danh sách duyệt (docs/PRIVACY.md). */

pub mod contract;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::NewFeedback;
use crate::error::AppError;
use crate::service::feedback::contract::{
    EvalCaseCandidate, ExampleCandidate, ExampleCandidatesResponse, FeedbackRequest, QueryFeedback,
    Rating,
};
use crate::state::AppState;

/// Ghi chú dài nhất của "Chưa đạt", đếm theo ký tự: đủ một hai câu, không thành chỗ dán dữ liệu.
const NOTE_MAX_CHARS: usize = 500;

/// Số câu "Tốt" tối đa trong danh sách duyệt.
const LIST_LIMIT: i64 = 200;

/// Số câu mẫu tối đa trong một lần xuất.
const EXPORT_LIMIT: i64 = 500;

/// Ghi đánh giá của người gọi cho câu trả lời query_id của chính họ.
pub async fn rate(
    state: &AppState,
    member: &Member,
    query_id: &str,
    request: FeedbackRequest,
) -> Result<QueryFeedback, AppError> {
    let id = parse_query_id(query_id)?;
    let note = build_note(&request)?;
    let new = NewFeedback {
        feedback: request.rating.as_db(),
        reason: request.reason.map(|r| r.as_db()),
        note: note.as_deref(),
    };
    let stored = state
        .store
        .set_feedback(&member.workspace, id, member.user.id, &new)
        .await?
        .ok_or_else(answer_not_found)?;
    QueryFeedback::from_stored(&stored)
        .ok_or_else(|| AppError::Internal("unknown feedback value in queries".to_string()))
}

/// Bỏ đánh giá của người gọi (bấm lại nút đang chọn); kéo theo bỏ lần duyệt câu mẫu.
pub async fn clear(state: &AppState, member: &Member, query_id: &str) -> Result<(), AppError> {
    let id = parse_query_id(query_id)?;
    let done = state
        .store
        .clear_feedback(&member.workspace, id, member.user.id)
        .await?;
    if done {
        Ok(())
    } else {
        Err(answer_not_found())
    }
}

/// Các câu "Tốt" của workspace cho admin duyệt.
pub async fn list_examples(
    state: &AppState,
    member: &Member,
) -> Result<ExampleCandidatesResponse, AppError> {
    member.require_admin()?;
    let rows = state
        .store
        .list_good_answers(&member.workspace, None, LIST_LIMIT)
        .await?;
    Ok(ExampleCandidatesResponse {
        examples: rows.into_iter().map(ExampleCandidate::from).collect(),
    })
}

/// Duyệt (approve true) hay bỏ duyệt một câu "Tốt" làm câu mẫu; ghi người duyệt và thời điểm.
pub async fn set_approval(
    state: &AppState,
    member: &Member,
    query_id: &str,
    approve: bool,
) -> Result<(), AppError> {
    member.require_admin()?;
    let id = parse_query_id(query_id)?;
    let approver = approve.then_some(member.user.id);
    let done = state
        .store
        .set_example_approval(&member.workspace, id, approver)
        .await?;
    if done {
        Ok(())
    } else {
        Err(AppError::NotFound(
            "Không có câu trả lời được đánh giá Tốt này".to_string(),
        ))
    }
}

/// Câu mẫu đã duyệt của một dataset, theo khuôn ca của bộ đánh giá (eval/dataco.json), để người viết bộ câu xem lại.
pub async fn export_examples(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<Vec<EvalCaseCandidate>, AppError> {
    member.require_admin()?;
    let id = Uuid::parse_str(dataset_id.trim())
        .map_err(|_| AppError::NotFound("Không có dataset này".to_string()))?;
    let rows = state
        .store
        .list_good_answers(&member.workspace, Some(id), EXPORT_LIMIT)
        .await?;
    let cases = rows
        .into_iter()
        .filter(|row| row.approved_at.is_some())
        .filter_map(|row| {
            let sql = row.sql_generated?;
            Some(EvalCaseCandidate {
                // Tiền tố riêng để khi chép vào file ca không trùng id q01, q02 viết tay.
                id: format!("approved-{}", &row.query_id.simple().to_string()[..8]),
                question: row.question,
                expected_sql: vec![sql],
                ordered: false,
            })
        })
        .collect();
    Ok(cases)
}

/// Ghi chú đã cắt khoảng trắng; rỗng thì None. Lý do hay ghi chú đi với "Tốt" là request sai.
fn build_note(request: &FeedbackRequest) -> Result<Option<String>, AppError> {
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if request.rating == Rating::Good && (request.reason.is_some() || note.is_some()) {
        return Err(AppError::BadRequest(
            "Lý do và ghi chú chỉ dùng khi đánh giá Chưa đạt".to_string(),
        ));
    }
    if note.is_some_and(|n| n.chars().count() > NOTE_MAX_CHARS) {
        return Err(AppError::BadRequest(format!(
            "Ghi chú dài quá {NOTE_MAX_CHARS} ký tự"
        )));
    }
    Ok(note.map(str::to_string))
}

fn parse_query_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| answer_not_found())
}

fn answer_not_found() -> AppError {
    AppError::NotFound("Không có câu trả lời này".to_string())
}
