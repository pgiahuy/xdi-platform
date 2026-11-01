// SPDX-License-Identifier: Apache-2.0
/* service/explain/claims.rs - cổng 1 của lời diễn giải câu "tại sao" (#201): mô hình trả lời kèm danh sách khẳng định
(số nào, của nhóm nào, chiều nào), code so từng khẳng định với bảng phân rã đã tính. ask/narrate.rs gọi
build_claim_check sau khi đọc câu trả lời; trượt thì nhờ mô hình viết lại một lần kèm lý do (build_rewrite_prompt),
vẫn trượt thì dùng câu khuôn. Từng khẳng định xét ở explain/claim_cell.rs. Hàm thuần, test ở explain/claims_tests.rs.

Vì sao so theo khẳng định chứ không dò số trong lời: một số có thể nằm đâu đó trong bảng mà không phải của nhóm lời
nêu. Khẳng định nói rõ số của nhóm nào, nên code so được đúng ô.

Cổng này chỉ so khẳng định với bảng:
- mỗi khẳng định trỏ vào một ô có thật (tổng, một nhóm hay một chiều, lượng và giá, phần trăm trong các ý); khẳng định
  có số thì chép đúng giá trị ô đó, kể cả dấu, nên số sai chiều là số sai;
- nhóm nêu là nguyên nhân chính đi cùng chiều chênh lệch và nằm trong ba nhóm đóng góp lớn nhất (theo trị tuyệt đối)
  của cột; nhóm kéo lại đi ngược chiều.
- quy mô là ý chính (Split::scale_main, explain/split.rs) thì phải có khẳng định về lượng: số lượng hai bên, hay phần
  do lượng (#260). Lời bỏ ý này (chỉ kể các nhóm khi CANCELED có 19 đơn so với 597 đơn của COMPLETE) là thiếu ý chính.
Lời có nói đúng như các khẳng định không là việc của cổng 2 (mô hình chấm độc lập, phần sau của #201). */

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::service::ask::contract::Explanation;
use crate::service::explain::claim_cell::check_claim;
use crate::service::explain::narrate_prompt::build_strong_groups;
use crate::service::explain::number::build_number_text_with;
use crate::service::explain::split::build_split;
use crate::service::explain::{build_fraction_digits, build_value_text, lower_first};

/// Câu trả lời của mô hình ở bước narrate, đúng build_narrate_schema: insight (câu mở đầu và các ý "Nguyên nhân
/// chính") kèm khai số.
#[derive(Debug, Clone, Deserialize)]
pub struct NarrateReply {
    pub headline: String,
    #[serde(default)]
    pub reasons: Vec<Reason>,
    #[serde(default)]
    pub claims: Vec<Claim>,
}

/// Một ý "Nguyên nhân chính": đầu ý ngắn và một câu.
#[derive(Debug, Clone, Deserialize)]
pub struct Reason {
    pub lead: String,
    pub text: String,
}

impl NarrateReply {
    /// Insight dạng một đoạn chữ, như build_template_summary nối câu khuôn: câu mở đầu rồi từng ý "Đầu ý: câu". Đây là
    /// chữ mô hình chấm đọc và chữ ghi vào Explanation.summary.
    pub fn build_text(&self) -> String {
        let mut text = self.headline.trim().to_string();
        for reason in &self.reasons {
            text.push_str(&format!(" {}: {}", reason.lead.trim(), reason.text.trim()));
        }
        text
    }
}

/// Một khẳng định. column và group trống khi khẳng định nói về cả độ đo hay về lượng và giá; group trống, column có khi
/// nói về phần cơ cấu, hiệu suất của cả một chiều. Serialize để gửi nguyên các khẳng định cho mô hình chấm (cổng 2).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Claim {
    pub field: ClaimField,
    #[serde(default)]
    pub column: String,
    #[serde(default)]
    pub group: String,
    /// Số chép từ bảng, có dấu như trong bảng (âm khi giảm, thấp hơn), không "%"; trống với cause, against, new và
    /// gone. Chiều nằm ở dấu, nên khẳng định không có trường chiều riêng; prompt gửi sẵn số có dấu ("value" của mỗi
    /// chênh lệch) để mô hình chép y nguyên thay vì tự thêm dấu.
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimField {
    BaseValue,
    CompareValue,
    Change,
    Contribution,
    PercentChange,
    ShareOfChange,
    MixEffect,
    RateEffect,
    BaseVolume,
    CompareVolume,
    BasePrice,
    ComparePrice,
    VolumeEffect,
    PriceEffect,
    IfVolumeUnchanged,
    /// Phần trăm của chênh lệch do lượng, do giá trị trung bình mỗi đơn vị (explain/split.rs).
    VolumeShare,
    PriceShare,
    /// Nhóm được nêu là nguyên nhân chính, không kèm số.
    Cause,
    /// Nhóm được nêu là đi ngược chiều, kéo lại một phần, không kèm số.
    Against,
    /// Nhóm được nêu là chỉ có ở bên được so (không có ở bên gốc), không kèm số.
    New,
    /// Nhóm được nêu là chỉ có ở bên gốc (không còn ở bên được so), không kèm số.
    Gone,
}

