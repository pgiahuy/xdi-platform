// SPDX-License-Identifier: Apache-2.0
/* service/explain/judge.rs - phần chữ của cổng 2 (#201): prompt cho mô hình chấm, JSON Schema của câu trả lời và cách
đọc câu trả lời. Mô hình chấm tự hiểu JSON số liệu nói gì rồi so với nghĩa của lời diễn giải câu "tại sao", để bắt chỗ
code không đọc được nghĩa câu: lời gán số của nhóm này cho nhóm khác, đảo hai kỳ, hay nêu thêm một lý do không khẳng
định nào đứng sau. Việc gọi mô hình và quá giờ ở ask/judge.rs. Hàm thuần, test ở explain/judge_tests.rs.

Mô hình chấm nhận lời sẽ trả cho người dùng và JudgeFacts: tên độ đo, kiểu so sánh và tên hai bên do code lấy từ bảng
phân rã, cùng các khẳng định đã qua cổng 1. Nó không nhận phần còn lại của bảng, câu hỏi hay prompt của lần viết lời:
khẳng định đã khớp bảng ở cổng 1, nên lời đúng với khẳng định là đúng với bảng. */

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::prompts::{self, fill};
use crate::service::ask::contract::Explanation;
use crate::service::explain::claims::Claim;
use crate::service::explain::comparison_name;
use crate::service::explain::split::build_split;

/// Kết luận của mô hình chấm. Mỗi phần tử của danh sách là một chỗ, câu tiếng Việt do mô hình chấm viết.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Match,
    /// Có chỗ diễn đạt hiểu được nhiều nghĩa nhưng không chắc là sai: lời vẫn trả cho người dùng, kèm các chỗ này.
    Warning(Vec<String>),
    /// Có ít nhất một chỗ trái với JSON hay nói điều JSON không có.
    Mismatch(Vec<String>),
}

/// Những gì mô hình chấm được biết, gửi nguyên dạng JSON. Bộ đo (service/eval_judge.rs) đọc cùng khuôn này từ file ca.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JudgeFacts {
    pub measure: String,
    /// "periods" hay "groups", như trường comparison của Explanation.
    pub comparison: String,
    pub base: String,
    pub compare: String,
    /// Tên của lượng và của giá trị trung bình mỗi đơn vị; trống khi bảng không tách lượng và giá.
    #[serde(default)]
    pub volume_label: String,
    #[serde(default)]
    pub price_label: String,
    pub claims: Vec<Claim>,
}

#[derive(Deserialize)]
struct JudgeReply {
    verdict: String,
    #[serde(default)]
    problems: Vec<String>,
}

/// JudgeFacts của một bảng phân rã: các tên lấy từ bảng do code tính, khẳng định là của lời vừa qua cổng 1.
pub fn build_judge_facts(explanation: &Explanation, claims: &[Claim]) -> JudgeFacts {
    let split = build_split(explanation);
    let split = split.as_ref();
    JudgeFacts {
        measure: explanation.measure.clone(),
        comparison: comparison_name(explanation.comparison).to_string(),
        base: explanation.base.label.clone(),
        compare: explanation.compare.label.clone(),
        volume_label: split.map(|s| s.volume_label.clone()).unwrap_or_default(),
        price_label: split.map(|s| s.price_label.clone()).unwrap_or_default(),
        claims: claims.to_vec(),
    }
}

/// Prompt cho mô hình chấm: luật ở backend/prompts/judge_answer.txt, nghĩa các khóa của câu "tại sao"
/// (judge_meaning_why.txt), JudgeFacts dạng JSON và lời.
pub fn build_judge_prompt(text: &str, facts: &JudgeFacts) -> String {
    build_judge_prompt_with(&prompts::JUDGE_MEANING_WHY, &build_judge_json(facts), text)
}

/// Luật chung điền nghĩa các khóa, JSON và lời. Tách nghĩa các khóa ra file riêng để đoạn trả lời câu thường sau này
/// chấm theo cùng bộ luật với file nghĩa của nó.
pub fn build_judge_prompt_with(
    meaning: &prompts::PromptFile,
    facts_json: &str,
    text: &str,
) -> String {
    let meaning = fill(meaning, &[]);
    fill(
        &prompts::JUDGE_ANSWER,
        &[("meaning", &meaning), ("facts", facts_json), ("text", text)],
    )
}

/// JudgeFacts dạng JSON gửi mô hình chấm. Mỗi khẳng định có số được thêm "amount": cùng số đó viết kiểu máy (dấu chấm
/// thập phân). Thử với Gemini: chỉ có số kiểu Việt Nam thì mô hình chấm đọc "181.592,21" thành khoảng 181 triệu rồi chê
/// câu "tăng khoảng 182 nghìn" là sai; luật về cách viết số trong prompt không đủ.
pub fn build_judge_json(facts: &JudgeFacts) -> String {
    let mut json = serde_json::to_value(facts).expect("serialize judge facts");
    if let Some(claims) = json["claims"].as_array_mut() {
        for claim in claims {
            let amount = claim["value"].as_str().and_then(parse_vietnamese_number);
            if let Some(amount) = amount {
                claim["amount"] = json!(amount);
            }
        }
    }
    serde_json::to_string_pretty(&json).expect("serialize judge facts")
}

/// "-1.283.400,00" thành -1283400.0; chuỗi rỗng hay không đọc được thì None.
pub(crate) fn parse_vietnamese_number(text: &str) -> Option<f64> {
    let plain = text.trim().replace('.', "").replace(',', ".");
    if plain.is_empty() {
        return None;
    }
    plain.parse().ok()
}

/// JSON Schema của câu trả lời, gửi kèm request (LlmRequest::json_schema).
pub fn build_judge_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "verdict": { "type": "string", "enum": ["match", "warning", "mismatch"] },
            "problems": {
                "type": "array",
                "items": { "type": "string" },
                "description": "One short Vietnamese sentence per wrong or unclear point, quoting the words of the text"
            }
        },
        "required": ["verdict", "problems"]
    })
}

/// Đọc câu trả lời của mô hình chấm. Err khi không đọc được: bên gọi coi như chưa chấm được, không coi là khớp.
/// "warning" hay "mismatch" mà không nêu chỗ nào thì kèm một câu chung, để người đọc hay lần viết lại có lý do.
pub fn parse_judge_reply(reply: &str) -> Result<Verdict, String> {
    let parsed: JudgeReply = serde_json::from_str(reply.trim())
        .map_err(|_| "Câu trả lời của mô hình chấm không phải JSON đúng khuôn".to_string())?;
    let problems = |fallback: &str| {
        let mut problems: Vec<String> = parsed
            .problems
            .iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if problems.is_empty() {
            problems.push(fallback.to_string());
        }
        problems
    };
    match parsed.verdict.as_str() {
        "match" => Ok(Verdict::Match),
        "warning" => Ok(Verdict::Warning(problems("Lời có chỗ chưa rõ nghĩa"))),
        "mismatch" => Ok(Verdict::Mismatch(problems("Lời chưa khớp các khẳng định"))),
        other => Err(format!("Mô hình chấm trả kết luận lạ: {other}")),
    }
}
