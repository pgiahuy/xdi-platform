// SPDX-License-Identifier: Apache-2.0
/* service/conversation/contract.rs - kiểu response của hai route đọc cuộc trò chuyện (api/conversations.rs). Một lượt
có câu hỏi, cách nó được hiểu và lời trả lời đã hiện (#201); bảng kết quả không lưu, mở lại một cuộc cũ thì hỏi lại câu
đó để xem bảng và biểu đồ. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::ask::contract::{AnswerVerdict, AskStatus};
use crate::service::feedback::contract::QueryFeedback;

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
    /// The rating the caller gave this answer; null when not rated.
    #[schema(required)]
    pub feedback: Option<QueryFeedback>,
    /// The answer text shown when the question was asked; null unless answered, and for questions recorded before
    /// answers were stored.
    #[schema(required)]
    pub answer: Option<SavedAnswer>,
}

/// The answer text of a recorded question, as shown when it was asked: `answer_text` of a plain question, or
/// `explanation.summary` of a "why" question. Result rows are not stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SavedAnswer {
    /// Vietnamese text for the end user.
    pub text: String,
    /// True when a language model wrote the text and it passed the checks; false when the backend wrote it.
    pub by_model: bool,
    /// Verdict of the independent model call that compared the meaning of the model's text with the computed facts
    /// (`match`, `warning`, `mismatch`); null when the check did not run.
    #[schema(required)]
    pub verdict: Option<AnswerVerdict>,
    /// Points the check found unclear in the model's text; empty when `by_model` is false.
    pub warnings: Vec<String>,
}

/// A conversation and its recorded turns, oldest first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationResponse {
    pub conversation: ConversationItem,
    pub turns: Vec<TurnItem>,
}
