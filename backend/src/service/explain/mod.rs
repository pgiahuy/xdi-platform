// SPDX-License-Identifier: Apache-2.0
/* service/explain/mod.rs - giải thích câu hỏi "tại sao": chọn hai lát cắt, tính chênh lệch độ đo, phân rã theo
từng chiều, xếp hạng đóng góp. Tổng các đóng góp phải bằng đúng chênh lệch. LLM chỉ diễn giải bảng số này.

Hiện có phần toán, hàm thuần không đọc SQL, DuckDB hay mô hình: đóng góp của từng nhóm và xếp hạng chiều (#39), tách
lượng và giá (#40); độ đo tỷ lệ tách cơ cấu và hiệu suất ở explain/ratio.rs. Đầu vào là số đã đọc từ DuckDB (chuỗi VARCHAR đổi sang Decimal), đầu ra là struct của hợp đồng
(ask/contract.rs) hoặc struct trung gian. SQL và các bước chạy ở explain/sql.rs, explain/breakdown.rs, explain/run.rs
(#38); test ở explain/tests.rs.

Cuối file là phần chữ của bước narrate (#41), cũng là hàm thuần: prompt gửi mô hình, tập số được phép, bước kiểm mọi
số trong lời diễn giải và câu khuôn dự phòng. Việc gọi mô hình, giới hạn thời gian và dựng AskStep ở ask/narrate.rs.

Mọi con số hiện ra tính bằng Decimal (rust_decimal, 28 chữ số), không qua f64: f64 làm sai chữ số cuối và tổng đóng
góp sẽ lệch chênh lệch. Phép tính tràn khoảng của Decimal trả None hoặc lỗi, không panic. EP (phần đóng góp) cũng
tính và so với ngưỡng bằng Decimal, để nhóm nằm đúng biên 0.1 hay 0.67 không bị loại; chỉ độ bất ngờ S là f64, vì cần
logarit và chỉ dùng để so thứ tự, không so với ngưỡng nào. */

pub mod annotate;
#[cfg(test)]
mod annotate_tests;
pub mod breakdown;
pub mod check;
#[cfg(test)]
pub(crate) mod check_tests;
pub mod choices;
mod claim_cell;
pub mod claims;
#[cfg(test)]
pub(crate) mod claims_tests;
pub mod dimension_pick;
#[cfg(test)]
mod dimension_pick_tests;
pub mod filter;
pub mod form;
#[cfg(test)]
mod frames_run_tests;
#[cfg(test)]
mod frames_tests;
pub mod group_base;
pub mod judge;
#[cfg(test)]
mod judge_tests;
pub mod latest;
pub mod measure;
mod narrate_prompt;
pub mod number;
pub mod period;
pub mod plain;
pub mod premise;
pub mod ratio;
pub mod ratio_breakdown;
#[cfg(test)]
mod ratio_tests;
pub mod run;
#[cfg(test)]
pub(crate) mod run_tests;
pub mod significance;
pub mod source;
#[cfg(test)]
mod source_tests;
pub mod split;
#[cfg(test)]
mod split_tests;
pub mod sql;
pub mod summary;
#[cfg(test)]
mod summary_tests;
#[cfg(test)]
mod tests;

pub use narrate_prompt::{build_narrate_prompt, MAX_REASONS};
pub use summary::build_template_summary;

use std::cmp::{Ordering, Reverse};
use std::collections::HashSet;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::service::ask::contract::{
    CellValue, Comparison, DimensionBreakdown, Explanation, GroupContribution, MeasureKind,
};
use crate::service::explain::number::{build_change_parts, MAX_FRACTION_DIGITS};
// Chỉ bước kiểm số cũ dùng, mà bước đó giờ chỉ test gọi (giữ câu khuôn đúng số).
#[cfg(test)]
use crate::service::explain::number::build_number_text_with;

// Hai ngưỡng của Adtributor (Bhagwan và cộng sự, NSDI 2014), giữ đúng như bài: một chiều "giải thích được" chênh lệch
// khi vài nhóm của nó cộng lại đủ 67% chênh lệch, và chỉ tính nhóm góp từ 10% trở lên, để nhóm nhỏ không làm chiều đó
// trông như giải thích được. Là Decimal vì so bằng f64 thì sai đúng tại biên: 0.57 + 0.1 ra 0.6699999999999999.
// Decimal::new không dùng được trong const; from_parts(lo, mid, hi, âm, scale) là cùng số: 67 với scale 2 là 0.67.
const EXPLAINED_SHARE: Decimal = Decimal::from_parts(67, 0, 0, false, 2);
const MIN_GROUP_SHARE: Decimal = Decimal::from_parts(1, 0, 0, false, 1);

/// Số chữ số thập phân của giá trong tách lượng và giá (#40): đủ cho tiền.
pub const PRICE_SCALE: u32 = 2;

/// Một nhóm của một chiều, như SQL phân rã trả về: giá trị cột chiều và độ đo của hai lát cắt. SQL bọc
/// `COALESCE(..., 0)` nên nhóm chỉ có ở một lát cắt vẫn có số 0 ở lát kia.
#[derive(Debug, Clone)]
pub struct GroupValues {
    /// None cho nhóm các dòng có cột chiều là null.
    pub value: Option<CellValue>,
    pub base_value: Decimal,
    pub compare_value: Decimal,
}

/// In một số Decimal thành chuỗi của hợp đồng: bỏ số 0 thừa ở cuối nhưng giữ ít nhất `min_scale` chữ số thập phân.
// Phép chia và phép nhân của rust_decimal cho số chữ số không cố định (4.99 / 2 ra 2.4950, 0 nhân -19.00 ra 0), nên
// chuẩn hóa rồi bù lại thì cùng một số luôn in một kiểu. normalize cũng bỏ dấu trừ của số 0: không in "-0.00".
pub fn build_decimal_text(value: Decimal, min_scale: u32) -> String {
    let mut number = value.normalize();
    if number.scale() < min_scale {
        number.rescale(min_scale);
    }
    number.to_string()
}

