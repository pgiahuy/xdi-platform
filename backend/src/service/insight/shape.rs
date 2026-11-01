// SPDX-License-Identifier: Apache-2.0
/* service/insight/shape.rs - hình dạng của bảng kết quả một câu thường (#227), từ vai trò cột đã có (ask/view.rs, dựng
từ cây cú pháp của câu đã chạy): một con số, chuỗi thời gian, theo nhóm, top N hay danh sách dòng. service/insight/mod.rs
chọn loại giải thích theo hình dạng này. Hàm thuần, test ở insight/tests.rs.

Bảng có một khóa nhóm (hay không khóa nào), và bảng theo đúng hai cột nhóm (insight/matrix.rs: tổ hợp đứng đầu, nhóm
đứng đầu của từng cột). Bảng thời gian kèm nhóm hay nhiều khóa hơn không có một câu giải thích ngắn nào đúng cho mọi ô,
nên chỉ ghi số dòng như danh sách dòng. */

use rust_decimal::Decimal;

use crate::engine::sql_shape::QueryShape;
use crate::service::ask::contract::{CellValue, ColumnRole, InsightKind, ResultTable};

/// Hình dạng đã nhận ra, kèm vị trí cột trong bảng kết quả.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultShape {
    pub kind: InsightKind,
    /// Cột độ đo được giải thích: cột tổng hợp đầu tiên. None với danh sách dòng.
    pub measure: Option<usize>,
    /// Cột khóa: cột thời gian của chuỗi, cột nhóm của bảng theo nhóm và top N.
    pub key: Option<usize>,
    /// Cột nhóm thứ hai của bảng theo hai cột nhóm (kind là groups); None với mọi bảng khác.
    pub second_key: Option<usize>,
}

/// None khi bảng không có dòng nào hay mọi ô đều trống: cảnh báo kết quả rỗng (ask/checks.rs) đã nói điều đó.
/// shape là hình dạng của câu đã chạy (engine/sql_shape.rs), để biết câu có LIMIT của mô hình không (top N).
pub fn build_result_shape(
    result: &ResultTable,
    shape: &QueryShape,
    limit_kept: bool,
) -> Option<ResultShape> {
    if result.rows.is_empty() || result.rows.iter().flatten().all(Option::is_none) {
        return None;
    }
    let with_role = |role: ColumnRole| -> Vec<usize> {
        result
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.role == role)
            .map(|(i, _)| i)
            .collect()
    };
    let measures = with_role(ColumnRole::Measure);
    let times = with_role(ColumnRole::Time);
    let dimensions = with_role(ColumnRole::Dimension);
    let rows = ResultShape {
        kind: InsightKind::Rows,
        measure: None,
        key: None,
        second_key: None,
    };
    let Some(&measure) = measures.first() else {
        return Some(rows);
    };
    let kind = match (times.as_slice(), dimensions.as_slice()) {
        ([], []) if result.rows.len() == 1 => InsightKind::SingleValue,
        ([_], []) if result.rows.len() >= 2 => InsightKind::TimeSeries,
        ([], [_]) if result.rows.len() >= 2 => {
            // LIMIT của mô hình kèm xếp giảm dần theo độ đo là câu hỏi top N; LIMIT do guard thêm (trần dòng) thì không.
            let top = limit_kept
                && shape.limit.is_some()
                && shape
                    .order_by
                    .first()
                    .is_some_and(|o| o.descending && o.output == Some(measure));
            if top {
                InsightKind::TopN
            } else {
                InsightKind::Groups
            }
        }
        // Hai cột nhóm, như "doanh thu theo danh mục và phân khúc khách hàng".
        ([], [_, _]) if result.rows.len() >= 2 => InsightKind::Groups,
        _ => return Some(rows),
    };
    let key = times.first().or(dimensions.first()).copied();
    Some(ResultShape {
        kind,
        measure: Some(measure),
        key,
        second_key: dimensions.get(1).copied().filter(|_| times.is_empty()),
    })
}

/// Giá trị số của một ô độ đo; None khi ô trống hay không phải số.
pub fn build_cell_decimal(cell: Option<&CellValue>) -> Option<Decimal> {
    match cell? {
        CellValue::Number(number) => number.to_string().parse().ok(),
        CellValue::String(text) => text.parse().ok(),
        CellValue::Boolean(_) => None,
    }
}
