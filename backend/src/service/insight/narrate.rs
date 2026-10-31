// SPDX-License-Identifier: Apache-2.0
/* service/insight/narrate.rs - lời của phần giải thích câu thường (#227): nhờ mô hình diễn đạt lại câu chính và các ý do
code viết (insight/facts.rs) thành một đoạn ngắn, rồi kiểm lời đó bằng code; không qua thì dùng chính các câu của code.
Đây là lần gọi mô hình viết lời duy nhất của câu thường: lời qua bước kiểm cũng là đoạn trả lời ngắn ở đầu thẻ
(ask/summary.rs). Mô hình chỉ nhận câu hỏi, bài học đã duyệt và các câu code đã viết (số đã tính, tên nhóm, tên kỳ), không
nhận dòng nào của bảng kết quả hay bảng phân rã (docs/PRIVACY.md); INSIGHT_TEXT=false thì không gọi mô hình. Danh sách
dòng và bảng nhóm theo cột nhạy cảm thì insight/mod.rs không gọi hàm này mà dùng build_code_narrated.

Bốn bước kiểm, trượt một bước là dùng câu của code (bước narrate ghi lý do):
- độ dài tối đa MAX_INSIGHT_CHARS;
- mọi số trong lời phải có nguyên văn trong các câu của code, phần trăm phải kèm "%" như code viết (bộ quét số của
  explain/mod.rs, build_text_number_check);
- trước mỗi độ lớn có chiều trong câu của code, chữ "tăng" hay "giảm" gần nhất phải đúng chiều (build_amount_direction_check);
- không có từ chỉ nguyên nhân: phần giải thích chỉ nói chênh lệch nằm ở đâu, không nói vì sao.

Khi #201 có khuôn JSON kèm `source` và mô hình chấm, phần này đi qua cùng cơ chế đó; hiện dùng các bước kiểm trên như
lời của câu "tại sao". */

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::prompts::{self, fill};
use crate::service::ask::contract::{AskStep, StepName};
use crate::service::ask::failures::{FailedStep, StepFailure};
use crate::service::ask::lessons::build_with_lessons;
use crate::service::ask::repair::build_user_text;
use crate::service::ask::run::build_duration_text;
use crate::service::explain::{
    allow_text, build_amount_direction_check, build_text_number_check, push_known_text,
};
use crate::service::insight::facts::InsightParts;

/// Số ký tự tối đa của lời; lời dài hơn thì dùng câu của code.
pub const MAX_INSIGHT_CHARS: usize = 600;

/// Từ chỉ nguyên nhân, viết thường, so theo từ trọn vẹn: phân rã chỉ cho biết chênh lệch nằm ở nhóm nào, không cho biết
/// vì sao, nên lời có những từ này là nói quá số liệu.
const CAUSAL_WORDS: [&str; 9] = [
    "do",
    "vì",
    "bởi",
    "nhờ",
    "khiến",
    "nguyên nhân",
    "gây ra",
    "dẫn đến",
    "dẫn tới",
];

/// Lời đã chọn, cờ "do mô hình viết" và bước cho timeline.
pub struct Narrated {
    pub summary: String,
    pub by_model: bool,
    pub step: AskStep,
    /// Bước kiểm chặn lời của mô hình: lý do và nguyên lời, cho nhật ký lỗi (#228); None trong mọi trường hợp khác.
    pub failure: Option<StepFailure>,
}

/// enabled là Config::insight_text; false thì không gọi mô hình. lessons là bài học đã duyệt liên quan câu hỏi
/// (ask/lessons.rs).
pub async fn narrate_insight(
    llm: &dyn LlmProvider,
    question: &str,
    parts: &InsightParts,
    enabled: bool,
    timeout: Duration,
    lessons: &[String],
) -> Narrated {
    let started = Instant::now();
    if !enabled {
        let detail = "INSIGHT_TEXT=false nên phần giải thích dùng câu do code viết";
        return build_code_narrated(parts, started, detail.to_string());
    }
    let prompt = build_with_lessons(build_insight_prompt(question, parts), lessons);
    let request = LlmRequest {
        task: LlmTask::Insight,
        question,
        prompt: &prompt,
        json_schema: None,
    };
    let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            let detail = format!(
                "Mô hình không viết được lời giải thích: {}",
                build_user_text(&error, "insight")
            );
            return fallback(parts, started, false, detail);
        }
        Err(_) => {
            let detail = format!(
                "Mô hình không trả lời trong {}",
                build_duration_text(timeout)
            );
            return fallback(parts, started, false, detail);
        }
    };
    let text = reply.trim();
    if let Err(problem) = build_insight_check(text, parts) {
        tracing::warn!(%problem, "insight text failed the checks");
        let failure = StepFailure::new(FailedStep::Narrate, &problem, Some(text));
        return Narrated {
            failure: Some(failure),
            ..fallback(parts, started, false, problem)
        };
    }
    Narrated {
        summary: text.to_string(),
        by_model: true,
        step: build_step(started, true, None),
        failure: None,
    }
}