impl ClaimField {
    pub const ALL: [ClaimField; 21] = [
        Self::BaseValue,
        Self::CompareValue,
        Self::Change,
        Self::Contribution,
        Self::PercentChange,
        Self::ShareOfChange,
        Self::MixEffect,
        Self::RateEffect,
        Self::BaseVolume,
        Self::CompareVolume,
        Self::BasePrice,
        Self::ComparePrice,
        Self::VolumeEffect,
        Self::PriceEffect,
        Self::IfVolumeUnchanged,
        Self::VolumeShare,
        Self::PriceShare,
        Self::Cause,
        Self::Against,
        Self::New,
        Self::Gone,
    ];

    /// Tên trong JSON, như serde đọc.
    pub fn name(self) -> &'static str {
        match self {
            Self::BaseValue => "base_value",
            Self::CompareValue => "compare_value",
            Self::Change => "change",
            Self::Contribution => "contribution",
            Self::PercentChange => "percent_change",
            Self::ShareOfChange => "share_of_change",
            Self::MixEffect => "mix_effect",
            Self::RateEffect => "rate_effect",
            Self::BaseVolume => "base_volume",
            Self::CompareVolume => "compare_volume",
            Self::BasePrice => "base_price",
            Self::ComparePrice => "compare_price",
            Self::VolumeEffect => "volume_effect",
            Self::PriceEffect => "price_effect",
            Self::IfVolumeUnchanged => "if_volume_unchanged",
            Self::VolumeShare => "volume_share",
            Self::PriceShare => "price_share",
            Self::Cause => "cause",
            Self::Against => "against",
            Self::New => "new",
            Self::Gone => "gone",
        }
    }
}

/// JSON Schema của câu trả lời narrate, gửi kèm request (LlmRequest::json_schema).
pub fn build_narrate_schema() -> serde_json::Value {
    let fields: Vec<&str> = ClaimField::ALL.iter().map(|f| f.name()).collect();
    json!({
        "type": "object",
        "properties": {
            "headline": {
                "type": "string",
                "description": "One Vietnamese sentence with the measure, the two sides and the change"
            },
            "reasons": {
                "type": "array",
                "description": "The main causes, most important first",
                "items": {
                    "type": "object",
                    "properties": {
                        "lead": { "type": "string", "description": "A short Vietnamese lead naming the cause" },
                        "text": { "type": "string", "description": "One Vietnamese sentence with its numbers" }
                    },
                    "required": ["lead", "text"]
                }
            },
            "claims": {
                "type": "array",
                "description": "One item for every number the headline and the reasons use and every group they name as a cause or as going against the change",
                "items": {
                    "type": "object",
                    "properties": {
                        "field": {
                            "type": "string",
                            "enum": fields,
                            "description": "Which number: the key it has in the JSON; cause for a group named as \
                                a main cause, against for a group named as going against the change, new or gone \
                                for a group named as existing only on the compared side or only on the base side"
                        },
                        "column": {
                            "type": "string",
                            "description": "Dimension column as in the breakdown; empty for the whole measure \
                                and for volume_price"
                        },
                        "group": {
                            "type": "string",
                            "description": "Group value as in the breakdown; empty when the claim is not about \
                                one group"
                        },
                        "value": {
                            "type": "string",
                            "description": "The number copied exactly from the JSON (for a change, its \"value\", \
                                with the sign), without %; empty for cause, against, new and gone"
                        }
                    },
                    "required": ["field", "column", "group", "value"]
                }
            }
        },
        "required": ["headline", "reasons", "claims"]
    })
}

