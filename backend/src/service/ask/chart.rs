// SPDX-License-Identifier: Apache-2.0
/* service/ask/chart.rs - chọn biểu đồ cho bảng kết quả của một câu đã trả lời, theo bộ luật ở ARCHITECTURE mục 6b (dựa
trên IBCS, International Business Communication Standards). service/ask/view.rs gọi build_chart sau khi có vai trò cột
(understanding/mod.rs), cho cả câu thường lẫn bảng tổng theo kỳ của câu "tại sao"; giao diện vẽ theo ChartSuggestion
(ask/contract.rs) từ chính các dòng của result.

Luật cần biết bốn điều về kết quả: đang so sánh gì (các nhóm, theo thời gian, cơ cấu theo thời gian), số đo cộng được
hay không, có bao nhiêu nhóm hay kỳ, và các số đo cùng đơn vị không. Ba điều đầu đọc từ vai trò cột, cây SQL đã chạy và
các dòng; đơn vị lấy từ hồ sơ cột (columns.unit, lớp ngữ nghĩa ghi), chưa có thì coi như khác đơn vị.

Mọi ngưỡng lấy từ mẫu biểu đồ của IBCS khi có; ngưỡng IBCS không quy định thì ghi rõ là chọn ở đây. Thác nước của câu
"tại sao" (luật 5) không chọn ở đây: giao diện vẽ nó từ phần phân rã (Explanation), bên cạnh biểu đồ của bảng tổng. */

use sqlparser::ast::{
    DuplicateTreatment, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Value,
};

use crate::engine::sql_shape::QueryShape;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{
    ChartKind, ChartLayout, ChartSuggestion, ColumnRole, ResultTable,
};

/// Tới số kỳ này thì cột đứng, nhiều hơn thì đường: mẫu biểu đồ đường của IBCS (C07) ghi đường hợp nhất khi có hơn 10
/// kỳ.
const COLUMN_PERIODS_MAX: usize = 10;

/// Cột chồng tối đa chừng này kỳ và chừng này phần: mẫu cột chồng của IBCS (C01) ghi không quá 25 kỳ, không quá 5 đến 6
/// chuỗi. Nhiều nhóm hơn thì vẽ 5 nhóm lớn nhất cộng một phần "Khác".
const STACK_PERIODS_MAX: usize = 25;
const STACK_PARTS_MAX: usize = 6;

/// Thanh ngang tối đa chừng này nhóm, còn lại gộp "Khác". IBCS không đặt số cho thanh đơn; 15 chọn ở đây để nhãn nhóm
/// còn đọc được trên màn hình hẹp, chưa đo.
const BAR_GROUPS_MAX: usize = 15;

/// Tối đa chừng này đường khi mỗi nhóm một đường. IBCS không đặt số; 5 chọn ở đây, nhiều hơn thì các đường chồng lên
/// nhau khó theo dõi.
const LINE_SERIES_MAX: usize = 5;

/// Tối đa chừng này số đo trong một biểu đồ (cạnh nhau hay mỗi số đo một biểu đồ nhỏ); nhiều hơn thì chỉ hiện bảng.
const MEASURES_MAX: usize = 4;

/// Đơn vị của số đo đếm dòng (COUNT): không phải đơn vị của cột nào, nhưng hai số đếm so được với nhau.
const COUNT_UNIT: &str = "count";

/// Đơn vị không so được với gì, kể cả với chính nó (điểm, cân nặng...): xem ColumnUnit::Other.
const OTHER_UNIT: &str = "other";

/// Số đo cộng được hay không. IBCS phân biệt số gốc với tỷ lệ: tỷ lệ vẽ cột hẹp để người đọc không lấy chiều cao cột
/// cộng dồn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeasureNature {
    /// SUM, COUNT không DISTINCT, và tổng hay hiệu của chúng: cộng các nhóm ra đúng tổng, nên chồng cột và gộp "Khác"
    /// được.
    Summable,
    /// COUNT DISTINCT: số gốc, nhưng một mã có thể nằm ở nhiều nhóm, nên cộng các nhóm có thể đếm trùng.
    DistinctCount,
    /// Trung bình, tỷ lệ, nhỏ nhất, lớn nhất và mọi biểu thức khác.
    Ratio,
}

/// Một số đo của kết quả: cột thứ index, loại và đơn vị.
#[derive(Debug, Clone)]
struct Measure {
    index: usize,
    nature: MeasureNature,
    unit: Option<String>,
}