/// Đóng góp của từng nhóm (`compare_value - base_value`), nhóm đóng góp lớn nhất (theo trị tuyệt đối) trước. Bằng nhau
/// thì theo chữ của giá trị nhóm, nhóm null cuối, để cùng dữ liệu luôn ra cùng thứ tự. None khi phép trừ tràn.
pub fn build_group_contributions(groups: &[GroupValues]) -> Option<Vec<GroupContribution>> {
    let mut rows = Vec::with_capacity(groups.len());
    for group in groups {
        let contribution = group.compare_value.checked_sub(group.base_value)?;
        rows.push((contribution, group));
    }
    rows.sort_by(|(a, group_a), (b, group_b)| {
        b.abs()
            .cmp(&a.abs())
            .then_with(|| compare_group_values(&group_a.value, &group_b.value))
    });
    let contributions = rows
        .into_iter()
        .map(|(contribution, group)| {
            // Hiệu giữ số chữ số của số hạng nhiều chữ số hơn, như phép trừ trên giấy.
            let scale = group.base_value.scale().max(group.compare_value.scale());
            GroupContribution {
                value: group.value.clone(),
                base_value: build_decimal_text(group.base_value, group.base_value.scale()),
                compare_value: build_decimal_text(group.compare_value, group.compare_value.scale()),
                contribution: build_decimal_text(contribution, scale),
                mix_effect: None,
                rate_effect: None,
            }
        })
        .collect();
    Some(contributions)
}

// Thứ tự phụ khi hai nhóm đóng góp bằng nhau: theo chữ, nhóm null sau cùng.
pub(crate) fn compare_group_values(a: &Option<CellValue>, b: &Option<CellValue>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => build_value_text(a).cmp(&build_value_text(b)),
    }
}

pub(crate) fn build_value_text(value: &CellValue) -> String {
    match value {
        CellValue::Number(number) => number.to_string(),
        CellValue::String(text) => text.clone(),
        CellValue::Boolean(flag) => flag.to_string(),
    }
}

/// Cách xếp nhóm và chiều, chọn một lần cho cả câu hỏi để mọi chiều so cùng một thước.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingMode {
    /// Adtributor: nhóm xét theo độ bất ngờ S_i giảm dần, chiều theo tổng S.
    Surprise,
    /// Độ đo có số âm: tỷ phần p_i, q_i không còn là phân phối nên S vô nghĩa. Nhóm xét theo EP_i giảm dần, chiều
    /// theo số nhóm cần rồi EP lớn nhất.
    Effect,
}

/// Chọn cách xếp: Surprise khi mọi chiều tính được S, ngược lại Effect.
pub fn build_ranking_mode(
    dimensions: &[Vec<GroupValues>],
    base_total: Decimal,
    compare_total: Decimal,
) -> RankingMode {
    let defined = dimensions
        .iter()
        .all(|groups| surprise_is_defined(groups, base_total, compare_total));
    if defined {
        RankingMode::Surprise
    } else {
        RankingMode::Effect
    }
}

// S tính được khi mọi nhóm không âm và cả hai tổng lớn hơn 0: khi đó p_i, q_i là phân phối. Một tổng bằng 0 thì
// p_i = b_i / B là 0 / 0, ra NaN.
fn surprise_is_defined(
    groups: &[GroupValues],
    base_total: Decimal,
    compare_total: Decimal,
) -> bool {
    let non_negative = groups
        .iter()
        .all(|group| group.base_value >= Decimal::ZERO && group.compare_value >= Decimal::ZERO);
    non_negative && base_total > Decimal::ZERO && compare_total > Decimal::ZERO
}

/// Sức giải thích của một chiều, chỉ để xếp chiều; không hiện ra.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionScore {
    /// Các nhóm đã chọn cộng lại đủ `EXPLAINED_SHARE` của chênh lệch.
    pub reaches_threshold: bool,
    /// Số nhóm đã chọn (k của Adtributor): ít nhóm hơn thì lời giải thích ngắn gọn hơn.
    pub group_count: usize,
    /// Tổng S_i của các nhóm đã chọn; 0 khi xếp theo Effect.
    pub surprise: f64,
    /// EP_i lớn nhất trong chiều, thứ tự phụ khi xếp theo Effect.
    pub top_effect: Decimal,
}

/// Vì sao một chiều không có điểm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
    /// Chênh lệch bằng 0: không có gì để phân rã (`explanation_unavailable`).
    NoChange,
    /// Các nhóm không cộng ra đúng tổng hai lát cắt (hoặc phép tính tràn khoảng Decimal): bỏ chiều này và ghi log.
    TotalsMismatch,
    /// Gọi với Surprise khi S không tính được (có số âm, hoặc một tổng bằng 0); chọn cách xếp bằng build_ranking_mode.
    SurpriseUndefined,
}

// Số liệu của một nhóm để xếp hạng: x_i và EP_i bằng Decimal để so ngưỡng đúng, S_i bằng f64.
struct GroupShare {
    contribution: Decimal,
    effect: Decimal,
    surprise: f64,
}

