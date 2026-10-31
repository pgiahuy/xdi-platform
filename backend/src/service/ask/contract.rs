// SPDX-License-Identifier: Apache-2.0
/* service/ask/contract.rs - hợp đồng của POST /workspaces/{ws}/datasets/{id}/ask: kiểu response và lựa chọn người
hỏi gửi kèm request. service/ask/mod.rs dựng AskResponse, api/query.rs trả nó ra JSON; đặt ở service chứ không ở api để
phụ thuộc vẫn đi một chiều (như UploadResult ở ingest.rs). Mẫu đúng kiểu ở ask/examples.rs. Quyết định và lý do ở
docs/ARCHITECTURE.md mục 8.

Chú thích /// thành mô tả trong đặc tả OpenAPI nên viết tiếng Anh; ghi chú cho đội dùng //. utoipa không đưa chú
thích của từng biến thể enum vào đặc tả, nên nghĩa của từng giá trị ghi ở chú thích của cả enum. Trường Option luôn có
trong JSON (null khi trống) nên đánh dấu required, để kiểu TypeScript là `T | null` thay vì trường tùy chọn.
openapi-typescript bỏ mô tả của trường Option chứa struct, nên khi nào trường đó là null ghi ở chú thích của struct
đích (và ở AskStatus); với struct dùng chung (UnderstoodItem, CellValue) thì ghi ở chú thích của struct chứa trường,
để người đọc schema.ts vẫn thấy. */

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::engine::llm::meter::LlmCall;
use crate::service::explain::annotate::BreakdownNotes;

// Kết quả bình thường của một câu hỏi, kể cả khi không có số. Câu không trả lời được vẫn là 200 có trạng thái trong
// body: giao diện cần hiện SQL bị từ chối, các bước và lựa chọn, mà ErrorBody chỉ có code và message.
/// Outcome of the question, and which fields of `AskResponse` are set:
/// - `answered`: the SQL ran. Set: `query_id`, `sql_generated`, `sql_guarded`, `understanding`, `result`; `chart`
///   when a chart fits; `explanation` for a "why" question (when the breakdown cannot be made, a warning
///   `explanation_unavailable` says why).
/// - `needs_choice`: the question can be read several ways; pick one option of each entry in `clarifications` and ask
///   again. This can happen before the model is called (a word matches several columns) or after (the two slices of
///   a "why" question cannot be told). Set: `message`, `clarifications`; `sql_generated` when the model was called.
///   Not recorded in the history, so `query_id` is null.
/// - `rejected`: the guard refused the SQL the model wrote in the `repair` round (when the guard refuses the model's
///   first SQL, for any reason, the question always gets that round), or the SQL the server built for a "why"
///   breakdown. Set: `message`, `query_id`, `sql_generated`. For the model's SQL, the `guard` step that stopped it has
///   `ok` false and the guard's reason in `detail`, and `message` is a fixed text asking to rephrase the question, since
///   the user cannot fix the SQL; for a breakdown, `message` gives the guard's reason. Also when
///   the model finds that no column of the dataset holds the information the question needs (a rating that is not
///   there, a forecast): no SQL runs, `sql_generated` is null, and `message` gives the reason and the closest columns.
/// - `failed`: the SQL could not run and the one fix round did not help, either because the fixed SQL failed too or
///   because the model failed during the fix; or the SQL ran past the time or memory limit, which gets no fix round.
///   Set: `message`, `query_id`, `sql_generated`; `sql_guarded` unless that SQL never passed the guard. The `run` or
///   `repair` step that stopped it has `ok` false.
///
/// A fix round starts when the analytics engine fails on the SQL, when the guard refuses it for any reason, or when the
/// model's reply cannot be read; there is at most one round per question, and the fixed SQL goes through the guard
/// again. The status names the step that stopped the question: the guard gives `rejected`, the engine or the fix round
/// gives `failed`. A model error before any SQL exists is not a status: the request gets 502 or 503.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AskStatus {
    Answered,
    NeedsChoice,
    Rejected,
    Failed,
}

