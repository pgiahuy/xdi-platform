// SPDX-License-Identifier: Apache-2.0
/* service/ask_chart_tests.rs - test luật gợi ý biểu đồ (ask_chart.rs): mỗi nhánh có một ca bắt đúng và ca gần giống
mà không được bắt. Kết quả và vai trò cột dựng tay, không cần DuckDB hay Postgres. Phần đầu là câu không đọc được vai
trò (mọi cột other), luật chỉ theo kiểu cột; phần sau là câu có vai trò từ build_column_roles. */

use super::ask_chart::build_chart;
use super::ask_contract::{
    CellValue, ChartKind, ChartSuggestion, ColumnRole, ResultColumn, ResultTable, ValueKind,
};

fn text(value: &str) -> Option<CellValue> {
    Some(CellValue::String(value.to_string()))
}

fn number(value: f64) -> Option<CellValue> {
    Some(CellValue::Number(value))
}

/// Kết quả đủ dòng (không bị cắt) từ danh sách cột (tên, kiểu) và các dòng.
fn table(columns: &[(&str, ValueKind)], rows: Vec<Vec<Option<CellValue>>>) -> ResultTable {
    ResultTable {
        columns: columns
            .iter()
            .map(|(name, kind)| ResultColumn {
                name: name.to_string(),
                kind: *kind,
                role: ColumnRole::Other,
            })
            .collect(),
        total_rows: rows.len() as u64,
        rows,
        truncated: false,
        limit_reached: false,
    }
}

/// Câu không đọc được vai trò: build_column_roles trả other cho mọi cột.
fn by_kind(result: &ResultTable) -> Option<ChartSuggestion> {
    build_chart(result, &vec![ColumnRole::Other; result.columns.len()])
}

/// Một cột chữ và một cột số, mỗi nhóm một dòng.
fn markets(count: usize) -> ResultTable {
    let rows = (0..count)
        .map(|i| vec![text(&format!("market_{i}")), number(i as f64)])
        .collect();
    table(
        &[("market", ValueKind::Text), ("revenue", ValueKind::Number)],
        rows,
    )
}

/// Một cột ngày và một cột số, mốc theo thứ tự đã cho.
fn months(dates: &[&str]) -> ResultTable {
    let rows = dates.iter().map(|d| vec![text(d), number(1.0)]).collect();
    table(
        &[("month", ValueKind::Date), ("revenue", ValueKind::Decimal)],
        rows,
    )
}

#[test]
fn one_dimension_and_one_measure_give_a_bar_chart() {
    let chart = by_kind(&markets(5)).expect("a bar chart");
    assert_eq!(chart.kind, ChartKind::Bar);
    assert_eq!(chart.x, "market");
    assert_eq!(chart.y, vec!["revenue".to_string()]);

    let result = table(
        &[
            ("revenue", ValueKind::Number),
            ("market", ValueKind::Text),
            ("orders", ValueKind::Decimal),
        ],
        vec![
            vec![number(1.0), text("Europe"), text("3")],
            vec![number(2.0), text("Africa"), text("4")],
        ],
    );
    // Hai độ đo có thể khác đơn vị (doanh thu và số đơn): vẽ chung một trục thì độ đo nhỏ thành cột phẳng.
    assert!(by_kind(&result).is_none());
}

#[test]
fn too_many_groups_or_one_row_give_no_chart() {
    assert!(by_kind(&markets(20)).is_some());
    assert!(by_kind(&markets(21)).is_none());
    assert!(by_kind(&markets(1)).is_none());
}

#[test]
fn time_and_measures_give_a_line_chart() {
    let chart = by_kind(&months(&["2017-01-01", "2017-02-01", "2017-03-01"])).expect("a line");
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(chart.x, "month");
    assert_eq!(chart.y, vec!["revenue".to_string()]);

    // Đường không bị giới hạn số nhóm như biểu đồ cột.
    let days: Vec<String> = (1..=28).map(|d| format!("2017-02-{d:02}")).collect();
    let days: Vec<&str> = days.iter().map(String::as_str).collect();
    assert!(by_kind(&months(&days)).is_some());

    let result = table(
        &[
            ("hour", ValueKind::Timestamp),
            ("orders", ValueKind::Number),
        ],
        vec![
            vec![text("2017-01-01T00:00:00"), number(1.0)],
            vec![text("2017-01-01T01:00:00"), number(2.0)],
        ],
    );
    assert_eq!(by_kind(&result).map(|c| c.kind), Some(ChartKind::Line));
}

/// Một cột chữ và một cột số, cột chữ có đúng các giá trị đã cho.
fn labels(values: &[&str]) -> ResultTable {
    let rows = values.iter().map(|v| vec![text(v), number(1.0)]).collect();
    table(
        &[("label", ValueKind::Text), ("revenue", ValueKind::Number)],
        rows,
    )
}

