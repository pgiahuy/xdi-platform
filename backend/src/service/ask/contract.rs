// SPDX-License-Identifier: Apache-2.0
/* service/ask/contract.rs - hợp đồng của POST /workspaces/{ws}/datasets/{id}/ask: kiểu response và lựa chọn người
hỏi gửi kèm request. service/ask.rs dựng AskResponse, api/query.rs trả nó ra JSON; đặt ở service chứ không ở api để
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
/// - `rejected`: the guard refused the SQL: the generated one for a reason a fix cannot change (for example it is not
///   a read-only SELECT, or it calls a function that is not allowed), or the one fixed in the `repair` round. Set:
///   `message` (the reason), `query_id`, `sql_generated`; the `guard` step that stopped it has `ok` false.
/// - `failed`: the SQL could not run and the one fix round did not help, either because the fixed SQL failed too or
///   because the model failed during the fix; or the SQL ran past the time or memory limit, which gets no fix round.
///   Set: `message`, `query_id`, `sql_generated`; `sql_guarded` unless that SQL never passed the guard. The `run` or
///   `repair` step that stopped it has `ok` false.
///
/// A fix round starts when the analytics engine fails on the SQL, or when the guard refuses it because a table or
/// column does not exist or because it could build a value too large (too deep, too many concatenated pieces, too
/// wide, a size argument over the limit); there is at most one round per question. The status names the step that stopped the
/// question: the guard gives `rejected`, the engine or the fix round gives `failed`. A model error before any SQL
/// exists is not a status: the request gets 502 or 503.
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
    #[schema(required)]
    pub understanding: Option<Understanding>,
    #[schema(required)]
    pub result: Option<ResultTable>,
    #[schema(required)]
    pub chart: Option<ChartSuggestion>,
    /// Notes shown next to the result; they never hide it.
    pub warnings: Vec<Warning>,
    #[schema(required)]
    pub explanation: Option<Explanation>,
    /// Questions back to the user, only for `needs_choice`. Every entry has `selected` null.
    pub clarifications: Vec<Choice>,
    /// What the backend did, in order, with timings. A step may appear twice, for example `run` before and after a
    /// `repair`.
    pub steps: Vec<AskStep>,
}

// Mỗi mục kèm đoạn SQL nó lấy ra, để người đọc đối chiếu được và để #35 kiểm thẻ luôn khớp câu đã chạy.
/// One thing the question was understood to mean.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UnderstoodItem {
    /// Text for the end user (Vietnamese) naming the measure, filter, grouping, order or period.
    pub label: String,
    /// Fragment of `sql_guarded` this item comes from.
    pub sql: String,
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

// Bốn loại đầu do kiểm hợp lý dựng (service/ask/checks.rs), hai loại sau do chính luồng hỏi. Thêm loại mới là đổi
// API: chạy make openapi.
/// Kind of note:
/// - `empty_result`: no rows.
/// - `all_null`: a result column holds only nulls.
/// - `filter_value_not_found`: the SQL filters on a value that does not occur in the column.
/// - `unusual_magnitude`: a value is far outside what the data makes likely, for example a ratio above 100%.
/// - `explanation_unavailable`: a "why" question was answered without a breakdown; the message says why.
/// - `choice_not_applied`: a choice sent with the question was not recognized and the default was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
    EmptyResult,
    AllNull,
    FilterValueNotFound,
    UnusualMagnitude,
    ExplanationUnavailable,
    ChoiceNotApplied,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Warning {
    pub code: WarningCode,
    /// Text for the end user (Vietnamese).
    pub message: String,
}

// Số trong phân rã là chuỗi chữ số thập phân như ô Decimal. Độ đo đổi sang DECIMAL ở từng dòng nguồn trước khi cộng
// (số chữ số do #39 chọn), nên mọi tổng và hiệu sau đó là phép tính đúng: không làm tròn từng nhóm, tổng đóng góp bằng
// đúng chênh lệch, và #41 so số trong lời với đúng các chuỗi này. Bảng kết quả thì cộng theo kiểu cột (DOUBLE cộng
// bằng số thực), nên giá trị hai lát cắt có thể lệch ô tương ứng của bảng ở chữ số cuối; con số của phân rã là con số
// ở đây.
/// Breakdown of a change between two slices (two periods or two groups); null in `AskResponse` unless an answered
/// "why" question has one. Numbers are exact strings of decimal digits.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Explanation {
    /// Measure that changed, text for the end user (Vietnamese).
    pub measure: String,
    /// Slice compared against.
    pub base: ExplanationSlice,
    /// Slice being explained.
    pub compare: ExplanationSlice,
    /// `compare.value - base.value`.
    pub change: String,
    /// One breakdown per dimension, the dimension that explains the change best first.
    pub dimensions: Vec<DimensionBreakdown>,
    #[schema(required)]
    pub volume_price: Option<VolumePrice>,
    /// Plain-language summary for the end user (Vietnamese). Every number in it appears in this breakdown.
    pub summary: String,
    /// False when the model's text failed the number check, or the model was not reached, and `summary` is a template
    /// sentence written by the backend.
    pub summary_by_model: bool,
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
}

/// Contribution of every group of one dimension; the contributions add up to `change`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DimensionBreakdown {
    /// Dimension column, as named in the dataset (not a name from `result.columns`).
    pub column: String,
    /// How to read the group values, like the `kind` of a result column.
    pub kind: ValueKind,
    /// SQL that computed the groups of both slices, after the guard.
    pub sql: String,
    /// Every group of the column, largest contribution (by absolute value) first.
    pub groups: Vec<GroupContribution>,
}

// value cùng column của DimensionBreakdown là khóa nhóm, để bấm một dòng thì truy được dòng nguồn (#34, #52).
/// One group of a dimension. `value` is null for the group of rows where the dimension column is null.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GroupContribution {
    /// Value of the dimension column for this group; null for rows where it is null.
    #[schema(required)]
    pub value: Option<CellValue>,
    pub base_value: String,
    pub compare_value: String,
    /// `compare_value - base_value` of this group.
    pub contribution: String,
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

// Tên bước theo luồng ở ARCHITECTURE mục 2, không khớp một-một với chín bước ở đó (repair và narrate là hai lần gọi mô
// hình riêng); giao diện tự đặt nhãn tiếng Việt cho từng tên. explain (code tính) và
// narrate (mô hình viết lời) tách riêng, để timeline cho thấy mô hình chỉ diễn giải số.
/// Step of the backend:
/// - `context`: read the dataset profile and build the prompt.
/// - `llm`: the language model writes SQL.
/// - `guard`: check the SQL, add row-level filters and a row limit.
/// - `run`: run the SQL on the analytics engine.
/// - `repair`: the language model fixes SQL that failed to run, or that the guard refused because a table or column
///   does not exist or because it could build a value too large (at most once).
/// - `explain`: compute the breakdown of a "why" question (code, no model).
/// - `narrate`: the language model writes the summary of the breakdown.
/// - `record`: save the question in the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepName {
    Context,
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
