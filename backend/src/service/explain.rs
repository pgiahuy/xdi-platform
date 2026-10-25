// SPDX-License-Identifier: Apache-2.0
/* service/explain.rs - giải thích câu hỏi "tại sao": chọn hai lát cắt, tính chênh lệch độ đo, phân rã theo
từng chiều, xếp hạng đóng góp. Tổng các đóng góp phải bằng đúng chênh lệch. LLM chỉ diễn giải bảng số này.

Hiện có phần toán, hàm thuần không đọc SQL, DuckDB hay mô hình: đóng góp của từng nhóm và xếp hạng chiều (#39), tách
lượng và giá (#40). Đầu vào là số đã đọc từ DuckDB (chuỗi VARCHAR đổi sang Decimal), đầu ra là struct của hợp đồng
(ask_contract.rs) hoặc struct trung gian. Phần dựng SQL và nối vào luồng hỏi làm ở #38, #39; test ở explain_tests.rs.

Cuối file là phần chữ của bước narrate (#41), cũng là hàm thuần: prompt gửi mô hình, tập số được phép, bước kiểm mọi
số trong lời diễn giải và câu khuôn dự phòng. Việc gọi mô hình, giới hạn thời gian và dựng AskStep ở ask_narrate.rs.

Mọi con số hiện ra tính bằng Decimal (rust_decimal, 28 chữ số), không qua f64: f64 làm sai chữ số cuối và tổng đóng
góp sẽ lệch chênh lệch. Phép tính tràn khoảng của Decimal trả None hoặc lỗi, không panic. EP (phần đóng góp) cũng
tính và so với ngưỡng bằng Decimal, để nhóm nằm đúng biên 0.1 hay 0.67 không bị loại; chỉ độ bất ngờ S là f64, vì cần
logarit và chỉ dùng để so thứ tự, không so với ngưỡng nào. */

use std::cmp::{Ordering, Reverse};
use std::collections::HashSet;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use super::ask_contract::{CellValue, DimensionBreakdown, Explanation, GroupContribution};

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
            }
        })
        .collect();
    Some(contributions)
}

// Thứ tự phụ khi hai nhóm đóng góp bằng nhau: theo chữ, nhóm null sau cùng.
fn compare_group_values(a: &Option<CellValue>, b: &Option<CellValue>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => build_value_text(a).cmp(&build_value_text(b)),
    }
}