/// Điểm của một chiều theo Adtributor. `base_total`, `compare_total` là giá trị hai lát cắt của cả câu hỏi (B, C);
/// các nhóm phải cộng ra đúng hai số này. Công thức và lý do ở #39:
/// `EP_i = x_i / D`, `S_i = 0.5 * (p_i * ln(p_i / m_i) + q_i * ln(q_i / m_i))` với `p_i = b_i / B`, `q_i = c_i / C`,
/// `m_i = (p_i + q_i) / 2`. Xét nhóm theo thứ tự của `mode`, lấy nhóm có `EP_i >= 0.1`, dừng khi tổng EP đủ 0.67.
// Không tự đổi Surprise sang Effect khi S không tính được: mọi chiều của một câu hỏi phải xếp cùng một cách, nên báo
// lỗi để bên gọi sửa chỗ chọn cách xếp thay vì lặng lẽ so hai thước khác nhau.
pub fn build_dimension_score(
    groups: &[GroupValues],
    base_total: Decimal,
    compare_total: Decimal,
    mode: RankingMode,
) -> Result<DimensionScore, ScoreError> {
    let base_sum = build_sum(groups.iter().map(|group| group.base_value));
    let compare_sum = build_sum(groups.iter().map(|group| group.compare_value));
    let change = compare_total.checked_sub(base_total);
    let (Some(base_sum), Some(compare_sum), Some(change)) = (base_sum, compare_sum, change) else {
        return Err(ScoreError::TotalsMismatch);
    };
    if base_sum != base_total || compare_sum != compare_total {
        return Err(ScoreError::TotalsMismatch);
    }
    if change.is_zero() {
        return Err(ScoreError::NoChange);
    }
    if mode == RankingMode::Surprise && !surprise_is_defined(groups, base_total, compare_total) {
        return Err(ScoreError::SurpriseUndefined);
    }
    let shares: Option<Vec<GroupShare>> = groups
        .iter()
        .map(|group| build_group_share(group, base_total, compare_total, change, mode))
        .collect();
    let mut shares = shares.ok_or(ScoreError::TotalsMismatch)?;
    // sort_by giữ nguyên thứ tự của các nhóm bằng nhau, nên kết quả không phụ thuộc may rủi.
    match mode {
        RankingMode::Surprise => shares.sort_by(|a, b| {
            b.surprise
                .total_cmp(&a.surprise)
                .then(b.effect.cmp(&a.effect))
        }),
        RankingMode::Effect => shares.sort_by_key(|s| Reverse(s.effect)),
    }
    let mut score = DimensionScore {
        reaches_threshold: false,
        group_count: 0,
        surprise: 0.0,
        top_effect: shares.iter().map(|s| s.effect).max().unwrap_or_default(),
    };
    // Cộng x_i của các nhóm đã chọn rồi so với 0.67 * D, thay vì cộng các EP_i: EP_i = x_i / D có thể là số thập phân
    // vô hạn (100 / 300) và bị làm tròn ở chữ số thứ 28, tổng các EP khi đó có thể lệch biên 0.67.
    let mut explained = Decimal::ZERO;
    for share in &shares {
        if !reaches_share(share.contribution, change, MIN_GROUP_SHARE) {
            continue;
        }
        explained = explained
            .checked_add(share.contribution)
            .ok_or(ScoreError::TotalsMismatch)?;
        score.group_count += 1;
        score.surprise += share.surprise;
        if reaches_share(explained, change, EXPLAINED_SHARE) {
            score.reaches_threshold = true;
            break;
        }
    }
    Ok(score)
}

// part / change >= share, tính không qua phép chia: nhân hai vế với change, đổi chiều so sánh khi change âm. Phép nhân
// trên Decimal là đúng, nên nhóm có EP đúng bằng 0.1 (0.20 thành 0.30 với D = 1.00) vẫn đạt. share nhỏ hơn 1 nên tích
// không tràn.
fn reaches_share(part: Decimal, change: Decimal, share: Decimal) -> bool {
    let bound = share * change;
    if change > Decimal::ZERO {
        part >= bound
    } else {
        part <= bound
    }
}

// Cộng bằng Decimal nên so với tổng được bằng ==. try_fold dừng ở lần cộng tràn đầu tiên và trả None, thay vì panic.
fn build_sum(mut values: impl Iterator<Item = Decimal>) -> Option<Decimal> {
    values.try_fold(Decimal::ZERO, |total, value| total.checked_add(value))
}

fn build_group_share(
    group: &GroupValues,
    base_total: Decimal,
    compare_total: Decimal,
    change: Decimal,
    mode: RankingMode,
) -> Option<GroupShare> {
    let contribution = group.compare_value.checked_sub(group.base_value)?;
    // EP_i chỉ dùng để xếp thứ tự nhóm và chiều; so ngưỡng dùng reaches_share, không dùng số đã làm tròn này.
    let effect = contribution.checked_div(change)?;
    let surprise = match mode {
        RankingMode::Surprise => {
            let p = to_f64(group.base_value) / to_f64(base_total);
            let q = to_f64(group.compare_value) / to_f64(compare_total);
            let m = (p + q) / 2.0;
            0.5 * (build_entropy_term(p, m) + build_entropy_term(q, m))
        }
        RankingMode::Effect => 0.0,
    };
    Some(GroupShare {
        contribution,
        effect,
        surprise,
    })
}

// p * ln(p / m) với quy ước 0 * ln(0) = 0: nhóm chỉ có ở một lát cắt (b_i = 0) có p = 0, ln(0) là âm vô cùng và tích
// sẽ ra NaN nếu tính thẳng.
fn build_entropy_term(p: f64, m: f64) -> f64 {
    if p == 0.0 {
        0.0
    } else {
        p * (p / m).ln()
    }
}

// Decimal luôn đổi được sang f64 (có thể mất chữ số cuối, không sao vì chỉ để xếp hạng).
fn to_f64(value: Decimal) -> f64 {
    value.to_f64().unwrap_or(0.0)
}

/// Một chiều đã phân rã kèm điểm của nó, để xếp trước khi đưa vào `Explanation.dimensions`.
#[derive(Debug, Clone)]
pub struct RankedDimension {
    pub breakdown: DimensionBreakdown,
    pub score: DimensionScore,
}