/// Đọc câu trả lời của mô hình. Err là lý do bằng tiếng Việt, cho bước narrate và cho lần viết lại.
pub fn parse_narrate_reply(reply: &str) -> Result<NarrateReply, String> {
    serde_json::from_str(reply.trim()).map_err(|error| {
        tracing::warn!(%error, "narrate reply is not the expected JSON");
        "Câu trả lời không phải JSON đúng khuôn {headline, reasons, claims}".to_string()
    })
}

/// Prompt của lần viết lại: prompt gốc cộng các lỗi và chỗ chưa rõ hai cổng bắt được ở lần trước, để mô hình biết
/// sửa chỗ nào. Câu thường (ask/summary.rs) dùng cùng hàm này.
pub fn build_rewrite_prompt(prompt: &str, problems: &[String]) -> String {
    let list: Vec<String> = problems.iter().map(|p| format!("- {p}")).collect();
    format!(
        "{prompt}\n\nYour previous reply did not pass the checks below (written in Vietnamese): some points are wrong \
         or can be read in more than one way. Write a new reply that fixes every one of them and still follows the \
         rules above.\n{}",
        list.join("\n")
    )
}

/// Cổng 1: Ok là số khẳng định đã khớp bảng; Err là mọi lý do trượt, không trùng nhau.
pub fn build_claim_check(
    claims: &[Claim],
    explanation: &Explanation,
) -> Result<usize, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    for claim in claims {
        if let Err(problem) = check_claim(claim, explanation) {
            if !problems.contains(&problem) {
                problems.push(problem);
            }
        }
    }
    if let Some(problem) = check_scale_claims(claims, explanation) {
        problems.push(problem);
    }
    problems.extend(check_strong_claims(claims, explanation));
    if problems.is_empty() {
        Ok(claims.len())
    } else {
        Err(problems)
    }
}

/// Nhóm một mình làm ra từ nửa chênh lệch trở lên ở một chiều gửi cho mô hình (build_strong_groups) mà lời không nhắc:
/// lời thiếu nguyên nhân chính. Đủ khi có một khẳng định bất kỳ về đúng nhóm đó (nguyên nhân, phần chênh lệch, số của
/// nhóm).
fn check_strong_claims(claims: &[Claim], explanation: &Explanation) -> Vec<String> {
    let mut problems = Vec::new();
    for (dimension, group) in build_strong_groups(explanation) {
        let Some(value) = group.value.as_ref().map(build_value_text) else {
            continue;
        };
        let named = claims
            .iter()
            .any(|c| c.column == dimension.column && c.group == value);
        if !named {
            let label = dimension.label.as_deref().unwrap_or(&dimension.column);
            problems.push(format!(
                "Lời thiếu nguyên nhân chính: theo {label}, nhóm {value} một mình làm ra từ một nửa chênh lệch trở lên; \
                 phải nêu nhóm này kèm khẳng định cause (column {}, group {value})",
                dimension.column
            ));
        }
    }
    problems
}

/// Quy mô là ý chính mà không khẳng định nào nói về lượng: lý do trượt, gửi lại cho mô hình ở lần viết lại. Đủ khi có
/// số lượng của cả hai bên, hay phần do lượng (số hay phần trăm).
fn check_scale_claims(claims: &[Claim], explanation: &Explanation) -> Option<String> {
    let split = build_split(explanation).filter(|s| s.scale_main)?;
    let has = |field: ClaimField| claims.iter().any(|c| c.field == field);
    let counts = has(ClaimField::BaseVolume) && has(ClaimField::CompareVolume);
    if counts || has(ClaimField::VolumeEffect) || has(ClaimField::VolumeShare) {
        return None;
    }
    let digits = build_fraction_digits(explanation);
    let volume = lower_first(&split.volume_label);
    Some(format!(
        "Lời thiếu ý chính: {volume} hai bên chênh nhiều ({} có {}, {} có {}) và chênh lệch chủ yếu do {volume}; ý đầu \
         tiên phải nói điều này, kèm khẳng định base_volume và compare_volume",
        explanation.base.label,
        build_number_text_with(&split.numbers.base_volume, digits),
        explanation.compare.label,
        build_number_text_with(&split.numbers.compare_volume, digits),
    ))
}
