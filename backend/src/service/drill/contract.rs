// SPDX-License-Identifier: Apache-2.0
/* service/drill/contract.rs - hợp đồng của POST /workspaces/{ws}/datasets/{id}/drill (truy dòng nguồn, #34): request
gửi query_id cùng khóa của chỗ được bấm, không bao giờ gửi SQL (docs/ARCHITECTURE.md mục 8). service/drill.rs dựng
DrillResponse, api/query.rs trả nó ra JSON; đặt ở service vì cùng lý do với ask/contract.rs, và tách file vì ask_contract
đã dài. Bảng dòng nguồn dùng lại ResultTable của ask, để giao diện vẽ bằng cùng component.

Như ask/contract.rs: chú thích /// thành mô tả trong đặc tả nên viết tiếng Anh, ghi chú cho đội dùng //. */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::ask::contract::{CellValue, ResultTable};

/// Cỡ trang khi request không ghi limit, và cỡ trang lớn nhất. 1000 bằng số dòng tối đa ResultTable gửi (SENT_ROWS).
pub const DEFAULT_DRILL_LIMIT: u32 = 100;
pub const MAX_DRILL_LIMIT: u32 = 1000;

// Hai loại tên khác nhau nên ở hai trường riêng: ô trỏ cột bằng tên trong result.columns, dòng phân rã trỏ cột bằng
// tên trong dataset. Gửi cả hai hay không gửi cái nào là 400.
/// Which number to trace back to its source rows: set exactly one of `cell` and `breakdown`. Rows come one page at a
/// time, in the order the analytics engine reads them.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DrillRequest {
    /// `AskResponse.query_id` of the answer the number belongs to.
    pub query_id: Uuid,
    /// A cell of `AskResponse.result`: the values of every `dimension` and `time` column of its row, one key per
    /// column. An empty list is the one row of a query without grouping keys.
    #[serde(default)]
    pub cell: Option<Vec<DrillKey>>,
    /// A row of the breakdown in `AskResponse.explanation`: the source rows of that group in that slice.
    #[serde(default)]
    pub breakdown: Option<BreakdownKey>,
    /// Number of source rows to skip. Default 0.
    #[serde(default)]
    pub offset: u64,
    /// Rows per page, 1 to 1000. Default 100.
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    DEFAULT_DRILL_LIMIT
}

// Giá trị gửi lại đúng như ô đã nhận (kể cả số dạng chuỗi của kind decimal), backend so bằng giá trị đó trên cùng biểu
// thức nhóm của câu đã chạy.
/// One grouping key of the clicked cell.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DrillKey {
    /// A name from `result.columns`, of a column with role `dimension` or `time`.
    pub column: String,
    /// The cell of that column in the clicked row, exactly as received; null for a SQL NULL.
    #[schema(required)]
    pub value: Option<CellValue>,
}

/// `base` or `compare`: one of the two slices of `AskResponse.explanation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SliceName {
    Base,
    Compare,
}

// Backend lấy điều kiện lát cắt và câu SQL của chiều từ queries.explanation (service/drill.rs), không từ request.
/// One group of a breakdown table in one slice.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BreakdownKey {
    pub slice: SliceName,
    /// `DimensionBreakdown.column`: a column name of the dataset, not of `result.columns`.
    pub column: String,
    /// `GroupContribution.value` of the group; null for the group of empty cells.
    #[schema(required)]
    pub value: Option<CellValue>,
}

/// One page of the source rows behind a number.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DrillResponse {
    /// The SQL that read this page, after the guard: the filters of the answered question, the keys of the clicked
    /// number, and the row filters of the member asking now (which may differ from the member who asked the question).
    pub sql: String,
    /// The page: every column of the dataset, including `_source_file`, the file each row came from. Here
    /// `result.total_rows` counts the rows of this page only.
    pub result: ResultTable,
    /// Number of source rows behind the number, over all pages.
    pub total_rows: u64,
}
