// SPDX-License-Identifier: Apache-2.0
/* service/explain/group_base.rs - nhóm gốc khi câu "tại sao" so hai nhóm (#244): một giá trị của cột ("Europe so với
LATAM"), hay mọi nhóm còn lại của cột đó ("vì sao Scotland rút học nhiều" so Scotland với các vùng khác gộp lại).
explain/check.rs dựng GroupBase khi kiểm form; explain/sql.rs lấy điều kiện lát cắt, explain/breakdown.rs,
explain/premise.rs và thẻ "Cách tính" (understanding/why.rs) lấy nhãn ở đây, để mọi chỗ gọi lát gốc cùng một tên.

Nhãn của phần còn lại viết thường ("các thị trường còn lại") vì nó luôn đứng giữa câu: câu khuôn "cao hơn các thị
trường còn lại", thẻ "Thị trường: Pacific Asia so với các thị trường còn lại", và lời của mô hình chép nhãn này. */

use std::collections::HashMap;

use sqlparser::ast::{BinaryOperator, Expr};

use crate::service::explain::form::{FieldError, FormAgg, WhyForm};
use crate::service::explain::lower_first;
use crate::service::explain::sql::{build_equals, build_ident, build_string};

/// Lát gốc của phép so hai nhóm.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupBase {
    /// Một giá trị của cột, đã kiểm có trong danh sách giá trị.
    Value(String),
    /// Mọi giá trị khác của cột, trừ nhóm đang so. Dòng trống ở cột này không thuộc nhóm nào nên không tính: `<>` với
    /// NULL không đúng, đúng như câu hỏi "so với các nhóm khác".
    Rest,
}

impl GroupBase {
    /// Điều kiện SQL của lát gốc: `"cột" = 'giá trị'`, hay `"cột" <> '<nhóm đang so>'` với phần còn lại.
    pub fn build_condition(&self, column: &str, compare: &str) -> Expr {
        match self {
            GroupBase::Value(value) => build_equals(column, value),
            GroupBase::Rest => Expr::BinaryOp {
                left: Box::new(build_ident(column)),
                op: BinaryOperator::NotEq,
                right: Box::new(build_string(compare)),
            },
        }
    }

    /// Nhãn cho người dùng: giá trị nhóm, hay "các <nghĩa tiếng Việt của cột> còn lại". labels là WhyPlan.labels; cột
    /// chưa có nghĩa tiếng Việt thì "các nhóm còn lại", vì tên cột gốc (tiếng Anh) ghép vào câu tiếng Việt khó đọc.
    /// average: lát gốc là trung bình mỗi nhóm còn lại (explain/rest_average.rs), nhãn mở đầu bằng "trung bình".
    pub fn build_label(
        &self,
        column: &str,
        labels: &HashMap<String, String>,
        average: bool,
    ) -> String {
        let rest = match labels.get(column) {
            Some(label) => format!("các {} còn lại", lower_first(label)),
            None => "các nhóm còn lại".to_string(),
        };
        match self {
            GroupBase::Value(value) => value.clone(),
            GroupBase::Rest if average => format!("trung bình {rest}"),
            GroupBase::Rest => rest,
        }
    }

    /// Cách viết trong lời báo lỗi gửi lại mô hình (tiếng Anh).
    pub fn build_error_text(&self, compare: &str) -> String {
        match self {
            GroupBase::Value(value) => value.clone(),
            GroupBase::Rest => format!("the groups other than {compare}"),
        }
    }
}

/// Một nhóm so với mọi nhóm còn lại mà độ đo là số đếm trên các dòng đã lọc bằng một giá trị ("số dòng giao trễ của
/// Pacific Asia so với mọi thị trường khác"): phần còn lại gộp lại lớn hơn một nhóm nhiều lần, nên chênh lệch chỉ nói
/// nhóm nhỏ hơn, không nói nhóm trễ nhiều hơn. Gemini hay điền như vậy cho câu "giao trễ nhiều hơn các thị trường khác";
/// lỗi chỉ đường sửa sang tỷ lệ dòng có giá trị đó (share), để mô hình sửa trong một vòng. Tổng tiền thì không chặn:
/// "doanh thu Pacific Asia so với phần còn lại" là câu hỏi về tổng.
pub(super) fn check_rest_measure(form: &WhyForm, errors: &mut Vec<FieldError>) {
    let rest = form
        .groups
        .as_ref()
        .is_some_and(|g| g.base.as_deref().is_none_or(|b| b.trim().is_empty()));
    let counts = form.metric.as_deref().is_none_or(|m| m.trim().is_empty())
        && form
            .measure
            .as_ref()
            .is_some_and(|m| m.agg == FormAgg::Count);
    let Some(filter) = form.filters.iter().find(|f| f.op == "=") else {
        return;
    };
    if rest && counts {
        errors.push(FieldError::new(
            "measure.agg",
            format!(
                "one group against all the other groups by a count only shows that the group is smaller; set \
                 \"measure\" to {{\"column\": \"{}\", \"agg\": \"share\", \"value\": \"{}\"}} and remove that filter, \
                 to compare the share of such rows",
                filter.column, filter.value
            ),
        ));
    }
}
