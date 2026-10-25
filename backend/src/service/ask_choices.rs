// SPDX-License-Identifier: Apache-2.0
/* service/ask_choices.rs - đổi cách hiểu mô hình ghi trong câu trả lời (ask_reply.rs) thành lựa chọn của hợp đồng hỏi
(Choice): giả định đã chọn sẵn và câu hỏi lại. Endpoint hỏi (#33) gọi các hàm ở đây sau khi đọc câu trả lời, đưa giả
định vào thẻ "Đã hiểu là" (#35) và câu hỏi lại vào clarifications.

Id do code dựng, không lấy của mô hình: loại lựa chọn cộng cụm chữ đã bỏ dấu, cùng cách với id lựa chọn cột của #28
(build_column_choice_id), nên cùng câu hỏi trên cùng phiên bản luôn ra cùng id dù mô hình chọn khác đi. Id lựa chọn chỉ
số theo cụm chữ chứ không theo tên chỉ số: tên chỉ số là phương án, người dùng đổi phương án thì id phải giữ nguyên để
lần hỏi sau áp được. Phương án không có trong ngữ cảnh (cột, chỉ số không có thật) bị bỏ.

Chỉ số chưa xác nhận luôn hiện rõ: phương án mang nhãn "chưa xác nhận", nhãn giả định nói rõ khi chỉ số đang dùng chưa
xác nhận, và câu trả lời dùng chỉ số đó mà mô hình không ghi giả định thì code tự thêm một giả định. */

use super::ask_context::{AskContext, ColumnProfile};
use super::ask_contract::{Choice, ChoiceOption};
use super::ask_phrases::{build_key, fold};
use super::ask_reply::{ChoiceKind, Clarify, SqlAnswer, Term, TermKind};
use crate::engine::store::MetricDefinition;

pub const COLUMN_CHOICE_PREFIX: &str = "column:";
pub const METRIC_CHOICE_PREFIX: &str = "metric:";
/// Một câu hỏi chỉ có một khoảng thời gian, nên mọi giả định về thời gian chung một id.
pub const TIME_CHOICE_ID: &str = "time:range";
/// Phương án "toàn bộ dữ liệu" của giả định thời gian.
pub const ALL_TIME: &str = "all";
/// Phần sau tiền tố khi giả định không gắn với cụm chữ nào của câu hỏi.
const NO_PHRASE: &str = "default";

/// Id của một lựa chọn. phrase là cụm chữ của câu hỏi mà lựa chọn nói tới.
pub fn build_choice_id(kind: ChoiceKind, phrase: Option<&str>) -> String {
    let phrase = phrase
        .map(fold)
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| NO_PHRASE.to_string());
    match kind {
        ChoiceKind::Time => TIME_CHOICE_ID.to_string(),
        ChoiceKind::Column => format!("{COLUMN_CHOICE_PREFIX}{phrase}"),
        ChoiceKind::Metric => format!("{METRIC_CHOICE_PREFIX}{phrase}"),
    }
}

