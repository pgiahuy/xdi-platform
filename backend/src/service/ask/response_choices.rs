// SPDX-License-Identifier: Apache-2.0
/* service/ask/response_choices.rs - phần lựa chọn của AskResponse mà endpoint hỏi (service/ask/mod.rs) dựng: câu hỏi lại
khi một cụm khớp nhiều cột (hỏi trước khi gọi mô hình), danh sách lựa chọn đã dùng (AskResponse.choices) và cảnh báo
choice_not_applied. Cách hiểu của mô hình thành Choice (giả định, câu hỏi lại của mô hình) ở ask/choices.rs; file này
chỉ gom kết quả đó cùng ngữ cảnh (ask/context.rs) thành dạng của hợp đồng.

Phương án hỏi lại nói bằng nghĩa của cột trước tên cột: người hỏi nghĩ theo chữ mình gõ, không biết dữ liệu chia thành
cột nào. */

use crate::service::ask::context::{AskContext, ColumnProfile};
use crate::service::ask::contract::{Choice, ChoiceOption, ChosenOption, Warning, WarningCode};

/// Số nhóm ví dụ trong nhãn một phương án: đủ để thấy cách chia đó là gì (châu lục hay vùng nhỏ hơn), không dài thành
/// cả danh sách.
const EXAMPLE_VALUES: usize = 3;

/// Lựa chọn câu trả lời đã dùng, theo dạng AskRequest.choices: cột ngữ cảnh chọn cho một cụm (khi còn cột khác để đổi,
/// hoặc người hỏi đã chọn), rồi giả định của câu trả lời (build_assumption_choices của ask/choices.rs). Không cần bỏ id
/// trùng: build_assumption_choices đã bỏ giả định trùng id với cột của ngữ cảnh.
pub fn build_used_choices(context: &AskContext, assumptions: &[Choice]) -> Vec<ChosenOption> {
    let picks = context
        .picks
        .iter()
        .filter(|pick| pick.chosen_by_user || !pick.others.is_empty())
        .map(|pick| build_chosen(&pick.choice_id, &pick.column));
    let chosen_assumptions = assumptions.iter().filter_map(|choice| {
        let selected = choice.selected.as_deref()?;
        Some(build_chosen(&choice.id, selected))
    });
    picks.chain(chosen_assumptions).collect()
}

/// Cảnh báo choice_not_applied cho mỗi lựa chọn người hỏi gửi mà câu trả lời không dùng: cụm không còn trong câu, cột
/// không còn là ứng viên, hay mô hình hiểu khác lựa chọn đó.
pub fn build_unapplied_warnings(sent: &[ChosenOption], used: &[ChosenOption]) -> Vec<Warning> {
    sent.iter()
        .filter(|choice| {
            !used
                .iter()
                .any(|u| u.choice_id == choice.choice_id && u.option_id == choice.option_id)
        })
        .map(|choice| Warning {
            code: WarningCode::ChoiceNotApplied,
            message: format!(
                "Lựa chọn \"{}\" không áp dụng được cho câu hỏi này, đã dùng cách hiểu mặc định",
                choice.option_id
            ),
        })
        .collect()
}

/// Lời hỏi lại khi một cụm khớp nhiều cột gần nhau. Hỏi bằng lời thường, không nói "cột".
pub fn build_ambiguity_message(context: &AskContext) -> String {
    match context.ambiguous.as_slice() {
        [word] => format!(
            "Bạn muốn xem \"{}\" theo cách nào? Mỗi cách cho ra kết quả khác nhau.",
            word.phrase
        ),
        words => format!(
            "Câu hỏi có {} chỗ hiểu được nhiều cách, mỗi cách cho ra kết quả khác nhau. Chọn cách hiểu cho từng chỗ \
             rồi hỏi lại.",
            words.len()
        ),
    }
}

/// Mỗi chỗ mơ hồ một câu hỏi lại, mỗi cột ứng viên một phương án. Id là id của ngữ cảnh (`column:` cộng cụm đã bỏ dấu),
/// để lần hỏi sau build_ask_context áp đúng lựa chọn.
pub fn build_ambiguity_choices(context: &AskContext) -> Vec<Choice> {
    context
        .ambiguous
        .iter()
        .map(|word| Choice {
            id: word.choice_id.clone(),
            label: format!("Hiểu \"{}\" là", word.phrase),
            selected: None,
            options: word
                .columns
                .iter()
                .map(|column| build_column_option(column, context))
                .collect(),
        })
        .collect()
}

/// Lý do hỏi lại cho bước context, ví dụ `"khu vực" khớp 2 cột: Order Region, Market`; None khi không mơ hồ.
pub fn build_ambiguity_detail(context: &AskContext) -> Option<String> {
    if context.ambiguous.is_empty() {
        return None;
    }
    let parts: Vec<String> = context
        .ambiguous
        .iter()
        .map(|word| {
            format!(
                "\"{}\" khớp {} cột: {}",
                word.phrase,
                word.columns.len(),
                word.columns.join(", ")
            )
        })
        .collect();
    Some(parts.join("; "))
}

fn build_chosen(choice_id: &str, option_id: &str) -> ChosenOption {
    ChosenOption {
        choice_id: choice_id.to_string(),
        option_id: option_id.to_string(),
    }
}