/// Answer to one question, with the evidence behind it. See `AskStatus` for which fields are set.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AskResponse {
    pub status: AskStatus,
    /// The question exactly as sent in `AskRequest.question`.
    pub question: String,
    /// Text for the end user (Vietnamese): the reason for `rejected` and `failed`, the prompt for `needs_choice`;
    /// null for `answered`.
    #[schema(required)]
    pub message: Option<String>,
    /// Id of the question in the history (`queries`); null for `needs_choice`.
    #[schema(required)]
    pub query_id: Option<Uuid>,
    /// Conversation this question belongs to (`AskRequest.conversation_id`, or a new one when it was empty). Send it
    /// back with the next question so the model reads it as a follow-up.
    pub conversation_id: Uuid,
    /// Dataset version the question ran on.
    pub version_id: Uuid,
    /// Language model configured for the question, for example `gemini-2.5-flash` or `mock`.
    pub model: String,
    // Trả cả lựa chọn mặc định lẫn lựa chọn đã trả lời cho câu hỏi lại: giao diện chỉ cần response gần nhất để hỏi
    // lại, không phải tự gom lựa chọn qua nhiều lần hỏi (thiếu một lựa chọn là backend hỏi lại chỗ đó mãi).
    /// Every choice this answer used, defaults included, in the shape of `AskRequest.choices`. To ask again with one
    /// choice changed, send this list with that entry replaced; for `needs_choice`, send it plus a pick for each
    /// clarification. Choices the backend did not recognize are not in it.
    pub choices: Vec<ChosenOption>,
    /// SQL written by the model, on logical table names. After a fix round, the fixed SQL.
    #[schema(required)]
    pub sql_generated: Option<String>,
    /// SQL that ran: the generated SQL after the guard added row-level filters and a row limit. Still on logical table
    /// names, no file paths.
    #[schema(required)]
    pub sql_guarded: Option<String>,
    /// Tables `sql_guarded` read, in the order the query first reads them: the dataset asked about, and related
    /// datasets of the same database snapshot when the SQL joins them. Empty unless the status is `answered`.
    pub tables_read: Vec<TableRead>,
    #[schema(required)]
    pub understanding: Option<Understanding>,
    #[schema(required)]
    pub result: Option<ResultTable>,
    #[schema(required)]
    pub chart: Option<ChartSuggestion>,
    /// Notes shown next to the result; they never hide it.
    pub warnings: Vec<Warning>,
    /// Breakdown of a "why" question; for other answered questions, the automatic breakdown behind `insight` when it
    /// has one (the change of a single number against the period before, or the period of a time series that changed
    /// most). Null otherwise.
    #[schema(required)]
    pub explanation: Option<Explanation>,
    /// What the result says, for an `answered` question that is not a "why" question and has at least one row with a
    /// value: chosen by the shape of the result, every number computed by the backend. Null otherwise; a "why" question
    /// has `explanation.summary`.
    #[schema(required)]
    pub insight: Option<Insight>,
    /// Short answer in Vietnamese for an `answered` question that is not a "why" question and has at least one row:
    /// `insight.summary` when the language model wrote it and it passed the checks (the model only sees the sentences
    /// the backend computed, never result rows), otherwise a template sentence written by the backend. A list of rows
    /// without aggregates always gets the template (row count and filters). Null otherwise; a "why" question has
    /// `explanation.summary`.
    #[schema(required)]
    pub answer_text: Option<String>,
    /// True when `answer_text` is the language model's text of `insight.summary`, false for the template sentence (no
    /// checked model text, a list of rows, or `ANSWER_TEXT=false`). Null when `answer_text` is null.
    #[schema(required)]
    pub answer_text_by_model: Option<bool>,
    /// Kind of the question chosen by the classify step; null when the question stopped before that step.
    #[schema(required)]
    pub question_type: Option<QuestionType>,
    /// Questions back to the user, only for `needs_choice`. Every entry has `selected` null.
    pub clarifications: Vec<Choice>,
    /// What the backend did, in order, with timings. A step may appear twice, for example `run` before and after a
    /// `repair`.
    pub steps: Vec<AskStep>,
    /// Every call to the language model for this question, in the order the calls finished, with wall time and tokens.
    /// The sum of `duration_ms` is the time spent waiting for the model; the rest of the step time is the backend.
    pub model_calls: Vec<LlmCall>,
}

/// A dataset the SQL of an answer read.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TableRead {
    pub dataset_id: Uuid,
    /// Dataset name as shown in the dataset list.
    pub name: String,
    /// Table name the SQL uses for this dataset.
    pub logical_table: String,
}

// Mỗi mục kèm đoạn SQL nó lấy ra, để người đọc đối chiếu được và để #35 kiểm thẻ luôn khớp câu đã chạy.
/// One thing the question was understood to mean.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UnderstoodItem {
    /// Text for the end user (Vietnamese) naming the measure, filter, grouping, order or period.
    pub label: String,
    /// Fragment of `sql_guarded` this item comes from.
    pub sql: String,
    /// True when the item is carried over from the previous turn of the conversation: the same as in that turn's card
    /// (same SQL fragment or same label) and not named in this question. Omitted when false.
    // Bỏ khi false để response của câu đơn lẻ không đổi; đánh dấu ở service/conversation/carryover.rs.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub from_previous: bool,
}

/// The "understood as" card, built from `sql_guarded`: what is measured, filtered, grouped, sorted, over which
/// period, and the assumptions made. Null in `AskResponse` unless the status is `answered`. `limit` is null when only
/// the guard's row cap applies; `time_range` is null when the SQL has no time filter.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Understanding {
    pub measures: Vec<UnderstoodItem>,
    /// Filters of the question, then the row-level filters the guard added for the user's data role (the label says
    /// which).
    pub filters: Vec<UnderstoodItem>,
    pub group_by: Vec<UnderstoodItem>,
    pub order_by: Vec<UnderstoodItem>,
    /// Row limit the question asked for, for example the top 3; null when only the guard's limit applies.
    #[schema(required)]
    pub limit: Option<UnderstoodItem>,
    /// Period the question covers; null when the SQL has no time filter.
    #[schema(required)]
    pub time_range: Option<UnderstoodItem>,
    /// Assumptions the user can change, including clarifications answered earlier; every entry has `selected` set.
    pub assumptions: Vec<Choice>,
}