/// None khi kết quả nên chỉ hiện bảng. roles theo thứ tự cột của result (understanding::build_column_roles).
pub fn build_chart(
    shape: &QueryShape,
    result: &ResultTable,
    roles: &[ColumnRole],
    profile: &[ColumnProfile],
) -> Option<ChartSuggestion> {
    // Một dòng là một con số (hay một nhóm duy nhất): biểu đồ không thêm thông tin gì, bảng đã hiện con số.
    if result.rows.len() < 2 {
        return None;
    }
    let keys: Vec<usize> = (0..roles.len())
        .filter(|&i| matches!(roles[i], ColumnRole::Dimension | ColumnRole::Time))
        .collect();
    let measures: Vec<Measure> = (0..roles.len())
        .filter(|&i| roles[i] == ColumnRole::Measure)
        .map(|i| build_measure(i, shape, profile))
        .collect();
    if measures.is_empty() || measures.len() > MEASURES_MAX {
        return None;
    }
    match keys.as_slice() {
        [key] => build_one_key_chart(*key, roles[*key], &measures, result),
        [first, second] => build_two_key_chart(*first, *second, roles, &measures, result),
        // Không khóa là bảng dòng thô; ba khóa trở lên là bảng chéo nhiều chiều, biểu đồ không vẽ đúng được.
        _ => None,
    }
}

/// Một khóa: so sánh các nhóm (thanh ngang) hay diễn biến theo thời gian (cột đứng hay đường).
fn build_one_key_chart(
    key: usize,
    role: ColumnRole,
    measures: &[Measure],
    result: &ResultTable,
) -> Option<ChartSuggestion> {
    let count = count_distinct(result, key);
    if count < 2 {
        return None;
    }
    let (kind, top) = match role {
        // Thời gian luôn trên trục ngang (IBCS); mọi kỳ đều vẽ, không gộp "Khác".
        ColumnRole::Time if count <= COLUMN_PERIODS_MAX => (ChartKind::Column, None),
        ColumnRole::Time => (ChartKind::Line, None),
        // Nhóm (cấu trúc) trên trục đứng nên là thanh ngang, xếp lớn trước; quá nhiều nhóm thì cắt.
        _ => {
            let top = (count > BAR_GROUPS_MAX).then_some(BAR_GROUPS_MAX);
            (ChartKind::Bar, top)
        }
    };
    let all_summable = measures.iter().all(|m| m.nature == MeasureNature::Summable);
    Some(ChartSuggestion {
        kind,
        x: result.columns[key].name.clone(),
        y: build_names(measures, result),
        layout: build_layout(measures),
        series: None,
        ratio: build_ratio_names(measures, result),
        top: top.map(to_u32),
        other_bucket: top.is_some() && all_summable && !result.truncated,
    })
}

/// Hai khóa, một thời gian và một nhóm, một số đo: cơ cấu theo thời gian. Số đo cộng được và không quá nhiều kỳ thì cột
/// chồng; còn lại mỗi nhóm một đường, khi không quá nhiều nhóm. Hai khóa đều là nhóm (không có thời gian) thì chỉ bảng.
fn build_two_key_chart(
    first: usize,
    second: usize,
    roles: &[ColumnRole],
    measures: &[Measure],
    result: &ResultTable,
) -> Option<ChartSuggestion> {
    let (time, group) = match (roles[first], roles[second]) {
        (ColumnRole::Time, ColumnRole::Dimension) => (first, second),
        (ColumnRole::Dimension, ColumnRole::Time) => (second, first),
        _ => return None,
    };
    let [measure] = measures else {
        return None;
    };
    let periods = count_distinct(result, time);
    let groups = count_distinct(result, group);
    if periods < 2 || groups < 2 {
        return None;
    }
    let stackable = measure.nature == MeasureNature::Summable && periods <= STACK_PERIODS_MAX;
    let (kind, top) = if stackable {
        // Một chỗ trong STACK_PARTS_MAX dành cho phần "Khác".
        let top = (groups > STACK_PARTS_MAX).then_some(STACK_PARTS_MAX - 1);
        (ChartKind::StackedColumn, top)
    } else if groups <= LINE_SERIES_MAX {
        (ChartKind::Line, None)
    } else {
        return None;
    };
    Some(ChartSuggestion {
        kind,
        x: result.columns[time].name.clone(),
        y: build_names(measures, result),
        layout: ChartLayout::Single,
        series: Some(result.columns[group].name.clone()),
        ratio: build_ratio_names(measures, result),
        top: top.map(to_u32),
        other_bucket: top.is_some() && !result.truncated,
    })
}

