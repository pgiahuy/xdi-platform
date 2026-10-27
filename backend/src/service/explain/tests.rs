// SPDX-License-Identifier: Apache-2.0
/* service/explain/tests.rs - test cho phần toán của câu "tại sao" (explain/mod.rs): đóng góp từng nhóm cộng đúng bằng
chênh lệch, thứ hạng chiều theo Adtributor (#39), tách lượng và giá với giá là chỗ làm tròn duy nhất (#40). Số của mẫu
why (ask/examples.rs) phải tính lại ra đúng từ các hàm này. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{CellValue, DimensionBreakdown, ValueKind};
use crate::service::ask::examples::build_why_example;
use crate::service::explain::*;

fn d(text: &str) -> Decimal {
    text.parse().expect("decimal literal")
}

fn group(value: Option<&str>, base: &str, compare: &str) -> GroupValues {
    GroupValues {
        value: value.map(|v| CellValue::String(v.to_string())),
        base_value: d(base),
        compare_value: d(compare),
    }
}

fn ranked(
    column: &str,
    groups: &[GroupValues],
    totals: (&str, &str),
    mode: RankingMode,
) -> RankedDimension {
    RankedDimension {
        breakdown: DimensionBreakdown {
            column: column.to_string(),
            kind: ValueKind::Text,
            sql: String::new(),
            groups: build_group_contributions(groups).expect("contributions"),
        },
        score: build_dimension_score(groups, d(totals.0), d(totals.1), mode).expect("score"),
    }
}

fn columns(dimensions: &[RankedDimension]) -> Vec<&str> {
    dimensions
        .iter()
        .map(|dimension| dimension.breakdown.column.as_str())
        .collect()
}

fn group_names(dimension: &RankedDimension) -> Vec<Option<String>> {
    dimension
        .breakdown
        .groups
        .iter()
        .map(|group| match &group.value {
            Some(CellValue::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

const TOTALS: (&str, &str) = ("1000.00", "840.00");

fn market() -> Vec<GroupValues> {
    vec![
        group(Some("USCA"), "200.00", "210.00"),
        group(Some("LATAM"), "300.00", "280.00"),
        group(Some("Europe"), "500.00", "350.00"),
    ]
}

fn segment() -> Vec<GroupValues> {
    vec![
        group(Some("Consumer"), "600.00", "500.00"),
        group(Some("Corporate"), "400.00", "340.00"),
    ]
}

/// Ví dụ của #39: Market cần một nhóm (Europe) để đủ 0.67 chênh lệch, Segment cần hai, nên Market đứng trước; trong
/// Market các nhóm theo trị tuyệt đối đóng góp.
#[test]
fn market_segment_example_ranks_market_first() {
    let dimensions = [market(), segment()];
    let mode = build_ranking_mode(&dimensions, d(TOTALS.0), d(TOTALS.1));
    assert_eq!(mode, RankingMode::Surprise);
    // Đưa Segment vào trước để chắc thứ tự đến từ điểm, không từ thứ tự đầu vào.
    let mut ranked_dimensions = vec![
        ranked("Segment", &segment(), TOTALS, mode),
        ranked("Market", &market(), TOTALS, mode),
    ];
    apply_dimension_order(&mut ranked_dimensions, mode);
    assert_eq!(columns(&ranked_dimensions), ["Market", "Segment"]);
    let (market, segment) = (&ranked_dimensions[0], &ranked_dimensions[1]);
    assert!(market.score.reaches_threshold);
    assert_eq!(market.score.group_count, 1);
    assert!(segment.score.reaches_threshold);
    assert_eq!(segment.score.group_count, 2);
    // S tính tay theo công thức của #39: S của Europe; Consumer cộng Corporate.
    assert!((market.score.surprise - 0.001_896_556_788).abs() < 1e-12);
    assert!((segment.score.surprise - 0.000_011_787_224).abs() < 1e-12);
    let names: Vec<Option<String>> = ["Europe", "LATAM", "USCA"]
        .iter()
        .map(|name| Some(name.to_string()))
        .collect();
    assert_eq!(group_names(market), names);
    let contributions: Vec<&str> = market
        .breakdown
        .groups
        .iter()
        .map(|group| group.contribution.as_str())
        .collect();
    assert_eq!(contributions, ["-150.00", "-20.00", "10.00"]);
}

/// Theo Surprise, nhóm nhỏ đổi mạnh (Africa, 20 thành 5) được xét trước nhóm lớn đổi ít, nên cần hai nhóm; theo
/// Effect thì nhóm lớn (EP 0.85) đủ một mình.
#[test]
fn surprise_order_differs_from_effect_order() {
    let groups = [
        group(Some("Europe"), "980", "895"),
        group(Some("Africa"), "20", "5"),
    ];
    let score = |mode| build_dimension_score(&groups, d("1000"), d("900"), mode).expect("score");
    assert_eq!(score(RankingMode::Surprise).group_count, 2);
    assert_eq!(score(RankingMode::Effect).group_count, 1);
}

/// Tổng đóng góp của mỗi chiều bằng đúng chênh lệch, cộng bằng Decimal.
#[test]
fn contributions_add_up_to_change() {
    let change = d(TOTALS.1) - d(TOTALS.0);
    for groups in [market(), segment()] {
        let sum: Decimal = build_group_contributions(&groups)
            .expect("contributions")
            .iter()
            .map(|group| d(&group.contribution))
            .sum();
        assert_eq!(sum, change);
    }
}

/// Số của mẫu why: đóng góp và thứ tự nhóm tính lại từ giá trị hai lát cắt phải ra đúng mẫu, và thứ tự chiều của mẫu
/// (Market trước Customer Segment, ngược thứ tự chữ cái) đúng là thứ tự Adtributor cho.
#[test]
fn why_example_matches_the_code() {
    let explanation = build_why_example().explanation.expect("explanation");
    let (base_total, compare_total) = (d(&explanation.base.value), d(&explanation.compare.value));
    let mut all_groups = Vec::new();
    // Đưa chiều và nhóm vào theo thứ tự ngược mẫu, để thứ tự ra là do tính, không do đầu vào.
    for dimension in explanation.dimensions.iter().rev() {
        let groups: Vec<GroupValues> = dimension
            .groups
            .iter()
            .rev()
            .map(|group| GroupValues {
                value: group.value.clone(),
                base_value: d(&group.base_value),
                compare_value: d(&group.compare_value),
            })
            .collect();
        let rebuilt = build_group_contributions(&groups).expect("contributions");
        let expected = serde_json::to_value(&dimension.groups).expect("serialize example");
        assert_eq!(serde_json::to_value(&rebuilt).expect("serialize"), expected);
        all_groups.push((dimension.column.clone(), groups));
    }
    let only_groups: Vec<Vec<GroupValues>> = all_groups.iter().map(|(_, g)| g.clone()).collect();
    let mode = build_ranking_mode(&only_groups, base_total, compare_total);
    assert_eq!(mode, RankingMode::Surprise);
    let mut dimensions: Vec<RankedDimension> = all_groups
        .iter()
        .map(|(column, groups)| {
            ranked(
                column,
                groups,
                (
                    explanation.base.value.as_str(),
                    explanation.compare.value.as_str(),
                ),
                mode,
            )
        })
        .collect();
    apply_dimension_order(&mut dimensions, mode);
    assert_eq!(columns(&dimensions), ["Market", "Customer Segment"]);
    // Tính tay: Market cần Europe, LATAM, Pacific Asia (S khoảng 0.000582); Customer Segment cần Consumer, Corporate
    // (S khoảng 0.000036).
    let (market, segment) = (&dimensions[0].score, &dimensions[1].score);
    assert!(market.reaches_threshold && segment.reaches_threshold);
    assert_eq!(market.group_count, 3);
    assert_eq!(segment.group_count, 2);
    assert!((market.surprise - 0.000_582_245_613_551).abs() < 1e-12);
    assert!((segment.surprise - 0.000_036_009_422_539).abs() < 1e-12);
}

/// Nhóm chỉ có ở lát cắt sau (b_i = 0): quy ước 0 * ln(0) = 0 giữ S hữu hạn, nhóm đó vẫn được chọn.
#[test]
fn group_only_in_one_slice_has_finite_surprise() {
    let groups = [
        group(Some("Europe"), "100.00", "100.00"),
        group(Some("Africa"), "0", "50.00"),
    ];
    let score = build_dimension_score(&groups, d("100.00"), d("150.00"), RankingMode::Surprise)
        .expect("score");
    assert!(score.surprise.is_finite() && score.surprise > 0.0);
    assert_eq!(score.group_count, 1);
    assert!(score.reaches_threshold);
    let contributions = build_group_contributions(&groups).expect("contributions");
    assert_eq!(contributions[0].base_value, "0");
    assert_eq!(contributions[0].contribution, "50.00");
    assert_eq!(contributions[1].contribution, "0.00");
}

/// Nhóm null là một nhóm như mọi nhóm khác, giá trị null; đóng góp bằng nhau thì nhóm null đứng cuối.
#[test]
fn null_group_is_kept_and_sorted_last_on_ties() {
    let groups = [
        group(None, "30.00", "20.00"),
        group(Some("Europe"), "50.00", "40.00"),
        group(Some("LATAM"), "20.00", "0.00"),
    ];
    let contributions = build_group_contributions(&groups).expect("contributions");
    assert!(matches!(&contributions[0].value, Some(CellValue::String(v)) if v == "LATAM"));
    assert!(matches!(&contributions[1].value, Some(CellValue::String(v)) if v == "Europe"));
    assert!(contributions[2].value.is_none());
    assert_eq!(contributions[2].contribution, "-10.00");
    // LATAM về 0 ở lát cắt sau (c_i = 0), S cao nhất (ln 2 / 10); rồi Europe: EP 0.5 + 0.25 đủ 0.67, nhóm null không
    // cần tới.
    let score = build_dimension_score(&groups, d("100.00"), d("60.00"), RankingMode::Surprise)
        .expect("score");
    assert!(score.reaches_threshold);
    assert_eq!(score.group_count, 2);
    assert!((score.surprise - 0.075_287_512_307).abs() < 1e-12);
    assert_eq!(score.top_effect, d("0.5"));
}

/// Nhóm có S cao nhất nhưng đi ngược chênh lệch (Up, EP -1.25 < 0.1) bị bỏ qua; nhóm tiếp theo (Down, EP 1.875) đủ
/// một mình, nên S của chiều chỉ là S của Down.
#[test]
fn high_surprise_group_below_min_share_is_skipped() {
    let groups = [
        group(Some("Up"), "100", "300"),
        group(Some("Down"), "600", "300"),
        group(Some("Flat"), "300", "240"),
    ];
    let score =
        build_dimension_score(&groups, d("1000"), d("840"), RankingMode::Surprise).expect("score");
    assert!(score.reaches_threshold);
    assert_eq!(score.group_count, 1);
    assert!((score.surprise - 0.015_574_822_771).abs() < 1e-12);
}

/// Ngưỡng so đúng tại biên. D = 100: hai nhóm x = 57 và x = 10 cộng đúng 0.67 (f64 cho 0.6699999999999999). D =
/// 1.00: nhóm 0.20 thành 0.30 có EP đúng 0.1 (f64 cho 0.09999999999999998). Nhóm còn lại đều dưới 0.1.
#[test]
fn thresholds_hold_exactly_at_the_boundary() {
    let mut groups = vec![
        group(Some("A"), "100", "157"),
        group(Some("B"), "100", "110"),
    ];
    groups.extend((0..4).map(|i| group(Some(&format!("C{i}")), "100", "108.25")));
    let score =
        build_dimension_score(&groups, d("600"), d("700"), RankingMode::Effect).expect("score");
    assert!(score.reaches_threshold);
    assert_eq!(score.group_count, 2);

    let mut groups = vec![
        group(Some("H"), "1.00", "1.60"),
        group(Some("G"), "0.20", "0.30"),
    ];
    groups.extend((0..4).map(|i| group(Some(&format!("R{i}")), "0.50", "0.575")));
    let score =
        build_dimension_score(&groups, d("3.20"), d("4.20"), RankingMode::Effect).expect("score");
    assert!(score.reaches_threshold);
    assert_eq!(score.group_count, 2);
}

/// Gọi với Surprise khi S không tính được (một tổng bằng 0 cho p = 0 / 0, hoặc có nhóm âm) thì báo lỗi, không tính
/// trên NaN.
#[test]
fn surprise_without_a_distribution_is_an_error() {
    let zero_base = [group(Some("New"), "0", "50.00")];
    let score = build_dimension_score(&zero_base, d("0"), d("50.00"), RankingMode::Surprise);
    assert_eq!(score, Err(ScoreError::SurpriseUndefined));
    let negative = [
        group(Some("North"), "150", "100"),
        group(Some("South"), "-50", "-60"),
    ];
    let score = build_dimension_score(&negative, d("100"), d("40"), RankingMode::Surprise);
    assert_eq!(score, Err(ScoreError::SurpriseUndefined));
    // Cùng dữ liệu, cách xếp Effect vẫn cho điểm.
    assert!(build_dimension_score(&zero_base, d("0"), d("50.00"), RankingMode::Effect).is_ok());
}

/// Độ đo có số âm (lợi nhuận): không tính S, chiều xếp theo số nhóm cần rồi EP lớn nhất.
#[test]
fn negative_measure_ranks_by_group_count_then_effect() {
    let totals = ("100", "40");
    let region = vec![
        group(Some("North"), "150", "100"),
        group(Some("South"), "-50", "-60"),
    ];
    let channel = vec![
        group(Some("Web"), "70", "30"),
        group(Some("Store"), "30", "10"),
    ];
    let promo = vec![
        group(Some("Yes"), "130", "75"),
        group(Some("No"), "-30", "-35"),
    ];
    let all = [region.clone(), channel.clone(), promo.clone()];
    let mode = build_ranking_mode(&all, d(totals.0), d(totals.1));
    assert_eq!(mode, RankingMode::Effect);
    let mut dimensions = vec![
        ranked("Channel", &channel, totals, mode),
        ranked("Region", &region, totals, mode),
        ranked("Promo", &promo, totals, mode),
    ];
    apply_dimension_order(&mut dimensions, mode);
    // Promo và Region cùng một nhóm; Promo có EP lớn hơn (55/60 so với 50/60). Channel cần hai nhóm.
    assert_eq!(columns(&dimensions), ["Promo", "Region", "Channel"]);
    assert_eq!(dimensions[2].score.group_count, 2);
    assert!(dimensions
        .iter()
        .all(|dimension| dimension.score.surprise == 0.0));
}

/// Một tổng bằng 0 thì p_i không tính được: dùng Effect như độ đo âm.
#[test]
fn zero_total_uses_effect() {
    let groups = vec![group(Some("New"), "0", "50.00")];
    assert_eq!(
        build_ranking_mode(&[groups], d("0"), d("50.00")),
        RankingMode::Effect
    );
}

/// D = 0: không có gì để phân rã, câu hỏi nhận cảnh báo explanation_unavailable.
#[test]
fn no_change_is_an_error() {
    let groups = [
        group(Some("Europe"), "60.00", "70.00"),
        group(Some("LATAM"), "40.00", "30.00"),
    ];
    let score = build_dimension_score(&groups, d("100.00"), d("100.00"), RankingMode::Surprise);
    assert_eq!(score, Err(ScoreError::NoChange));
}

/// Chiều có tổng nhóm lệch giá trị hai lát cắt bị bỏ, kể cả khi tổng tràn khoảng Decimal (không panic).
#[test]
fn totals_mismatch_is_an_error() {
    let score = build_dimension_score(&market(), d("1000.00"), d("841.00"), RankingMode::Surprise);
    assert_eq!(score, Err(ScoreError::TotalsMismatch));
    let huge = [
        group(Some("a"), "1", "1"),
        GroupValues {
            value: None,
            base_value: Decimal::MAX,
            compare_value: Decimal::MAX,
        },
    ];
    let score = build_dimension_score(&huge, Decimal::MAX, Decimal::ONE, RankingMode::Effect);
    assert_eq!(score, Err(ScoreError::TotalsMismatch));
    let overflow = [GroupValues {
        value: None,
        base_value: Decimal::MIN,
        compare_value: Decimal::MAX,
    }];
    assert!(build_group_contributions(&overflow).is_none());
}

/// Chiều không đủ 0.67 đứng sau chiều đạt, kể cả khi tên đứng trước theo chữ cái và S lớn hơn (Brand: nhóm A có S cao
/// nhưng đi ngược chênh lệch, chỉ G với EP 0.625 được chọn). Cùng điểm thì theo tên cột, kể cả tên có dấu cách, dấu
/// nháy.
#[test]
fn dimensions_below_threshold_come_last_and_ties_go_by_name() {
    let mut brand = vec![
        group(Some("A"), "300", "600"),
        group(Some("G"), "200", "100"),
    ];
    brand.extend((0..25).map(|i| group(Some(&format!("g{i}")), "20", "5.6")));
    let product: Vec<GroupValues> = (0..20)
        .map(|i| group(Some(&format!("P{i}")), "50.00", "42.00"))
        .collect();
    let mode = RankingMode::Surprise;
    let mut dimensions = vec![
        ranked("Product", &product, TOTALS, mode),
        ranked("Brand", &brand, TOTALS, mode),
        ranked(r#"Order "Market""#, &market(), TOTALS, mode),
        ranked("Customer Market", &market(), TOTALS, mode),
    ];
    apply_dimension_order(&mut dimensions, mode);
    assert_eq!(
        columns(&dimensions),
        ["Customer Market", r#"Order "Market""#, "Brand", "Product"]
    );
    let (market, brand, product) = (
        &dimensions[0].score,
        &dimensions[2].score,
        &dimensions[3].score,
    );
    assert!(!brand.reaches_threshold);
    assert_eq!(brand.group_count, 1);
    assert!(brand.surprise > market.surprise);
    assert!(!product.reaches_threshold);
    assert_eq!(product.group_count, 0);
}

/// Không in "-0.00": hiệu bằng 0 và số 0 mang dấu âm đều in không dấu.
#[test]
fn zero_is_never_printed_negative() {
    let mut negative_zero = Decimal::new(0, 2);
    negative_zero.set_sign_negative(true);
    assert_eq!(build_decimal_text(negative_zero, 2), "0.00");
    let groups = [group(Some("Europe"), "-500.00", "-500.00")];
    let contributions = build_group_contributions(&groups).expect("contributions");
    assert_eq!(contributions[0].contribution, "0.00");
}

fn volume_price(v0: &str, v1: &str, m0: &str, m1: &str) -> VolumePriceNumbers {
    build_volume_price(d(v0), d(v1), d(m0), d(m1), PRICE_SCALE).expect("volume and price")
}

/// Số của mẫu why tính lại ra đúng mẫu (#40).
#[test]
fn why_example_volume_price_matches_the_code() {
    let numbers = volume_price("5000", "4400", "9402500.00", "8119100.00");
    assert_eq!(numbers.base_price, "1880.50");
    assert_eq!(numbers.compare_price, "1845.25");
    assert_eq!(numbers.volume_effect, "-1117725.00");
    assert_eq!(numbers.price_effect, "-165675.00");
    assert_eq!(numbers.if_volume_unchanged, "9226250.00");
    let example = build_why_example().explanation.expect("explanation");
    let split = example.volume_price.expect("volume_price");
    let from_example = VolumePriceNumbers {
        base_volume: split.base_volume,
        compare_volume: split.compare_volume,
        base_price: split.base_price,
        compare_price: split.compare_price,
        volume_effect: split.volume_effect,
        price_effect: split.price_effect,
        if_volume_unchanged: split.if_volume_unchanged,
    };
    assert_eq!(numbers, from_example);
}

/// Giá làm tròn, phần lệch do làm tròn dồn vào price_effect.
#[test]
fn rounded_price_moves_the_gap_into_price_effect() {
    let numbers = volume_price("3", "4", "10.00", "13.00");
    assert_eq!(numbers.base_price, "3.33");
    assert_eq!(numbers.compare_price, "3.25");
    assert_eq!(numbers.volume_effect, "3.29");
    assert_eq!(numbers.price_effect, "-0.29");
    assert_eq!(numbers.if_volume_unchanged, "9.75");
}

/// Chia 2 cho ra chữ số thứ ba: in đủ, không làm tròn thêm.
#[test]
fn volume_effect_keeps_a_third_decimal() {
    let numbers = volume_price("2", "3", "5.00", "7.47");
    assert_eq!(numbers.base_price, "2.50");
    assert_eq!(numbers.compare_price, "2.49");
    assert_eq!(numbers.volume_effect, "2.495");
    assert_eq!(numbers.price_effect, "-0.025");
    assert_eq!(numbers.if_volume_unchanged, "4.98");
}

/// Nửa xa số 0, cả với độ đo âm: 1.005 thành 1.01, -1.005 thành -1.01 (kiểu ngân hàng sẽ ra 1.00).
#[test]
fn price_rounds_half_away_from_zero() {
    let numbers = volume_price("2", "2", "2.01", "-2.01");
    assert_eq!(numbers.base_price, "1.01");
    assert_eq!(numbers.compare_price, "-1.01");
    assert_eq!(numbers.volume_effect, "0.00");
    assert_eq!(numbers.price_effect, "-4.02");
    assert_eq!(numbers.if_volume_unchanged, "-2.02");
}

/// Lượng không đổi: volume_effect là 0 (không in "-0.00" dù giá âm), cả chênh lệch là do giá.
#[test]
fn same_volume_puts_everything_in_price_effect() {
    let numbers = volume_price("10", "10", "-100.00", "-90.00");
    assert_eq!(numbers.base_price, "-10.00");
    assert_eq!(numbers.compare_price, "-9.00");
    assert_eq!(numbers.volume_effect, "0.00");
    assert_eq!(numbers.price_effect, "10.00");
    assert_eq!(numbers.if_volume_unchanged, "-90.00");
}

/// Lượng của một lát cắt bằng 0 thì không có giá: None, volume_price là null.
#[test]
fn zero_volume_has_no_split() {
    assert!(build_volume_price(d("0"), d("4"), d("0"), d("13.00"), PRICE_SCALE).is_none());
    assert!(build_volume_price(d("3"), d("0"), d("10.00"), d("0"), PRICE_SCALE).is_none());
}

/// Lưới số nhỏ, cả số âm và M = 0: ở mọi điểm hai phần cộng lại đúng bằng chênh lệch, volume_effect đúng công thức từ
/// giá đã làm tròn, giá là số gần M / V nhất (nửa thì xa số 0), và số 0 không in dấu trừ.
#[test]
fn small_grid_adds_up_everywhere() {
    let half_cent = d("0.005");
    // Bước 23 xu không đi qua 0, nên thêm 0 và hai số sát 0.
    let cents: Vec<i64> = (-250..=250).step_by(23).chain([0, 1, -1]).collect();
    for v0 in 1..=7 {
        for v1 in 1..=7 {
            for &m0 in &cents {
                for &m1 in &cents {
                    let (v0, v1) = (Decimal::from(v0), Decimal::from(v1));
                    let (m0, m1) = (Decimal::new(m0, 2), Decimal::new(m1, 2));
                    let numbers =
                        build_volume_price(v0, v1, m0, m1, PRICE_SCALE).expect("volume and price");
                    let point = format!("V0={v0} V1={v1} M0={m0} M1={m1}");
                    let (p0, p1) = (d(&numbers.base_price), d(&numbers.compare_price));
                    let volume_effect = d(&numbers.volume_effect);
                    assert_eq!(volume_effect + d(&numbers.price_effect), m1 - m0, "{point}");
                    assert_eq!(
                        volume_effect * Decimal::TWO,
                        (v1 - v0) * (p0 + p1),
                        "{point}"
                    );
                    for (price, measure, volume) in [(p0, m0, v0), (p1, m1, v1)] {
                        // |P * V - M| <= 0.005 * V; đúng bằng nửa thì P phải xa số 0 hơn M / V.
                        let gap = price * volume - measure;
                        assert!(gap.abs() <= half_cent * volume, "{point}");
                        if gap.abs() == half_cent * volume {
                            assert!((price * volume).abs() > measure.abs(), "{point}");
                        }
                    }
                    let texts = [
                        &numbers.volume_effect,
                        &numbers.price_effect,
                        &numbers.if_volume_unchanged,
                    ];
                    for text in texts {
                        assert!(
                            !(text.starts_with('-') && d(text).is_zero()),
                            "{point}: {text}"
                        );
                    }
                }
            }
        }
    }
}