// Một kiểu cho cả giả định (đã chọn sẵn) và câu hỏi lại khi mơ hồ (chưa chọn): giao diện vẽ cùng một kiểu chip, và
// request trả lời cả hai bằng cùng một danh sách ChosenOption. Id do backend đặt, giao diện chỉ gửi lại nguyên văn;
// backend không giữ trạng thái giữa hai lần hỏi, mọi thứ cần để hỏi lại nằm trong câu hỏi và các lựa chọn.
// Id đặt từ loại lựa chọn và đối tượng của nó, không từ chữ của SQL: câu "tại sao" hỏi lại sau khi gọi mô hình, lần
// hỏi sau mô hình viết SQL khác; id sinh từ SQL sẽ đổi, lựa chọn bị bỏ qua và người dùng bị hỏi lại mãi.
/// A point where the question can be read several ways. To change it, ask the same question again with
/// `AskResponse.choices`, the entry for this `id` replaced.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Choice {
    /// Id set by the backend; send it back unchanged. Stable for the same question on the same dataset version: made
    /// from the kind of choice and what it is about (a word and its candidate columns, the period to compare with),
    /// never from the text of the SQL, so a pick still applies when the model writes different SQL.
    pub id: String,
    /// Text for the end user (Vietnamese): what can be read several ways, for example which column a word means.
    pub label: String,
    /// Id of the option in use; null when the user has to pick (`needs_choice`).
    #[schema(required)]
    pub selected: Option<String>,
    pub options: Vec<ChoiceOption>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChoiceOption {
    /// Opaque id set by the backend; send it back unchanged.
    pub id: String,
    /// Text for the end user (Vietnamese).
    pub label: String,
}

// Id không nhận ra thì bỏ qua thay vì 400: id phụ thuộc vào SQL mô hình viết, hỏi lại có thể ra SQL khác và một giả
// định cũ không còn. Trả 400 thì người dùng kẹt, không hỏi tiếp được.
/// The pick for one `Choice`. A `choice_id` or `option_id` the backend does not recognize (for example a choice that
/// no longer applies) is ignored, left out of `AskResponse.choices`, and reported with a warning `choice_not_applied`:
/// an assumption falls back to its default, a clarification is asked again.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChosenOption {
    /// `Choice.id` from an earlier answer.
    pub choice_id: String,
    /// One of the `ChoiceOption.id` of that choice.
    pub option_id: String,
}

// Ô gửi theo kiểu cột (ValueKind), quyết định theo kiểu chứ không theo từng giá trị: cùng một cột luôn cùng một dạng.
// Số JavaScript chỉ giữ đúng số nguyên tới 2^53 và không giữ đúng số thập phân, nên kiểu số rộng hơn gửi dạng chuỗi.
/// How to read the cells of a column, decided by the column type. Any cell may also be null (SQL NULL).
/// - `text`: JSON string.
/// - `number`: JSON number, for types a JavaScript number holds exactly: integers up to 32 bits, FLOAT, DOUBLE. NaN
///   and infinity are sent as null.
/// - `decimal`: JSON string of decimal digits, integer or not, for example "5000" or "-1234.50", for BIGINT
///   (COUNT returns one), HUGEINT, unsigned 64 and 128 bit integers and DECIMAL, which a JavaScript number would
///   round. Convert with `Number()` only to draw a chart or to sort; comparing the strings sorts "10" before "9".
/// - `boolean`: JSON boolean.
/// - `date`: JSON string `YYYY-MM-DD`.
/// - `timestamp`: JSON string in ISO 8601, `YYYY-MM-DDTHH:MM:SS[.ffffff]` for TIMESTAMP (no time zone, as stored),
///   ending in `Z` for TIMESTAMPTZ (converted to UTC).
/// - `other`: JSON string as the analytics engine prints the value: TIME, INTERVAL, lists, structs and other types.
///
/// A `date` or `timestamp` with a year after 9999 or before 1 AD uses the ISO 8601 expanded form: `+10000-01-01`;
/// year 1 BC is `0000`, 2 BC is `-0001`. Infinite dates and times are sent as null. A result column of type TIME_NS,
/// TIME WITH TIME ZONE, BIT, BIGNUM, VARIANT, GEOMETRY or a type the backend does not recognize is not read: the `run`
/// step fails and the fix round casts it to text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Text,
    Number,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Other,
}

// Vai trò lấy từ cây cú pháp của sql_guarded (#35): cột trong GROUP BY là khóa nhóm, cột tổng hợp là độ đo. Giao diện
// biết ô nào bấm được để truy dòng nguồn, và khóa của dòng là giá trị các cột dimension và time.
/// What a result column is:
/// - `dimension`: a grouping key that is not time, for example a market.
/// - `time`: a grouping key on time, for example a quarter.
/// - `measure`: an aggregate, for example a sum; its cells can be drilled into.
/// - `other`: anything else, for example a column of a query without aggregates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnRole {
    Dimension,
    Time,
    Measure,
    Other,
}