#[test]
fn text_months_and_dates_give_a_line_chart() {
    // Dạng Gemini trả cho "doanh số theo tháng của năm 2017" trên DataCo: strftime(..., '%Y-%m').
    let months: Vec<String> = (1..=12).map(|m| format!("2017-{m:02}")).collect();
    let months: Vec<&str> = months.iter().map(String::as_str).collect();
    let chart = by_kind(&labels(&months)).expect("a line");
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(chart.x, "label");

    let days = labels(&["2017-01-30", "2017-01-31", "2017-02-01"]);
    assert_eq!(by_kind(&days).map(|c| c.kind), Some(ChartKind::Line));
    // Sắp giảm dần thì không vẽ đường, cũng không đổi sang cột.
    assert!(by_kind(&labels(&["2017-02", "2017-01"])).is_none());
}

#[test]
fn text_that_only_looks_like_time_gives_a_bar_chart() {
    for values in [
        ["2017", "2018"],
        ["2017-Q1", "2017-Q2"],
        ["2017-13", "2017-14"],
        ["2017/01", "2017/02"],
        ["2017-01", "Europe"],
        ["AB12-34", "AB12-35"],
    ] {
        let chart = by_kind(&labels(&values));
        assert_eq!(chart.map(|c| c.kind), Some(ChartKind::Bar), "{values:?}");
    }
}

#[test]
fn time_out_of_order_missing_or_cut_gives_no_line() {
    assert!(by_kind(&months(&["2017-03-01", "2017-02-01"])).is_none());
    assert!(by_kind(&months(&["2017-02-01", "2017-02-01"])).is_none());

    let mut with_null = months(&["2017-01-01", "2017-02-01"]);
    with_null.rows[1][0] = None;
    assert!(by_kind(&with_null).is_none());

    let mut cut = months(&["2017-01-01", "2017-02-01"]);
    cut.total_rows = 5000;
    cut.truncated = true;
    assert!(by_kind(&cut).is_none());
}

#[test]
fn no_single_axis_gives_no_chart() {
    let row = || vec![text("a"), text("b"), number(1.0)];
    let two_dimensions = table(
        &[
            ("market", ValueKind::Text),
            ("segment", ValueKind::Text),
            ("revenue", ValueKind::Number),
        ],
        vec![row(), row()],
    );
    assert!(by_kind(&two_dimensions).is_none());

    let dimension_and_time = table(
        &[
            ("market", ValueKind::Text),
            ("month", ValueKind::Date),
            ("revenue", ValueKind::Number),
        ],
        vec![
            vec![text("Europe"), text("2017-01-01"), number(1.0)],
            vec![text("Europe"), text("2017-02-01"), number(2.0)],
        ],
    );
    assert!(by_kind(&dimension_and_time).is_none());

    // Năm, tháng dạng số nguyên trông như một độ đo: không đoán là thời gian.
    let numbers_only = table(
        &[
            ("month", ValueKind::Decimal),
            ("revenue", ValueKind::Number),
        ],
        vec![vec![text("1"), number(1.0)], vec![text("2"), number(2.0)]],
    );
    assert!(by_kind(&numbers_only).is_none());
}

#[test]
fn no_measure_two_measures_or_other_kinds_give_no_chart() {
    let no_measure = table(
        &[("market", ValueKind::Text)],
        vec![vec![text("Europe")], vec![text("Africa")]],
    );
    assert!(by_kind(&no_measure).is_none());

    let measures = |count: usize| {
        let mut columns = vec![("market", ValueKind::Text)];
        columns.extend(std::iter::repeat_n(("m", ValueKind::Number), count));
        let row = |name: &str| {
            let mut cells = vec![text(name)];
            cells.extend(std::iter::repeat_n(number(1.0), count));
            cells
        };
        table(&columns, vec![row("Europe"), row("Africa")])
    };
    assert_eq!(by_kind(&measures(1)).map(|c| c.y.len()), Some(1));
    assert!(by_kind(&measures(2)).is_none());

    for kind in [ValueKind::Boolean, ValueKind::Other] {
        let result = table(
            &[
                ("market", ValueKind::Text),
                ("flag", kind),
                ("revenue", ValueKind::Number),
            ],
            vec![
                vec![text("Europe"), Some(CellValue::Boolean(true)), number(1.0)],
                vec![text("Africa"), Some(CellValue::Boolean(false)), number(2.0)],
            ],
        );
        assert!(by_kind(&result).is_none());
    }
}