/// Xếp chiều giải thích chênh lệch rõ nhất lên đầu: chiều đạt ngưỡng 0.67 trước; rồi theo Surprise là tổng S giảm dần,
/// số nhóm tăng dần; theo Effect là số nhóm tăng dần, EP lớn nhất giảm dần; cuối cùng theo tên cột cho ổn định.
pub fn apply_dimension_order(dimensions: &mut [RankedDimension], mode: RankingMode) {
    dimensions.sort_by(|a, b| {
        let (x, y) = (&a.score, &b.score);
        let by_score = match mode {
            RankingMode::Surprise => y
                .surprise
                .total_cmp(&x.surprise)
                .then(x.group_count.cmp(&y.group_count)),
            RankingMode::Effect => x
                .group_count
                .cmp(&y.group_count)
                .then(y.top_effect.cmp(&x.top_effect)),
        };
        y.reaches_threshold
            .cmp(&x.reaches_threshold)
            .then(by_score)
            .then_with(|| a.breakdown.column.cmp(&b.breakdown.column))
    });
}

/// Các số của `VolumePrice` (ask/contract.rs), đã in thành chuỗi; nhãn và SQL do bên gọi điền.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumePriceNumbers {
    pub base_volume: String,
    pub compare_volume: String,
    pub base_price: String,
    pub compare_price: String,
    pub volume_effect: String,
    pub price_effect: String,
    pub if_volume_unchanged: String,
}

/// Tách chênh lệch thành phần do lượng và phần do giá, Shapley hai nhân tố (#40). V0, V1 là lượng, M0, M1 là độ đo
/// của hai lát cắt, `scale` là số chữ số của giá (`PRICE_SCALE`). None khi một lượng bằng 0 (không có giá) hoặc phép
/// tính tràn; khi đó `volume_price` là null và phân rã theo chiều vẫn trả.
// Giá là chỗ làm tròn duy nhất (quy tắc ở VolumePrice và ARCHITECTURE mục 8): mọi số sau tính đúng từ giá đã làm tròn,
// price_effect lấy bằng phép trừ nên volume_effect + price_effect luôn bằng đúng M1 - M0. Chia 2 có thể thêm một chữ
// số (2.495 khi giá có hai chữ số), in đủ, không làm tròn thêm.
pub fn build_volume_price(
    base_volume: Decimal,
    compare_volume: Decimal,
    base_measure: Decimal,
    compare_measure: Decimal,
    scale: u32,
) -> Option<VolumePriceNumbers> {
    if base_volume.is_zero() || compare_volume.is_zero() {
        return None;
    }
    let base_price = build_price(base_measure, base_volume, scale)?;
    let compare_price = build_price(compare_measure, compare_volume, scale)?;
    let change = compare_measure.checked_sub(base_measure)?;
    let volume_effect = compare_volume
        .checked_sub(base_volume)?
        .checked_mul(base_price.checked_add(compare_price)?)?
        .checked_div(Decimal::TWO)?;
    let price_effect = change.checked_sub(volume_effect)?;
    let if_volume_unchanged = base_volume.checked_mul(compare_price)?;
    Some(VolumePriceNumbers {
        base_volume: build_decimal_text(base_volume, 0),
        compare_volume: build_decimal_text(compare_volume, 0),
        base_price: build_decimal_text(base_price, scale),
        compare_price: build_decimal_text(compare_price, scale),
        volume_effect: build_decimal_text(volume_effect, scale),
        price_effect: build_decimal_text(price_effect, scale),
        if_volume_unchanged: build_decimal_text(if_volume_unchanged, scale),
    })
}

// Giá = độ đo / lượng, làm tròn nửa xa số 0 (2.005 thành 2.01, -2.005 thành -2.01), cách làm tròn người dùng quen; làm
// tròn kiểu ngân hàng (nửa về số chẵn) sẽ cho 2.00 và khó giải thích. Phép chia của Decimal giữ 28 chữ số có nghĩa, nên
// thương chỉ làm tròn sai khi độ đo lớn cỡ 10^24, xa mức của DECIMAL(18, 2).
fn build_price(measure: Decimal, volume: Decimal, scale: u32) -> Option<Decimal> {
    let quotient = measure.checked_div(volume)?;
    Some(quotient.round_dp_with_strategy(scale, RoundingStrategy::MidpointAwayFromZero))
}

/* Phần chữ của bước narrate (#41, #201). Prompt ở explain/narrate_prompt.rs: mô hình viết insight từ số code đã tính,
kèm danh sách khẳng định: mỗi số và mỗi nhóm insight dùng trỏ vào một ô của bảng; số trong khẳng định ghi có dấu như ô
của bảng, nên chiều nằm ở dấu. Cổng 1 (explain/claims.rs) so từng khẳng định với đúng ô; cổng 2 (explain/judge.rs) là
một lần gọi mô hình riêng so lời với các khẳng định. Code không đọc lời văn.

Câu khuôn (explain/summary.rs) do code viết từ bảng, không qua hai cổng. Test giữ nó đúng số bằng bước kiểm số cũ
(build_number_check): số trong câu phải có nguyên văn trong bảng; giá trị nhóm, nhãn, tên cột và tên độ đo chép nguyên
văn được miễn (build_known_texts). */

/// Số ký tự tối đa của lời diễn giải; câu của mô hình dài hơn thì bỏ, dùng câu khuôn. Câu khuôn cũng bị cắt ở mức
/// này.
pub const MAX_SUMMARY_CHARS: usize = 1000;

/// Giá trị nhóm dài quá thì cắt bớt khi đưa vào câu khuôn, để câu khuôn không vượt MAX_SUMMARY_CHARS và đoạn dữ liệu
/// dài trong giá trị nhóm không lọt nguyên văn ra giao diện.
pub(crate) const TEMPLATE_GROUP_CHARS: usize = 80;

// Gửi mô hình hai chiều đầu, mỗi chiều ba nhóm đầu: các nhóm đã xếp theo đóng góp giảm dần nên ba nhóm đầu là phần
// đáng kể; gửi cả bảng thì prompt dài mà phần đuôi gần như không đổi.
pub(crate) const NARRATE_DIMENSIONS: usize = 2;
pub(crate) const NARRATE_GROUPS: usize = 3;

