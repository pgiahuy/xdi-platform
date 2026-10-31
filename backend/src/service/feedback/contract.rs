// SPDX-License-Identifier: Apache-2.0
/* service/feedback/contract.rs - kiểu request và response của đánh giá câu trả lời và câu mẫu (#212), route ở
api/feedback.rs. QueryFeedback cũng nằm trong mỗi lượt của cuộc trò chuyện (conversation/contract.rs), để mở lại cuộc
thì nút đã bấm hiện lại. Chú thích /// thành mô tả trong đặc tả OpenAPI nên viết tiếng Anh. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::store::{ExampleRow, StoredFeedback};

/// `good` is the button for a good answer, `bad` the one for an answer that falls short.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Good,
    Bad,
}

impl Rating {
    /// Giá trị của queries.feedback (migration 0001 đặt hai chữ này từ trước khi có nút).
    pub fn as_db(self) -> &'static str {
        match self {
            Rating::Good => "correct",
            Rating::Bad => "incorrect",
        }
    }

    fn from_db(text: &str) -> Option<Self> {
        match text {
            "correct" => Some(Rating::Good),
            "incorrect" => Some(Rating::Bad),
            _ => None,
        }
    }
}

/// Why an answer is `bad`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackReason {
    /// The question was understood wrongly.
    Misunderstood,
    /// The numbers are wrong.
    WrongNumbers,
    /// The explanation is hard to follow.
    UnclearExplanation,
    Other,
}

impl FeedbackReason {
    pub fn as_db(self) -> &'static str {
        match self {
            FeedbackReason::Misunderstood => "misunderstood",
            FeedbackReason::WrongNumbers => "wrong_numbers",
            FeedbackReason::UnclearExplanation => "unclear_explanation",
            FeedbackReason::Other => "other",
        }
    }

    fn from_db(text: &str) -> Option<Self> {
        [
            FeedbackReason::Misunderstood,
            FeedbackReason::WrongNumbers,
            FeedbackReason::UnclearExplanation,
            FeedbackReason::Other,
        ]
        .into_iter()
        .find(|reason| reason.as_db() == text)
    }
}

/// Rating of one answer by the member who asked it. Sending a new rating replaces the old one.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FeedbackRequest {
    pub rating: Rating,
    /// Only with `bad`; may be left out.
    #[serde(default)]
    pub reason: Option<FeedbackReason>,
    /// Optional short note, only with `bad`, at most 500 characters.
    #[serde(default)]
    pub note: Option<String>,
}

/// The stored rating of an answer.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QueryFeedback {
    pub rating: Rating,
    #[schema(required)]
    pub reason: Option<FeedbackReason>,
    #[schema(required)]
    pub note: Option<String>,
    pub rated_at: DateTime<Utc>,
    /// An admin approved the answer as an example question: its question and SQL are now examples in the prompt of
    /// later questions on the same dataset that share a word with it.
    pub approved: bool,
}

impl QueryFeedback {
    /// None khi chữ trong database không phải hai giá trị đã biết (chỉ xảy ra nếu ai đó sửa tay).
    pub fn from_stored(stored: &StoredFeedback) -> Option<Self> {
        Some(QueryFeedback {
            rating: Rating::from_db(&stored.feedback)?,
            reason: stored.reason.as_deref().and_then(FeedbackReason::from_db),
            note: stored.note.clone(),
            rated_at: stored.rated_at,
            approved: stored.approved,
        })
    }
}

/// An answer rated `good`, for admins to review as an example question.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExampleCandidate {
    pub query_id: Uuid,
    /// Null when the dataset was deleted.
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
    #[schema(required)]
    pub dataset_name: Option<String>,
    pub question: String,
    /// SQL the model wrote, before the guard; for a why question, the SQL built from its form.
    #[schema(required)]
    pub sql: Option<String>,
    /// Short text of the "understood as" card; null when the question was not asked in a conversation.
    #[schema(required)]
    pub understood: Option<String>,
    /// Rows of the result when it was asked; results are not stored.
    #[schema(required)]
    pub row_count: Option<i64>,
    /// A why question: its example in the prompt is the form the model filled, not SQL.
    pub why: bool,
    /// Name or email of who rated it; null when the account was deleted.
    #[schema(required)]
    pub rated_by: Option<String>,
    pub rated_at: DateTime<Utc>,
    #[schema(required)]
    pub approved_by: Option<String>,
    /// Null until an admin approves it as an example question.
    #[schema(required)]
    pub approved_at: Option<DateTime<Utc>>,
}

impl From<ExampleRow> for ExampleCandidate {
    fn from(row: ExampleRow) -> Self {
        ExampleCandidate {
            query_id: row.query_id,
            dataset_id: row.dataset_id,
            dataset_name: row.dataset_name,
            question: row.question,
            sql: row.sql_generated,
            understood: row.understood,
            row_count: row.row_count,
            why: row.is_why,
            rated_by: row.rated_by,
            rated_at: row.rated_at,
            approved_by: row.approved_by,
            approved_at: row.approved_at,
        }
    }
}

/// Answers rated `good`, newest rating first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExampleCandidatesResponse {
    pub examples: Vec<ExampleCandidate>,
}

/// One approved example as a candidate case for the evaluation set, in the format of the files in `eval/` (for
/// example `eval/dataco.json`). Review each case before adding it: check the SQL and set `ordered`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EvalCaseCandidate {
    pub id: String,
    pub question: String,
    pub expected_sql: Vec<String>,
    /// Always false here; set it to true when the row order is part of the answer (top N, time series).
    pub ordered: bool,
}