/// Kết quả hai dòng, cột tên c0, c1, ... theo kiểu và vai trò đã cho; ô dựng theo kiểu, trục thời gian tăng dần.
fn with_roles(columns: &[(ValueKind, ColumnRole)]) -> Option<(ChartKind, String, Vec<String>)> {
    let names: Vec<String> = (0..columns.len()).map(|i| format!("c{i}")).collect();
    let kinds: Vec<(&str, ValueKind)> = names
        .iter()
        .zip(columns)
        .map(|(name, (kind, _))| (name.as_str(), *kind))
        .collect();
    let rows = (1..=2)
        .map(|r| {
            columns
                .iter()
                .map(|(kind, _)| match kind {
                    ValueKind::Text => text(&format!("group_{r}")),
                    ValueKind::Date => text(&format!("2017-01-0{r}")),
                    ValueKind::Decimal => text(&r.to_string()),
                    ValueKind::Boolean => Some(CellValue::Boolean(r == 1)),
                    _ => number(r as f64),
                })
                .collect()
        })
        .collect();
    let roles: Vec<ColumnRole> = columns.iter().map(|(_, role)| *role).collect();
    build_chart(&table(&kinds, rows), &roles).map(|c| (c.kind, c.x, c.y))
}

fn expected(kind: ChartKind, x: &str, y: &str) -> Option<(ChartKind, String, Vec<String>)> {
    Some((kind, x.to_string(), vec![y.to_string()]))
}

#[test]
fn roles_pick_the_axis_and_the_measure() {
    use ColumnRole::{Dimension, Measure, Other, Time};
    use ValueKind::{Date, Decimal, Number, Text};
    assert_eq!(
        with_roles(&[(Text, Dimension), (Number, Measure)]),
        expected(ChartKind::Bar, "c0", "c1")
    );
    // Cột other bên cạnh khóa (ví dụ upper(market)) không đổi luật.
    assert_eq!(
        with_roles(&[(Text, Other), (Decimal, Measure), (Text, Dimension)]),
        expected(ChartKind::Bar, "c2", "c1")
    );
    // Đường vẽ độ đo đầu, như mẫu why của ask_examples.rs.
    assert_eq!(
        with_roles(&[(Date, Time), (Number, Measure), (Number, Measure)]),
        expected(ChartKind::Line, "c0", "c1")
    );
    // Khóa dạng số: năm là time thì đường, mã sản phẩm là dimension thì cột, không coi là độ đo.
    assert_eq!(
        with_roles(&[(Decimal, Time), (Number, Measure)]),
        expected(ChartKind::Line, "c0", "c1")
    );
    assert_eq!(
        with_roles(&[(Decimal, Dimension), (Number, Measure)]),
        expected(ChartKind::Bar, "c0", "c1")
    );
}

#[test]
fn roles_without_one_axis_and_a_number_give_no_chart() {
    use ColumnRole::{Dimension, Measure, Other, Time};
    use ValueKind::{Boolean, Date, Number, Text};
    let none = [
        vec![(Text, Dimension), (Number, Measure), (Number, Measure)],
        vec![(Text, Dimension), (Date, Time), (Number, Measure)],
        vec![(Text, Dimension), (Text, Dimension)],
        vec![(Number, Measure)],
        vec![(Text, Other), (Text, Other)],
        // Độ đo dạng chữ (MIN của một cột chữ) và khóa boolean không vẽ được.
        vec![(Text, Dimension), (Text, Measure)],
        vec![(Boolean, Dimension), (Number, Measure)],
    ];
    for columns in none {
        assert_eq!(with_roles(&columns), None, "{columns:?}");
    }
}

#[test]
fn text_months_are_time_even_as_a_dimension() {
    let dimension = [ColumnRole::Dimension, ColumnRole::Measure];
    let months = labels(&["2017-01", "2017-02", "2017-03"]);
    let chart = build_chart(&months, &dimension).map(|c| c.kind);
    assert_eq!(chart, Some(ChartKind::Line));

    // Quý dạng chữ chỉ là thời gian khi vai trò nói là time.
    let quarters = labels(&["2017-Q1", "2017-Q2"]);
    let chart = build_chart(&quarters, &dimension).map(|c| c.kind);
    assert_eq!(chart, Some(ChartKind::Bar));
    let time = [ColumnRole::Time, ColumnRole::Measure];
    let chart = build_chart(&quarters, &time).map(|c| c.kind);
    assert_eq!(chart, Some(ChartKind::Line));
}

#[test]
fn number_time_marks_are_compared_as_numbers() {
    let months = |values: &[&str]| {
        let rows = values.iter().map(|v| vec![text(v), number(1.0)]).collect();
        table(
            &[
                ("month", ValueKind::Decimal),
                ("revenue", ValueKind::Number),
            ],
            rows,
        )
    };
    let time = [ColumnRole::Time, ColumnRole::Measure];
    // So chuỗi thì "10" đứng trước "9".
    let chart = build_chart(&months(&["8", "9", "10"]), &time).map(|c| c.kind);
    assert_eq!(chart, Some(ChartKind::Line));
    assert!(build_chart(&months(&["10", "9"]), &time).is_none());
}