// Chữ chiều theo kiểu so sánh: giữa hai nhóm không có trước sau, nên "tăng" thành "cao hơn", "giảm" thành "thấp hơn",
// "không đổi" thành "bằng". Câu khuôn, prompt narrate và bước kiểm số cùng dùng chữ này.
pub(crate) fn build_direction_word(word: &'static str, comparison: Comparison) -> &'static str {
    match (comparison, word) {
        (Comparison::Groups, "tăng") => "cao hơn",
        (Comparison::Groups, "giảm") => "thấp hơn",
        (Comparison::Groups, "không đổi") => "bằng",
        _ => word,
    }
}

/// "giảm 1.152.666,24" giữa hai kỳ, "thấp hơn 1.152.666,24" giữa hai nhóm; số 0 là "không đổi" hay "bằng".
pub fn build_direction_text(text: &str, digits: u32, comparison: Comparison) -> String {
    match build_change_parts(text, digits) {
        (Some("không đổi"), _) => build_direction_word("không đổi", comparison).to_string(),
        (Some(direction), amount) => {
            format!("{} {amount}", build_direction_word(direction, comparison))
        }
        (None, amount) => amount,
    }
}

/// Mọi chuỗi số được phép xuất hiện trong lời diễn giải: các số của bảng phân rã viết kiểu Việt Nam như prompt gửi
/// (build_number_text) và trị tuyệt đối của chúng (để "giảm 1.283.400,00" nhận khi change là "-1283400.00"), cộng
/// số trong nhãn, tên cột và tên độ đo (để "quý 3/2017" nhận "3" và "2017" từ nhãn lát cắt). Số trong câu hỏi không
/// vào tập này: câu hỏi là chữ người dùng gõ, không phải kết quả đã tính. Giá trị nhóm có chữ cái cũng không góp số
/// vào đây; chúng được miễn theo cách khác (build_known_texts). Giá trị nhóm không có chữ cái (kiểu số như 1 hay
/// 0.25, ngày dạng "01/07/2017") thì góp các dãy chữ số của nó, giữ nguyên cách viết vì đó là tên nhóm.
#[cfg(test)]
pub fn build_allowed_numbers(explanation: &Explanation) -> HashSet<String> {
    let digits = build_fraction_digits(explanation);
    let mut allowed = HashSet::new();
    allow_number(&mut allowed, &explanation.change, digits);
    allow_number(&mut allowed, &explanation.base.value, digits);
    allow_number(&mut allowed, &explanation.compare.value, digits);
    allow_number(&mut allowed, &explanation.base.rows, 0);
    allow_number(&mut allowed, &explanation.compare.rows, 0);
    if let Some(percent) = &explanation.percent_change {
        allow_percent(&mut allowed, percent);
    }
    for dimension in &explanation.dimensions {
        for text in dimension.mix_effect.iter().chain(&dimension.rate_effect) {
            allow_number(&mut allowed, text, digits);
        }
        for group in &dimension.groups {
            allow_number(&mut allowed, &group.base_value, digits);
            allow_number(&mut allowed, &group.compare_value, digits);
            allow_number(&mut allowed, &group.contribution, digits);
            for text in group.mix_effect.iter().chain(&group.rate_effect) {
                allow_number(&mut allowed, text, digits);
            }
            if let Some(value) = &group.value {
                let text = build_value_text(value);
                if !has_letter(&text) {
                    allow_text(&mut allowed, &text);
                }
            }
        }
    }
    for dimension in &explanation.notes.dimensions {
        for group in &dimension.groups {
            for percent in group.share_of_change.iter().chain(&group.percent_change) {
                allow_percent(&mut allowed, percent);
            }
        }
    }
    // Câu khuôn tách lượng và giá theo explain/split.rs (không có lượng thì theo số dòng), nên số được phép lấy từ đó.
    let split = split::build_split(explanation);
    if let Some(split) = &split {
        let n = &split.numbers;
        for text in [
            &n.base_volume,
            &n.compare_volume,
            &n.base_price,
            &n.compare_price,
            &n.volume_effect,
            &n.price_effect,
            &n.if_volume_unchanged,
        ] {
            allow_number(&mut allowed, text, digits);
        }
    }
    allow_text(&mut allowed, &explanation.measure);
    allow_text(&mut allowed, &explanation.base.label);
    allow_text(&mut allowed, &explanation.compare.label);
    for dimension in &explanation.dimensions {
        allow_text(&mut allowed, &dimension.column);
    }
    if let Some(split) = &split {
        allow_text(&mut allowed, &split.volume_label);
        allow_text(&mut allowed, &split.price_label);
    }
    allowed
}

/// Số chữ số lẻ khi viết số của bảng thành chữ: tỷ lệ giữ đủ RATE_SCALE chữ số, độ đo khác hai chữ số như bảng kết
/// quả. Prompt, tập số được phép và câu khuôn cùng dùng, nên số mô hình chép từ prompt khớp chuỗi bước kiểm chờ.
pub(crate) fn build_fraction_digits(explanation: &Explanation) -> u32 {
    match explanation.measure_kind {
        MeasureKind::Ratio => ratio::RATE_SCALE,
        MeasureKind::Total => MAX_FRACTION_DIGITS,
    }
}

/// Có chữ cái (mọi hệ chữ, không chỉ ASCII): "Europe", "Giảm 10%" có; "12", "0.25", "01/07/2017" không.
pub fn has_letter(text: &str) -> bool {
    text.chars().any(char::is_alphabetic)
}

// Một chuỗi số của bảng, viết như prompt gửi: thêm chuỗi đó và trị tuyệt đối (bỏ dấu trừ đứng trước). So nguyên văn,
// không chuẩn hóa: "1.283.400" và "1.283.400,00" là hai số khác nhau, và cách viết thô "1283400.00" không nhận.
// Phần trăm do annotate.rs tính (% thay đổi, % tác động) là số được phép kèm dấu %: lưu thành "%41,5" để bước kiểm
// phân biệt với "41,5" đứng một mình. Phần trăm mô hình tự tính thì không có trong tập này nên vẫn bị loại.
#[cfg(test)]
fn allow_percent(allowed: &mut HashSet<String>, text: &str) {
    let text = build_number_text_with(text, 1);
    allowed.insert(format!("%{}", text.trim_start_matches('-')));
}

#[cfg(test)]
fn allow_number(allowed: &mut HashSet<String>, text: &str, digits: u32) {
    let text = build_number_text_with(text, digits);
    if let Some(abs) = text.strip_prefix('-') {
        allowed.insert(abs.to_string());
    }
    allowed.insert(text);
}

pub(crate) fn comparison_name(comparison: Comparison) -> &'static str {
    match comparison {
        Comparison::Periods => "periods",
        Comparison::Groups => "groups",
    }
}

