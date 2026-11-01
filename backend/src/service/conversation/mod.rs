// SPDX-License-Identifier: Apache-2.0
/* service/conversation/mod.rs - cuộc trò chuyện (#164): câu sau hiểu theo các câu trước. Bọc luồng hỏi (service/ask/):
đọc vài lượt gần nhất của cuộc, dựng khối "Hội thoại trước" (history.rs) cho prompt cùng danh sách câu hỏi trước cho
bước phân loại, gọi ask, rồi gắn câu vừa ghi vào cuộc kèm phần đã hiểu dạng chữ. Luồng hỏi chỉ nhận khối lịch sử dạng
chữ và các câu hỏi trước (AskHistory), không biết gì thêm về cuộc trò chuyện.

Chỉ người tạo đọc và hỏi tiếp được một cuộc, kể cả admin của workspace: câu hỏi của một người là việc riêng của họ,
cũng như hiện chưa có màn nào cho xem lịch sử câu hỏi của người khác. Cuộc của người khác, của dataset khác hay của
workspace khác đều trả 404 như id không có, để không lộ là id đó tồn tại. */

pub mod carryover;
pub mod contract;
pub mod history;
#[cfg(test)]
mod history_tests;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{
    CatalogColumnRow, Conversation, Dataset, DatasetRelation, StoredFeedback, StoredTurn,
};
use crate::service::ask::contract::{AskResponse, AskStatus, ChosenOption};
use crate::service::ask::scope::pick_start_table;
use crate::service::ask::{AskHistory, StartTable};
use crate::service::conversation::carryover::{apply_carryover, find_previous};
use crate::service::conversation::contract::{
    ConversationItem, ConversationListResponse, ConversationResponse, SavedAnswer, TurnItem,
};
use crate::service::conversation::history::{
    build_history_section, build_understood_text, HISTORY_TURNS,
};
use crate::service::feedback::contract::QueryFeedback;
use crate::{error::AppError, state::AppState};

/// Độ dài tối đa của tên cuộc (lấy từ câu hỏi đầu), tính theo ký tự.
const TITLE_MAX_CHARS: usize = 80;

/// Số cuộc tối đa trả về trong danh sách; giao diện chỉ hiện vài cuộc gần nhất.
const LIST_LIMIT: i64 = 50;

/// Số lượt tối đa khi mở lại một cuộc.
const TURNS_LIMIT: i64 = 200;

/// Hỏi một câu trong cuộc trò chuyện. conversation_id None thì mở cuộc mới, sau khi luồng hỏi đã kiểm dataset.
pub async fn ask(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    question: &str,
    choices: &[ChosenOption],
    conversation_id: Option<Uuid>,
) -> Result<AskResponse, AppError> {
    let dataset = parse_id(dataset_id)?;
    let existing = match conversation_id {
        Some(id) => Some(load_owned(state, member, &[dataset], id).await?),
        None => None,
    };
    ask_on_table(
        state,
        member,
        dataset,
        question,
        choices,
        existing,
        StartTable::Given,
    )
    .await
}

/// Hỏi trên cả một dataset (#257): `tables` là mọi bảng của dataset. Bảng bắt đầu là `pinned` nếu người hỏi ghim (phải
/// thuộc dataset), không thì do service/ask/scope.rs chọn, ưu tiên bảng của cuộc đang tiếp. Cuộc tiếp tục được khi nó
/// nằm trên một bảng bất kỳ của dataset, nên câu sau bắt đầu từ bảng khác vẫn ở cùng cuộc.
pub async fn ask_in_dataset(
    state: &AppState,
    member: &Member,
    dataset: &DatasetScope<'_>,
    question: &str,
    choices: &[ChosenOption],
    conversation_id: Option<Uuid>,
) -> Result<AskResponse, AppError> {
    let ids: Vec<Uuid> = dataset.tables.iter().map(|t| t.id).collect();
    let existing = match conversation_id {
        Some(id) => Some(load_owned(state, member, &ids, id).await?),
        None => None,
    };
    let chosen_by = match dataset.pinned {
        Some(_) => StartTable::Given,
        None => StartTable::Picked,
    };
    let start = match dataset.pinned {
        Some(id) if ids.contains(&id) => Some(id),
        Some(_) => return Err(AppError::NotFound("Dataset không có bảng này".to_string())),
        None => pick_start_table(
            question,
            dataset.tables,
            dataset.columns,
            dataset.relations,
            existing.as_ref().map(|c| c.dataset_id),
        ),
    };
    let Some(start) = start else {
        return Err(AppError::BadRequest(
            "Dataset chưa có bảng nào; chụp bảng hay nạp file trước khi hỏi".to_string(),
        ));
    };
    ask_on_table(state, member, start, question, choices, existing, chosen_by).await
}

