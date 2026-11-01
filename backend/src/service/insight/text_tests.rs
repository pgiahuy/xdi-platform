// SPDX-License-Identifier: Apache-2.0
/* service/insight/text_tests.rs - test câu do code viết cho các lỗi gặp khi chạy Gemini thật trên database mẫu (01/11):
tỷ lệ viết "0,95" thay cho "95%" và bị nói "các nhóm khá đều nhau" (insight/facts.rs, insight/rate.rs), bảng theo hai
cột nhóm chỉ được nói "Có 150 dòng kết quả" (insight/matrix.rs), chuỗi thời gian không nêu tổng các kỳ. Hàm thuần. */

use rust_decimal::Decimal;
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::service::explain::check_tests::column;
use crate::service::insight::facts::{
    build_group_parts, build_series_parts, build_top_parts, MeasureText, Point, ValueStyle,
};
use crate::service::insight::matrix::{build_matrix_parts, Cell};
use crate::service::insight::narrate::build_insight_check;
use crate::service::insight::plan::PlainTable;
use crate::service::insight::rate::build_value_style;

fn point(label: &str, value: &str) -> Point {
    Point {
        label: label.to_string(),
        value: value.parse().unwrap(),
    }
}

fn rate(label: &str) -> MeasureText<'_> {
    MeasureText {
        label,
        additive: false,
        style: ValueStyle::Rate,
    }
}

fn average(label: &str) -> MeasureText<'_> {
    MeasureText {
        label,
        additive: false,
        style: ValueStyle::Number,
    }
}

/// Tỷ lệ giao trễ theo cách giao: 95,3% so với 38,6% không phải "khá đều nhau" dù không nhóm nào gấp đôi trung bình.
#[test]
fn rate_groups_are_percents_with_the_highest_and_lowest() {
    let groups = [
        point("Second Class", "0.7672"),
        point("First Class", "0.9527"),
        point("Standard Class", "0.3861"),
        point("Same Day", "0.4615"),
    ];
    let parts = build_group_parts(rate("tỷ lệ giao trễ"), "cách giao", &groups, None);
    assert!(parts.notable);
    assert_eq!(parts.title, "First Class đứng đầu với 95,3%.");
    assert_eq!(parts.points.len(), 1);
    assert_eq!(parts.points[0].lead, "Cao nhất và thấp nhất");
    assert_eq!(
        parts.points[0].text,
        "First Class (95,3%) cao hơn Standard Class (38,6%) 56,7 điểm phần trăm."
    );
    // Lời của mô hình chép đúng các số đó thì qua kiểm số.
    let text = "First Class có tỷ lệ giao trễ cao nhất với 95,3%, cao hơn Standard Class (38,6%) 56,7 điểm phần trăm.";
    assert_eq!(build_insight_check(text, &parts), Ok(()));
}

/// Trung bình chênh dưới 10% của nhóm thấp nhất thì không có gì nổi bật; chênh nhiều hơn thì nêu cao nhất, thấp nhất.
#[test]
fn averages_are_even_only_when_the_gap_is_small() {
    let close = [
        point("USCA", "65.78"),
        point("Africa", "65.41"),
        point("Europe", "63.01"),
    ];
    let parts = build_group_parts(average("lợi nhuận trung bình"), "thị trường", &close, None);
    assert!(!parts.notable);
    assert_eq!(parts.points[0].lead, "Không có nhóm nào nổi bật");
    assert!(!parts.points[0].text.contains("lệch hẳn mức trung bình"));

    let apart = [
        point("USCA", "65.78"),
        point("Europe", "63.01"),
        point("Pacific Asia", "48.8"),
    ];
    let parts = build_group_parts(average("lợi nhuận trung bình"), "thị trường", &apart, None);
    assert!(parts.notable);
    assert_eq!(
        parts.points[0].text,
        "USCA (65,78) cao hơn Pacific Asia (48,8) 16,98."
    );
}