/// Mọi dãy chữ số trong một đoạn chữ, ví dụ "Quý 3/2017" cho "3" và "2017".
pub fn allow_text(allowed: &mut HashSet<String>, text: &str) {
    for number in scan_numbers(text, &RatioContext::default()) {
        allowed.insert(number.text);
    }
}

/// Các đoạn chữ dữ liệu được miễn kiểm khi xuất hiện nguyên văn trong summary: nhãn hai lát cắt, tên độ đo, tên
/// cột, giá trị nhóm (dạng chữ của build_value_text), nhãn lượng và giá. Số nằm trong một đoạn như vậy ("Giảm 10%",
/// "Quý 3/2017") là một phần của tên gọi, không phải con số mô hình viết. Câu hỏi không nằm trong danh sách này.
/// Hai loại đoạn không được miễn (push_known_text):
/// - đoạn không có chữ cái (giá trị nhóm kiểu số): số của nó đã vào tập được phép; miễn theo đoạn thì "giảm 12%"
///   lọt khi có nhóm "12";
/// - đoạn tự chứa một số kèm chữ tỷ lệ ("tăng 99 phần trăm"): đó là lời gài trong dữ liệu, không phải tên gọi.
#[cfg(test)]
pub fn build_known_texts(explanation: &Explanation) -> Vec<String> {
    let mut texts = Vec::new();
    push_known_text(&mut texts, &explanation.measure);
    push_known_text(&mut texts, &explanation.base.label);
    push_known_text(&mut texts, &explanation.compare.label);
    for text in explanation
        .slice_column
        .iter()
        .chain(&explanation.slice_column_label)
    {
        push_known_text(&mut texts, text);
    }
    for dimension in &explanation.dimensions {
        push_known_text(&mut texts, &dimension.column);
        if let Some(label) = &dimension.label {
            push_known_text(&mut texts, label);
        }
        for group in &dimension.groups {
            if let Some(value) = &group.value {
                push_known_text(&mut texts, &build_value_text(value));
            }
        }
    }
    if let Some(split) = split::build_split(explanation) {
        push_known_text(&mut texts, &split.volume_label);
        push_known_text(&mut texts, &split.price_label);
    }
    texts
}

pub fn push_known_text(texts: &mut Vec<String>, text: &str) {
    if text.is_empty() || !has_letter(text) {
        return;
    }
    let context = RatioContext::default();
    // Chỉ xét cụm chữ (symbols = false): "Giảm 10%" vẫn là tên gọi hợp lệ vì "%" là ký hiệu hay gặp trong dữ liệu.
    let planted = scan_numbers(text, &context)
        .iter()
        .any(|number| follows_ratio(&text[number.end..], &context, false));
    if !planted {
        texts.push(text.to_string());
    }
}

/// Bước kiểm số cũ (#41), giờ chỉ test dùng để giữ câu khuôn do code viết đúng số của bảng: lời của mô hình do cổng 1
/// (khẳng định so với bảng, explain/claims.rs) và cổng 2 (mô hình chấm, explain/judge.rs) kiểm.
/// Kiểm mọi số trong lời diễn giải. Một số là sai khi: nó nằm ngoài tập số được phép của bảng (so nguyên văn, không
/// tính dấu trừ đứng trước), hoặc nó đứng trước một ký hiệu hay chữ chỉ tỷ lệ (`%`, `％`, `‰`, "3x", "gấp 3",
/// "phần trăm", "percent", "pct", "lần", bỏ qua khoảng trắng giữa số và ký hiệu) vì bảng phân rã không có tỷ lệ
/// nào. Số nằm trong một đoạn chữ dữ liệu xuất hiện nguyên văn thì bỏ qua, trừ khi nó kết thúc đúng ở cuối đoạn và
/// ngay sau đoạn là chữ tỷ lệ ("Giảm 10%" chép từ nhóm "Giảm 10" vẫn bị loại). Err là các số sai, không trùng nhau,
/// để ghi log và đưa vào detail của bước narrate.
#[cfg(test)]
pub fn build_number_check(summary: &str, explanation: &Explanation) -> Result<(), Vec<String>> {
    let allowed = build_allowed_numbers(explanation);
    let known = build_known_texts(explanation);
    build_text_number_check(summary, &allowed, &known, &explanation.measure)
}