fn build_value_text(value: &CellValue) -> String {
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

/// Các số của `VolumePrice` (ask_contract.rs), đã in thành chuỗi; nhãn và SQL do bên gọi điền.
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

/* Phần chữ của bước narrate (#41). Mô hình chỉ được diễn giải bảng phân rã đã tính, nên prompt gửi đúng các số của
bảng và bước kiểm (build_number_check) so nguyên văn từng số trong câu trả lời với các chuỗi số của bảng: mô hình
viết "1283400" thay "1283400.00", làm tròn, hay thêm phần trăm đều bị loại và ask_narrate.rs dùng câu khuôn. */

/// Số ký tự tối đa của lời diễn giải; câu của mô hình dài hơn thì bỏ, dùng câu khuôn.
pub const MAX_SUMMARY_CHARS: usize = 1000;

// Gửi mô hình hai chiều đầu, mỗi chiều ba nhóm đầu: các nhóm đã xếp theo đóng góp giảm dần nên ba nhóm đầu là phần
// đáng kể; gửi cả bảng thì prompt dài mà phần đuôi gần như không đổi.
const NARRATE_DIMENSIONS: usize = 2;
const NARRATE_GROUPS: usize = 3;

/// Prompt của bước narrate: luật viết tiếng Anh như prompt sinh SQL và prompt sửa (#29, #32), dữ liệu là bảng phân
/// rã dạng JSON. Chỉ gửi hai chiều đầu, ba nhóm mỗi chiều, và volume_price khi có. Câu hỏi gốc đứng riêng để mock
/// tra câu trả lời theo nó (xem LlmRequest).
pub fn build_narrate_prompt(question: &str, explanation: &Explanation) -> String {
    let dimensions: Vec<serde_json::Value> = explanation
        .dimensions
        .iter()
        .take(NARRATE_DIMENSIONS)
        .map(|dimension| {
            let groups: Vec<serde_json::Value> = dimension
                .groups
                .iter()
                .take(NARRATE_GROUPS)
                .map(|group| {
                    serde_json::json!({
                        "value": group.value,
                        "base_value": group.base_value,
                        "compare_value": group.compare_value,
                        "contribution": group.contribution,
                    })
                })
                .collect();
            serde_json::json!({ "column": dimension.column, "groups": groups })
        })
        .collect();
    let volume_price = explanation.volume_price.as_ref().map(|split| {
        serde_json::json!({
            "volume_label": split.volume_label,
            "price_label": split.price_label,
            "base_volume": split.base_volume,
            "compare_volume": split.compare_volume,
            "base_price": split.base_price,
            "compare_price": split.compare_price,
            "volume_effect": split.volume_effect,
            "price_effect": split.price_effect,
            "if_volume_unchanged": split.if_volume_unchanged,
        })
    });
    let breakdown = serde_json::json!({
        "measure": explanation.measure,
        "base": { "label": explanation.base.label, "value": explanation.base.value },
        "compare": { "label": explanation.compare.label, "value": explanation.compare.value },
        "change": explanation.change,
        "dimensions": dimensions,
        "volume_price": volume_price,
    });
    let json = serde_json::to_string_pretty(&breakdown).expect("serialize narrate breakdown");
    format!(
        "The breakdown below was computed from the data; every number in it is exact. Write a short summary of it \
         in Vietnamese for the user who asked the question.\n\
         Rules:\n\
         - Vietnamese, at most four sentences, at most {MAX_SUMMARY_CHARS} characters.\n\
         - Copy every number exactly as it appears in the JSON, decimal digits included: never round, never add or \
         remove digits, never use a thousands separator, never compute a new number such as a percentage.\n\
         - A negative change means the measure went down: say \"giảm\" only when change is negative, \"tăng\" only \
         when it is positive.\n\
         - Do not give causes that are not in the JSON.\n\
         - Reply with the summary only, no introduction, no title.\n\n\
         Question: {question}\n\
         Breakdown:\n{json}"
    )
}

/// Mọi chuỗi số được phép xuất hiện trong lời diễn giải: các số của bảng phân rã và trị tuyệt đối của chúng (để
/// "giảm 1283400.00" nhận khi change là "-1283400.00"), cộng số trong nhãn, tên cột, giá trị nhóm và câu hỏi (để
/// "quý 3/2017" nhận "3" và "2017" từ nhãn lát cắt).
pub fn build_allowed_numbers(question: &str, explanation: &Explanation) -> HashSet<String> {
    let mut allowed = HashSet::new();
    allow_number(&mut allowed, &explanation.change);
    allow_number(&mut allowed, &explanation.base.value);
    allow_number(&mut allowed, &explanation.compare.value);
    for dimension in &explanation.dimensions {
        for group in &dimension.groups {
            allow_number(&mut allowed, &group.base_value);
            allow_number(&mut allowed, &group.compare_value);
            allow_number(&mut allowed, &group.contribution);
        }
    }
    if let Some(split) = &explanation.volume_price {
        for text in [
            &split.base_volume,
            &split.compare_volume,
            &split.base_price,
            &split.compare_price,
            &split.volume_effect,
            &split.price_effect,
            &split.if_volume_unchanged,
        ] {
            allow_number(&mut allowed, text);
        }
    }
    allow_text(&mut allowed, question);
    allow_text(&mut allowed, &explanation.measure);
    allow_text(&mut allowed, &explanation.base.label);
    allow_text(&mut allowed, &explanation.compare.label);
    for dimension in &explanation.dimensions {
        allow_text(&mut allowed, &dimension.column);
        for group in &dimension.groups {
            if let Some(value) = &group.value {
                allow_text(&mut allowed, &build_value_text(value));
            }
        }
    }
    if let Some(split) = &explanation.volume_price {
        allow_text(&mut allowed, &split.volume_label);
        allow_text(&mut allowed, &split.price_label);
    }
    allowed
}

// Một chuỗi số của bảng: thêm nguyên chuỗi và trị tuyệt đối (bỏ dấu trừ đứng trước). So nguyên văn, không chuẩn hóa:
// "1283400" và "1283400.00" là hai số khác nhau.
fn allow_number(allowed: &mut HashSet<String>, text: &str) {
    allowed.insert(text.to_string());
    if let Some(abs) = text.strip_prefix('-') {
        allowed.insert(abs.to_string());
    }
}

// Mọi dãy chữ số trong một đoạn chữ, ví dụ "Quý 3/2017" cho "3" và "2017".
fn allow_text(allowed: &mut HashSet<String>, text: &str) {
    for (number, _) in scan_numbers(text) {
        allowed.insert(number);
    }
}

/// Kiểm mọi số trong lời diễn giải: mỗi dãy chữ số (không tính dấu trừ đứng trước) phải có nguyên văn trong
/// `allowed`, và số đứng ngay trước `%` luôn sai vì bảng phân rã không có phần trăm (mô hình phải không tự tính).
/// Err là các số sai, không trùng nhau, để ghi log và đưa vào detail của bước narrate.
pub fn build_number_check(summary: &str, allowed: &HashSet<String>) -> Result<(), Vec<String>> {
    let mut wrong = Vec::new();
    for (number, before_percent) in scan_numbers(summary) {
        if (before_percent || !allowed.contains(&number)) && !wrong.contains(&number) {
            wrong.push(number);
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong)
    }
}

// Quét các dãy chữ số trong text: một số là dãy chữ số và dấu chấm bắt đầu bằng chữ số, bỏ dấu chấm thừa ở cuối
// ("năm 2017." vẫn là "2017"); dấu trừ đứng trước không thuộc số. Cờ bool báo số đứng ngay trước '%'. Cách viết
// "1.283.400,00" cho các dãy "1.283.400" và "00", không dãy nào khớp một số của bảng, nên bị loại mà không cần hiểu
// dấu ngăn nghìn.
fn scan_numbers(text: &str) -> Vec<(String, bool)> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        let mut end = i;
        while end > start && chars[end - 1] == '.' {
            end -= 1;
        }
        if end > start {
            found.push((
                chars[start..end].iter().collect(),
                chars.get(i) == Some(&'%'),
            ));
        }
    }
    found
}