// untagged: serde ghi thẳng giá trị bên trong (5, "Europe", true), không bọc tên biến thể. Deserialize vì request truy
// dòng nguồn (drill/contract.rs) gửi lại đúng giá trị ô đã nhận.
/// One cell: a JSON number, string or boolean; read it with the `kind` of its column.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum CellValue {
    Number(f64),
    String(String),
    Boolean(bool),
}

// Tên duy nhất để biểu đồ, tiêu đề CSV và khóa drill trỏ cột bằng tên mà không nhầm. DuckDB trả tên trùng nguyên văn,
// nên backend đổi bằng build_unique_names; drill (#34) dùng lại đúng hàm đó để từ tên tìm lại cột.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResultColumn {
    /// Column name as the SQL returns it, made unique: from left to right, a name already used gets the smallest
    /// suffix `_2`, `_3`, ... that no column has, so `a, a, a_2` becomes `a, a_3, a_2`.
    pub name: String,
    pub kind: ValueKind,
    pub role: ColumnRole,
}

/// Tên cột duy nhất cho ResultColumn.name, theo quy tắc ghi trên trường đó. Hậu tố tránh mọi tên đã có trong kết quả,
/// kể cả tên đứng sau, nên không đổi ra một tên trùng với cột phía sau.
pub fn build_unique_names(names: &[String]) -> Vec<String> {
    let present: HashSet<&str> = names.iter().map(String::as_str).collect();
    let mut used: HashSet<String> = HashSet::new();
    let mut unique = Vec::with_capacity(names.len());
    for name in names {
        let mut candidate = name.clone();
        let mut suffix = 2;
        // Tên mới không được trùng tên đã dùng, cũng không trùng tên gốc của một cột khác dù cột đó chưa tới lượt.
        while used.contains(&candidate)
            || (candidate != *name && present.contains(candidate.as_str()))
        {
            candidate = format!("{name}_{suffix}");
            suffix += 1;
        }
        used.insert(candidate.clone());
        unique.push(candidate);
    }
    unique
}

// Mảng theo vị trí thay vì object theo tên cột: gọn hơn khi nhiều dòng. Bảng của v0.1.0 phân trang trong các dòng đã
// gửi và tải CSV đúng các dòng đó; tải đủ mọi dòng là endpoint riêng, làm sau.
/// Rows of the result; null in `AskResponse` unless the status is `answered`. At most 1000 rows are sent.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResultTable {
    pub columns: Vec<ResultColumn>,
    /// One array per row, cells in the order of `columns`; null is a SQL NULL.
    pub rows: Vec<Vec<Option<CellValue>>>,
    /// Number of rows `sql_guarded` returned. This is the full result unless `limit_reached`.
    pub total_rows: u64,
    /// True when `rows` holds fewer rows than `total_rows`.
    pub truncated: bool,
    /// True when `total_rows` equals the guard's row cap (whether the guard added the LIMIT or lowered one the SQL
    /// had): the full result may have more rows, so show
    /// `total_rows` as "at least".
    pub limit_reached: bool,
}

// Bộ luật chọn biểu đồ theo IBCS ở ARCHITECTURE mục 6, dựng ở service/ask/chart.rs. Thời gian luôn nằm trên trục ngang,
// nhóm (cấu trúc) nằm trên trục đứng, nên so sánh nhóm là thanh ngang.
/// Kind of chart:
/// - `bar`: horizontal bars, one per group of `x`, largest first; compares groups.
/// - `column`: vertical columns over the periods of `x`; up to 10 periods.
/// - `line`: a line with a marker on each period of `x`; more than 10 periods, or one line per group of `series`.
/// - `stacked_column`: vertical columns over the periods of `x`, each split into the groups of `series`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Column,
    Line,
    StackedColumn,
}

/// How the measures in `y` are drawn:
/// - `single`: one measure.
/// - `grouped`: side by side on one scale, because the measures share a unit.
/// - `panels`: one small chart per measure sharing the axis of `x`, because the units differ or are unknown; two
///   scales in one chart are not used, since choosing them can make the same numbers look like anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartLayout {
    Single,
    Grouped,
    Panels,
}

/// Suggested chart; null in `AskResponse` when the result is better shown as a table only (one value, too many
/// groups, more than one grouping column without time). Every name is a name from `result.columns`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChartSuggestion {
    pub kind: ChartKind,
    /// Category axis: a group column for `bar`, a time column for the other kinds.
    pub x: String,
    /// Measures to draw, at most 4.
    pub y: Vec<String>,
    pub layout: ChartLayout,
    /// For `stacked_column`, and for `line` with one line per group: the group column whose values become the parts
    /// or the lines. Null otherwise.
    #[schema(required)]
    pub series: Option<String>,
    /// Measures in `y` that are not totals (averages, ratios, minimum, maximum): drawn as narrow columns, and never
    /// summed into a "Khác" entry.
    pub ratio: Vec<String>,
    /// Most groups to draw, largest first: bars for `bar`, parts for `stacked_column`. Null when every group is drawn.
    #[schema(required)]
    pub top: Option<u32>,
    /// The groups after `top` can be summed into one entry labelled "Khác": every measure is a sum or a plain count,
    /// and `result` holds every row. False: they are left out, and the chart says how many.
    pub other_bucket: bool,
}