/// Giả định của câu trả lời thành Choice, theo thứ tự mô hình ghi, rồi giả định code tự thêm cho chỉ số chưa xác nhận.
pub fn build_assumption_choices(
    answer: &SqlAnswer,
    context: &AskContext,
    metrics: &[MetricDefinition],
) -> Vec<Choice> {
    let mut choices: Vec<Choice> = Vec::new();
    for assumption in &answer.assumptions {
        let id = build_choice_id(assumption.kind, assumption.phrase.as_deref());
        // Lựa chọn cột của #28 cho cùng cụm đã có (giả định của code hay lựa chọn của người dùng): giữ cái của code.
        if context.picks.iter().any(|p| p.choice_id == id) || choices.iter().any(|c| c.id == id) {
            continue;
        }
        let names = std::iter::once(&assumption.chosen).chain(&assumption.alternatives);
        let options = build_options(assumption.kind, names, context, metrics);
        // Phương án đã chọn không có thật thì giả định vô nghĩa; chỗ sai đó để guard và bước kiểm terms (#33) xử lý.
        if !options.iter().any(|o| o.id == assumption.chosen) {
            continue;
        }
        let label = match find_unconfirmed(assumption.kind, &assumption.chosen, metrics) {
            Some(name) => format!(
                "{} (chỉ số \"{name}\" chưa được xác nhận)",
                assumption.label
            ),
            None => assumption.label.clone(),
        };
        choices.push(Choice {
            id,
            label,
            selected: Some(assumption.chosen.clone()),
            options,
        });
    }
    for term in answer.terms.iter().filter(|t| t.kind == TermKind::Metric) {
        let Some(metric) = term.value.as_deref().and_then(|n| find_metric(metrics, n)) else {
            continue;
        };
        let id = build_choice_id(ChoiceKind::Metric, Some(&term.text));
        if metric.confirmed || choices.iter().any(|c| c.id == id) {
            continue;
        }
        choices.push(Choice {
            id,
            label: format!(
                "Chỉ số \"{}\" chưa được xác nhận: công thức đang dùng là bản nháp",
                metric.name
            ),
            selected: Some(metric.name.clone()),
            options: vec![build_metric_option(metric)],
        });
    }
    choices
}

/// Câu hỏi lại thành Choice chưa chọn. None khi không còn phương án nào có thật: #33 coi như mô hình trả lời sai.
pub fn build_clarify_choice(
    clarify: &Clarify,
    context: &AskContext,
    metrics: &[MetricDefinition],
) -> Option<Choice> {
    let options = build_options(clarify.kind, clarify.options.iter(), context, metrics);
    if options.is_empty() {
        // Ghi câu hỏi lại và các phương án mô hình đưa (tên cột, tên chỉ số, giá trị hay kỳ; không có dòng dữ liệu)
        // để chẩn đoán vì sao không phương án nào khớp ngữ cảnh.
        tracing::info!(kind = ?clarify.kind, question = %clarify.question, options = ?clarify.options,
            "clarify has no option found in the data");
        return None;
    }
    Some(Choice {
        id: build_choice_id(clarify.kind, clarify.phrase.as_deref()),
        label: clarify.question.clone(),
        selected: None,
        options,
    })
}

/// Chỉ số câu trả lời nói đã dùng (terms) mà SQL không có đúng định nghĩa của nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricMismatch {
    pub metric: String,
    /// Phần định nghĩa không thấy trong SQL: công thức, hoặc điều kiện đi kèm.
    pub missing: String,
}

/// So theo chữ, bỏ khoảng trắng, không phân biệt hoa thường. Hàm không hiểu SQL nên chỉ để kiểm thêm, không chặn câu:
/// SQL viết công thức khác chữ mà cùng nghĩa (đổi thứ tự hai vế) vẫn bị báo. #33 hay #36 quyết định cách hiện.
pub fn build_metric_mismatches(
    terms: &[Term],
    metrics: &[MetricDefinition],
    sql: &str,
) -> Vec<MetricMismatch> {
    let sql = compact(sql);
    let mut mismatches: Vec<MetricMismatch> = Vec::new();
    for term in terms.iter().filter(|t| t.kind == TermKind::Metric) {
        let Some(metric) = term.value.as_deref().and_then(|n| find_metric(metrics, n)) else {
            continue;
        };
        if mismatches.iter().any(|m| m.metric == metric.name) {
            continue;
        }
        let parts = std::iter::once(&metric.expression).chain(metric.filter.as_ref());
        if let Some(missing) = parts.into_iter().find(|p| !sql.contains(&compact(p))) {
            mismatches.push(MetricMismatch {
                metric: metric.name.clone(),
                missing: missing.clone(),
            });
        }
    }
    mismatches
}

