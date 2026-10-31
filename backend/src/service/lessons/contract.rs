// SPDX-License-Identifier: Apache-2.0
/* service/lessons/contract.rs - kiểu request và response của nhật ký lỗi và bài học (#228), route ở api/lessons.rs.
Chú thích /// thành mô tả trong đặc tả OpenAPI nên viết tiếng Anh. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::store::{ErrorEntryRow, LessonRow};

/// The step where the model went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStep {
    /// The SQL guard refused the SQL.
    Guard,
    /// The SQL passed the guard but failed to run.
    Run,
    /// A query that would count rows twice over a one-to-many join was blocked.
    Fanout,
    /// The form of a why question was invalid.
    WhyForm,
    /// The number or direction check rejected the text the model wrote.
    Narrate,
    /// The member who asked rated the answer `bad`.
    Feedback,
}

impl ErrorStep {
    fn from_db(text: &str) -> Option<Self> {
        match text {
            "guard" => Some(ErrorStep::Guard),
            "run" => Some(ErrorStep::Run),
            "fanout" => Some(ErrorStep::Fanout),
            "why_form" => Some(ErrorStep::WhyForm),
            "narrate" => Some(ErrorStep::Narrate),
            "feedback" => Some(ErrorStep::Feedback),
            _ => None,
        }
    }
}

/// One row of the error log.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorEntry {
    pub id: Uuid,
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
    #[schema(required)]
    pub dataset_name: Option<String>,
    /// The recorded question; null when it was not recorded (waiting for a choice) or was deleted.
    #[schema(required)]
    pub query_id: Option<Uuid>,
    pub question: String,
    pub step: ErrorStep,
    pub error: String,
    /// What the model returned at that step: SQL, the form as JSON, or the text it wrote.
    #[schema(required)]
    pub model_output: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl ErrorEntry {
    /// None khi step trong database không phải giá trị đã biết (chỉ khi ai đó sửa tay).
    pub fn from_row(row: ErrorEntryRow) -> Option<Self> {
        Some(ErrorEntry {
            id: row.id,
            dataset_id: row.dataset_id,
            dataset_name: row.dataset_name,
            query_id: row.query_id,
            question: row.question,
            step: ErrorStep::from_db(&row.step)?,
            error: row.error,
            model_output: row.model_output,
            created_at: row.created_at,
        })
    }
}

/// Newest rows first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorLogResponse {
    pub entries: Vec<ErrorEntry>,
}

/// A short lesson for the model, written by an admin.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Lesson {
    pub id: Uuid,
    /// Null for a lesson of the whole workspace.
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
    #[schema(required)]
    pub dataset_name: Option<String>,
    pub text: String,
    /// Approved and in use: only these lessons go into prompts.
    pub enabled: bool,
    /// The error log row it was written from.
    #[schema(required)]
    pub error_id: Option<Uuid>,
    #[schema(required)]
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Who last enabled it; null for a draft never enabled.
    #[schema(required)]
    pub approved_by: Option<String>,
    #[schema(required)]
    pub approved_at: Option<DateTime<Utc>>,
}

impl From<LessonRow> for Lesson {
    fn from(row: LessonRow) -> Self {
        Lesson {
            id: row.id,
            dataset_id: row.dataset_id,
            dataset_name: row.dataset_name,
            text: row.text,
            enabled: row.enabled,
            error_id: row.error_id,
            created_by: row.created_by,
            created_at: row.created_at,
            approved_by: row.approved_by,
            approved_at: row.approved_at,
        }
    }
}

/// Newest first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LessonsResponse {
    pub lessons: Vec<Lesson>,
}

/// A new lesson, saved as a draft that is not used until enabled.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LessonRequest {
    /// One or two short sentences, at most 300 characters.
    pub text: String,
    /// Leave out for a lesson of the whole workspace.
    #[serde(default)]
    pub dataset_id: Option<Uuid>,
    /// The error log row the lesson comes from, if any.
    #[serde(default)]
    pub error_id: Option<Uuid>,
}

/// Turn a lesson on (approve it, recording who and when) or off.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LessonStateRequest {
    pub enabled: bool,
}
