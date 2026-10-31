// SPDX-License-Identifier: Apache-2.0
/* service/explain/summary_tests.rs - test của lời tóm tắt do code viết (summary.rs) trên ba trường hợp có thật của
DataCo (#186): COMPLETE với CANCELED (cột trùng hai nhóm bị bỏ, chênh lệch do số dòng), Consumer với Corporate (dàn
đều, nhóm lớn nhất chênh nhiều nhất), tháng 5 với tháng 4 (bang mới có đơn, nhóm khác thường). Explanation dựng tay
rồi qua đúng annotate_dimensions như bước explain; lời nào cũng phải qua bước kiểm số của narrate. */

use rust_decimal::Decimal;

use crate::service::ask::contract::{
    CellValue, Comparison, DimensionBreakdown, Explanation, ExplanationSlice, MeasureKind,
    SummaryPointKind, ValueKind,
};
use crate::service::ask::examples::build_why_example;
use crate::service::explain::annotate::{
    annotate_dimensions, apply_group_notes, build_percent_change, build_percent_text,
    build_value_presences, Totals,
};
use crate::service::explain::summary::{build_summary_parts, build_template_summary};
use crate::service::explain::{build_group_contributions, build_number_check, GroupValues};

/// Một chiều: tên cột và các nhóm (giá trị, lát gốc, lát so sánh).
type Dim<'a> = (&'a str, Vec<(&'a str, &'a str, &'a str)>);

struct Case<'a> {
    measure: &'a str,
    comparison: Comparison,
    slice_column: Option<(&'a str, &'a str)>,
    base: (&'a str, &'a str, &'a str),
    compare: (&'a str, &'a str, &'a str),
    dimensions: Vec<Dim<'a>>,
    labels: &'a [(&'a str, &'a str)],
}

fn d(text: &str) -> Decimal {
    text.parse().expect("decimal")
}

/// Explanation như bước explain dựng: chiều tính từ giá trị hai lát cắt, rồi annotate_dimensions.
fn build(case: Case) -> Explanation {
    let (base_total, compare_total) = (d(case.base.1), d(case.compare.1));
    let change = compare_total - base_total;
    let dimensions: Vec<DimensionBreakdown> = case
        .dimensions
        .iter()
        .map(|(column, groups)| {
            // Cột mã số (Department Id) là cột số như DuckDB trả, để luật giữ cột chữ khi trùng có tác dụng.
            let numeric = groups
                .iter()
                .all(|(value, _, _)| value.parse::<f64>().is_ok());
            let values: Vec<GroupValues> = groups
                .iter()
                .map(|(value, base, compare)| GroupValues {
                    value: Some(if numeric {
                        CellValue::Number(value.parse().expect("number"))
                    } else {
                        CellValue::String(value.to_string())
                    }),
                    base_value: d(base),
                    compare_value: d(compare),
                })
                .collect();
            DimensionBreakdown {
                column: column.to_string(),
                label: case
                    .labels
                    .iter()
                    .find(|(c, _)| c == column)
                    .map(|(_, l)| l.to_string()),
                kind: if numeric {
                    ValueKind::Number
                } else {
                    ValueKind::Text
                },
                sql: String::new(),
                groups: build_group_contributions(&values).expect("contributions"),
                mix_effect: None,
                rate_effect: None,
                has_outliers: false,
            }
        })
        .collect();
    let totals = Totals {
        base: base_total,
        compare: compare_total,
        change,
    };
    let mut dimensions = dimensions;
    let mut presences: Vec<_> = dimensions.iter().map(build_value_presences).collect();
    // Như bước explain thật (breakdown.rs): ghi chú vào bảng và xếp chiều trước, rồi mới đọc ghi chú cho lời tóm tắt.
    apply_group_notes(&mut dimensions, &mut presences, totals);
    let notes = annotate_dimensions(&dimensions, &presences, totals);
    let slice = |(label, value, rows): (&str, &str, &str)| ExplanationSlice {
        label: label.to_string(),
        filter: String::new(),
        value: value.to_string(),
        rows: rows.to_string(),
    };
    Explanation {
        measure: case.measure.to_string(),
        measure_kind: MeasureKind::Total,
        comparison: case.comparison,
        counts_rows: false,
        slice_column: case.slice_column.map(|(c, _)| c.to_string()),
        slice_column_label: case.slice_column.map(|(_, l)| l.to_string()),
        base: slice(case.base),
        compare: slice(case.compare),
        change: change.to_string(),
        percent_change: build_percent_change(totals).map(build_percent_text),
        dimensions,
        volume_price: None,
        summary: String::new(),
        summary_title: String::new(),
        summary_points: Vec::new(),
        summary_by_model: false,
        summary_warnings: Vec::new(),
        summary_verdict: None,
        dropped_dimensions: notes.dropped.clone(),
        notes,
    }
}

/// COMPLETE với CANCELED (Benefit per order): mọi đơn COMPLETE trả DEBIT và giao xong, mọi đơn CANCELED trả TRANSFER và
/// "Shipping canceled". Số dòng mỗi bên đặt sao cho trung bình mỗi dòng gần nhau.
fn complete_canceled() -> Explanation {
    build(Case {
        measure: "Tổng Benefit per order",
        comparison: Comparison::Groups,
        slice_column: Some(("Order Status", "trạng thái đơn hàng")),
        base: ("COMPLETE", "1321735.75", "1450"),
        compare: ("CANCELED", "75345.63", "82"),
        dimensions: vec![
            (
                "Type",
                vec![("DEBIT", "1321735.75", "0"), ("TRANSFER", "0", "75345.63")],
            ),
            (
                "Customer Country",
                vec![
                    ("EE. UU.", "830000.00", "47451.70"),
                    ("Puerto Rico", "491735.75", "27893.93"),
                ],
            ),
            (
                "Delivery Status",
                vec![
                    ("Late delivery", "741421.11", "0"),
                    ("Advance shipping", "318714.58", "0"),
                    ("Shipping on time", "261600.06", "0"),
                    ("Shipping canceled", "0", "75345.63"),
                ],
            ),
            (
                "Customer Segment",
                vec![
                    ("Consumer", "690000.00", "41495.58"),
                    ("Corporate", "410000.00", "23045.71"),
                    ("Home Office", "221735.75", "10804.34"),
                ],
            ),
        ],
        labels: &[
            ("Type", "hình thức thanh toán"),
            ("Delivery Status", "trạng thái giao hàng"),
            ("Customer Country", "quốc gia khách hàng"),
            ("Customer Segment", "phân khúc khách hàng"),
        ],
    })
}

/// Consumer với Corporate (Sales): Corporate thấp hơn khoảng như nhau ở mọi phòng ban; Department Id trùng Name.
fn consumer_corporate() -> Explanation {
    let departments = vec![
        ("Fan Shop", "8869203.67", "5216292.76"),
        ("Apparel", "4130801.34", "2408676.13"),
        ("Golf", "2388833.71", "1405012.47"),
        ("Footwear", "2103444.82", "1197929.23"),
    ];
    let ids = vec![
        ("7", "8869203.67", "5216292.76"),
        ("4", "4130801.34", "2408676.13"),
        ("5", "2388833.71", "1405012.47"),
        ("3", "2103444.82", "1197929.23"),
    ];
    build(Case {
        measure: "Tổng Sales",
        comparison: Comparison::Groups,
        slice_column: Some(("Customer Segment", "phân khúc khách hàng")),
        base: ("Consumer", "17492283.54", "9000"),
        compare: ("Corporate", "10227910.59", "6400"),
        dimensions: vec![("Department Id", ids), ("Department Name", departments)],
        labels: &[
            ("Department Name", "bộ phận"),
            ("Department Id", "mã bộ phận"),
        ],
    })
}

/// Tháng 5 với tháng 4 (doanh thu): vài bang tháng 4 chưa có doanh thu, PR giảm.
fn may_april() -> Explanation {
    build(Case {
        measure: "Doanh thu",
        comparison: Comparison::Periods,
        slice_column: None,
        base: ("Tháng 4/2017", "2580.45", "6"),
        compare: ("Tháng 5/2017", "3928.55", "11"),
        dimensions: vec![(
            "Customer State",
            vec![
                ("CA", "771.12", "1319.99"),
                ("IL", "0", "524.96"),
                ("PR", "1809.33", "1323.69"),
                ("AZ", "0", "387.96"),
                ("VA", "0", "371.95"),
            ],
        )],
        labels: &[("Customer State", "bang khách hàng")],
    })
}

#[test]
fn complete_canceled_drops_the_columns_that_rename_the_groups() {
    let explanation = complete_canceled();
    let template = build_template_summary(&explanation);
    assert_eq!(
        template,
        "Tổng Benefit per order của trạng thái đơn hàng CANCELED thấp hơn COMPLETE 1.246.390,12 (75.345,63 so với \
         1.321.735,75). Không nhóm nào khác thường: theo quốc gia khách hàng và phân khúc khách hàng, không nhóm nào \
         thay đổi khác hẳn mức chung (thấp hơn 94,3%). Bỏ qua hình thức thanh toán và trạng thái giao hàng: theo \
         hình thức thanh toán, COMPLETE chỉ có DEBIT, CANCELED chỉ có TRANSFER; theo trạng thái giao hàng, COMPLETE \
         chỉ có Late delivery, Advance shipping và Shipping on time, CANCELED chỉ có Shipping canceled. Các cột này \
         chỉ là tên khác của hai nhóm đang so, không giải thích được gì."
    );
    build_number_check(&template, &explanation).unwrap();
}

/// Lời tóm tắt bỏ qua Type và Delivery Status, nhưng annotate không bỏ chiều nào khỏi bảng phân rã đã dựng: bỏ qua chỉ
/// nằm trong ghi chú nội bộ và `dropped_dimensions`. Trên dữ liệu thật, explain/dimension_pick.rs bỏ hai chiều này khỏi
/// bảng trước bước annotate và đưa chúng vào ghi chú (dimension_pick_tests.rs). Hai chiều này chỉ có nhóm một bên nên
/// có nhóm khác thường và đứng trước hai chiều dàn đều.
#[test]
fn the_breakdown_keeps_the_dimensions_the_summary_skips() {
    let explanation = complete_canceled();
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert_eq!(
        columns,
        [
            "Type",
            "Delivery Status",
            "Customer Country",
            "Customer Segment"
        ]
    );
    let skipped: Vec<&str> = explanation
        .notes
        .dropped
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert_eq!(skipped, ["Type", "Delivery Status"]);
    assert_eq!(
        explanation.dimensions[0].label.as_deref(),
        Some("hình thức thanh toán")
    );
}

#[test]
fn consumer_corporate_is_even_and_the_largest_groups_lead() {
    let explanation = consumer_corporate();
    let (title, points) = build_summary_parts(&explanation);
    assert!(title.starts_with("Tổng Sales của phân khúc khách hàng Corporate thấp hơn Consumer"));
    let kinds: Vec<SummaryPointKind> = points.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, [SummaryPointKind::Even, SummaryPointKind::Dropped]);
    assert_eq!(
        points[0].text,
        "theo bộ phận, không nhóm nào thay đổi khác hẳn mức chung (thấp hơn 41,5%). Chênh lệch lớn nhất ở Fan Shop \
         và Apparel chỉ vì đây là những nhóm có tổng Sales lớn nhất."
    );
    assert_eq!(points[1].text, "Mã bộ phận chia nhóm giống hệt bộ phận.");
    build_number_check(&build_template_summary(&explanation), &explanation).unwrap();
}

#[test]
fn may_april_names_the_new_states_and_the_one_that_fell() {
    let explanation = may_april();
    let (title, points) = build_summary_parts(&explanation);
    assert_eq!(
        title,
        "Doanh thu Tháng 5/2017 tăng 1.348,10 so với Tháng 4/2017, từ 2.580,45 lên 3.928,55."
    );
    // Không có lượng (số đơn) nên không có ý "Lý do chính": các nhóm khác thường đứng đầu.
    assert_eq!(points[0].kind, SummaryPointKind::Outliers);
    assert_eq!(points[0].lead, "Khác thường theo bang khách hàng");
    assert_eq!(
        points[0].text,
        "CA tăng 71,2% trong khi mức chung tăng 52,2%, chiếm 40,7% chênh lệch; IL mới có ở Tháng 5/2017 (tăng \
         524,96), chiếm 38,9% chênh lệch; PR giảm 26,8% trong khi mức chung tăng 52,2%, kéo ngược 36% chênh lệch."
    );
    build_number_check(&build_template_summary(&explanation), &explanation).unwrap();
}

/// Không còn số liệu để so với mức chung (lát gốc âm): vẫn nêu nhóm góp nhiều nhất cùng chiều.
#[test]
fn a_negative_base_falls_back_to_the_largest_group() {
    let explanation = build(Case {
        measure: "Tổng lợi nhuận",
        comparison: Comparison::Periods,
        slice_column: None,
        base: ("Quý 2/2017", "-100", "10"),
        compare: ("Quý 3/2017", "350", "12"),
        dimensions: vec![(
            "Market",
            vec![("Europe", "-300", "-50"), ("LATAM", "200", "400")],
        )],
        labels: &[],
    });
    let (_, points) = build_summary_parts(&explanation);
    assert_eq!(points[0].kind, SummaryPointKind::Largest);
    assert_eq!(
        points[0].text,
        "theo Market, riêng nhóm Europe, tổng lợi nhuận tăng 250."
    );
}

/// Nhóm đổi dấu (lỗ thành lãi) không có % thay đổi nhưng là nhóm khác thường, và lời nói rõ hai giá trị.
#[test]
fn a_sign_flip_is_an_outlier() {
    let explanation = build(Case {
        measure: "Tổng lợi nhuận",
        comparison: Comparison::Periods,
        slice_column: None,
        base: ("Quý 2/2017", "-100", "10"),
        compare: ("Quý 3/2017", "350", "12"),
        dimensions: vec![(
            "Market",
            vec![("Europe", "-300", "100"), ("LATAM", "200", "250")],
        )],
        labels: &[],
    });
    let europe = &explanation.dimensions[0].groups[0];
    assert!(europe.sign_flip && europe.outlier);
    assert_eq!(europe.percent_change, None);
    let (_, points) = build_summary_parts(&explanation);
    assert_eq!(points[0].kind, SummaryPointKind::Outliers);
    assert_eq!(
        points[0].text,
        "Europe đổi dấu, từ -300 ở Quý 2/2017 sang 100 ở Quý 3/2017 (tăng 400), chiếm 88,9% chênh lệch."
    );
    build_number_check(&build_template_summary(&explanation), &explanation).unwrap();
}

/// Một nhóm đi ngược chiều mà không khác thường (chiều không có nhóm khác thường) thì có ý "Ngược chiều".
#[test]
fn a_group_against_the_change_is_named() {
    let explanation = build(Case {
        measure: "Doanh thu",
        comparison: Comparison::Periods,
        slice_column: None,
        base: ("Quý 2/2017", "10000", "100"),
        compare: ("Quý 3/2017", "10500", "100"),
        dimensions: vec![(
            "Market",
            vec![("Europe", "5000", "5600"), ("LATAM", "5000", "4900")],
        )],
        labels: &[],
    });
    let (_, points) = build_summary_parts(&explanation);
    let against = points
        .iter()
        .find(|p| p.kind == SummaryPointKind::Against)
        .expect("against point");
    assert_eq!(against.text, "theo Market, LATAM giảm 100.");
}

/// Quy mô là lý do chính: ý "Lý do chính" nêu số đơn và trung bình mỗi đơn của hai bên, để thấy hai mức đó có gần nhau.
#[test]
fn a_scale_reason_names_both_counts_and_averages() {
    let explanation = build_why_example().explanation.expect("why example");
    let (_, points) = build_summary_parts(&explanation);
    let reason = points
        .iter()
        .find(|p| p.kind == SummaryPointKind::Reason)
        .expect("reason");
    assert!(
        reason.text.starts_with(
            "chủ yếu do số đơn (Quý 2/2017 có 5.000, Quý 3/2017 có 4.400; giá trị đơn trung bình 1.880,50 và \
             1.845,25)."
        ),
        "{}",
        reason.text
    );
    build_number_check(&build_template_summary(&explanation), &explanation).unwrap();
}

/// Độ đo đếm không có ý "do quy mô": nó chính là quy mô.
#[test]
fn a_count_measure_has_no_scale_reason() {
    let mut explanation = build_why_example().explanation.expect("why example");
    explanation.counts_rows = true;
    let (_, points) = build_summary_parts(&explanation);
    assert!(points.iter().all(|p| p.kind != SummaryPointKind::Reason));
}
