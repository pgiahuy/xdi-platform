// SPDX-License-Identifier: Apache-2.0
/* service/conversation/contract.rs - kiểu response của hai route đọc cuộc trò chuyện (api/conversations.rs). Một lượt
chỉ có câu hỏi và cách nó được hiểu; bảng kết quả không lưu, mở lại một cuộc cũ thì hỏi lại câu đó để xem số. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::ask::contract::AskStatus;

/// One conversation on a dataset.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationItem {
    pub id: Uuid,
    /// The first question of the conversation, shortened.
    pub title: String,
    pub created_at: DateTime<Utc>,
}

/// Conversations of the calling user on a dataset, newest first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationListResponse {
    pub conversations: Vec<ConversationItem>,
}

/// One recorded question of a conversation. Questions waiting for a choice (`needs_choice`) are not recorded.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TurnItem {
    /// Id of the question in the history; use it to trace source rows like a single question.
    pub query_id: Uuid,
    pub question: String,
    /// `answered`, `rejected` or `failed`.
    pub status: AskStatus,
    /// SQL that ran (`sql_guarded`); null when the guard refused it.
    #[schema(required)]
    pub sql: Option<String>,
    /// Short text of the "understood as" card (measures, filters, grouping, period); null unless answered.
    #[schema(required)]
    pub understood: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A conversation and its recorded turns, oldest first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationResponse {
    pub conversation: ConversationItem,
    pub turns: Vec<TurnItem>,
}