/// Câu khuôn thay cho lời của mô hình khi mô hình lỗi, quá giờ, trả câu không dùng được, hoặc không cần gọi
/// (chênh lệch bằng 0). Mọi số lấy nguyên chuỗi từ bảng phân rã nên câu khuôn qua được build_number_check (test ở
/// ask_narrate_tests.rs kiểm điều này, theo tiêu chí của #41).
pub fn build_template_summary(explanation: &Explanation) -> String {
    let change = explanation
        .change
        .parse::<Decimal>()
        .unwrap_or(Decimal::ZERO);
    let (measure, base, compare) = (
        &explanation.measure,
        &explanation.base,
        &explanation.compare,
    );
    let mut text = if change.is_zero() {
        format!(
            "{measure} {} không đổi so với {}: {}.",
            compare.label, base.label, compare.value
        )
    } else if change < Decimal::ZERO {
        format!(
            "{measure} {} giảm {} so với {}, từ {} xuống {}.",
            compare.label,
            explanation.change.trim_start_matches('-'),
            base.label,
            base.value,
            compare.value
        )
    } else {
        format!(
            "{measure} {} tăng {} so với {}, từ {} lên {}.",
            compare.label, explanation.change, base.label, base.value, compare.value
        )
    };
    // Các chiều và nhóm đã xếp sẵn: nhóm đầu của chiều đầu là nhóm đóng góp nhiều nhất.
    if let Some(top) = explanation
        .dimensions
        .first()
        .and_then(|d| d.groups.first())
    {
        let group = top
            .value
            .as_ref()
            .map(build_value_text)
            .unwrap_or_else(|| "giá trị trống".to_string());
        text.push_str(&format!(
            " Theo {}, nhóm {group} đóng góp nhiều nhất ({}).",
            explanation.dimensions[0].column, top.contribution
        ));
    }
    if let Some(split) = &explanation.volume_price {
        text.push_str(&format!(
            " Nếu {} giữ nguyên như {}, {measure} {} là {}.",
            split.volume_label, base.label, compare.label, split.if_volume_unchanged
        ));
    }
    text
}