/// Phương án có thật trong ngữ cảnh, giữ thứ tự, bỏ trùng và bỏ chuỗi rỗng. Id của phương án cột và chỉ số là tên
/// đúng như trong ngữ cảnh, không phải chữ mô hình viết, nên hai cách viết của cùng một cột chỉ thành một phương án.
fn build_options<'a>(
    kind: ChoiceKind,
    names: impl Iterator<Item = &'a String>,
    context: &AskContext,
    metrics: &[MetricDefinition],
) -> Vec<ChoiceOption> {
    let mut options: Vec<ChoiceOption> = Vec::new();
    for name in names.filter(|n| !n.trim().is_empty()) {
        let option = match kind {
            ChoiceKind::Column => find_column(context, name).map(|c| ChoiceOption {
                id: c.name.clone(),
                label: c.name.clone(),
            }),
            ChoiceKind::Metric => find_metric(metrics, unquote(name)).map(build_metric_option),
            // Kỳ không có danh sách để đối chiếu; bước kiểm terms của #33 kiểm kỳ có dữ liệu.
            ChoiceKind::Time => Some(build_time_option(name)),
        };
        if let Some(option) = option {
            if !options.iter().any(|o| o.id == option.id) {
                options.push(option);
            }
        }
    }
    options
}

/// Bỏ khoảng trắng và một cặp nháy bao ngoài (", ', `). Prompt in tên cột trong nháy kép và dặn mô hình viết tên cột
/// trong nháy kép trong SQL, nên mô hình hay viết phương án là "\"Sales\"" thay vì "Sales"; không bỏ thì không phương
/// án nào khớp và câu hỏi lại thành lỗi.
fn unquote(name: &str) -> &str {
    let name = name.trim();
    for quote in ['"', '\'', '`'] {
        if let Some(inner) = name
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.trim();
        }
    }
    name
}

/// Cột mà mô hình đưa làm phương án. Thử lần lượt: đúng tên; tên so không phân biệt hoa thường và cách đặt dấu thanh
/// (build_key, như find_metric); rồi bí danh của hồ sơ ngữ nghĩa, vì prompt in bí danh cạnh tên cột ("Also called")
/// nên mô hình có khi đưa bí danh. Bí danh chỉ nhận khi đúng một cột có nó, để không đoán cột thay người dùng.
fn find_column<'a>(context: &'a AskContext, name: &str) -> Option<&'a ColumnProfile> {
    let name = unquote(name);
    if let Some(column) = context.columns.iter().find(|c| c.name == name) {
        return Some(column);
    }
    let key = build_key(name);
    if let Some(column) = context.columns.iter().find(|c| build_key(&c.name) == key) {
        return Some(column);
    }
    let mut by_alias = context
        .columns
        .iter()
        .filter(|c| c.aliases.iter().any(|a| build_key(a) == key));
    match (by_alias.next(), by_alias.next()) {
        (Some(column), None) => Some(column),
        _ => None,
    }
}

fn build_metric_option(metric: &MetricDefinition) -> ChoiceOption {
    let label = if metric.confirmed {
        metric.name.clone()
    } else {
        format!("{} (chưa xác nhận)", metric.name)
    };
    ChoiceOption {
        id: metric.name.clone(),
        label,
    }
}

fn build_time_option(period: &str) -> ChoiceOption {
    let label = if period == ALL_TIME {
        "Toàn bộ dữ liệu".to_string()
    } else {
        period.to_string()
    };
    ChoiceOption {
        id: period.to_string(),
        label,
    }
}

/// Tên chỉ số như mô hình viết, so không phân biệt hoa thường và cách đặt dấu thanh ("hoà" và "hòa").
fn find_metric<'a>(metrics: &'a [MetricDefinition], name: &str) -> Option<&'a MetricDefinition> {
    let key = build_key(name);
    metrics.iter().find(|m| build_key(&m.name) == key)
}

/// Tên chỉ số chưa xác nhận khi giả định là về chỉ số và phương án đã chọn là chỉ số chưa xác nhận.
fn find_unconfirmed(
    kind: ChoiceKind,
    chosen: &str,
    metrics: &[MetricDefinition],
) -> Option<String> {
    if kind != ChoiceKind::Metric {
        return None;
    }
    find_metric(metrics, chosen)
        .filter(|m| !m.confirmed)
        .map(|m| m.name.clone())
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}
