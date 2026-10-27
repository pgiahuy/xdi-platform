// SPDX-License-Identifier: Apache-2.0
/* service/understanding/mod.rs - thẻ "Đã hiểu là" (Understanding của ask/contract.rs) và vai trò từng cột kết quả, dựng
bằng code từ câu đã chạy, không nhờ mô hình, nên thẻ luôn khớp sql_guarded. Endpoint hỏi (#33) đọc sql_guarded bằng
engine/sql_shape.rs rồi gọi build_understanding và build_column_roles; nhãn tiếng Việt ở understanding/labels.rs. Test ở
understanding/tests.rs.

Mỗi UnderstoodItem.sql là một đoạn nguyên văn của sql_guarded (QueryShape in mọi đoạn từ cây của chính câu đó), để người
đọc đối chiếu được. Các phần của thẻ:
- measures: cột kết quả tổng hợp.
- filters: vế lọc của câu, trừ khoảng thời gian; rồi vế HAVING; rồi điều kiện phân quyền guard thêm
  (GuardedSql::row_filters), nhãn "Phân quyền: ...". Vế phân quyền nằm trong hộp đọc bảng của guard nên được nhận ra
  bằng chữ và không lặp lại như vế của câu.
- time_range: cụm vế liền nhau đầu tiên trong WHERE câu ngoài cùng mà mỗi vế so một biểu thức thời gian với một hằng;
  vế thời gian khác ở lại filters. Chỉ xét WHERE câu ngoài cùng vì đoạn SQL của cụm phải liền một khúc trong câu.
- limit: chỉ khi LIMIT là của câu mô hình viết, không phải trần guard thêm hay hạ xuống.
- assumptions: giả định đã dựng ở ask/choices.rs (build_assumption_choices), nhận nguyên danh sách để thẻ và
  AskResponse.choices cùng một id.

Câu "tại sao" (#38) không đọc cây cú pháp: thẻ dựng thẳng từ form đã kiểm (build_why_understanding, thêm ở #38). */

pub mod labels;
#[cfg(test)]
mod tests;
pub mod why;

use sqlparser::ast::{BinaryOperator, Expr};

use crate::engine::sql_shape::{QueryShape, ShapeFilter};
use crate::engine::store::MetricDefinition;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{Choice, ColumnRole, Understanding, UnderstoodItem};
use crate::service::understanding::labels::{
    build_comparison, build_filter_label, build_literal_text, build_measure_labels,
    build_row_filter_label, build_subject_label, is_time_expr,
};

/// Thẻ của một câu thường đã chạy.
/// - generated_had_limit: LIMIT trong sql_guarded là của câu mô hình viết, tức guard giữ nguyên LIMIT
///   (GuardedSql::limit_note là None). False thì không có mục limit.
/// - profile: hồ sơ cột của phiên bản được hỏi; metrics: chỉ số của dataset (Store::list_metrics).
/// - row_filters: GuardedSql::row_filters.
/// - assumptions: kết quả của ask_choices::build_assumption_choices cho câu trả lời có sql_generated; thẻ chép
///   nguyên, không dựng lại id.
pub fn build_understanding(
    shape: &QueryShape,
    generated_had_limit: bool,
    profile: &[ColumnProfile],
    metrics: &[MetricDefinition],
    row_filters: &[String],
    assumptions: &[Choice],
) -> Understanding {
    // Vế của hộp phân quyền có trong shape (guard bọc bảng) và trong row_filters; chỉ giữ một lần, ở cuối với nhãn
    // phân quyền.
    let question_filters: Vec<&ShapeFilter> = shape
        .filters
        .iter()
        .filter(|f| !(f.in_table_box && row_filters.contains(&f.sql)))
        .collect();
    let (time_range, mut filters) = build_filter_items(&question_filters, profile);
    filters.extend(
        shape
            .having
            .iter()
            .map(|f| build_item(build_filter_label(&f.tree, profile), &f.sql)),
    );
    filters.extend(
        row_filters
            .iter()
            .map(|sql| build_item(build_row_filter_label(sql, profile), sql)),
    );
    Understanding {
        measures: build_measures(shape, metrics, profile),
        filters,
        group_by: build_group_items(shape, profile),
        order_by: build_order_items(shape, metrics, profile),
        limit: shape
            .limit
            .as_ref()
            .filter(|_| generated_had_limit)
            .map(|n| build_item(format!("{n} dòng đầu"), &format!("LIMIT {n}"))),
        time_range,
        assumptions: assumptions.to_vec(),
    }
}

