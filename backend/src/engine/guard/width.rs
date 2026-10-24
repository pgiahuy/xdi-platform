// SPDX-License-Identifier: Apache-2.0
/* engine/guard/width.rs - giới hạn hai thứ mà luật số tầng (depth.rs) và số mảnh ghép (pieces.rs) không đếm: độ dài
mỗi chuỗi hằng viết trong câu, và số mục trong danh sách cột của mỗi SELECT. build_guarded_sql (mod.rs) gọi
check_query_width cùng các luật kia.

Vì sao: chép một giá trị lớn ra nhiều cột không cần ghép (`SELECT v, v, v, ...`), mà mỗi bản chép là một chuỗi riêng
DuckDB dựng lúc chuẩn bị câu. Đo qua guard trước khi có luật này: chuỗi hằng 16 KB chép ra 8000 cột cho RSS đỉnh 690 MB
dù không ghép gì; thêm ghép qua các tầng thì nhân lên (600 cột, 49 lần, 2,5 GB). Chuỗi hằng tối đa 1000 byte và 200 cột
mỗi SELECT giữ các ca không phụ thuộc dữ liệu đã đo dưới 140 MB (bảng số đo ở docs/GUARD.md, mục Số tầng, mảnh ghép,
độ rộng). Chuỗi dài lấy từ dữ liệu thì không bị luật này giới hạn: đó là phần tính lúc chạy, trong trần của
engine/query.rs, còn phần nhân theo số dòng thì để tiến trình con (#119). */

use std::ops::ControlFlow;

use sqlparser::ast::{Query, Select, ValueWithSpan, Visit, Visitor};

/// Độ dài tối đa của một chuỗi hằng, tính bằng byte: đủ cho tên, mã, định dạng ngày, mẫu tìm kiếm.
pub const MAX_LITERAL_BYTES: usize = 1000;

/// Số mục tối đa trong danh sách cột của một SELECT; `*` và `t.*` tính là một mục.
pub const MAX_SELECT_ITEMS: usize = 200;

/// Err là lý do từ chối (tiếng Việt).
pub(super) fn check_query_width(query: &Query) -> Result<(), String> {
    match query.visit(&mut WidthChecker) {
        ControlFlow::Break(reason) => Err(reason),
        ControlFlow::Continue(()) => Ok(()),
    }
}

struct WidthChecker;

impl Visitor for WidthChecker {
    type Break = String;

    // Mọi chuỗi hằng ở mọi chỗ, kể cả `DATE '...'` và chuỗi trong IN (...).
    fn pre_visit_value(&mut self, value: &ValueWithSpan) -> ControlFlow<String> {
        let bytes = value.clone().into_string().map_or(0, |text| text.len());
        if bytes > MAX_LITERAL_BYTES {
            return ControlFlow::Break(format!(
                "Câu SQL có chuỗi hằng dài {bytes} byte, tối đa {MAX_LITERAL_BYTES}: viết lại không cần chuỗi dài như vậy"
            ));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<String> {
        let items = select.projection.len();
        if items > MAX_SELECT_ITEMS {
            return ControlFlow::Break(format!(
                "Câu SELECT có {items} cột, tối đa {MAX_SELECT_ITEMS}: chọn ít cột hơn, hoặc dùng *"
            ));
        }
        ControlFlow::Continue(())
    }
}
