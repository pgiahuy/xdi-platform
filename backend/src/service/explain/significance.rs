// SPDX-License-Identifier: Apache-2.0
/* service/explain/significance.rs - ngưỡng "không nói quá" chung cho phần giải thích của mọi câu trả lời (#227): chênh
lệch quá nhỏ so với giá trị gốc, hay một bên có quá ít dòng, thì lời nói "không có thay đổi đáng kể" thay vì kể chiều và
phân rã. Đặt ở explain/ để cảnh báo mẫu nhỏ của câu "tại sao" (#199) dùng cùng ngưỡng, không đặt một ngưỡng thứ hai.
Hàm thuần, test ở service/insight/tests.rs. */

use rust_decimal::Decimal;

/// Một bên có ít dòng hơn ngần này thì chênh lệch có thể chỉ do vài dòng (#199 nêu ví dụ 9 dòng so với 4 dòng). Chọn
/// 30 như ví dụ của #199; chưa đo trên dữ liệu nào.
pub const SMALL_SAMPLE_ROWS: i64 = 30;

/// Chênh lệch dưới ngần này phần trăm của bên lớn hơn thì coi là không đáng kể. So với bên lớn hơn chứ không với giá trị
/// gốc, để giá trị gốc bằng 0 hay âm vẫn so được.
pub const MIN_CHANGE_PERCENT: i64 = 2;

/// Mức đáng kể của một chênh lệch giữa hai giá trị.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeWeight {
    Notable,
    /// Chênh lệch dưới MIN_CHANGE_PERCENT.
    Tiny,
    /// Một bên có ít hơn SMALL_SAMPLE_ROWS dòng.
    SmallSample,
}

/// rows là số dòng của hai bên khi biết (hai kỳ đã đếm); None khi không đếm được, chỉ xét độ lớn.
pub fn build_change_weight(
    base: Decimal,
    compare: Decimal,
    rows: Option<(i64, i64)>,
) -> ChangeWeight {
    if let Some((base_rows, compare_rows)) = rows {
        if base_rows.min(compare_rows) < SMALL_SAMPLE_ROWS {
            return ChangeWeight::SmallSample;
        }
    }
    let larger = base.abs().max(compare.abs());
    let change = (compare - base).abs();
    // change / larger < MIN / 100, nhân chéo để không chia: larger bằng 0 thì cả hai bằng 0, không có chênh lệch.
    if larger.is_zero() || change * Decimal::from(100) < Decimal::from(MIN_CHANGE_PERCENT) * larger
    {
        ChangeWeight::Tiny
    } else {
        ChangeWeight::Notable
    }
}
