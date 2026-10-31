// SPDX-License-Identifier: Apache-2.0
/* service/explain/filter.rs - kiểm bộ lọc của form câu "tại sao" (filters, explain/form.rs) và dựng vế WHERE của từng
bộ lọc. explain/check.rs gọi check_filters khi kiểm form; vế dựng ở đây vào mọi câu SQL của explain/sql.rs. Hàm thuần,
test ở explain/frames_tests.rs.

Phép nhận (#244): `=` và `in` trên cột chiều, giá trị phải có nguyên văn trong danh sách giá trị như trước; `>`, `>=`,
`<`, `<=`, `between` trên cột số (giá trị là một số), cột thời gian (giá trị là một kỳ: 2017, 2017-Q3, 2017-05 hay
2017-05-31) hoặc cột chiều có danh sách giá trị (so theo thứ tự chữ, như kỳ học "2014B" < "2014J"). Mô hình không viết
SQL: số được đọc lại thành số rồi mới in, ngày đổi thành TIMESTAMP, chữ in qua sqlparser.

Kỳ trong bộ lọc ngày tính trọn kỳ: "between 2017-03 và 2017-06" là từ ngày 1/3 tới hết ngày 30/6, "> 2017-05" là từ
1/6/2017. Viết thành hai vế `>=` và `<` thay cho BETWEEN, vì cột có giờ phút thì BETWEEN '2017-06-30' bỏ mất cả ngày cuối. */

use std::str::FromStr;

use rust_decimal::Decimal;
use sqlparser::ast::Expr;

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::explain::check::{check_value, find_column, wrong_role, WhyProfile};
use crate::service::explain::filter_bound::{parse_date_period, Bound};
use crate::service::explain::form::{FieldError, FormFilter, WhyForm};
use crate::service::explain::sql::{build_equals, build_ident, build_string, build_time_expr};
use crate::service::profile::is_number_type;

/// Các phép của trường op, đúng enum của schema (ask/why_prompt.rs).
pub const FILTER_OPS: [&str; 7] = ["=", "in", "between", ">", ">=", "<", "<="];

/// Kiểm mọi bộ lọc; trả vế WHERE của các bộ lọc đúng. Lỗi ghi vào errors, không dừng ở bộ lọc sai đầu tiên.
pub(super) fn check_filters(
    form: &WhyForm,
    profile: &WhyProfile,
    errors: &mut Vec<FieldError>,
) -> Vec<Expr> {
    let slice_column = form.groups.as_ref().map(|g| g.column.as_str());
    let mut filters = Vec::new();
    for (i, filter) in form.filters.iter().enumerate() {
        let field = format!("filters[{i}]");
        if !FILTER_OPS.contains(&filter.op.as_str()) {
            errors.push(FieldError::new(
                format!("{field}.op"),
                format!(
                    "\"{}\" is not supported; use one of {}",
                    filter.op,
                    FILTER_OPS.join(", ")
                ),
            ));
            continue;
        }
        let column_field = format!("{field}.column");
        let Some(column) = find_column(profile, &filter.column, &column_field, errors) else {
            continue;
        };
        if slice_column == Some(column.name.as_str()) {
            errors.push(FieldError::new(
                column_field,
                "this column already holds the two groups; remove the filter",
            ));
            continue;
        }
        let checked = FilterCheck {
            profile,
            column,
            filter,
            field: &field,
        };
        filters.extend(checked.build(errors).unwrap_or_default());
    }
    filters
}

/// Cột lọc bằng đúng một giá trị (`=`): chỉ còn một nhóm nên không là chiều phân rã. Cột lọc bằng `in` hay một khoảng
/// vẫn còn nhiều nhóm và vẫn được xét.
pub fn build_single_value_columns(form: &WhyForm) -> Vec<String> {
    form.filters
        .iter()
        .filter(|f| f.op == "=")
        .map(|f| f.column.clone())
        .collect()
}

/// Một bộ lọc đang kiểm, cùng cột đã tìm thấy trong hồ sơ.
struct FilterCheck<'a> {
    profile: &'a WhyProfile<'a>,
    column: &'a ProfileColumn,
    filter: &'a FormFilter,
    field: &'a str,
}