/// Phần chính của bước kiểm số cũ (build_number_check): allowed là tập số được phép, known là các đoạn chữ dữ liệu được
/// miễn (đã qua push_known_text), measure là tên độ đo để biết "lần" là đơn vị đếm hay chữ tỷ lệ. Đoạn trả lời câu thường
/// (ask/summary.rs) cũng từng dùng hàm này; giờ hai loại câu đều qua hai cổng (#201), chỉ test còn dùng.
pub fn build_text_number_check(
    summary: &str,
    allowed: &HashSet<String>,
    known: &[String],
    measure: &str,
) -> Result<(), Vec<String>> {
    let context = RatioContext {
        lan_is_unit: measure_counts_times(measure),
    };
    // Vị trí (byte) của mọi lần xuất hiện nguyên văn của các đoạn chữ dữ liệu trong summary.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for text in known {
        for (start, _) in summary.match_indices(text.as_str()) {
            spans.push((start, start + text.len()));
        }
    }
    let mut wrong = Vec::new();
    for number in scan_numbers(summary, &context) {
        let known = spans
            .iter()
            .find(|&&(start, end)| number.start >= start && number.end <= end);
        if let Some(&(_, end)) = known {
            if number.end == end && number.derived && !wrong.contains(&number.text) {
                wrong.push(number.text);
            }
            continue;
        }
        // Chỉ "%" theo sau mới là phần trăm code đã tính; "gấp 3", "3 lần" vẫn là tỷ lệ mô hình tự nghĩ ra.
        let is_percent = summary[number.end..].trim_start().starts_with(['%', '％']);
        let allowed_percent =
            number.derived && is_percent && allowed.contains(&format!("%{}", number.text));
        if allowed_percent {
            continue;
        }
        if (number.derived || !allowed.contains(&number.text)) && !wrong.contains(&number.text) {
            wrong.push(number.text);
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong)
    }
}

// Độ đo đếm số lần ("Số lần giao trễ"): "lần" sau một số là đơn vị đếm, không phải chữ tỷ lệ.
fn measure_counts_times(measure: &str) -> bool {
    measure
        .to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .any(|word| word == "lần")
}

/// Một dãy chữ số tìm được trong text. `start`, `end` là vị trí byte, để so với các đoạn chữ dữ liệu.
struct ScannedNumber {
    start: usize,
    end: usize,
    text: String,
    /// Đứng cạnh ký hiệu hay chữ chỉ tỷ lệ (trước hoặc sau số).
    derived: bool,
}

/// Ngữ cảnh của luật tỷ lệ.
#[derive(Default)]
struct RatioContext {
    /// "lần" là đơn vị đếm của độ đo (measure_counts_times), không phải chữ tỷ lệ.
    lan_is_unit: bool,
}

// Quét các dãy chữ số trong text: một số là dãy chữ số, dấu chấm và dấu phẩy bắt đầu bằng chữ số, như cách viết
// kiểu Việt Nam "1.283.400,00". Dấu phẩy chỉ thuộc số khi có chữ số ở cả hai bên, để "3, 4 và 5" vẫn là ba số; dấu
// chấm thừa ở cuối bị bỏ ("năm 2017." vẫn là "2017"); dấu trừ đứng trước không thuộc số. is_numeric nhận cả chữ số
// ngoài ASCII (chữ số toàn khổ, chữ số mũ...): chúng không bao giờ có trong tập được phép nên luôn bị loại. Cách
// viết thô "1283400.00" hay kiểu Anh "1,283,400.00" là một dãy không khớp số nào của bảng, nên bị loại.
fn scan_numbers(text: &str, context: &RatioContext) -> Vec<ScannedNumber> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].1.is_numeric() {
            i += 1;
            continue;
        }
        let first = i;
        while i < chars.len()
            && (chars[i].1.is_numeric() || chars[i].1 == '.' || is_decimal_comma(&chars, i))
        {
            i += 1;
        }
        let mut last = i;
        while last > first && chars[last - 1].1 == '.' {
            last -= 1;
        }
        let start = chars[first].0;
        let end = chars.get(last).map(|(byte, _)| *byte).unwrap_or(text.len());
        found.push(ScannedNumber {
            start,
            end,
            text: text[start..end].to_string(),
            derived: follows_ratio(&text[end..], context, true) || preceded_by_ratio(text, start),
        });
    }
    found
}

// Dấu phẩy ở vị trí i nằm giữa hai chữ số.
fn is_decimal_comma(chars: &[(usize, char)], i: usize) -> bool {
    chars[i].1 == ','
        && i > 0
        && chars[i - 1].1.is_numeric()
        && chars.get(i + 1).is_some_and(|(_, c)| c.is_numeric())
}

// Sau một số, bỏ qua khoảng trắng (char::is_whitespace nhận cả NBSP U+00A0 và U+202F, kiểu gõ hay gặp trước "%")
// rồi tìm ký hiệu hay chữ chỉ tỷ lệ. Chữ phải đứng riêng ("lần" chứ không phải đầu của một từ dài hơn, "x" chứ
// không phải đầu của "xuống"). symbols = false thì chỉ xét cụm chữ, dùng khi quyết một đoạn chữ dữ liệu có được
// miễn không (push_known_text).
fn follows_ratio(rest: &str, context: &RatioContext, symbols: bool) -> bool {
    let rest = rest.trim_start();
    if symbols {
        if rest.starts_with(['%', '％', '‰']) {
            return true;
        }
        // "3x", "3 x": x phải đứng một mình.
        if rest.starts_with(['x', 'X'])
            && !rest[1..].chars().next().is_some_and(char::is_alphanumeric)
        {
            return true;
        }
    }
    let lower = rest.to_lowercase();
    // Khoảng trắng giữa các chữ của cụm nhận mọi khoảng trắng ("phần trăm" với NBSP vẫn bắt).
    for phrase in [
        &["phần", "trăm"][..],
        &["percent"][..],
        &["per", "cent"][..],
        &["pct"][..],
    ] {
        if let Some(after) = match_phrase(&lower, phrase) {
            if !after.chars().next().is_some_and(char::is_alphanumeric) {
                return true;
            }
        }
    }
    // "lần lượt" là trạng từ, không phải chữ tỷ lệ.
    if !context.lan_is_unit {
        if let Some(after) = lower.strip_prefix("lần") {
            let boundary = !after.chars().next().is_some_and(char::is_alphanumeric);
            if boundary && !after.trim_start().starts_with("lượt") {
                return true;
            }
        }
    }
    false
}