// Bốn loại đầu do kiểm hợp lý dựng (service/ask/checks.rs), ba loại sau do chính luồng hỏi. Thêm loại mới là đổi
// API: chạy make openapi.
/// Kind of note:
/// - `empty_result`: no rows.
/// - `all_null`: a result column holds only nulls.
/// - `filter_value_not_found`: the SQL filters on a value that does not occur in the column.
/// - `unusual_magnitude`: a value is far outside what the data makes likely, for example a ratio above 100%.
/// - `explanation_unavailable`: a "why" question was answered without a breakdown; the message says why.
/// - `choice_not_applied`: a choice sent with the question was not recognized and the default was used.
/// - `premise_mismatch`: a "why" question assumes a direction (a drop, a rise) that the computed change does not have;
///   the breakdown explains the actual change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
    EmptyResult,
    AllNull,
    FilterValueNotFound,
    UnusualMagnitude,
    ExplanationUnavailable,
    ChoiceNotApplied,
    PremiseMismatch,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Warning {
    pub code: WarningCode,
    /// Text for the end user (Vietnamese).
    pub message: String,
}

/// Kind of question by the purpose of the analysis, as the classify step decided: `quantitative` (a number, a list or
/// a comparison, answered by one SQL query) or `causal` (why a number changed or differs between two groups, answered by
/// a breakdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    Quantitative,
    Causal,
}

// Kết luận hai cổng của #201, cho lời của câu "tại sao" và lời phần giải thích của câu thường; giao diện vẽ thành cờ màu
// ở bước viết lời của tab Các bước.
/// Verdict of the two checks on a text written by the language model: `match` (agrees with the computed numbers),
/// `warning` (some parts can be read several ways, not certainly wrong), `mismatch` (wrong on every attempt; the model's
/// text is replaced by a template sentence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnswerVerdict {
    Match,
    Warning,
    Mismatch,
}

// Số trong phân rã là chuỗi chữ số thập phân như ô Decimal. Độ đo đổi sang DECIMAL ở từng dòng nguồn trước khi cộng
// (số chữ số do #39 chọn), nên mọi tổng và hiệu sau đó là phép tính đúng: không làm tròn từng nhóm, tổng đóng góp bằng
// đúng chênh lệch, và cổng 1 của #201 so số mô hình khai với đúng các chuỗi này. Bảng kết quả thì cộng theo kiểu cột
// (DOUBLE cộng bằng số thực), nên giá trị hai lát cắt có thể lệch ô tương ứng của bảng ở chữ số cuối; con số của phân
// rã là con số ở đây.
// Phần giải thích của câu thường (#227), dựng ở service/insight/: code chọn loại theo hình dạng bảng kết quả và viết câu
// chính cùng các ý; mô hình chỉ diễn đạt lại, và lời của nó qua hai cổng như lời của câu "tại sao" (#201): code kiểm
// số, chiều, từ chỉ nguyên nhân; rồi mô hình chấm so nghĩa của lời với các câu của code.
/// Explanation of a plain answer, chosen by the shape of the result.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Insight {
    pub kind: InsightKind,
    /// Main finding, one sentence written by the backend (Vietnamese).
    pub title: String,
    /// Points under the main finding, written by the backend (Vietnamese).
    pub points: Vec<InsightPoint>,
    /// Plain-language text for the end user (Vietnamese): the language model's wording of `title` and `points` when it
    /// passed the checks, otherwise `title` and `points` joined.
    pub summary: String,
    /// True when `summary` was written by the language model; false when it is the backend's text (the model failed,
    /// timed out, its text failed the checks on every attempt, or `INSIGHT_TEXT=false`). The checks: every number and
    /// direction must come from the points, no causal word, then a separate model call compares the meaning of the text
    /// with `title` and `points`; when a check fails or finds an unclear point, the model rewrites once.
    pub summary_by_model: bool,
    /// Parts of `summary` that the separate check found unclear but not certainly wrong, one Vietnamese sentence each,
    /// written by the checking model; show them with `summary`. Empty when nothing was unclear or `summary` is the
    /// backend's text.
    pub summary_warnings: Vec<String>,
    /// Outcome of the two checks on the model's text: `match` or `warning` when `summary` is the model's text,
    /// `mismatch` when the model's text failed every attempt and `summary` is the backend's text, null when no check ran
    /// (the model was not called, failed or timed out, or the checking model failed).
    #[schema(required)]
    pub summary_verdict: Option<AnswerVerdict>,
    /// False when the numbers show nothing notable: a change below the threshold, too few rows to conclude, groups of
    /// about the same size, or a plain list of rows.
    pub notable: bool,
}

/// - `single_value`: one number; compared with the period before when the question names a period.
/// - `time_series`: one measure over periods; overall direction and the period that changed most against the one
///   before, broken down by dimension in `AskResponse.explanation`.
/// - `groups`: one measure over the groups of one column; share of the top group, how many groups make 80% of the
///   total, groups far from the mean.
/// - `top_n`: the first groups by a measure (the SQL has a limit); their share of the total and the gap between the
///   first two.
/// - `rows`: anything else, such as rows without aggregates; only the number of rows and the filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    SingleValue,
    TimeSeries,
    Groups,
    TopN,
    Rows,
}