/// Phương án là một cột, nói bằng nghĩa của nó trước rồi mới tới tên cột trong ngoặc, ví dụ "5 nhóm, như Europe, LATAM,
/// Pacific Asia (Market)": người hỏi nhận ra cách chia qua dữ liệu, còn tên cột để ai cần thì đối chiếu với câu SQL.
/// Không có gì để tóm tắt thì chỉ còn tên cột. Id vẫn là tên cột, vì ngữ cảnh so lựa chọn gửi lại theo tên cột.
fn build_column_option(column: &str, context: &AskContext) -> ChoiceOption {
    let summary = context
        .columns
        .iter()
        .find(|c| c.name == column)
        .and_then(build_column_summary);
    let label = match summary {
        Some(summary) => format!("{summary} ({column})"),
        None => column.to_string(),
    };
    ChoiceOption {
        id: column.to_string(),
        label,
    }
}

/// Mô tả của hồ sơ (#23) nếu có, rồi vài nhóm nhiều dòng nhất. Mô tả nói được nghĩa ("thị trường theo châu lục"), còn
/// không có thì chính các nhóm cho người hỏi đoán ra nghĩa. Danh sách giá trị chỉ là mẫu thì không nêu tổng số nhóm, vì
/// không biết chắc.
fn build_column_summary(profile: &ColumnProfile) -> Option<String> {
    let examples: Vec<&str> = profile
        .values
        .iter()
        .take(EXAMPLE_VALUES)
        .map(|v| v.value.as_str())
        .collect();
    let groups = match (examples.is_empty(), profile.values_complete) {
        (true, _) => None,
        (false, true) if profile.values.len() <= EXAMPLE_VALUES => {
            Some(format!("gồm {}", examples.join(", ")))
        }
        (false, true) => Some(format!(
            "{} nhóm, như {}",
            profile.values.len(),
            examples.join(", ")
        )),
        (false, false) => Some(format!("nhiều nhóm, như {}", examples.join(", "))),
    };
    match (&profile.description, groups) {
        (Some(description), Some(groups)) => Some(format!("{description}: {groups}")),
        (Some(description), None) => Some(description.clone()),
        (None, Some(groups)) => Some(groups),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ask::context::ColumnPick;

    fn empty_context() -> AskContext {
        AskContext {
            columns: vec![],
            hints: vec![],
            ambiguous: vec![],
            picks: vec![],
        }
    }

    fn pick(choice_id: &str, column: &str, others: &[&str], chosen_by_user: bool) -> ColumnPick {
        ColumnPick {
            choice_id: choice_id.to_string(),
            phrase: String::new(),
            column: column.to_string(),
            others: others.iter().map(|o| o.to_string()).collect(),
            chosen_by_user,
        }
    }

    /// Cột chỉ một ứng viên mà người hỏi không chọn thì không có gì để đổi, nên không vào choices.
    #[test]
    fn used_choices_keep_changeable_picks_then_assumptions() {
        let context = AskContext {
            picks: vec![
                pick("column:khu vuc", "Market", &["Order Region"], false),
                pick("column:san pham", "Product Name", &[], false),
                pick("column:quoc gia", "Order Country", &[], true),
            ],
            ..empty_context()
        };
        let assumptions = [
            Choice {
                id: "time:range".to_string(),
                label: String::new(),
                selected: Some("all".to_string()),
                options: vec![],
            },
            Choice {
                id: "metric:doanh thu".to_string(),
                label: String::new(),
                selected: None,
                options: vec![],
            },
        ];
        let used = build_used_choices(&context, &assumptions);
        let pairs: Vec<(&str, &str)> = used
            .iter()
            .map(|c| (c.choice_id.as_str(), c.option_id.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("column:khu vuc", "Market"),
                ("column:quoc gia", "Order Country"),
                ("time:range", "all")
            ]
        );
    }

    #[test]
    fn sent_choice_is_applied_only_when_the_same_option_is_used() {
        let used = [build_chosen("time:range", "2017")];
        let sent = [
            build_chosen("time:range", "2017"),
            build_chosen("time:range", "2018"),
        ];
        let warnings = build_unapplied_warnings(&sent, &used);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("2018"));
    }

    fn profile(name: &str, description: Option<&str>, values: &[&str]) -> ColumnProfile {
        ColumnProfile {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            data_type: "VARCHAR".to_string(),
            role: "dimension".to_string(),
            time_format: None,
            description: description.map(str::to_string),
            aliases: vec![],
            is_sensitive: false,
            values: values
                .iter()
                .map(|v| crate::service::ask::values::ValueEntry {
                    value: v.to_string(),
                    aliases: vec![],
                })
                .collect(),
            values_complete: true,
            matched: true,
            unit: None,
            aggregation: crate::engine::store::ColumnAggregation::Sum,
            join_key: false,
            metric_column: false,
            brief: false,
        }
    }

    /// Hồ sơ có mô tả (#23) thì phương án nói bằng nghĩa trước; chưa có thì chỉ bằng các nhóm của dữ liệu.
    #[test]
    fn option_label_speaks_by_meaning_first() {
        let regions = [
            "Western Europe",
            "South America",
            "Central America",
            "Oceania",
        ];
        let context = AskContext {
            columns: vec![
                profile(
                    "Market",
                    Some("Thị trường theo châu lục"),
                    &["Europe", "LATAM"],
                ),
                profile("Order Region", None, &regions),
                profile("Notes", None, &[]),
            ],
            ..empty_context()
        };
        let label = |column: &str| build_column_option(column, &context).label;
        assert_eq!(
            label("Market"),
            "Thị trường theo châu lục: gồm Europe, LATAM (Market)"
        );
        assert_eq!(
            label("Order Region"),
            "4 nhóm, như Western Europe, South America, Central America (Order Region)"
        );
        assert_eq!(label("Notes"), "Notes");
        assert_eq!(build_column_option("Market", &context).id, "Market");
    }
}