// Khớp một cụm chữ ở đầu `lower` (đã viết thường), trả phần còn lại sau cụm.
fn match_phrase<'a>(lower: &'a str, words: &[&str]) -> Option<&'a str> {
    let mut rest = lower;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            if !rest.chars().next().is_some_and(char::is_whitespace) {
                return None;
            }
            rest = rest.trim_start();
        }
        rest = rest.strip_prefix(word)?;
    }
    Some(rest)
}

// "gấp" đứng ngay trước một số ("gấp 3") cũng là cách nói tỷ lệ.
fn preceded_by_ratio(text: &str, start: usize) -> bool {
    text[..start]
        .trim_end()
        .to_lowercase()
        .strip_suffix("gấp")
        .is_some_and(|head| !head.chars().last().is_some_and(char::is_alphanumeric))
}

// "cao hơn 85.185" thành "cao hơn Pass 85.185": tên nhóm gốc đứng giữa chữ chiều và số, như cách nói thường. "bằng"
// không có số đi kèm thì thành "bằng Pass".
pub(crate) fn between(amount: &str, base: &str) -> String {
    for word in ["cao hơn", "thấp hơn"] {
        if let Some(number) = amount.strip_prefix(word) {
            return format!("{word} {base}{number}");
        }
    }
    if amount == "bằng" {
        return format!("bằng {base}");
    }
    format!("{amount} so với {base}")
}

/// Phần chung của bước kiểm chiều: trước mỗi lần độ lớn (đã viết không dấu) đứng riêng trong summary, chữ "tăng" hay
/// "giảm" gần nhất trong cùng câu phải là chữ chờ. Phần giải thích của câu thường (service/insight/) cũng dùng. Err là
/// các độ lớn bị nói ngược chiều.
pub fn build_amount_direction_check(
    summary: &str,
    changes: &[(String, &str)],
) -> Result<(), Vec<String>> {
    let mut wrong = Vec::new();
    for (amount, expected) in changes {
        for (start, _) in summary.match_indices(amount.as_str()) {
            if !stands_alone(summary, start, amount.len()) {
                continue;
            }
            if let Some(said) = last_direction_word(&summary[..start]) {
                if said != *expected && !wrong.contains(amount) {
                    wrong.push(amount.clone());
                }
            }
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong)
    }
}

// Số đứng riêng, để "85.185" không khớp giữa "185.185,00": ký tự liền trước không phải chữ số hay dấu ngăn của số,
// liền sau không phải chữ số hay dấu phẩy rồi chữ số (phần thập phân). Dấu phẩy rồi khoảng trắng là ngắt câu.
fn stands_alone(text: &str, start: usize, len: usize) -> bool {
    let before = text[..start].chars().next_back();
    let joins_before = before.is_some_and(|c| c.is_ascii_digit() || c == '.' || c == ',');
    let mut after = text[start + len..].chars();
    let joins_after = match after.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some(',') => after.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    };
    !joins_before && !joins_after
}

// Chữ chiều gần nhất phía trước, chỉ trong câu đang xét: sau ". ", "; ", "! " hay "? " cuối cùng. Dấu chấm ngăn nghìn
// không có khoảng trắng theo sau nên không bị coi là hết câu.
fn last_direction_word(before: &str) -> Option<&'static str> {
    let sentence_start = [". ", "; ", "! ", "? "]
        .iter()
        .filter_map(|mark| before.rfind(mark).map(|i| i + mark.len()))
        .max()
        .unwrap_or(0);
    let sentence = &before[sentence_start..];
    let up = sentence.rfind("tăng");
    let down = sentence.rfind("giảm");
    match (up, down) {
        (Some(u), Some(d)) => Some(if u > d { "tăng" } else { "giảm" }),
        (Some(_), None) => Some("tăng"),
        (None, Some(_)) => Some("giảm"),
        (None, None) => None,
    }
}

// Cùng chiều: cùng dấu, hoặc cùng bằng 0. contribution không đọc được coi như khác chiều.
fn same_direction(contribution: &str, change: Decimal) -> bool {
    match contribution.parse::<Decimal>() {
        Ok(contribution) => (contribution < Decimal::ZERO) == (change < Decimal::ZERO),
        Err(_) => false,
    }
}

/// Tên thứ được đếm khi đếm một cột mã: tên tiếng Việt bỏ chữ "mã" đứng đầu ("Mã đơn hàng" thành "đơn hàng"), để nhãn đọc
/// "Số đơn hàng" chứ không phải "Số Mã đơn hàng". Chưa có tên tiếng Việt thì dùng tên cột.
pub(crate) fn build_entity_name(column: &str, label: Option<&str>) -> String {
    let Some(label) = label.map(str::trim).filter(|l| !l.is_empty()) else {
        return column.to_string();
    };
    let lowered = lower_first(label);
    match lowered.strip_prefix("mã ") {
        Some(rest) if !rest.trim().is_empty() => rest.trim().to_string(),
        _ => lowered,
    }
}

// Hạ chữ đầu khi một nhãn đứng giữa câu ("Số đơn" thành "số đơn"), để câu ghép đọc tự nhiên. Từ viết tắt toàn hoa
// ("GMV") giữ nguyên: chỉ hạ khi ký tự thứ hai không phải chữ hoa.
pub(crate) fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    if chars.next().is_some_and(char::is_uppercase) {
        return text.to_string();
    }
    first.to_lowercase().collect::<String>() + &text[first.len_utf8()..]
}

// Nâng chữ đầu khi một nghĩa tiếng Việt viết thường ("mã bộ phận") đứng đầu câu.
pub(crate) fn upper_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

// Cắt còn tối đa max ký tự, thêm "…" khi phải cắt. Lùi về khoảng trắng cuối trước mức cắt để không cắt ngang một số
// hay một từ (một dãy chữ số không chứa khoảng trắng); chỉ khi không có khoảng trắng nào (một từ dài liền) mới cắt
// cứng.
pub(crate) fn truncate_text(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    let cut = match cut.rfind(char::is_whitespace) {
        Some(pos) => &cut[..pos],
        None => &cut,
    };
    format!("{cut}…")
}