/// One point of an insight: a short lead (shown in bold) and the sentence after it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct InsightPoint {
    /// Lead of the point (Vietnamese), for example the words for "concentration".
    pub lead: String,
    /// Rest of the point (Vietnamese).
    pub text: String,
}

/// Breakdown of a change between two slices (two periods or two groups); see `AskResponse.explanation` for when it is
/// set. Numbers are exact strings of decimal digits.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Explanation {
    /// Measure that changed, text for the end user (Vietnamese).
    pub measure: String,
    /// How the measure adds up over groups, which decides how to read the numbers below.
    pub measure_kind: MeasureKind,
    /// What the two slices are, which decides the words for the change: up or down between two periods, higher or
    /// lower between two groups.
    pub comparison: Comparison,
    /// True when the measure counts rows (a count, not a sum): the measure is then itself the size of each slice.
    pub counts_rows: bool,
    /// Column whose two values are compared, for `groups`; null for `periods`.
    #[schema(required)]
    pub slice_column: Option<String>,
    /// Vietnamese name of `slice_column` from the column profile; null when the profile has none.
    #[schema(required)]
    pub slice_column_label: Option<String>,
    /// Slice compared against.
    pub base: ExplanationSlice,
    /// Slice being explained.
    pub compare: ExplanationSlice,
    /// `compare.value - base.value`.
    pub change: String,
    /// `change / base.value * 100`, rounded to one decimal; null when `base.value` is zero or negative.
    #[schema(required)]
    pub percent_change: Option<String>,
    /// One breakdown per dimension, the dimension that explains the change best first.
    pub dimensions: Vec<DimensionBreakdown>,
    #[schema(required)]
    pub volume_price: Option<VolumePrice>,
    /// Plain-language summary for the end user (Vietnamese), as one text: `summary_title` then each point as
    /// "lead: text". When `summary_by_model` the model wrote it from the numbers the backend computed, its claims were
    /// checked against this breakdown and an independent model call compared the meaning of the text with the claims
    /// and the names of the two slices; otherwise the backend wrote it from a template.
    pub summary: String,
    /// Main answer, one sentence (Vietnamese): written by the model when `summary_by_model`, otherwise by the backend
    /// from the numbers above.
    pub summary_title: String,
    /// Points under the main answer (Vietnamese). When `summary_by_model`, the main causes the model wrote, all of kind
    /// `reason`; otherwise written by the backend, each said only when the numbers support it.
    pub summary_points: Vec<SummaryPoint>,
    /// False when `summary` is a template sentence written by the backend: the change is zero (the model is not
    /// called), the model was not reached, the independent check could not judge the text, or
    /// the text failed the checks twice (one rewrite is asked).
    pub summary_by_model: bool,
    /// Parts of the model's text that the independent check found unclear but not certainly wrong, one Vietnamese
    /// sentence each, written by the checking model; show them with `summary`. Empty when nothing was unclear or
    /// `summary` is a template sentence.
    pub summary_warnings: Vec<String>,
    /// Outcome of the two checks on the model's text: `match` or `warning` when `summary` is the model's text,
    /// `mismatch` when the model's text failed every attempt and `summary` is the template, null when no check ran (no
    /// change to explain, or the model or the checking model failed).
    #[schema(required)]
    pub summary_verdict: Option<AnswerVerdict>,
    /// Dimensions left out of `dimensions` or of the summary, with the reason; show them as a note with the
    /// breakdown so that a missing table is explained.
    pub dropped_dimensions: Vec<DroppedDimension>,
    // Ghi chú nội bộ cho lời tóm tắt (% tác động, nhóm khác thường, chiều bỏ qua), không gửi qua API: annotate.rs
    // tính, summary.rs và cổng 1 của narrate (explain/claim_cell.rs) đọc. Bảng `dimensions` ở trên không đổi vì ghi
    // chú này.
    #[serde(skip)]
    pub notes: BreakdownNotes,
}

// Tỷ lệ không cộng được theo nhóm nên phân rã khác (explain/ratio.rs): giá trị của lát cắt và của nhóm là tỷ lệ, đóng
// góp của nhóm tách thành phần cơ cấu và phần hiệu suất.
/// - `total`: a sum or a count; group values add up to the slice value, a contribution is `compare_value - base_value`.
/// - `ratio`: a ratio or an average (numerator over denominator); slice and group values are ratios rounded to a fixed
///   number of decimals, a contribution is `mix_effect + rate_effect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeasureKind {
    Total,
    Ratio,
}

// Hai nhóm (Pass và Withdrawn) không có trước sau: nói "Withdrawn tăng so với Pass" làm người đọc tưởng một nhóm đổi
// theo thời gian. Lời viết theo khuôn, prompt của mô hình và giao diện chọn chữ theo trường này.
/// - `periods`: the slices are two periods; `compare` is the later one.
/// - `groups`: the slices are two values of one column, with no order in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Periods,
    Groups,
}