/// Cùng đơn vị đã biết thì vẽ cạnh nhau trên một thang đo; khác hay chưa biết thì mỗi số đo một biểu đồ nhỏ, không
/// dùng hai trục.
fn build_layout(measures: &[Measure]) -> ChartLayout {
    let [first, rest @ ..] = measures else {
        return ChartLayout::Single;
    };
    if rest.is_empty() {
        return ChartLayout::Single;
    }
    let shared = first.unit.as_deref().filter(|unit| *unit != OTHER_UNIT);
    let same_unit = shared.is_some() && rest.iter().all(|m| m.unit.as_deref() == shared);
    if same_unit {
        ChartLayout::Grouped
    } else {
        ChartLayout::Panels
    }
}

fn build_names(measures: &[Measure], result: &ResultTable) -> Vec<String> {
    measures
        .iter()
        .map(|m| result.columns[m.index].name.clone())
        .collect()
}

fn build_ratio_names(measures: &[Measure], result: &ResultTable) -> Vec<String> {
    measures
        .iter()
        .filter(|m| m.nature == MeasureNature::Ratio)
        .map(|m| result.columns[m.index].name.clone())
        .collect()
}

/// Số giá trị khác nhau của một cột trong các dòng đã gửi. Kết quả bị cắt thì đây chỉ là số tối thiểu, đủ cho các
/// ngưỡng ở đây vì chúng đều nhỏ hơn số dòng gửi.
fn count_distinct(result: &ResultTable, column: usize) -> usize {
    let mut seen = std::collections::HashSet::new();
    for row in &result.rows {
        let key = serde_json::to_string(&row[column]).unwrap_or_default();
        seen.insert(key);
    }
    seen.len()
}

fn to_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Loại và đơn vị của cột kết quả index. Cây là None (câu có `*`) thì không biết gì: coi như tỷ lệ, chưa rõ đơn vị.
fn build_measure(index: usize, shape: &QueryShape, profile: &[ColumnProfile]) -> Measure {
    let tree = shape.outputs.get(index).and_then(|o| o.tree.as_ref());
    let (nature, unit) = match tree {
        Some(expr) => read_measure(expr, profile),
        None => (MeasureNature::Ratio, None),
    };
    Measure {
        index,
        nature,
        unit,
    }
}

/// Đọc biểu thức của một số đo. Bỏ qua lớp ngoặc, CAST và ROUND, vì chúng không đổi loại hay đơn vị. Tổng hay hiệu của
/// hai số cộng được cùng đơn vị (doanh thu trừ chi phí) vẫn cộng được; mọi phép khác (chia, nhân) ra tỷ lệ, chưa rõ đơn
/// vị.
fn read_measure(expr: &Expr, profile: &[ColumnProfile]) -> (MeasureNature, Option<String>) {
    match expr {
        Expr::Nested(inner) => read_measure(inner, profile),
        Expr::Cast { expr: inner, .. } => read_measure(inner, profile),
        Expr::Function(function) => read_aggregate(function, profile),
        Expr::BinaryOp { left, op, right } => {
            let additive = matches!(
                op,
                sqlparser::ast::BinaryOperator::Plus | sqlparser::ast::BinaryOperator::Minus
            );
            let (left_nature, left_unit) = read_measure(left, profile);
            let (right_nature, right_unit) = read_measure(right, profile);
            let both_summable =
                left_nature == MeasureNature::Summable && right_nature == MeasureNature::Summable;
            if additive && both_summable && left_unit == right_unit {
                (MeasureNature::Summable, left_unit)
            } else {
                (MeasureNature::Ratio, None)
            }
        }
        _ => (MeasureNature::Ratio, None),
    }
}

