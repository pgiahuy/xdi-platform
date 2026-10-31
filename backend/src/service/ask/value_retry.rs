// SPDX-License-Identifier: Apache-2.0
/* service/ask/value_retry.rs - gửi lại lượt viết SQL một lần khi câu lọc sai giá trị (#230): câu SQL (hay terms của
mô hình) lọc theo một giá trị mà cột không có (luật filter_value_not_found của ask/checks.rs), trong khi danh sách giá
trị của cột đó chưa được gửi đủ (ask/context.rs chỉ gửi đủ khi tổng nhỏ, hay khi cột liên quan, và tới 200 giá trị).
Mô hình có thể đã đoán cách viết giá trị; gửi đủ danh sách của cột thì nó chọn được giá trị đúng.

ask/mod.rs gọi build_resend_columns sau lần chạy đầu, apply_full_values cho từng cột, rồi dựng lại prompt và gọi mô
hình thêm đúng một lần; bước này ghi vào tab Các bước. "Đã gửi đủ" nghĩa là danh sách đủ thật sự nằm trong prompt đã
gửi (prompt.rs values_in_prompt ở mức chi tiết build_within_limit đã chọn), không chỉ là đủ trong ngữ cảnh. Cột đã gửi
đủ thì không gửi lại: cảnh báo giữ như cũ. Lần gửi lại là bước thêm: nó lỗi (mô hình hay chạy SQL) hoặc không answered
thì ask/mod.rs giữ kết quả lần đầu. Không gửi lại cho người hỏi bị lọc dòng theo vai dữ liệu: người đó không nhận danh
sách giá trị nào (ask/group_prompt.rs). Chỉ xét cột của bảng đang chọn, như luật filter_value_not_found. */

use crate::engine::store::ProfileRole;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::filter_values::MissingValue;
use crate::service::ask::prompt::{values_in_prompt, Detail};
use crate::service::ask::values::ValueIndex;

/// Tên các cột (mỗi cột một lần) có giá trị lọc không có trong cột mà prompt đã gửi (ở mức chi tiết detail) chưa có
/// đủ danh sách của cột: cột chiều, không nhạy cảm, và danh sách không đủ hay bị build_within_limit bỏ.
pub fn build_resend_columns(
    missing: &[MissingValue],
    columns: &[ColumnProfile],
    detail: Detail,
) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for m in missing {
        let resendable = columns.iter().any(|c| {
            c.name == m.column
                && c.role == ProfileRole::Dimension.as_str()
                && !c.is_sensitive
                && !(c.values_complete && values_in_prompt(c, detail))
        });
        if resendable && !names.contains(&m.column) {
            names.push(m.column.clone());
        }
    }
    names
}

/// Gửi lại đủ danh sách giá trị của một cột, bỏ trần (ask/mod.rs, khi câu SQL lọc theo một giá trị cột không có mà danh
/// sách của cột chưa có đủ trong prompt). Cột được đánh dấu picked (cột liên quan): prompt gửi lại có hạ mức chi tiết
/// thì danh sách của nó vẫn còn. Trả false khi không có gì để gửi thêm: cột nhạy cảm, không phải cột chiều, không có giá
/// trị nào, hay danh sách đủ đã ở trên một cột liên quan.
pub fn apply_full_values(columns: &mut [ColumnProfile], index: &ValueIndex, name: &str) -> bool {
    let Some(column) = columns.iter_mut().find(|c| c.name == name) else {
        return false;
    };
    if column.role != ProfileRole::Dimension.as_str() || column.is_sensitive {
        return false;
    }
    let Some(values) = index.columns.iter().find(|v| v.column_id == column.id) else {
        return false;
    };
    let already = column.values.len() == values.values.len() && column.picked && !column.brief;
    if values.values.is_empty() || already {
        return false;
    }
    column.values = values.values.clone();
    column.values_complete = !values.dropped;
    column.brief = false;
    column.picked = true;
    true
}