// filter là điều kiện SQL, chỉ để hiện và kiểm. Truy dòng nguồn từ một dòng phân rã gửi tên lát cắt (base, compare),
// không gửi SQL: backend lấy điều kiện đã lưu cùng câu hỏi, client không đưa SQL vào được.
/// One of the two slices.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExplanationSlice {
    /// Text for the end user (Vietnamese) naming the slice, for example a quarter.
    pub label: String,
    /// SQL condition that selects the rows of the slice, on logical names.
    pub filter: String,
    /// Value of the measure on the slice.
    pub value: String,
    /// Number of rows of the slice.
    pub rows: String,
}

/// One point of the summary: a short lead (shown in bold) and the sentence after it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SummaryPoint {
    pub kind: SummaryPointKind,
    /// Lead of the point (Vietnamese), for example "Lý do chính".
    pub lead: String,
    /// Rest of the point (Vietnamese).
    pub text: String,
}

/// - `reason`: what mostly makes the change (the volume and the value per unit, or the mix and the rate); every point
///   written by the model has this kind.
/// - `outliers`: groups that changed much more or much less than the whole, or exist on one side only.
/// - `even`: no group stands out; the largest contributions are the largest groups.
/// - `largest`: the group with the largest contribution in the direction of the change, when the numbers do not allow
///   comparing groups with the whole (a base slice of zero or less, or a change too close to zero).
/// - `against`: a group that moved the other way.
/// - `dropped`: dimensions the summary leaves out and why; they stay in `Explanation.dimensions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SummaryPointKind {
    Reason,
    Outliers,
    Even,
    Largest,
    Against,
    Dropped,
}

/// Contribution of every group of one dimension; the contributions add up to `change`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DimensionBreakdown {
    /// Dimension column, as named in the dataset (not a name from `result.columns`).
    pub column: String,
    /// Vietnamese name of the column from the column profile; null when the profile has none.
    #[schema(required)]
    pub label: Option<String>,
    /// How to read the group values, like the `kind` of a result column.
    pub kind: ValueKind,
    /// SQL that computed the groups of both slices, after the guard.
    pub sql: String,
    /// Every group of the column, largest contribution (by absolute value) first.
    pub groups: Vec<GroupContribution>,
    /// Ratio measures only: sum of `mix_effect` of every group; `mix_effect + rate_effect` is `change`.
    #[schema(required)]
    pub mix_effect: Option<String>,
    /// Ratio measures only: sum of `rate_effect` of every group.
    #[schema(required)]
    pub rate_effect: Option<String>,
    /// True when at least one group is an `outlier`. Dimensions with outliers come first, each part in the order of
    /// how well the dimension explains the change.
    pub has_outliers: bool,
}

// Bảng phân rã đọc các trường dưới đây thay vì tự tính lại (#186): luật "khác thường", "mới", "không còn", "đổi dấu"
// chỉ viết một lần ở explain/annotate.rs, cùng chỗ lời tóm tắt đọc, nên bảng và lời không thể nói khác nhau.
/// Where a group has rows: `both` slices, only the compared slice (`new`), or only the base slice (`gone`). For a
/// ratio, a group has rows in a slice when its denominator there is not zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupPresence {
    #[default]
    Both,
    New,
    Gone,
}

/// One dimension left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct DroppedDimension {
    pub column: String,
    /// Vietnamese name of the column from the column profile; null when the profile has none.
    #[schema(required)]
    pub label: Option<String>,
    pub reason: DropReason,
    /// Column with the same groups, for `duplicates`.
    #[schema(required)]
    pub duplicate_of: Option<String>,
    /// For `separates_slices`: values found only in the base slice, as text.
    pub base_only: Vec<String>,
    /// For `separates_slices`: values found only in the compared slice, as text.
    pub compare_only: Vec<String>,
}

/// - `separates_slices`: every value of the column is found in one slice only, so the column only renames the two
///   slices.
/// - `duplicates`: the column splits the rows into exactly the groups of another column (`duplicate_of`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    SeparatesSlices,
    Duplicates,
}

// value cùng column của DimensionBreakdown là khóa nhóm, để bấm một dòng thì truy được dòng nguồn (#34, #52).
/// One group of a dimension. `value` is null for the group of rows where the dimension column is null.
// Default: các trường ghi chú (presence tới outlier) do annotate.rs điền sau, nên chỗ dựng nhóm viết
// `..Default::default()` cho chúng.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct GroupContribution {
    /// Value of the dimension column for this group; null for rows where it is null.
    #[schema(required)]
    pub value: Option<CellValue>,
    pub base_value: String,
    pub compare_value: String,
    /// Part of `change` from this group: `compare_value - base_value` for a total, `mix_effect + rate_effect` for a
    /// ratio.
    pub contribution: String,
    /// Ratio measures only: part of `change` because this group's share of the denominator changed,
    /// `(compare_share - base_share) * (base_value + compare_value) / 2`. For a group with no denominator in one slice,
    /// the ratio of that slice is taken equal to the other slice, so the whole contribution is a mix effect.
    #[schema(required)]
    pub mix_effect: Option<String>,
    /// Ratio measures only: part of `change` because the ratio inside this group changed,
    /// `(base_share + compare_share) / 2 * (compare_value - base_value)`.
    #[schema(required)]
    pub rate_effect: Option<String>,
    pub presence: GroupPresence,
    /// `contribution / change * 100`, one decimal: negative when the group moved against the change, above 100 when
    /// other groups offset it. Null when the change is close to zero (under 0.5% of the larger slice), where these
    /// percents grow without meaning.
    #[schema(required)]
    pub share_of_change: Option<String>,
    /// `(compare_value - base_value) / base_value * 100`, one decimal. Null when the group is `new` or `gone`, when
    /// `base_value` is zero or negative, or when `sign_flip`.
    #[schema(required)]
    pub percent_change: Option<String>,
    /// True when the base and compared values are both non-zero with opposite signs, for example a profit that
    /// turned into a loss.
    pub sign_flip: bool,
    /// True when the group stands out: its `share_of_change` is at least 1 (in absolute value) and its
    /// `percent_change` is at least 10 points away from the `percent_change` of the whole, or it is `new`, `gone` or a
    /// `sign_flip`.
    pub outlier: bool,
}