/// Vai trò của từng cột kết quả, theo thứ tự. column_count là số cột DuckDB trả: câu có `*` hay là phép tập hợp thì
/// không biết chắc cột nào ứng với biểu thức nào, nên mọi cột là other (giao diện không cho truy dòng nguồn từ chúng).
pub fn build_column_roles(
    shape: &QueryShape,
    profile: &[ColumnProfile],
    column_count: usize,
) -> Vec<ColumnRole> {
    let readable = !shape.set_operation
        && shape.outputs.len() == column_count
        && shape.outputs.iter().all(|o| o.tree.is_some());
    if !readable {
        return vec![ColumnRole::Other; column_count];
    }
    let keys: Vec<usize> = shape.group_by.iter().filter_map(|k| k.output).collect();
    shape
        .outputs
        .iter()
        .enumerate()
        .map(|(i, output)| match &output.tree {
            _ if output.is_aggregate => ColumnRole::Measure,
            Some(tree) if keys.contains(&i) && is_time_expr(tree, profile) => ColumnRole::Time,
            Some(_) if keys.contains(&i) => ColumnRole::Dimension,
            _ => ColumnRole::Other,
        })
        .collect()
}

fn build_item(label: String, sql: &str) -> UnderstoodItem {
    UnderstoodItem {
        label,
        sql: sql.to_string(),
    }
}

fn build_measures(
    shape: &QueryShape,
    metrics: &[MetricDefinition],
    profile: &[ColumnProfile],
) -> Vec<UnderstoodItem> {
    // Phép tập hợp: cột của vế đầu chưa chắc đo cùng thứ với các vế sau, nên không ghi độ đo.
    if shape.set_operation {
        return Vec::new();
    }
    shape
        .outputs
        .iter()
        .filter(|o| o.is_aggregate)
        .filter_map(|o| {
            let tree = o.tree.as_ref()?;
            let (_, full) = build_measure_labels(tree, &o.name, metrics, profile);
            Some(build_item(full, &o.expr))
        })
        .collect()
}

/// Khóa trỏ tới một cột kết quả (bí danh, số thứ tự) thì đoạn SQL là biểu thức của cột đó, để người đọc thấy nhóm
/// theo gì chứ không chỉ thấy `month` hay `1`.
fn build_group_items(shape: &QueryShape, profile: &[ColumnProfile]) -> Vec<UnderstoodItem> {
    shape
        .group_by
        .iter()
        .map(|key| {
            let output = key.output.and_then(|i| shape.outputs.get(i));
            match output.and_then(|o| o.tree.as_ref().map(|tree| (o, tree))) {
                Some((output, tree)) => {
                    build_item(build_subject_label(tree, profile, false), &output.expr)
                }
                None => build_item(build_subject_label(&key.tree, profile, false), &key.sql),
            }
        })
        .collect()
}

fn build_order_items(
    shape: &QueryShape,
    metrics: &[MetricDefinition],
    profile: &[ColumnProfile],
) -> Vec<UnderstoodItem> {
    shape
        .order_by
        .iter()
        .map(|order| {
            let output = order.output.and_then(|i| shape.outputs.get(i));
            let subject = match output.and_then(|o| o.tree.as_ref().map(|tree| (o, tree))) {
                Some((output, tree)) if output.is_aggregate => {
                    build_measure_labels(tree, &output.name, metrics, profile).0
                }
                Some((_, tree)) => build_subject_label(tree, profile, false),
                None => build_subject_label(&order.tree, profile, false),
            };
            let direction = if order.descending {
                "giảm dần"
            } else {
                "tăng dần"
            };
            build_item(format!("{subject}, {direction}"), &order.sql)
        })
        .collect()
}