/// Prompt: luật tiếng Anh ở backend/prompts/insight.txt, dữ liệu là câu chính, các ý và cờ notable dạng JSON.
pub fn build_insight_prompt(question: &str, parts: &InsightParts) -> String {
    let points: Vec<String> = parts
        .points
        .iter()
        .map(|p| format!("{}: {}", p.lead, p.text))
        .collect();
    let data = serde_json::json!({
        "main": parts.title,
        "points": points,
        "notable": parts.notable,
    });
    let json = serde_json::to_string_pretty(&data).expect("serialize insight points");
    fill(
        &prompts::INSIGHT,
        &[
            ("max_chars", &MAX_INSIGHT_CHARS.to_string()),
            ("question", question),
            ("points", &json),
        ],
    )
}

/// Err là lý do (tiếng Việt, cho chi tiết bước narrate) khi lời không qua một bước kiểm.
pub fn build_insight_check(text: &str, parts: &InsightParts) -> Result<(), String> {
    if text.is_empty() {
        return Err("Mô hình trả về câu trống".to_string());
    }
    if text.chars().count() > MAX_INSIGHT_CHARS {
        return Err(format!("Lời của mô hình dài hơn {MAX_INSIGHT_CHARS} ký tự"));
    }
    let code_text = build_code_text(parts);
    let mut allowed = HashSet::new();
    allow_text(&mut allowed, &code_text);
    allow_percents(&mut allowed, &code_text);
    let mut known = Vec::new();
    for name in &parts.names {
        push_known_text(&mut known, name);
    }
    if let Err(wrong) = build_text_number_check(text, &allowed, &known, &parts.title) {
        return Err(format!(
            "Lời của mô hình có số không nằm trong các câu đã tính: {}",
            wrong.join(", ")
        ));
    }
    if let Err(wrong) = build_amount_direction_check(text, &parts.changes) {
        return Err(format!(
            "Lời của mô hình nói ngược chiều trước: {}",
            wrong.join(", ")
        ));
    }
    if let Some(word) = find_causal_word(text) {
        return Err(format!(
            "Lời của mô hình dùng từ chỉ nguyên nhân \"{word}\""
        ));
    }
    Ok(())
}

/// Câu của code nối thành một đoạn: Insight.summary khi không dùng lời của mô hình.
pub fn build_code_text(parts: &InsightParts) -> String {
    let mut text = parts.title.clone();
    for point in &parts.points {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!("{}: {}", point.lead, point.text));
    }
    text
}

// Mỗi số đứng ngay trước "%" trong câu của code thành "%<số>", dạng bước kiểm số dùng cho phần trăm đã tính.
fn allow_percents(allowed: &mut HashSet<String>, text: &str) {
    for (index, _) in text.match_indices('%') {
        let number: String = text[..index]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
            .collect::<Vec<char>>()
            .into_iter()
            .rev()
            .collect();
        if !number.is_empty() {
            allowed.insert(format!("%{number}"));
        }
    }
}

/// Từ chỉ nguyên nhân đầu tiên có trong lời, so theo từ trọn vẹn không phân biệt hoa thường.
pub fn find_causal_word(text: &str) -> Option<&'static str> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    CAUSAL_WORDS.into_iter().find(|phrase| {
        let parts: Vec<&str> = phrase.split(' ').collect();
        words
            .windows(parts.len())
            .any(|window| window.iter().zip(&parts).all(|(w, p)| w == p))
    })
}

/// Câu của code thay cho lời của mô hình, không gọi mô hình; detail là lý do, ghi ở bước narrate (ok = true: không có
/// gì hỏng).
pub fn build_code_narrated(parts: &InsightParts, started: Instant, detail: String) -> Narrated {
    fallback(parts, started, true, detail)
}

fn fallback(parts: &InsightParts, started: Instant, ok: bool, detail: String) -> Narrated {
    Narrated {
        summary: build_code_text(parts),
        by_model: false,
        step: build_step(started, ok, Some(detail)),
        failure: None,
    }
}

fn build_step(started: Instant, ok: bool, detail: Option<String>) -> AskStep {
    AskStep {
        name: StepName::Narrate,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        ok,
        detail,
    }
}
