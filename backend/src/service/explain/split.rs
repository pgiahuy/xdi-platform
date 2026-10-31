// SPDX-License-Identifier: Apache-2.0
/* service/explain/split.rs - tách chênh lệch của độ đo tổng thành phần do lượng (như số đơn) và phần do giá trị trung
bình mỗi đơn vị (#40, #201), dùng chung cho prompt insight của mô hình (explain/narrate_prompt.rs), câu khuôn
(explain/summary.rs), cổng 1 (explain/claim_cell.rs) và JSON gửi mô hình chấm (explain/judge.rs), kèm phần trăm của mỗi
phần trong chênh lệch.

Chỉ tách khi plan có lượng mang nghĩa nghiệp vụ (Explanation.volume_price, như số đơn) và độ đo là một tổng. Độ đo
đếm (số đơn, số dòng) thì không tách: chính nó đã là quy mô, "chủ yếu do số đơn" là nói lại câu hỏi; lời đọc theo các
nhóm (#186, bảng các trường hợp). Không tách theo số dòng: một dòng
dữ liệu không có nghĩa kinh doanh chung (DataCo một dòng là một món trong đơn), nên "do số dòng dữ liệu" không giải
thích được gì cho người đọc; trước đây câu khuôn còn gọi số dòng là lý do chính chỉ vì trung bình mỗi dòng lệch dưới
10%, sai cả khi tính đúng. Không có lượng thì insight giải thích theo các nhóm.

Hàm thuần, tính lại từ Explanation mỗi lần gọi và không lưu, nên API không thêm trường. Test ở explain/split_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::Explanation;
use crate::service::explain::annotate::build_percent_text;
use crate::service::explain::VolumePriceNumbers;

/// Phần do lượng và phần do giá trị trung bình mỗi đơn vị của một chênh lệch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub volume_label: String,
    pub price_label: String,
    pub numbers: VolumePriceNumbers,
    /// Phần trăm của chênh lệch do lượng, do giá, một chữ số lẻ ("37.5"). None khi hai phần ngược chiều nhau: một phần
    /// khi đó vượt 100% hay âm, người đọc không hiểu được.
    pub volume_share: Option<String>,
    pub price_share: Option<String>,
}

/// Tách của một Explanation; None khi plan không có lượng (volume_price null: độ đo tỷ lệ, không có cột lượng, hay một
/// lượng bằng 0) hay khi độ đo là phép đếm (counts_rows).
pub fn build_split(e: &Explanation) -> Option<Split> {
    if e.counts_rows {
        return None;
    }
    let vp = e.volume_price.as_ref()?;
    let (volume_label, price_label) = (vp.volume_label.clone(), vp.price_label.clone());
    let numbers = VolumePriceNumbers {
        base_volume: vp.base_volume.clone(),
        compare_volume: vp.compare_volume.clone(),
        base_price: vp.base_price.clone(),
        compare_price: vp.compare_price.clone(),
        volume_effect: vp.volume_effect.clone(),
        price_effect: vp.price_effect.clone(),
        if_volume_unchanged: vp.if_volume_unchanged.clone(),
    };
    let (volume_share, price_share) = build_shares(e, &numbers).unzip();
    Some(Split {
        volume_label,
        price_label,
        numbers,
        volume_share,
        price_share,
    })
}

// Phần trăm của từng phần trong chênh lệch, chỉ khi cả hai phần cùng chiều chênh lệch (hay bằng 0), để mỗi phần nằm
// trong 0 tới 100%.
fn build_shares(e: &Explanation, numbers: &VolumePriceNumbers) -> Option<(String, String)> {
    let change = parse(&e.change)?;
    if change.is_zero() {
        return None;
    }
    let volume = parse(&numbers.volume_effect)?;
    let price = parse(&numbers.price_effect)?;
    let along =
        |part: Decimal| part.is_zero() || (part > Decimal::ZERO) == (change > Decimal::ZERO);
    if !along(volume) || !along(price) {
        return None;
    }
    let share = |part: Decimal| {
        part.checked_mul(Decimal::ONE_HUNDRED)?
            .checked_div(change)
            .map(build_percent_text)
    };
    Some((share(volume)?, share(price)?))
}

fn parse(text: &str) -> Option<Decimal> {
    text.parse().ok()
}
