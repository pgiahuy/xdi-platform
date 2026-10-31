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

Quy mô là ý chính (scale_main, #260) khi lượng của bên nhiều gấp từ MIN_SCALE_RATIO lần bên ít và phần do lượng lớn hơn
phần do giá: như COMPLETE 597 đơn với CANCELED 19 đơn, tổng chênh chủ yếu vì số đơn, không vì mỗi đơn lời hơn. Khi đó
lời của mô hình phải nêu ý này: cổng 1 (explain/claims.rs) đòi khẳng định về lượng, mô hình chấm (explain/judge.rs) nhận
cờ này và coi lời bỏ ý này là thiếu ý chính.

Hàm thuần, tính lại từ Explanation mỗi lần gọi và không lưu, nên API không thêm trường. Test ở explain/split_tests.rs. */

use rust_decimal::Decimal;

use crate::service::ask::contract::Explanation;
use crate::service::explain::annotate::build_percent_text;
use crate::service::explain::VolumePriceNumbers;

/// Bên nhiều lượng gấp từ ngần này lần bên ít thì quy mô là ý chính (khi phần do lượng cũng lớn hơn phần do giá). Gấp
/// đôi là mức người đọc không thể bỏ qua: dù mỗi đơn vị như nhau, tổng của bên nhiều đã gấp đôi bên ít, nên so hai tổng
/// mà không nói số lượng là nói thiếu. Dưới mức đó (5.000 với 4.400 đơn) lượng vẫn có thể là phần lớn hơn, câu khuôn
/// vẫn nói, nhưng không bắt lời mô hình phải nói.
pub const MIN_SCALE_RATIO: i64 = 2;

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
    /// Quy mô là ý chính: lượng hai bên chênh từ MIN_SCALE_RATIO lần và phần do lượng lớn hơn phần do giá.
    pub scale_main: bool,
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
    let scale_main = is_scale_main(&numbers).unwrap_or(false);
    Some(Split {
        volume_label,
        price_label,
        numbers,
        volume_share,
        price_share,
        scale_main,
    })
}

// None khi một số không đọc được hay một lượng không dương: khi đó không bắt lời phải nói gì thêm.
fn is_scale_main(numbers: &VolumePriceNumbers) -> Option<bool> {
    let (base, compare) = (
        parse(&numbers.base_volume)?,
        parse(&numbers.compare_volume)?,
    );
    let (small, large) = (base.min(compare), base.max(compare));
    if small <= Decimal::ZERO {
        return None;
    }
    let skewed = large >= small.checked_mul(Decimal::from(MIN_SCALE_RATIO))?;
    let volume_bigger = parse(&numbers.volume_effect)?.abs() >= parse(&numbers.price_effect)?.abs();
    Some(skewed && volume_bigger)
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
