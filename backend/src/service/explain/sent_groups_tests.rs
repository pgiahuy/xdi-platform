// SPDX-License-Identifier: Apache-2.0
/* service/explain/sent_groups_tests.rs - test explain/sent_groups.rs: mô hình chấm nhận đủ số của các nhóm mô hình viết đã
nhận. Hai bảng phân rã dựng lại từ hai câu thật bị chấm sai với Gemini: số đơn Năm 2016 so với Năm 2015 (sales_demo,
thành viên chỉ thấy Europe hay Puerto Rico) và số sinh viên 2014J so với trung bình các mã kỳ còn lại (edu_demo). Mô hình
viết chỉ khai cause, new, gone mà vẫn viết số của nhóm trong lời; giờ các số đó và tên bên có, bên không có của nhóm chỉ
ở một bên đều có trong JSON gửi mô hình chấm. Hàm thuần, không cần DuckDB hay Postgres. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{
    CellValue, Comparison, DimensionBreakdown, Explanation, ExplanationSlice, GroupContribution,
    MeasureKind, ValueKind,
};
use crate::service::explain::annotate::{
    annotate_dimensions, apply_group_notes, build_value_presences, Totals,
};
use crate::service::explain::claims::{Claim, ClaimField};
use crate::service::explain::judge::{build_judge_facts, build_judge_json};
use crate::service::explain::sent_groups::build_group_claims;

/// Một chiều: cột, nhãn, các nhóm (giá trị, bên gốc, bên được so), đã xếp theo phần chênh lệch như bước explain.
type Dimension<'a> = (&'a str, &'a str, Vec<(CellValue, &'a str, &'a str)>);

fn build_explanation(
    comparison: Comparison,
    sides: [(&str, &str); 2],
    dimensions: Vec<Dimension>,
) -> Explanation {
    let [(base_label, base_value), (compare_label, compare_value)] = sides;
    let base: Decimal = base_value.parse().unwrap();
    let compare: Decimal = compare_value.parse().unwrap();
    let slice = |label: &str, value: &str| ExplanationSlice {
        label: label.to_string(),
        filter: String::new(),
        value: value.to_string(),
        rows: "0".to_string(),
    };
    let mut dimensions: Vec<DimensionBreakdown> = dimensions
        .into_iter()
        .map(|(column, label, groups)| DimensionBreakdown {
            column: column.to_string(),
            label: Some(label.to_string()),
            kind: ValueKind::Text,
            sql: String::new(),
            groups: groups
                .into_iter()
                .map(|(value, base, compare)| GroupContribution {
                    value: Some(value),
                    base_value: base.to_string(),
                    compare_value: compare.to_string(),
                    contribution: (compare.parse::<Decimal>().unwrap()
                        - base.parse::<Decimal>().unwrap())
                    .to_string(),
                    ..Default::default()
                })
                .collect(),
            mix_effect: None,
            rate_effect: None,
            has_outliers: false,
        })
        .collect();
    let totals = Totals {
        base,
        compare,
        change: compare - base,
    };
    let mut presences: Vec<_> = dimensions.iter().map(build_value_presences).collect();
    apply_group_notes(&mut dimensions, &mut presences, totals);
    let notes = annotate_dimensions(&dimensions, &presences, totals);
    Explanation {
        measure: "Số".to_string(),
        measure_kind: MeasureKind::Total,
        comparison,
        counts_rows: false,
        slice_column: None,
        slice_column_label: None,
        base: slice(base_label, base_value),
        compare: slice(compare_label, compare_value),
        change: (compare - base).to_string(),
        percent_change: None,
        dimensions,
        volume_price: None,
        summary: String::new(),
        summary_title: String::new(),
        summary_points: Vec::new(),
        summary_by_model: false,
        summary_warnings: Vec::new(),
        summary_verdict: None,
        summary_failed_check: None,
        dropped_dimensions: Vec::new(),
        notes,
    }
}

fn text(value: &str) -> CellValue {
    CellValue::String(value.to_string())
}

fn claim(field: ClaimField, column: &str, group: &str) -> Claim {
    Claim {
        field,
        column: column.to_string(),
        group: group.to_string(),
        value: String::new(),
    }
}

/// Số đơn Năm 2016 so với Năm 2015 với vai dữ liệu Europe hay Puerto Rico: thị trường và vùng giao hàng.
fn orders_by_year() -> Explanation {
    build_explanation(
        Comparison::Periods,
        [("Năm 2015", "13133"), ("Năm 2016", "8690")],
        vec![
            (
                "order_region",
                "vùng giao hàng",
                vec![
                    (text("Western Europe"), "4828", "51"),
                    (text("Central America"), "1778", "0"),
                    (text("Northern Europe"), "1801", "33"),
                    (text("Eastern Europe"), "0", "1137"),
                    (text("Rest"), "4726", "7469"),
                ],
            ),
            (
                "market",
                "thị trường",
                vec![
                    (text("Europe"), "8315", "1274"),
                    (text("LATAM"), "3290", "0"),
                    (text("USCA"), "0", "3238"),
                    (text("Pacific Asia"), "1528", "2907"),
                    (text("Africa"), "0", "1271"),
                ],
            ),
        ],
    )
}

/// Các khẳng định gửi mô hình chấm, dạng (field, column, group, value) cho dễ so.
fn judged(e: &Explanation, written: &[Claim]) -> Vec<(String, String, String, String)> {
    build_judge_facts(e, written)
        .claims
        .iter()
        .map(|c| {
            (
                c.field.name().to_string(),
                c.column.clone(),
                c.group.clone(),
                c.value.clone(),
            )
        })
        .collect()
}

fn has(
    list: &[(String, String, String, String)],
    field: &str,
    column: &str,
    group: &str,
    value: &str,
) -> bool {
    list.iter()
        .any(|(f, c, g, v)| f == field && c == column && g == group && v == value)
}

#[test]
fn judge_sees_the_numbers_of_groups_named_without_numbers() {
    let e = orders_by_year();
    // Đúng các khẳng định Gemini khai ở lần chạy lại: chỉ cause, gone, new.
    let written = [
        claim(ClaimField::Cause, "order_region", "Western Europe"),
        claim(ClaimField::Cause, "market", "Europe"),
        claim(ClaimField::Gone, "order_region", "Central America"),
        claim(ClaimField::Gone, "market", "LATAM"),
        claim(ClaimField::New, "market", "USCA"),
    ];
    let list = judged(&e, &written);
    assert!(
        has(&list, "contribution", "market", "Europe", "-7.041"),
        "{list:?}"
    );
    assert!(has(
        &list,
        "contribution",
        "order_region",
        "Western Europe",
        "-4.777"
    ));
    assert!(has(&list, "base_value", "market", "LATAM", "3.290"));
    assert!(has(&list, "compare_value", "market", "LATAM", "0"));
    assert!(has(&list, "contribution", "market", "USCA", "3.238"));
    assert!(has(&list, "compare_value", "market", "USCA", "3.238"));
    assert!(has(
        &list,
        "base_value",
        "order_region",
        "Central America",
        "1.778"
    ));
    // Khẳng định của mô hình viết đứng đầu và không bị lặp lại.
    assert_eq!(
        list.len(),
        list.iter().collect::<std::collections::HashSet<_>>().len()
    );
    assert_eq!(
        list[0],
        (
            "cause".into(),
            "order_region".into(),
            "Western Europe".into(),
            String::new()
        )
    );
    let gone = list
        .iter()
        .filter(|c| c.0 == "gone" && c.2 == "LATAM")
        .count();
    assert_eq!(gone, 1);
}

#[test]
fn presence_claims_name_the_side_with_and_without_a_value() {
    let e = orders_by_year();
    let facts = build_judge_facts(&e, &[]);
    let json: serde_json::Value = serde_json::from_str(&build_judge_json(&facts)).unwrap();
    let find = |field: &str, group: &str| -> serde_json::Value {
        json["claims"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["field"] == field && c["group"] == group)
            .cloned()
            .unwrap_or_else(|| panic!("no {field} claim for {group}: {json}"))
    };
    for group in ["LATAM", "Central America"] {
        let gone = find("gone", group);
        assert_eq!(gone["has_value_in"], "Năm 2015");
        assert_eq!(gone["no_value_in"], "Năm 2016");
    }
    let new = find("new", "USCA");
    assert_eq!(new["has_value_in"], "Năm 2016");
    assert_eq!(new["no_value_in"], "Năm 2015");
    // Khẳng định không nói về có mặt thì không có hai khóa này.
    assert!(find("contribution", "Europe").get("has_value_in").is_none());
}

/// 2014J so với trung bình các mã kỳ còn lại theo năm và tháng bắt đầu (bảng trước khi luật chiều đi theo hai nhóm bỏ hai
/// chiều này, explain/dimension_pick.rs): mô hình viết nêu "năm 2014 cao hơn 8.225,67 (336,5%)" và "năm 2013 không có
/// sinh viên ở 2014J".
#[test]
fn judge_sees_group_numbers_against_the_rest_average() {
    let e = build_explanation(
        Comparison::Groups,
        [
            ("trung bình các mã kỳ còn lại", "6947.33"),
            ("2014J", "10670"),
        ],
        vec![
            (
                "courses.presentation_year",
                "năm",
                vec![
                    (CellValue::Number(2014.0), "2444.33", "10670"),
                    (CellValue::Number(2013.0), "4503", "0"),
                ],
            ),
            (
                "courses.presentation_start_month",
                "tháng bắt đầu",
                vec![
                    (CellValue::Number(10.0), "2943.33", "10670"),
                    (CellValue::Number(2.0), "4004", "0"),
                ],
            ),
        ],
    );
    let written = [
        claim(ClaimField::Cause, "courses.presentation_year", "2014"),
        claim(ClaimField::Gone, "courses.presentation_year", "2013"),
    ];
    let list = judged(&e, &written);
    let year = "courses.presentation_year";
    assert!(
        has(&list, "contribution", year, "2014", "8.225,67"),
        "{list:?}"
    );
    assert!(has(&list, "percent_change", year, "2014", "336,5"));
    assert!(has(&list, "base_value", year, "2014", "2.444,33"));
    assert!(has(&list, "base_value", year, "2013", "4.503"));
    let month = "courses.presentation_start_month";
    assert!(has(&list, "contribution", month, "10", "7.726,67"));
    assert!(has(&list, "percent_change", month, "10", "262,5"));
    let json = build_judge_json(&build_judge_facts(&e, &written));
    assert!(
        json.contains("\"has_value_in\": \"trung bình các mã kỳ còn lại\""),
        "{json}"
    );
    assert!(json.contains("\"no_value_in\": \"2014J\""), "{json}");
}

/// Nhóm ngoài các nhóm gửi mô hình viết mà một khẳng định nêu tên cũng có số; nhóm không ai nêu và không được gửi thì
/// không có.
#[test]
fn a_named_group_outside_the_sent_ones_gets_its_numbers_too() {
    let e = orders_by_year();
    let sent_only = build_group_claims(&e, &[]);
    assert!(!sent_only.iter().any(|c| c.group == "Africa"));
    assert!(!sent_only.iter().any(|c| c.group == "Pacific Asia"));
    let named = build_group_claims(
        &e,
        &[claim(ClaimField::Against, "thị trường", "Pacific Asia")],
    );
    assert!(named.iter().any(|c| c.field == ClaimField::Contribution
        && c.group == "Pacific Asia"
        && c.value == "1.379"));
    assert!(!named.iter().any(|c| c.group == "Africa"));
}
