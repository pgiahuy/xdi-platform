// SPDX-License-Identifier: Apache-2.0
/* service/explain/measure_column.rs - độ đo là một cột của form câu "tại sao" (measure, explain/form.rs): kiểm cột với
cách tổng hợp agg rồi đổi thành PlanMeasure. explain/measure.rs gọi khi form không nêu chỉ số; phần chỉ số (đọc công
thức của tổ chức) vẫn ở đó. Test ở explain/check_tests.rs và explain/frames_tests.rs.

Năm cách tổng hợp, mỗi cách một dạng phân rã:
- sum, count: cộng được theo nhóm, phân rã bằng phép trừ từng nhóm (#39).
- avg, và sum trên cột có cách gộp avg (#185): trung bình mỗi dòng, tử SUM(x) trên mẫu COUNT(x), phân rã thành phần cơ
  cấu và phần hiệu suất (explain/ratio.rs) như chỉ số AVG(x).
- share (#244): tỷ lệ dòng có một giá trị ("tỷ lệ rút học", "tỷ lệ giao trễ"): tử COUNT(CASE WHEN "cột" = 'giá trị'
  THEN 1 END), mẫu COUNT(*), cùng đường tỷ lệ. Giá trị phải có trong danh sách giá trị của cột như bộ lọc "=".
- count_distinct (#244): số giá trị khác nhau của một cột key ("số khách hàng"). Không tách lượng và giá, vì lượng
  cũng là một số đếm phân biệt; chiều nào một giá trị rơi vào nhiều nhóm thì bị bỏ (xem Additive::CountDistinct). */

use sqlparser::ast::Expr;

use crate::engine::store::{ColumnAggregation, ProfileColumn, ProfileRole};
use crate::service::explain::check::{check_value, find_column, WhyProfile};
use crate::service::explain::form::{FieldError, FormAgg, FormMeasure};
use crate::service::explain::measure::{
    build_plan_measure, build_vietnamese_name, is_exact_type, parse_expr, Additive, PlanMeasure,
};
use crate::service::explain::sql::{build_equals, build_ident};
use crate::service::explain::{build_entity_name, lower_first};
use crate::service::profile::is_number_type;

const FIELD: &str = "measure.column";

pub(super) fn check_column_measure(
    measure: &FormMeasure,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Option<PlanMeasure> {
    let column = find_column(profile, &measure.column, FIELD, errors)?;
    let ident = build_ident(&column.name);
    match measure.agg {
        FormAgg::Sum => {
            check_number(column, "sum", errors)?;
            // Cột avg (mỗi giá trị mô tả một dòng, như số tín chỉ của một sinh viên): tổng của nó vô nghĩa và chênh
            // lệch tổng chủ yếu do số dòng của hai lát cắt, nên so trung bình mỗi dòng.
            if column.aggregation == ColumnAggregation::Avg.as_str() {
                return build_average(column);
            }
            // Tên tiếng Việt của hồ sơ ("Doanh thu") đọc tự nhiên hơn "Tổng Sales"; chưa có thì ghép từ tên cột.
            let label = match build_vietnamese_name(column) {
                Some(name) => name.to_string(),
                None => format!("Tổng {}", column.name),
            };
            build_plan_measure(label, None, build_sum(column), None)
        }
        FormAgg::Avg => {
            check_number(column, "avg", errors)?;
            build_average(column)
        }
        FormAgg::Count => {
            let label = format!("Số dòng có {}", build_name(column));
            build_plan_measure(label, None, Additive::Count { value: Some(ident) }, None)
        }
        FormAgg::CountDistinct => build_distinct(column, errors),
        FormAgg::Share => build_share(measure, column, profile, errors),
    }
}

/// sum và avg cần cột số có vai trò measure.
fn check_number(column: &ProfileColumn, agg: &str, errors: &mut Vec<FieldError>) -> Option<()> {
    if column.role == ProfileRole::Measure.as_str() && is_number_type(&column.data_type) {
        return Some(());
    }
    errors.push(FieldError::new(
        FIELD,
        format!(
            "\"{}\" is a {} column of type {}; \"{agg}\" needs a number column of role measure",
            column.name, column.role, column.data_type
        ),
    ));
    None
}

fn build_sum(column: &ProfileColumn) -> Additive {
    Additive::Sum {
        decimal: !is_exact_type(&column.data_type),
        value: build_ident(&column.name),
    }
}

/// Trung bình mỗi dòng như AVG(x) của chỉ số: tử SUM(x), mẫu COUNT(x); AVG bỏ dòng x trống ở cả tử và mẫu.
fn build_average(column: &ProfileColumn) -> Option<PlanMeasure> {
    let rows = Additive::Count {
        value: Some(build_ident(&column.name)),
    };
    let label = format!("Trung bình {}", build_name(column));
    build_plan_measure(label, None, build_sum(column), Some(rows))
}

fn build_distinct(column: &ProfileColumn, errors: &mut Vec<FieldError>) -> Option<PlanMeasure> {
    if column.role != ProfileRole::Key.as_str() {
        errors.push(FieldError::new(
            FIELD,
            format!(
                "\"{}\" has role {}; \"count_distinct\" needs a column of role key, such as a customer or order id",
                column.name, column.role
            ),
        ));
        return None;
    }
    // "Mã khách hàng" thành "Số khách hàng", như nhãn lượng của #40.
    let entity = build_entity_name(&column.name, build_vietnamese_name(column));
    let additive = Additive::CountDistinct {
        value: build_ident(&column.name),
    };
    build_plan_measure(format!("Số {entity}"), None, additive, None)
}

fn build_share(
    measure: &FormMeasure,
    column: &ProfileColumn,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Option<PlanMeasure> {
    let field = "measure.value";
    if column.role != ProfileRole::Dimension.as_str() {
        errors.push(FieldError::new(
            FIELD,
            format!(
                "\"{}\" has role {}; \"share\" needs a column of role dimension and one of its values in \"value\"",
                column.name, column.role
            ),
        ));
        return None;
    }
    let Some(text) = measure.value.as_deref().filter(|v| !v.trim().is_empty()) else {
        errors.push(FieldError::new(
            field,
            "\"share\" needs \"value\": the value whose share of rows is explained, such as the late status",
        ));
        return None;
    };
    let value = check_value(profile, column, text, field, errors)?;
    // Điều kiện in qua sqlparser rồi đọc lại thành cây, như công thức chỉ số: dấu nháy trong giá trị không cắt được câu.
    let hit: Expr = parse_expr(&format!(
        "CASE WHEN {} THEN 1 END",
        build_equals(&column.name, &value)
    ))
    .ok()?;
    let label = format!("Tỷ lệ {} là {value}", build_name(column));
    let rows = Additive::Count { value: None };
    build_plan_measure(
        label,
        None,
        Additive::Count { value: Some(hit) },
        Some(rows),
    )
}

/// Tên cột giữa câu: tên tiếng Việt viết thường chữ đầu, chưa có thì tên cột.
fn build_name(column: &ProfileColumn) -> String {
    match build_vietnamese_name(column) {
        Some(name) => lower_first(name),
        None => column.name.clone(),
    }
}