/// Hàm tổng hợp: SUM, COUNT cộng được; COUNT DISTINCT là số gốc không cộng được; AVG, MEDIAN, MIN, MAX là tỷ lệ mang
/// đơn vị của cột bên trong. Hàm có FILTER hay OVER, hay hàm khác, coi như tỷ lệ chưa rõ đơn vị.
fn read_aggregate(
    function: &Function,
    profile: &[ColumnProfile],
) -> (MeasureNature, Option<String>) {
    let unknown = (MeasureNature::Ratio, None);
    let FunctionArguments::List(list) = &function.args else {
        return unknown;
    };
    if function.filter.is_some() || function.over.is_some() {
        return unknown;
    }
    let name = function.name.to_string().to_lowercase();
    let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
    let first = match list.args.first() {
        Some(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))) => Some(expr),
        _ => None,
    };
    match (name.as_str(), first) {
        // ROUND(x, 2) làm tròn để hiện, x vẫn là số đo bên trong.
        ("round", Some(inner)) => read_measure(inner, profile),
        ("count", _) if distinct => (MeasureNature::DistinctCount, Some(COUNT_UNIT.to_string())),
        ("count", _) => (MeasureNature::Summable, Some(COUNT_UNIT.to_string())),
        ("sum", Some(inner)) => (MeasureNature::Summable, read_unit(inner, profile)),
        ("avg" | "mean" | "median" | "min" | "max", Some(inner)) => {
            (MeasureNature::Ratio, read_unit(inner, profile))
        }
        _ => unknown,
    }
}

/// Đơn vị của giá trị bên trong hàm tổng hợp: đơn vị của cột nếu giá trị là đúng một cột, kể cả qua CAST hay CASE
/// (SUM(CASE WHEN năm = 2017 THEN "Order Item Total" END) vẫn là tiền). Biểu thức khác thì chưa biết.
fn read_unit(expr: &Expr, profile: &[ColumnProfile]) -> Option<String> {
    let column = read_value_column(expr)?;
    profile
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&column))
        .and_then(|c| c.unit.clone())
}

/// Tên cột khi biểu thức chỉ trả giá trị của đúng một cột. CASE thì mọi nhánh THEN, ELSE (trừ NULL) phải cùng một cột.
fn read_value_column(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(ident) => Some(ident.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.clone()),
        Expr::Nested(inner) => read_value_column(inner),
        Expr::Cast { expr: inner, .. } => read_value_column(inner),
        Expr::Case {
            conditions,
            else_result,
            ..
        } => {
            let results = conditions
                .iter()
                .map(|when| &when.result)
                .chain(else_result.as_deref())
                .filter(|result| !is_null(result));
            let mut columns = results.map(read_value_column);
            let Some(Some(first)) = columns.next() else {
                return None;
            };
            columns
                .all(|column| column.as_deref() == Some(first.as_str()))
                .then_some(first)
        }
        _ => None,
    }
}

