// SPDX-License-Identifier: Apache-2.0
/* service/conversation/mod.rs - cuộc trò chuyện (#164): câu sau hiểu theo các câu trước. Bọc luồng hỏi (service/ask/):
đọc vài lượt gần nhất của cuộc, dựng khối "Hội thoại trước" (history.rs) cho prompt, gọi ask, rồi gắn câu vừa ghi vào
cuộc kèm phần đã hiểu dạng chữ. Luồng hỏi chỉ nhận khối lịch sử dạng chữ, không biết gì thêm về cuộc trò chuyện.

Chỉ người tạo đọc và hỏi tiếp được một cuộc, kể cả admin của workspace: câu hỏi của một người là việc riêng của họ,
cũng như hiện chưa có màn nào cho xem lịch sử câu hỏi của người khác. Cuộc của người khác, của dataset khác hay của
workspace khác đều trả 404 như id không có, để không lộ là id đó tồn tại. */

pub mod contract;
pub mod history;
#[cfg(test)]
mod history_tests;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::{Conversation, StoredTurn};
use crate::service::ask::contract::{AskResponse, AskStatus, ChosenOption};
use crate::service::conversation::contract::{
    ConversationItem, ConversationListResponse, ConversationResponse, TurnItem,
};
use crate::service::conversation::history::{
    build_history_section, build_understood_text, HISTORY_TURNS,
};
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
    let ws = &member.workspace;
    let dataset = parse_id(dataset_id)?;
    let existing = match conversation_id {
        Some(id) => Some(load_owned(state, member, dataset, id).await?),
        None => None,
    };
    let turns = match &existing {
        Some(conversation) => {
            state
                .store
                .list_turns(ws, conversation.id, HISTORY_TURNS)
                .await?
        }
        None => Vec::new(),
    };
    let history = build_history_section(&turns);
    let mut response =
        crate::service::ask::ask(state, member, dataset_id, question, choices, &history).await?;

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
    // Câu needs_choice không ghi lịch sử nên không có lượt để gắn; người hỏi gửi lại kèm lựa chọn và conversation_id.
    if let Some(query_id) = response.query_id {
        let understood = response
            .understanding
            .as_ref()
            .and_then(build_understood_text);
        state
            .store
            .attach_turn(ws, query_id, conversation, understood.as_deref())
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

/// Một cuộc của người gọi cùng các lượt của nó.
pub async fn open(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    conversation_id: &str,
) -> Result<ConversationResponse, AppError> {
    let dataset = parse_id(dataset_id)?;
    let id = Uuid::parse_str(conversation_id.trim()).map_err(|_| not_found())?;
    let conversation = load_owned(state, member, dataset, id).await?;
    let turns = state
        .store
        .list_turns(&member.workspace, conversation.id, TURNS_LIMIT)
        .await?;
    Ok(ConversationResponse {
        conversation: build_item(&conversation),
        turns: turns.into_iter().map(build_turn).collect(),
    })
}

/// Cuộc của người gọi trên đúng dataset này; mọi trường hợp khác là 404 (lý do ở đầu file).
async fn load_owned(
    state: &AppState,
    member: &Member,
    dataset: Uuid,
    id: Uuid,
) -> Result<Conversation, AppError> {
    let conversation = state
        .store
        .get_conversation(&member.workspace, id)
        .await?
        .ok_or_else(not_found)?;
    if conversation.created_by != member.user.id || conversation.dataset_id != dataset {
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
    TurnItem {
        query_id: turn.query_id,
        question: turn.question,
        status,
        sql: turn.sql_guarded,
        understood: turn.understood,
        created_at: turn.created_at,
    }
}
