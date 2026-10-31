// SPDX-License-Identifier: Apache-2.0
/* service/explain/filter_bound.rs - một đầu của khoảng trong bộ lọc câu "tại sao" (explain/filter.rs, #244): số, giá
trị chữ hay một kỳ, và vế SQL của từng phép so trên đầu đó. Tách khỏi explain/filter.rs vì phần kỳ có luật riêng: kỳ
tính trọn, nên "> 2017-05" là từ đầu tháng 6 và "<= 2017-05" là trước đầu tháng 6. Test ở explain/frames_tests.rs. */

use rust_decimal::Decimal;
use sqlparser::ast::{BinaryOperator, Expr, Value};

use crate::service::explain::period::{Grain, Period};
use crate::service::explain::sql::{build_binary, build_string, build_timestamp};

/// Một đầu của khoảng lọc.
pub(super) enum Bound {
    Number(Decimal),
    Text(String),
    /// Một kỳ trọn vẹn: "> 2017-05" là từ đầu tháng 6.
    Period(Period),
}

impl Bound {
    pub(super) fn is_before_or_same(&self, other: &Bound) -> bool {
        match (self, other) {
            (Bound::Number(a), Bound::Number(b)) => a <= b,
            (Bound::Text(a), Bound::Text(b)) => a <= b,
            (Bound::Period(a), Bound::Period(b)) => match (a.range(), b.range()) {
                (Some((start, _)), Some((_, end))) => start < end,
                _ => false,
            },
            _ => false,
        }
    }

    /// `subject >= đầu`.
    pub(super) fn build_from(&self, subject: &Expr) -> Expr {
        build_binary(subject.clone(), BinaryOperator::GtEq, self.build_start())
    }

    /// `subject <= giá trị`, hay `subject < đầu kỳ sau` với một kỳ.
    pub(super) fn build_until(&self, subject: &Expr) -> Expr {
        match self {
            Bound::Period(_) => build_binary(subject.clone(), BinaryOperator::Lt, self.build_end()),
            _ => build_binary(subject.clone(), BinaryOperator::LtEq, self.build_start()),
        }
    }

    /// `subject > giá trị`, hay `subject >= đầu kỳ sau` với một kỳ.
    pub(super) fn build_after(&self, subject: &Expr) -> Expr {
        match self {
            Bound::Period(_) => {
                build_binary(subject.clone(), BinaryOperator::GtEq, self.build_end())
            }
            _ => build_binary(subject.clone(), BinaryOperator::Gt, self.build_start()),
        }
    }

    /// `subject < giá trị`, hay `subject < đầu kỳ` với một kỳ.
    pub(super) fn build_before(&self, subject: &Expr) -> Expr {
        build_binary(subject.clone(), BinaryOperator::Lt, self.build_start())
    }

    /// Giá trị, hay ngày đầu của kỳ.
    fn build_start(&self) -> Expr {
        match self {
            Bound::Number(number) => build_number(*number),
            Bound::Text(text) => build_string(text),
            Bound::Period(period) => build_timestamp(period.range().unwrap_or_default().0),
        }
    }

    /// Ngày đầu của kỳ sau; với số và chữ là chính giá trị.
    fn build_end(&self) -> Expr {
        match self {
            Bound::Period(period) => build_timestamp(period.range().unwrap_or_default().1),
            _ => self.build_start(),
        }
    }
}

fn build_number(number: Decimal) -> Expr {
    Expr::Value(Value::Number(number.normalize().to_string(), false).with_empty_span())
}

/// Kỳ của một giá trị ngày trong bộ lọc: thử lần lượt ngày, tháng, quý, năm theo cách viết của Period::parse.
pub(super) fn parse_date_period(text: &str) -> Option<Period> {
    [
        Grain::Day,
        Grain::Week,
        Grain::Month,
        Grain::Quarter,
        Grain::Year,
    ]
    .into_iter()
    .find_map(|grain| Period::parse(grain, text).ok())
}