/// Tách khoảng thời gian (time_range) khỏi các vế lọc còn lại, giữ thứ tự của câu.
fn build_filter_items(
    filters: &[&ShapeFilter],
    profile: &[ColumnProfile],
) -> (Option<UnderstoodItem>, Vec<UnderstoodItem>) {
    let bounds: Vec<Option<TimeBound>> = filters
        .iter()
        .map(|f| {
            (!f.in_table_box)
                .then(|| build_time_bound(&f.tree, profile))
                .flatten()
        })
        .collect();
    let start = bounds.iter().position(Option::is_some);
    let end = start.map(|s| {
        s + bounds[s..]
            .iter()
            .take_while(|bound| bound.is_some())
            .count()
    });
    let time_range = match (start, end) {
        (Some(start), Some(end)) => {
            let run: Vec<&TimeBound> = bounds[start..end].iter().flatten().collect();
            let sql: Vec<&str> = filters[start..end].iter().map(|f| f.sql.as_str()).collect();
            // Vế liền nhau của cùng một WHERE in cách nhau đúng " AND " (sqlparser in phép AND như vậy).
            Some(build_item(build_time_label(&run), &sql.join(" AND ")))
        }
        _ => None,
    };
    let in_run = |i: usize| matches!((start, end), (Some(s), Some(e)) if (s..e).contains(&i));
    let others = filters
        .iter()
        .enumerate()
        .filter(|(i, _)| !in_run(*i))
        .map(|(_, f)| build_item(build_filter_label(&f.tree, profile), &f.sql))
        .collect();
    (time_range, others)
}

/// Một vế so biểu thức thời gian với hằng.
struct TimeBound {
    /// Chữ của biểu thức thời gian, để biết hai vế liền nhau có cùng biểu thức không.
    subject_sql: String,
    subject_label: String,
    /// "từ 2017-07-01", "đến trước 2017-10-01", "là 2017".
    phrase: String,
}

fn build_time_bound(expr: &Expr, profile: &[ColumnProfile]) -> Option<TimeBound> {
    let (subject, phrase) = match expr {
        Expr::Between {
            expr: subject,
            negated: false,
            low,
            high,
        } => {
            let (low, high) = (build_literal_text(low)?, build_literal_text(high)?);
            (subject.as_ref(), format!("từ {low} đến {high}"))
        }
        _ => {
            let comparison = build_comparison(expr)?;
            let value = comparison.value;
            let phrase = match comparison.op {
                BinaryOperator::Eq => format!("là {value}"),
                BinaryOperator::GtEq => format!("từ {value}"),
                BinaryOperator::Gt => format!("sau {value}"),
                BinaryOperator::Lt => format!("đến trước {value}"),
                BinaryOperator::LtEq => format!("đến hết {value}"),
                _ => return None,
            };
            (comparison.subject, phrase)
        }
    };
    is_time_expr(subject, profile).then(|| TimeBound {
        subject_sql: subject.to_string(),
        subject_label: build_subject_label(subject, profile, false),
        phrase,
    })
}

/// "Ngày đặt hàng (order date) từ 2017-07-01 đến trước 2017-10-01": tên biểu thức chỉ ghi lại khi nó đổi.
fn build_time_label(run: &[&TimeBound]) -> String {
    let mut label = String::new();
    let mut previous: Option<&str> = None;
    for bound in run {
        if previous == Some(bound.subject_sql.as_str()) {
            label.push(' ');
        } else {
            if previous.is_some() {
                label.push_str(", ");
            }
            label.push_str(&bound.subject_label);
            label.push(' ');
        }
        label.push_str(&bound.phrase);
        previous = Some(&bound.subject_sql);
    }
    label
}