impl FilterCheck<'_> {
    /// Vế WHERE của bộ lọc (một vế, hay hai vế với khoảng ngày); None khi sai, lỗi đã ghi.
    fn build(&self, errors: &mut Vec<FieldError>) -> Option<Vec<Expr>> {
        match self.filter.op.as_str() {
            "=" => {
                let value = self.check_listed(&self.filter.value, "value", errors)?;
                Some(vec![build_equals(&self.column.name, &value)])
            }
            "in" => self.build_in(errors).map(|expr| vec![expr]),
            "between" => self.build_between(errors),
            op => self.build_compare(op, errors),
        }
    }

    fn build_in(&self, errors: &mut Vec<FieldError>) -> Option<Expr> {
        let values = &self.filter.values;
        if values.len() < 2 {
            errors.push(FieldError::new(
                format!("{}.values", self.field),
                "\"in\" needs two or more values in \"values\"; use \"=\" with \"value\" for one value",
            ));
            return None;
        }
        let mut list = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let field = format!("values[{i}]");
            list.extend(
                self.check_listed(value, &field, errors)
                    .map(|v| build_string(&v)),
            );
        }
        (list.len() == values.len()).then(|| Expr::InList {
            expr: Box::new(build_ident(&self.column.name)),
            list,
            negated: false,
        })
    }

    fn build_between(&self, errors: &mut Vec<FieldError>) -> Option<Vec<Expr>> {
        let [low, high] = self.filter.values.as_slice() else {
            errors.push(FieldError::new(
                format!("{}.values", self.field),
                "\"between\" needs exactly two values in \"values\": the low end and the high end, both included",
            ));
            return None;
        };
        let low = self.build_bound(low, "values[0]", errors);
        let high = self.build_bound(high, "values[1]", errors);
        let (low, high) = (low?, high?);
        if !low.is_before_or_same(&high) {
            errors.push(FieldError::new(
                format!("{}.values", self.field),
                "the low end is after the high end; write the smaller value first",
            ));
            return None;
        }
        Some(vec![
            low.build_from(&self.subject()),
            high.build_until(&self.subject()),
        ])
    }

    fn build_compare(&self, op: &str, errors: &mut Vec<FieldError>) -> Option<Vec<Expr>> {
        let bound = self.build_bound(&self.filter.value, "value", errors)?;
        let subject = self.subject();
        let expr = match op {
            ">" => bound.build_after(&subject),
            ">=" => bound.build_from(&subject),
            "<" => bound.build_before(&subject),
            _ => bound.build_until(&subject),
        };
        Some(vec![expr])
    }

    /// Biểu thức bên trái phép so: biểu thức ngày với cột thời gian (cột ngày dạng chữ đọc qua strptime), tên cột với
    /// cột khác.
    fn subject(&self) -> Expr {
        if self.is_time() {
            if let Some(expr) = build_time_expr(self.column) {
                return expr;
            }
        }
        build_ident(&self.column.name)
    }

    fn is_time(&self) -> bool {
        self.column.role == ProfileRole::Time.as_str() && build_time_expr(self.column).is_some()
    }

    /// Một đầu của khoảng, đọc theo kiểu cột: kỳ với cột thời gian, số với cột số, giá trị trong danh sách với cột chiều.
    fn build_bound(&self, text: &str, part: &str, errors: &mut Vec<FieldError>) -> Option<Bound> {
        let field = format!("{}.{part}", self.field);
        if self.is_time() {
            return match parse_date_period(text) {
                Some(period) => Some(Bound::Period(period)),
                None => {
                    errors.push(FieldError::new(
                        field,
                        format!(
                            "\"{text}\" is not a date of \"{}\"; write 2017, 2017-Q3, 2017-05 or 2017-05-31",
                            self.column.name
                        ),
                    ));
                    None
                }
            };
        }
        if is_number_type(&self.column.data_type) {
            return match Decimal::from_str(text.trim()) {
                Ok(number) => Some(Bound::Number(number)),
                Err(_) => {
                    errors.push(FieldError::new(
                        field,
                        format!(
                            "\"{text}\" is not a number; \"{}\" is a number column, write digits with a dot for decimals",
                            self.column.name
                        ),
                    ));
                    None
                }
            };
        }
        if self.column.role == ProfileRole::Dimension.as_str() {
            return self.check_listed(text, part, errors).map(Bound::Text);
        }
        errors.push(FieldError::new(
            format!("{}.op", self.field),
            format!(
                "\"{}\" works on a number column, a column of role time or a column of role dimension; \"{}\" is \
                 a {} column of type {}",
                self.filter.op, self.column.name, self.column.role, self.column.data_type
            ),
        ));
        None
    }

    /// Giá trị của cột chiều, có nguyên văn trong danh sách. Cột không phải chiều thì báo vai trò sai.
    fn check_listed(
        &self,
        value: &str,
        part: &str,
        errors: &mut Vec<FieldError>,
    ) -> Option<String> {
        if self.column.role != ProfileRole::Dimension.as_str() {
            errors.push(wrong_role(
                self.column,
                &format!("{}.column", self.field),
                "dimension",
            ));
            return None;
        }
        check_value(
            self.profile,
            self.column,
            value,
            &format!("{}.{part}", self.field),
            errors,
        )
    }
}