fn is_null(expr: &Expr) -> bool {
    matches!(expr, Expr::Value(value) if value.value == Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::sql_shape::build_query_shape;
    use crate::service::ask::contract::{CellValue, ResultColumn, ValueKind};
    use ColumnRole::{Dimension, Measure as MeasureRole, Time};

    fn profile(name: &str, unit: Option<&str>) -> ColumnProfile {
        ColumnProfile {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            data_type: "DOUBLE".to_string(),
            role: "measure".to_string(),
            time_format: None,
            description: None,
            aliases: vec![],
            is_sensitive: false,
            values: vec![],
            values_complete: false,
            matched: false,
            unit: unit.map(str::to_string),
            aggregation: crate::engine::store::ColumnAggregation::Sum,
        }
    }

    fn dataco_profile() -> Vec<ColumnProfile> {
        vec![
            profile("Order Item Total", Some("money")),
            profile("Order Profit Per Order", Some("money")),
            profile("Order Item Quantity", Some("quantity")),
            profile("Days for shipping (real)", Some("duration")),
            profile("Days for shipment (scheduled)", Some("duration")),
            profile("Order Item Profit Ratio", Some("percent")),
        ]
    }

    /// Bảng kết quả với tên cột và các dòng số; vai trò cột truyền riêng vào build_chart.
    fn table(names: &[&str], rows: Vec<Vec<f64>>) -> ResultTable {
        ResultTable {
            columns: names
                .iter()
                .map(|n| ResultColumn {
                    name: n.to_string(),
                    kind: ValueKind::Number,
                    role: ColumnRole::Other,
                })
                .collect(),
            total_rows: rows.len() as u64,
            rows: rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|v| Some(CellValue::Number(v)))
                        .collect()
                })
                .collect(),
            truncated: false,
            limit_reached: false,
        }
    }

    /// Mỗi nhóm một dòng: khóa là số thứ tự nhóm, rồi một giá trị cho mỗi số đo.
    fn one_key_rows(groups: usize, measures: usize) -> Vec<Vec<f64>> {
        (0..groups)
            .map(|g| {
                std::iter::once(g as f64)
                    .chain(std::iter::repeat_n(1.0, measures))
                    .collect()
            })
            .collect()
    }

    /// Mỗi cặp (kỳ, nhóm) một dòng, rồi một số đo.
    fn two_key_rows(periods: usize, groups: usize) -> Vec<Vec<f64>> {
        (0..periods)
            .flat_map(|p| (0..groups).map(move |g| vec![p as f64, g as f64, 1.0]))
            .collect()
    }

    fn chart(sql: &str, roles: &[ColumnRole], result: &ResultTable) -> Option<ChartSuggestion> {
        let shape = build_query_shape(sql).unwrap();
        build_chart(&shape, result, roles, &dataco_profile())
    }

    fn summary(chart: &ChartSuggestion) -> (ChartKind, ChartLayout, Option<u32>, bool) {
        (chart.kind, chart.layout, chart.top, chart.other_bucket)
    }

    /// Luật 1: so sánh nhóm là thanh ngang; quá 15 nhóm thì cắt, số đo cộng được thì gộp "Khác".
    #[test]
    fn groups_are_horizontal_bars_with_a_top() {
        let sql = r#"SELECT "Market" AS market, SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1"#;
        let few = chart(
            sql,
            &[Dimension, MeasureRole],
            &table(&["market", "revenue"], one_key_rows(5, 1)),
        )
        .unwrap();
        assert_eq!(
            summary(&few),
            (ChartKind::Bar, ChartLayout::Single, None, false)
        );
        assert_eq!(
            (few.x.as_str(), few.y.clone()),
            ("market", vec!["revenue".to_string()])
        );

        let many = chart(
            sql,
            &[Dimension, MeasureRole],
            &table(&["market", "revenue"], one_key_rows(40, 1)),
        )
        .unwrap();
        assert_eq!(
            summary(&many),
            (ChartKind::Bar, ChartLayout::Single, Some(15), true)
        );

        // Trung bình không cộng được: vẫn cắt 15 nhóm nhưng không gộp "Khác", và vẽ cột hẹp.
        let avg = r#"SELECT "Market" AS market, AVG("Order Item Total") AS avg_total FROM dataco GROUP BY 1"#;
        let ratio = chart(
            avg,
            &[Dimension, MeasureRole],
            &table(&["market", "avg_total"], one_key_rows(40, 1)),
        )
        .unwrap();
        assert_eq!(
            summary(&ratio),
            (ChartKind::Bar, ChartLayout::Single, Some(15), false)
        );
        assert_eq!(ratio.ratio, ["avg_total"]);
    }

    /// Luật 2: thời gian trên trục ngang; tới 10 kỳ là cột, hơn 10 là đường.
    #[test]
    fn time_is_columns_up_to_ten_periods_then_lines() {
        let sql = r#"SELECT month(d) AS month, SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1"#;
        let roles = [Time, MeasureRole];
        let quarters = chart(
            sql,
            &roles,
            &table(&["month", "revenue"], one_key_rows(4, 1)),
        )
        .unwrap();
        assert_eq!(quarters.kind, ChartKind::Column);
        let months = chart(
            sql,
            &roles,
            &table(&["month", "revenue"], one_key_rows(12, 1)),
        )
        .unwrap();
        assert_eq!(months.kind, ChartKind::Line);
    }

    /// Luật 3: một dòng không vẽ.
    #[test]
    fn one_row_is_table_only() {
        let sql = r#"SELECT "Market" AS market, SUM("Order Item Total") AS revenue FROM dataco GROUP BY 1"#;
        assert!(chart(
            sql,
            &[Dimension, MeasureRole],
            &table(&["market", "revenue"], one_key_rows(1, 1))
        )
        .is_none());
    }

    /// Luật 4: thời gian và nhóm với một tổng là cột chồng; quá 6 nhóm thì 5 nhóm cộng "Khác". Đếm phân biệt không
    /// cộng được nên mỗi nhóm một đường.
    #[test]
    fn composition_over_time_is_stacked_columns() {
        let sql = r#"SELECT month(d) AS month, "Market" AS market, SUM("Order Item Total") AS revenue
                     FROM dataco GROUP BY 1, 2"#;
        let roles = [Time, Dimension, MeasureRole];
        let stacked = chart(
            sql,
            &roles,
            &table(&["month", "market", "revenue"], two_key_rows(12, 2)),
        )
        .unwrap();
        assert_eq!(
            summary(&stacked),
            (ChartKind::StackedColumn, ChartLayout::Single, None, false)
        );
        assert_eq!(stacked.series.as_deref(), Some("market"));

        let many = chart(
            sql,
            &roles,
            &table(&["month", "market", "revenue"], two_key_rows(12, 8)),
        )
        .unwrap();
        assert_eq!(
            summary(&many),
            (ChartKind::StackedColumn, ChartLayout::Single, Some(5), true)
        );

        let distinct = r#"SELECT month(d) AS month, "Market" AS market, COUNT(DISTINCT "Order Id") AS orders
                          FROM dataco GROUP BY 1, 2"#;
        let lines = chart(
            distinct,
            &roles,
            &table(&["month", "market", "orders"], two_key_rows(12, 2)),
        )
        .unwrap();
        assert_eq!(
            (lines.kind, lines.series.as_deref()),
            (ChartKind::Line, Some("market"))
        );
    }

    /// Luật 6, 7: cùng đơn vị thì cạnh nhau, khác hay chưa biết đơn vị thì mỗi số đo một biểu đồ.
    #[test]
    fn measures_share_a_scale_only_with_the_same_unit() {
        let roles = [Dimension, MeasureRole, MeasureRole];
        let rows = table(&["mode", "a", "b"], one_key_rows(4, 2));
        let days = r#"SELECT "Shipping Mode" AS mode, AVG("Days for shipping (real)") AS a,
                      AVG("Days for shipment (scheduled)") AS b FROM dataco GROUP BY 1"#;
        assert_eq!(
            chart(days, &roles, &rows).unwrap().layout,
            ChartLayout::Grouped
        );

        let mixed = r#"SELECT "Shipping Mode" AS mode, SUM("Order Item Total") AS a,
                       COUNT(DISTINCT "Order Id") AS b FROM dataco GROUP BY 1"#;
        assert_eq!(
            chart(mixed, &roles, &rows).unwrap().layout,
            ChartLayout::Panels
        );

        let unknown = r#"SELECT "Shipping Mode" AS mode, SUM("Sales") AS a, SUM("Sales") AS b
                         FROM dataco GROUP BY 1"#;
        assert_eq!(
            chart(unknown, &roles, &rows).unwrap().layout,
            ChartLayout::Panels
        );
    }

    /// Doanh thu theo tháng năm nay và năm trước: hai tổng tiền qua CASE, 12 tháng, nên hai đường trên một thang đo.
    #[test]
    fn year_over_year_months_are_two_lines_on_one_scale() {
        let sql = r#"SELECT month(d) AS month,
                       SUM(CASE WHEN year(d) = 2017 THEN "Order Item Total" END) AS revenue_2017,
                       SUM(CASE WHEN year(d) = 2016 THEN "Order Item Total" ELSE NULL END) AS revenue_2016
                     FROM dataco GROUP BY 1"#;
        let result = table(
            &["month", "revenue_2017", "revenue_2016"],
            one_key_rows(12, 2),
        );
        let chart = chart(sql, &[Time, MeasureRole, MeasureRole], &result).unwrap();
        assert_eq!(
            summary(&chart),
            (ChartKind::Line, ChartLayout::Grouped, None, false)
        );
        assert!(chart.ratio.is_empty());
    }

    #[test]
    fn measure_types_follow_the_aggregate() {
        let read = |sql: &str| {
            let shape = build_query_shape(sql).unwrap();
            read_measure(shape.outputs[0].tree.as_ref().unwrap(), &dataco_profile())
        };
        let money = Some("money".to_string());
        assert_eq!(
            read(r#"SELECT ROUND(SUM("Order Item Total"), 2) FROM t"#),
            (MeasureNature::Summable, money.clone())
        );
        assert_eq!(
            read(r#"SELECT SUM("Order Item Total") - SUM("Order Profit Per Order") FROM t"#),
            (MeasureNature::Summable, money.clone())
        );
        assert_eq!(
            read(r#"SELECT AVG("Order Item Total") FROM t"#),
            (MeasureNature::Ratio, money)
        );
        assert_eq!(
            read(r#"SELECT SUM("Order Item Total") / COUNT(*) FROM t"#),
            (MeasureNature::Ratio, None)
        );
        assert_eq!(
            read(r#"SELECT COUNT(DISTINCT "Order Id") FROM t"#),
            (MeasureNature::DistinctCount, Some("count".to_string()))
        );
    }
}