/// Mọi bảng của một dataset và phần pick_start_table cần, do service/catalog/ đọc.
pub struct DatasetScope<'a> {
    pub tables: &'a [Dataset],
    pub columns: &'a [CatalogColumnRow],
    pub relations: &'a [DatasetRelation],
    /// Bảng người hỏi ghim (`table_id`), None để backend chọn.
    pub pinned: Option<Uuid>,
}

/// Phần chung của hai đường hỏi: đọc lịch sử của cuộc (nếu có), hỏi trên bảng `dataset`, rồi ghi lượt vào cuộc (hay mở
/// cuộc mới trên bảng đó).
async fn ask_on_table(
    state: &AppState,
    member: &Member,
    dataset: Uuid,
    question: &str,
    choices: &[ChosenOption],
    existing: Option<Conversation>,
    start: StartTable,
) -> Result<AskResponse, AppError> {
    let ws = &member.workspace;
    let dataset_id = dataset.to_string();
    let turns = match &existing {
        Some(conversation) => {
            state
                .store
                .list_turns(ws, conversation.id, HISTORY_TURNS)
                .await?
        }
        None => Vec::new(),
    };
    let history = AskHistory {
        section: build_history_section(&turns),
        questions: turns.iter().map(|turn| turn.question.clone()).collect(),
    };
    let mut response = crate::service::ask::ask(
        state,
        member,
        &dataset_id,
        question,
        choices,
        &history,
        start,
    )
    .await?;

    let conversation = match existing {
        Some(conversation) => conversation.id,
        None => {
            let title = build_title(question);
            state
                .store
                .create_conversation(ws, dataset, member.user.id, &title)
                .await?
        }
    };
    // Mục giống lượt trước mà câu này không nhắc tới thì ghi "từ câu trước"; đánh dấu trước khi lưu, nhưng phần lưu
    // (build_understood_text) chỉ có nhãn nên không đổi.
    if let (Some(understanding), Some(previous)) =
        (response.understanding.as_mut(), find_previous(&turns))
    {
        apply_carryover(understanding, question, previous);
    }
    // Câu needs_choice không ghi lịch sử nên không có lượt để gắn; người hỏi gửi lại kèm lựa chọn và conversation_id.
    if let Some(query_id) = response.query_id {
        let understood = response
            .understanding
            .as_ref()
            .and_then(build_understood_text);
        // Lời đã hiện được lưu để mở lại cuộc (tải lại trang) vẫn thấy câu trả lời; không đưa vào khối lịch sử gửi mô
        // hình (history.rs), khối đó chỉ có câu hỏi, SQL và phần đã hiểu.
        let answer = build_saved_answer(&response).and_then(|a| serde_json::to_value(a).ok());
        state
            .store
            .attach_turn(
                ws,
                query_id,
                conversation,
                understood.as_deref(),
                answer.as_ref(),
            )
            .await?;
    }
    response.conversation_id = conversation;
    Ok(response)
}

/// Các cuộc của người gọi trên dataset, mới nhất trước.
pub async fn list(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<ConversationListResponse, AppError> {
    let dataset = parse_id(dataset_id)?;
    let conversations = state
        .store
        .list_conversations(&member.workspace, dataset, member.user.id, LIST_LIMIT)
        .await?;
    Ok(ConversationListResponse {
        conversations: conversations.iter().map(build_item).collect(),
    })
}

/// Các cuộc của người gọi trên mọi bảng của dataset `source_id`, mới nhất trước.
pub async fn list_in_dataset(
    state: &AppState,
    member: &Member,
    source_id: Uuid,
) -> Result<ConversationListResponse, AppError> {
    let conversations = state
        .store
        .list_dataset_conversations(&member.workspace, source_id, member.user.id, LIST_LIMIT)
        .await?;
    Ok(ConversationListResponse {
        conversations: conversations.iter().map(build_item).collect(),
    })
}

/// Một cuộc của người gọi cùng các lượt của nó.
pub async fn open(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    conversation_id: &str,
) -> Result<ConversationResponse, AppError> {
    let dataset = parse_id(dataset_id)?;
    let id = Uuid::parse_str(conversation_id.trim()).map_err(|_| not_found())?;
    let conversation = load_owned(state, member, &[dataset], id).await?;
    let turns = state
        .store
        .list_turns(&member.workspace, conversation.id, TURNS_LIMIT)
        .await?;
    Ok(ConversationResponse {
        conversation: build_item(&conversation),
        turns: turns.into_iter().map(build_turn).collect(),
    })
}

/// Cuộc của người gọi trên một trong các bảng `tables`; mọi trường hợp khác là 404 (lý do ở đầu file).
async fn load_owned(
    state: &AppState,
    member: &Member,
    tables: &[Uuid],
    id: Uuid,
) -> Result<Conversation, AppError> {
    let conversation = state
        .store
        .get_conversation(&member.workspace, id)
        .await?
        .ok_or_else(not_found)?;
    if conversation.created_by != member.user.id || !tables.contains(&conversation.dataset_id) {
        return Err(not_found());
    }
    Ok(conversation)
}

fn parse_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| AppError::NotFound("Không có dataset này".to_string()))
}

