// SPDX-License-Identifier: Apache-2.0
/* service/explain/premise.rs - kiểm tiền đề của câu "tại sao" (#164): câu hỏi nói một hướng ("vì sao giảm") mà chênh
lệch tính được ngược hướng hoặc bằng 0 thì thêm cảnh báo premise_mismatch. explain/run.rs gọi
build_premise_warning ngay khi có tổng hai lát cắt, trước bước phân rã. Phân rã và lời của mô hình vẫn như cũ: chúng
giải thích chênh lệch thực tế, cảnh báo chỉ báo người hỏi rằng điều họ giả định không đúng. Hàm thuần, test ở cuối file.

Hướng đọc bằng từ khóa trên câu đã bỏ dấu, theo từ trọn vẹn, như WHY_PHRASES của explain/form.rs. Câu có cả từ chỉ
tăng lẫn từ chỉ giảm, hay không có từ nào, thì không cảnh báo: thà bỏ sót còn hơn cảnh báo sai. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{Warning, WarningCode};
use crate::service::ask::phrases::fold;
use crate::service::explain::check::{PlanSlices, WhyPlan};
use crate::service::explain::number::{build_change_parts, MAX_FRACTION_DIGITS};
use crate::service::explain::ratio::RATE_SCALE;

/// Hướng người hỏi giả định.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskedDirection {
    Up,
    Down,
}

/// Từ chỉ giảm, đã bỏ dấu: "giảm", "sụt", "đi xuống", "thấp hơn", "ít hơn", "hụt".
const DOWN_WORDS: [&str; 12] = [
    "giam",
    "sut",
    "xuong",
    "thap hon",
    "it hon",
    "hut",
    "drop",
    "dropped",
    "decrease",
    "decreased",
    "fell",
    "lower",
];

/// Từ chỉ tăng, đã bỏ dấu: "tăng", "lên", "cao hơn", "nhiều hơn".
const UP_WORDS: [&str; 10] = [
    "tang",
    "len",
    "cao hon",
    "nhieu hon",
    "increase",
    "increased",
    "rise",
    "rose",
    "grew",
    "higher",
];

/// Cụm có chữ trùng với từ chỉ hướng sau khi bỏ dấu ("giám đốc" thành "giam doc", "quà tặng" thành "qua tang"). Bỏ các
/// cụm này trước khi tìm hướng, nếu không câu "vì sao doanh thu của giám đốc A cao" bị đọc là hỏi vì sao giảm.
const NOT_DIRECTION: [&str; 6] = [
    "giam doc",
    "giam sat",
    "giam dinh",
    "qua tang",
    "tang lop",
    "tang ham",
];

/// Hướng câu hỏi nói tới; None khi không nói, hay nói cả hai hướng.
pub fn build_asked_direction(question: &str) -> Option<AskedDirection> {
    // Dấu cách hai đầu để so theo ranh giới từ bằng contains, như build_question_kind.
    let mut words = format!(" {} ", fold(question));
    for phrase in NOT_DIRECTION {
        words = words.replace(&format!(" {phrase} "), " ");
    }
    let has = |list: &[&str]| list.iter().any(|w| words.contains(&format!(" {w} ")));
    match (has(&UP_WORDS), has(&DOWN_WORDS)) {
        (true, false) => Some(AskedDirection::Up),
        (false, true) => Some(AskedDirection::Down),
        _ => None,
    }
}

/// Nhãn hai lát cắt và độ đo cho lời cảnh báo, lấy từ WhyPlan bằng build_premise_labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremiseLabels {
    pub measure: String,
    pub base: String,
    pub compare: String,
    /// true khi so hai kỳ, false khi so hai nhóm.
    pub periods: bool,
    /// Số chữ số lẻ khi viết chênh lệch: tỷ lệ giữ RATE_SCALE chữ số, độ đo khác hai chữ số như bảng kết quả.
    pub fraction_digits: u32,
}

/// Nhãn như bảng phân rã: "Quý 2/2017" và "Quý 3/2017", hay hai giá trị nhóm.
pub fn build_premise_labels(plan: &WhyPlan) -> PremiseLabels {
    let (base, compare, periods) = match &plan.slices {
        PlanSlices::Periods { base, compare, .. } => (base.label(), compare.label(), true),
        PlanSlices::Groups { base, compare, .. } => (base.clone(), compare.clone(), false),
    };
    let fraction_digits = if plan.measure.denominator.is_some() {
        RATE_SCALE
    } else {
        MAX_FRACTION_DIGITS
    };
    PremiseLabels {
        measure: plan.measure.label.clone(),
        base,
        compare,
        periods,
        fraction_digits,
    }
}

/// Cảnh báo khi hướng câu hỏi khác chênh lệch compare - base. None khi khớp, khi câu không nói hướng, hay khi phép trừ
/// tràn (bước phân rã tự báo lỗi đó).
pub fn build_premise_warning(
    question: &str,
    labels: &PremiseLabels,
    base: Decimal,
    compare: Decimal,
) -> Option<Warning> {
    let asked = build_asked_direction(question)?;
    let change = compare.checked_sub(base)?;
    let matches = match asked {
        AskedDirection::Up => change > Decimal::ZERO,
        AskedDirection::Down => change < Decimal::ZERO,
    };
    if matches {
        return None;
    }
    Some(Warning {
        code: WarningCode::PremiseMismatch,
        message: build_premise_message(asked, labels, change),
    })
}

// "Bạn hỏi vì sao giảm, nhưng Doanh thu Quý 3/2017 tăng 5.107,10 so với Quý 2/2017." Hai nhóm thì nói "cao hơn",
// "thấp hơn", vì "Europe tăng so với LATAM" đọc không tự nhiên.
fn build_premise_message(asked: AskedDirection, labels: &PremiseLabels, change: Decimal) -> String {
    let PremiseLabels {
        measure,
        base,
        compare,
        periods,
        fraction_digits,
    } = labels;
    let asked_text = match (asked, periods) {
        (AskedDirection::Up, true) => "tăng",
        (AskedDirection::Down, true) => "giảm",
        (AskedDirection::Up, false) => "cao hơn",
        (AskedDirection::Down, false) => "thấp hơn",
    };
    if change.is_zero() {
        return format!(
            "Bạn hỏi vì sao {asked_text}, nhưng {measure} {compare} bằng {base}, không có chênh lệch."
        );
    }
    let (_, amount) = build_change_parts(&change.to_string(), *fraction_digits);
    let actual = match (change > Decimal::ZERO, periods) {
        (true, true) => "tăng",
        (false, true) => "giảm",
        (true, false) => "cao hơn",
        (false, false) => "thấp hơn",
    };
    format!(
        "Bạn hỏi vì sao {asked_text}, nhưng {measure} {compare} {actual} {amount} so với {base}."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_is_read_from_whole_words_without_accents() {
        for (question, expected) in [
            (
                "Tại sao doanh thu quý 3 năm 2017 giảm?",
                Some(AskedDirection::Down),
            ),
            ("vi sao doanh thu sut", Some(AskedDirection::Down)),
            ("Vì sao doanh thu đi xuống", Some(AskedDirection::Down)),
            ("Tại sao Europe thấp hơn LATAM", Some(AskedDirection::Down)),
            ("Why did revenue drop in Q3?", Some(AskedDirection::Down)),
            ("Vì sao doanh thu tháng 5 tăng", Some(AskedDirection::Up)),
            ("tai sao loi nhuan len", Some(AskedDirection::Up)),
            ("Tại sao Europe cao hơn LATAM", Some(AskedDirection::Up)),
            // Không nói hướng, hay nói cả hai: không đoán.
            ("Tại sao doanh thu quý 3 khác quý 2", None),
            ("Vì sao doanh thu tăng mà lợi nhuận giảm", None),
            // "giám đốc", "quà tặng" không phải từ chỉ hướng; "giảm giá" thì vẫn là "giảm".
            ("Vì sao doanh thu của giám đốc A cao", None),
            (
                "Tại sao doanh thu quà tặng thấp hơn quý trước",
                Some(AskedDirection::Down),
            ),
            // Theo từ trọn vẹn: "tanga", "giamb" không khớp.
            ("Tại sao tanga giamb", None),
        ] {
            assert_eq!(build_asked_direction(question), expected, "{question}");
        }
    }

    fn period_labels() -> PremiseLabels {
        PremiseLabels {
            measure: "Doanh thu".to_string(),
            base: "Quý 2/2017".to_string(),
            compare: "Quý 3/2017".to_string(),
            periods: true,
            fraction_digits: MAX_FRACTION_DIGITS,
        }
    }

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    #[test]
    fn asking_why_down_when_it_went_up_warns() {
        let warning = build_premise_warning(
            "Vì sao doanh thu giảm?",
            &period_labels(),
            d("100"),
            d("5207.10"),
        )
        .unwrap();
        assert_eq!(warning.code, WarningCode::PremiseMismatch);
        assert_eq!(
            warning.message,
            "Bạn hỏi vì sao giảm, nhưng Doanh thu Quý 3/2017 tăng 5.107,10 so với Quý 2/2017."
        );
    }

    #[test]
    fn asking_why_up_when_it_went_down_or_stayed_warns() {
        let plan = period_labels();
        let down =
            build_premise_warning("Vì sao doanh thu tăng?", &plan, d("300.5"), d("100")).unwrap();
        assert_eq!(
            down.message,
            "Bạn hỏi vì sao tăng, nhưng Doanh thu Quý 3/2017 giảm 200,5 so với Quý 2/2017."
        );
        let same =
            build_premise_warning("Vì sao doanh thu tăng?", &plan, d("100"), d("100")).unwrap();
        assert_eq!(
            same.message,
            "Bạn hỏi vì sao tăng, nhưng Doanh thu Quý 3/2017 bằng Quý 2/2017, không có chênh lệch."
        );
    }

    #[test]
    fn matching_or_unstated_direction_does_not_warn() {
        let plan = period_labels();
        assert!(
            build_premise_warning("Vì sao doanh thu giảm?", &plan, d("200"), d("100")).is_none()
        );
        assert!(
            build_premise_warning("Vì sao doanh thu tăng?", &plan, d("100"), d("200")).is_none()
        );
        assert!(build_premise_warning(
            "Vì sao doanh thu quý 3 như vậy?",
            &plan,
            d("100"),
            d("200")
        )
        .is_none());
    }

    #[test]
    fn groups_are_compared_with_higher_and_lower() {
        let plan = PremiseLabels {
            base: "LATAM".to_string(),
            compare: "Europe".to_string(),
            periods: false,
            ..period_labels()
        };
        let warning =
            build_premise_warning("Tại sao Europe thấp hơn LATAM", &plan, d("100"), d("150"))
                .unwrap();
        assert_eq!(
            warning.message,
            "Bạn hỏi vì sao thấp hơn, nhưng Doanh thu Europe cao hơn 50 so với LATAM."
        );
    }
}