/// Top N của một tỷ lệ: chênh giữa hạng đầu và hạng kế bằng điểm phần trăm, không kèm phần trăm của chênh lệch.
#[test]
fn top_n_of_a_rate_gives_the_gap_in_points() {
    let shown = [point("Scotland", "0.3712"), point("Wales", "0.3523")];
    let parts = build_top_parts(rate("tỷ lệ rút học"), &shown, Some((Decimal::ONE, 13)));
    assert_eq!(parts.title, "Scotland đứng đầu với 37,1%.");
    assert_eq!(
        parts.points[0].text,
        "Scotland cao hơn Wales 1,9 điểm phần trăm."
    );
}

fn parse(sql: &str) -> sqlparser::ast::Expr {
    Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_expr())
        .unwrap()
}

/// Ba dạng tỷ lệ mô hình đã viết trên database mẫu, và hai dạng không phải tỷ lệ.
#[test]
fn rate_is_read_from_the_sql_and_the_values() {
    let mut late = column(1, "late_delivery_risk", "BOOLEAN", "dimension", 2);
    late.min_value = Some("false".to_string());
    let mut result = column(2, "final_result", "VARCHAR", "dimension", 4);
    result.min_value = None;
    let mut score = column(3, "score", "DOUBLE", "measure", 100);
    score.min_value = Some("0".to_string());
    let mut margin = column(4, "profit_ratio", "DOUBLE", "measure", 100);
    margin.unit = Some("percent".to_string());
    let columns = [late, result, score, margin];
    let tables = [PlainTable {
        table: "t",
        columns: &columns,
        row_count: None,
    }];
    let shares = ["0.95".parse::<Decimal>().unwrap(), "0.39".parse().unwrap()];
    let style =
        |sql: &str, values: &[Decimal]| build_value_style(Some(&parse(sql)), &tables, values);
    assert_eq!(
        style(
            r#"AVG(CAST(orders."late_delivery_risk" AS DOUBLE))"#,
            &shares
        ),
        ValueStyle::Rate
    );
    assert_eq!(
        style(
            r#"CAST(SUM(CASE WHEN students."final_result" = 'Withdrawn' THEN 1 ELSE 0 END) AS DOUBLE) / COUNT(students."id_student")"#,
            &shares
        ),
        ValueStyle::Rate
    );
    assert_eq!(
        style(
            r#"ROUND(COUNT(CASE WHEN "late_delivery_risk" = 'true' THEN 1 END) / NULLIF(COUNT(*), 0), 4)"#,
            &shares
        ),
        ValueStyle::Rate
    );
    let margins = ["-0.05".parse::<Decimal>().unwrap(), "0.12".parse().unwrap()];
    assert_eq!(style(r#"AVG("profit_ratio")"#, &margins), ValueStyle::Rate);
    // count_if chia count (Gemini viết cho "tỷ lệ rút học theo vùng").
    assert_eq!(
        style(
            r#"CAST(count_if("final_result" = 'Withdrawn') AS DOUBLE) / count(*)"#,
            &shares
        ),
        ValueStyle::Rate
    );
    // Trung bình điểm không phải tỷ lệ.
    assert_eq!(style(r#"AVG("score")"#, &shares), ValueStyle::Number);
    // Tỷ lệ đã nhân 100 ("Tỷ lệ giao trễ (%)"): giá trị là phần trăm.
    let hundred = [
        "95.27".parse::<Decimal>().unwrap(),
        "38.61".parse().unwrap(),
    ];
    assert_eq!(
        style(
            r#"CAST(SUM(CASE WHEN T1."delivery_status" = 'Late delivery' THEN 1 ELSE 0 END) AS DOUBLE) * 100 / COUNT(*)"#,
            &hundred
        ),
        ValueStyle::Percent
    );
    assert_eq!(
        style(
            r#"100.0 * COUNT(CASE WHEN "late_delivery_risk" THEN 1 END) / COUNT(*)"#,
            &hundred
        ),
        ValueStyle::Percent
    );
    // Dạng Gemini viết cho "tỷ lệ rút học theo vùng" lần chạy sau.
    assert_eq!(
        style(
            r#"(1.0 * COUNT_IF("final_result" = 'Withdrawn') / COUNT(*)) * 100"#,
            &hundred
        ),
        ValueStyle::Percent
    );
    assert_eq!(style(r#"AVG("score") * 100"#, &hundred), ValueStyle::Number);
    let groups = [
        point("First Class", "95.27"),
        point("Standard Class", "38.61"),
    ];
    let percent = MeasureText {
        label: "tỷ lệ giao trễ",
        additive: false,
        style: ValueStyle::Percent,
    };
    let parts = build_group_parts(percent, "cách giao", &groups, None);
    assert_eq!(
        parts.points[0].text,
        "First Class (95,3%) cao hơn Standard Class (38,6%) 56,7 điểm phần trăm."
    );
}

fn cell(first: &str, second: &str, value: i64) -> Cell {
    Cell {
        first: first.to_string(),
        second: second.to_string(),
        value: Decimal::from(value),
    }
}

/// Doanh thu theo danh mục và phân khúc khách hàng: tổ hợp đứng đầu, ba tổ hợp lớn nhất, nhóm đứng đầu từng cột.
#[test]
fn two_columns_give_top_combinations_and_each_leader() {
    let cells = [
        cell("Fishing", "Consumer", 400),
        cell("Fishing", "Corporate", 200),
        cell("Cleats", "Consumer", 300),
        cell("Cleats", "Corporate", 50),
        cell("Camping", "Consumer", 30),
        cell("Camping", "Corporate", 20),
    ];
    let parts = build_matrix_parts(
        MeasureText::total("Doanh thu"),
        ("Danh mục", "Phân khúc khách hàng"),
        &cells,
        true,
    );
    assert!(parts.notable);
    assert_eq!(
        parts.title,
        "Fishing, phân khúc khách hàng Consumer đứng đầu với 400, chiếm 40% tổng doanh thu."
    );
    let texts: Vec<String> = parts
        .points
        .iter()
        .map(|p| format!("{}: {}", p.lead, p.text))
        .collect();
    assert_eq!(
        texts,
        [
            "Các tổ hợp lớn nhất: Fishing, phân khúc khách hàng Consumer (400); Cleats, phân khúc khách hàng Consumer \
             (300); Fishing, phân khúc khách hàng Corporate (200): cùng chiếm 90% tổng.",
            "Đứng đầu theo danh mục: Fishing với 600, chiếm 60%.",
            "Đứng đầu theo phân khúc khách hàng: Consumer với 730, chiếm 73%.",
        ]
    );
    // Bảng bị cắt hay độ đo không cộng được: chỉ cao nhất và thấp nhất, không nói tỷ trọng.
    let parts = build_matrix_parts(average("Điểm trung bình"), ("Môn", "Kỳ"), &cells, true);
    assert_eq!(parts.title, "Fishing, kỳ Consumer cao nhất với 400.");
    assert_eq!(parts.points[0].text, "Camping, kỳ Corporate (20).");
}

/// Câu chung "tình hình doanh thu" trả lời theo tháng: chuỗi có đủ kỳ thì ý đầu là tổng các kỳ.
#[test]
fn complete_series_of_a_total_starts_with_the_sum() {
    let series = [
        point("Tháng 9/2017", "400"),
        point("Tháng 10/2017", "380"),
        point("Tháng 11/2017", "800"),
    ];
    let parts = build_series_parts(MeasureText::total("Doanh thu"), &series, None, None, true);
    assert_eq!(parts.points[0].lead, "Tổng các kỳ");
    assert_eq!(
        parts.points[0].text,
        "3 kỳ cộng lại, tổng doanh thu là 1.580."
    );
    // Bảng bị cắt hay độ đo không cộng được thì không có ý tổng.
    let parts = build_series_parts(MeasureText::total("Doanh thu"), &series, None, None, false);
    assert_ne!(parts.points[0].lead, "Tổng các kỳ");
    let parts = build_series_parts(
        rate("tỷ lệ giao trễ"),
        &[
            point("Tháng 10/2016", "0.5495"),
            point("Tháng 11/2016", "0.6519"),
        ],
        None,
        None,
        true,
    );
    assert_eq!(
        parts.points[0].text,
        "Từ Tháng 10/2016 tới Tháng 11/2016, tỷ lệ giao trễ tăng 10,2 điểm phần trăm."
    );
}
