// SPDX-License-Identifier: Apache-2.0
/* service/report/contract.rs - kiểu request và response của báo cáo, route ở api/reports.rs. Một mục dùng lại kiểu của
câu trả lời (ResultTable, ChartSuggestion, Understanding ở ask/contract.rs; SavedAnswer ở conversation/contract.rs) để
giao diện vẽ bảng, biểu đồ và phần "Cách tính" bằng đúng code của trang hỏi. Chú thích /// thành mô tả trong đặc tả
OpenAPI nên viết tiếng Anh. */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::ask::contract::{
    AnswerVerdict, ChartSuggestion, QuestionType, ResultTable, Understanding,
};
use crate::service::conversation::contract::SavedAnswer;

/// `manual`: answers the user added one by one; `automatic`: built by the backend for one dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Manual,
    Automatic,
}

/// One report of the caller, without its items.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportSummary {
    pub id: Uuid,
    pub title: String,
    /// Opening paragraph written by the owner (Vietnamese); empty when none.
    pub intro: String,
    pub kind: ReportKind,
    /// Dataset of an automatic report (`DatasetSummary.id`); null for a manual report.
    #[schema(required)]
    pub dataset_id: Option<Uuid>,
    /// Language model calls made while building an automatic report; null for a manual report.
    #[schema(required)]
    pub model_calls: Option<u32>,
    pub item_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Reports of the caller, last changed first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportListResponse {
    pub reports: Vec<ReportSummary>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateReportRequest {
    pub title: String,
    #[serde(default)]
    pub intro: String,
}

/// Fields left out keep their value.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateReportRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub intro: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AddReportItemRequest {
    /// `AskResponse.query_id` (or `TurnItem.query_id`) of an answered question the caller asked.
    pub query_id: Uuid,
}

/// Every item id of the report, in the new order.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReorderReportRequest {
    pub item_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AutoReportRequest {
    /// Dataset to analyse (`DatasetSummary.id`).
    pub dataset_id: Uuid,
}

/// The automatic report just built, and the sections that could not be built.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AutoReportResponse {
    pub report: ReportSummary,
    /// One Vietnamese sentence per planned section left out, with the reason.
    pub skipped: Vec<String>,
}

/// A report with every item rendered again from the stored questions.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportResponse {
    pub report: ReportSummary,
    pub items: Vec<ReportItem>,
    /// When the items were run again; numbers of every item are as of this time, on the stored data versions.
    pub rendered_at: DateTime<Utc>,
}

/// Outcome of the number check (the backend compares every number and direction of the model's text with the computed
/// facts): `passed` (the model's text passed; it is the shown text, or it failed the meaning check afterwards and the
/// backend's text replaced it, see `ReportItem.meaning_check`), `replaced` (the model's text failed this check and the
/// shown text was written by the backend), `not_needed` (the backend wrote the text from the numbers, no model text to
/// check).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NumberCheck {
    Passed,
    Replaced,
    NotNeeded,
}

/// Table and data version an item read.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportTableVersion {
    pub table_id: Uuid,
    pub name: String,
    pub version: i32,
    /// When this version was snapshotted or uploaded.
    pub captured_at: DateTime<Utc>,
    /// False when the table has a newer version: the item still reads this one.
    pub is_current: bool,
}

/// The breakdown kept with an item: the change between two periods or two groups and the groups of its first dimension
/// that made most of it. Numbers are decimal strings as in `Explanation`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportBreakdown {
    pub measure: String,
    pub base_label: String,
    pub compare_label: String,
    pub base_value: String,
    pub compare_value: String,
    pub change: String,
    /// Label of the dimension the groups belong to.
    pub dimension: String,
    pub groups: Vec<ReportBreakdownGroup>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportBreakdownGroup {
    /// Group value as text; empty for a missing value.
    pub name: String,
    pub base_value: String,
    pub compare_value: String,
    pub contribution: String,
    /// Share of the total change, decimal string; null when the change is zero.
    #[schema(required)]
    pub share_of_change: Option<String>,
}

/// Facts the backend computed from the numbers of the item when it was rendered (no language model): the main finding
/// and its points, Vietnamese.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportFacts {
    pub title: String,
    pub points: Vec<String>,
}

/// One item of a report, rendered again: the stored question and answer, and the result of running its stored SQL
/// again through the guard with the caller's access, on the data versions the question read.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReportItem {
    pub id: Uuid,
    pub query_id: Uuid,
    pub question: String,
    pub asked_at: DateTime<Utc>,
    pub question_type: QuestionType,
    /// Tables and versions the question read, the starting table first.
    pub tables: Vec<ReportTableVersion>,
    /// The answer text shown when the question was asked, with its checks. Null when it was not stored, or when the
    /// caller may now see fewer rows than when the question was asked (the stored text could cite rows outside them).
    #[schema(required)]
    pub answer: Option<SavedAnswer>,
    /// Number check of `answer`; null when `answer` is null.
    #[schema(required)]
    pub number_check: Option<NumberCheck>,
    /// Meaning check of the model's text (a separate model call compared it with the computed facts): `mismatch` means
    /// the model's text failed and the shown text is the backend's; never a verdict on the backend's text. Null when it
    /// did not run or `answer` is null.
    #[schema(required)]
    pub meaning_check: Option<AnswerVerdict>,
    /// Facts computed again from `result`; null when the result shape has none.
    #[schema(required)]
    pub facts: Option<ReportFacts>,
    /// How the numbers were computed: measures, filters (including the caller's row filters), grouping, period.
    #[schema(required)]
    pub understanding: Option<Understanding>,
    /// SQL that ran now, after the guard.
    #[schema(required)]
    pub sql: Option<String>,
    /// Result rows, at most as many as an answer sends; `total_rows` is the full count.
    #[schema(required)]
    pub result: Option<ResultTable>,
    #[schema(required)]
    pub chart: Option<ChartSuggestion>,
    #[schema(required)]
    pub breakdown: Option<ReportBreakdown>,
    /// Vietnamese notes on how this rendering differs from when the question was asked (row count, newer table
    /// version, fewer visible rows).
    pub notes: Vec<String>,
    /// Vietnamese message when the stored SQL could not run again; the other result fields are then null.
    #[schema(required)]
    pub error: Option<String>,
}
