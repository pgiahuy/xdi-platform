// SPDX-License-Identifier: Apache-2.0
/* service/explain_choices.rs - giả định của câu "tại sao" mà người hỏi đổi được (thẻ "Đã hiểu là", #35): kỳ so sánh của
câu so hai kỳ (id why:base) và chỉ số chưa xác nhận (build_assumption_choices của ask_choices.rs, #29). explain_run.rs
gọi sau khi kiểm form; lần hỏi sau người hỏi gửi lại lựa chọn, explain_check.rs áp nó lên base của form
(build_chosen_base). Test ở explain_check_tests.rs.

Phương án của kỳ so sánh đặt theo nghĩa (previous_quarter, same_quarter_last_year) chứ không theo kỳ cụ thể, như mẫu why
của ask_examples.rs: cùng câu hỏi luôn ra cùng id phương án, kể cả khi mô hình viết base khác đi. */

use super::ask_choices::build_assumption_choices;
use super::ask_context::AskContext;
use super::ask_contract::{Choice, ChoiceOption};
use super::ask_reply::{SqlAnswer, Term, TermKind};
use super::explain_check::{PlanSlices, WhyPlan};
use super::explain_period::{Grain, Period};
use crate::engine::store::MetricDefinition;

/// Id lựa chọn kỳ so sánh của câu so hai kỳ, theo dạng why:<trường> của #38. Phương án: previous_<grain>,
/// same_<grain>_last_year, hoặc một kỳ viết như trong form.
pub const BASE_CHOICE_ID: &str = "why:base";

/// Giả định của câu "tại sao": kỳ so sánh khi so hai kỳ, rồi chỉ số chưa xác nhận (build_assumption_choices của #29,
/// với terms của form cộng một term cho chỉ số form dùng, để chỉ số mô hình quên ghi vào terms vẫn được báo).
pub fn build_why_assumptions(
    context: &AskContext,
    metrics: &[MetricDefinition],
    terms: &[Term],
    plan: &WhyPlan,
) -> Vec<Choice> {
    let mut choices = Vec::new();
    if let PlanSlices::Periods { base, compare, .. } = &plan.slices {
        choices.push(build_base_choice(*base, *compare));
    }
    let mut terms = terms.to_vec();
    if let Some(metric) = &plan.measure.metric {
        let listed = terms
            .iter()
            .any(|t| t.kind == TermKind::Metric && t.value.as_deref() == Some(metric.as_str()));
        if !listed {
            terms.push(Term {
                text: metric.clone(),
                kind: TermKind::Metric,
                column: None,
                value: Some(metric.clone()),
            });
        }
    }
    let answer = SqlAnswer {
        terms,
        assumptions: Vec::new(),
        sql: String::new(),
    };
    choices.extend(build_assumption_choices(&answer, context, metrics));
    choices
}

/// Lựa chọn kỳ so sánh: kỳ liền trước, cùng kỳ năm trước (trừ khi so theo năm, hai kỳ đó trùng nhau), và kỳ đang dùng
/// nếu nó khác cả hai (mô hình hay người hỏi chọn một kỳ khác).
pub fn build_base_choice(base: Period, compare: Period) -> Choice {
    let grain = compare.grain;
    let mut options = vec![(
        format!("previous_{}", grain.as_str()),
        compare.previous(),
        format!("{} liền trước", grain.noun()),
    )];
    if grain != Grain::Year {
        options.push((
            format!("same_{}_last_year", grain.as_str()),
            compare.same_last_year(),
            "Cùng kỳ năm trước".to_string(),
        ));
    }
    let selected = match options.iter().find(|(_, period, _)| *period == base) {
        Some((id, _, _)) => id.clone(),
        None => {
            options.push((base.text(), base, "Kỳ đã chọn".to_string()));
            base.text()
        }
    };
    Choice {
        id: BASE_CHOICE_ID.to_string(),
        label: format!("So {} với", compare.label()),
        selected: Some(selected),
        options: options
            .into_iter()
            .map(|(id, period, name)| ChoiceOption {
                id,
                label: format!("{name} ({})", period.label()),
            })
            .collect(),
    }
}