// Shapley hai nhân tố (#40): phần tương tác chia đôi. Giá là phép chia duy nhất và là chỗ duy nhất làm tròn: giá giữ
// một số chữ số cố định (do #40 chọn), mọi số sau đó tính đúng từ giá đã làm tròn, và price_effect lấy bằng phép trừ
// nên phần lệch do làm tròn dồn vào đó; volume_effect + price_effect luôn bằng đúng change.
/// Change split into a volume effect and a price effect, for a measure that is volume times price (for example
/// orders times average order value); null in `Explanation` for other measures.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VolumePrice {
    /// Text for the end user (Vietnamese) naming the volume, for example the number of orders.
    pub volume_label: String,
    /// Text for the end user (Vietnamese) naming the price, for example the average order value.
    pub price_label: String,
    /// SQL that computed the volume and the measure of both slices, after the guard.
    pub sql: String,
    pub base_volume: String,
    pub compare_volume: String,
    /// Measure divided by volume, rounded to a fixed number of decimals; the only rounded numbers here.
    pub base_price: String,
    pub compare_price: String,
    /// `(compare_volume - base_volume) * (base_price + compare_price) / 2`, exact from the rounded prices.
    pub volume_effect: String,
    /// `change - volume_effect`, so that the two effects add up to `change` exactly.
    pub price_effect: String,
    /// Value of the measure if the volume had stayed at `base_volume`: `base_volume * compare_price`.
    pub if_volume_unchanged: String,
}

// Tên bước theo luồng ở ARCHITECTURE mục 2, không khớp một-một với các bước ở đó (classify, repair và narrate là ba lần
// gọi mô hình riêng); giao diện tự đặt nhãn tiếng Việt cho từng tên. explain (code tính) và
// narrate (mô hình viết lời) tách riêng, để timeline cho thấy mô hình chỉ diễn giải số.
/// Step of the backend:
/// - `context`: read the dataset profile and build the prompt.
/// - `classify`: the language model tells a plain question from a question that asks for a cause; when the model
///   fails, the keyword rule decides and the step is not ok.
/// - `llm`: the language model writes SQL.
/// - `guard`: check the SQL, add row-level filters and a row limit.
/// - `run`: run the SQL on the analytics engine.
/// - `repair`: the language model fixes SQL that failed to run, or that the guard refused because a table or column
///   does not exist or because it could build a value too large (at most once).
/// - `explain`: compute the breakdown of a "why" question (code, no model).
/// - `narrate`: the language model writes the summary of the breakdown, or the `insight.summary` of a plain question
///   (also used as its `answer_text`).
/// - `record`: save the question in the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepName {
    Context,
    Classify,
    Llm,
    Guard,
    Run,
    Repair,
    Explain,
    Narrate,
    Record,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AskStep {
    pub name: StepName,
    pub duration_ms: u64,
    /// False for a step that failed. A failed step does not always stop the question: a failed `run` or `guard` can be
    /// followed by `repair`, a failed `narrate` by a template summary. `AskStatus` tells whether the question stopped.
    pub ok: bool,
    /// Text for the end user (Vietnamese), for example the guard's reason or the engine error.
    #[schema(required)]
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::build_unique_names;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn unique_names_keep_distinct_names() {
        assert_eq!(
            build_unique_names(&names(&["market", "revenue"])),
            names(&["market", "revenue"])
        );
    }

    #[test]
    fn unique_names_never_reuse_a_name_of_another_column() {
        assert_eq!(
            build_unique_names(&names(&["a", "a", "a"])),
            names(&["a", "a_2", "a_3"])
        );
        assert_eq!(
            build_unique_names(&names(&["a", "a", "a_2"])),
            names(&["a", "a_3", "a_2"])
        );
        assert_eq!(
            build_unique_names(&names(&["a_2", "a", "a"])),
            names(&["a_2", "a", "a_3"])
        );
        assert_eq!(
            build_unique_names(&names(&["a_2", "a_2", "a"])),
            names(&["a_2", "a_2_2", "a"])
        );
    }
}