fn not_found() -> AppError {
    AppError::NotFound("Không có cuộc trò chuyện này".to_string())
}

fn build_title(question: &str) -> String {
    let question = question.trim();
    if question.chars().count() <= TITLE_MAX_CHARS {
        return question.to_string();
    }
    let short: String = question.chars().take(TITLE_MAX_CHARS - 3).collect();
    format!("{}...", short.trim_end())
}

fn build_item(conversation: &Conversation) -> ConversationItem {
    ConversationItem {
        id: conversation.id,
        table_id: conversation.dataset_id,
        title: conversation.title.clone(),
        created_at: conversation.created_at,
    }
}

fn build_turn(turn: StoredTurn) -> TurnItem {
    let status = match turn.status.as_str() {
        "ok" => AskStatus::Answered,
        "rejected" => AskStatus::Rejected,
        _ => AskStatus::Failed,
    };
    let feedback = build_turn_feedback(&turn);
    TurnItem {
        query_id: turn.query_id,
        question: turn.question,
        status,
        sql: turn.sql_guarded,
        understood: turn.understood,
        created_at: turn.created_at,
        feedback,
        // Bản ghi không đọc được (khuôn JSON đổi) thì coi như không có lời, lượt vẫn mở được.
        answer: turn
            .answer
            .and_then(|value| serde_json::from_value(value).ok()),
    }
}

/// Lời trả lời đã hiện của một câu answered: đoạn trả lời của câu thường, hay lời giải thích của câu "tại sao" (câu
/// "tại sao" không có answer_text). Kết luận của mô hình chấm giữ cả khi lời là câu của code (mismatch); các chỗ chưa rõ
/// chỉ thuộc lời của mô hình.
pub fn build_saved_answer(response: &AskResponse) -> Option<SavedAnswer> {
    if response.status != AskStatus::Answered {
        return None;
    }
    if let Some(text) = &response.answer_text {
        let by_model = response.answer_text_by_model.unwrap_or(false);
        let insight = response.insight.as_ref();
        return Some(SavedAnswer {
            text: text.clone(),
            by_model,
            verdict: insight.and_then(|i| i.summary_verdict),
            failed_check: insight.and_then(|i| i.summary_failed_check),
            warnings: match insight {
                Some(insight) if by_model => insight.summary_warnings.clone(),
                _ => Vec::new(),
            },
        });
    }
    let explanation = response.explanation.as_ref()?;
    Some(SavedAnswer {
        text: explanation.summary.clone(),
        by_model: explanation.summary_by_model,
        verdict: explanation.summary_verdict,
        failed_check: explanation.summary_failed_check,
        warnings: if explanation.summary_by_model {
            explanation.summary_warnings.clone()
        } else {
            Vec::new()
        },
    })
}

/// Đánh giá của lượt (#212); None khi chưa đánh giá. Hai cột feedback và feedback_at luôn ghi cùng nhau
/// (engine/store/feedback.rs), nên thiếu một trong hai cũng coi như chưa đánh giá.
fn build_turn_feedback(turn: &StoredTurn) -> Option<QueryFeedback> {
    let stored = StoredFeedback {
        feedback: turn.feedback.clone()?,
        reason: turn.feedback_reason.clone(),
        note: turn.feedback_note.clone(),
        rated_at: turn.feedback_at?,
        approved: turn.example_approved,
    };
    QueryFeedback::from_stored(&stored)
}
