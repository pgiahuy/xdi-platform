// SPDX-License-Identifier: Apache-2.0
/* service/insight/narrate.rs - lời của phần giải thích câu thường (#227): nhờ mô hình diễn đạt lại câu chính và các ý do
code viết (insight/facts.rs) thành một đoạn ngắn, rồi kiểm lời đó bằng code; không qua thì dùng chính các câu của code.
Đây là lần gọi mô hình viết lời duy nhất của câu thường: lời qua bước kiểm cũng là đoạn trả lời ngắn ở đầu thẻ
(ask/summary.rs). Mô hình chỉ nhận câu hỏi, bài học đã duyệt và các câu code đã viết (số đã tính, tên nhóm, tên kỳ), không
nhận dòng nào của bảng kết quả hay bảng phân rã (docs/PRIVACY.md); INSIGHT_TEXT=false thì không gọi mô hình. Danh sách
dòng và bảng nhóm theo cột nhạy cảm thì insight/mod.rs không gọi hàm này mà dùng build_code_narrated.

Hai cổng (#201), như lời của câu "tại sao" (ask/narrate.rs):
- cổng 1 là bốn bước kiểm bằng code (build_insight_check): độ dài tối đa MAX_INSIGHT_CHARS; mọi số trong lời phải có
  nguyên văn trong các câu của code, phần trăm phải kèm "%" như code viết (bộ quét số của explain/mod.rs,
  build_text_number_check); trước mỗi độ lớn có chiều trong câu của code, chữ "tăng" hay "giảm" gần nhất phải đúng
  chiều (build_amount_direction_check); không có từ chỉ nguyên nhân, vì phần giải thích chỉ nói chênh lệch nằm ở đâu;
- cổng 2 là mô hình chấm (ask/judge.rs), một lần gọi riêng: nhận lời và các câu code đã viết, so nghĩa của lời với các
  câu đó, trả match, warning hay mismatch. Mô hình chấm cũng không nhận dòng nào của bảng kết quả.
Trượt cổng 1, bị chấm mismatch hay warning thì mô hình viết lại một lần, kèm các lỗi và chỗ chưa rõ của lần trước. Vẫn
không khớp thì dùng câu của code (kết luận mismatch); lần trước chỉ có chỗ chưa rõ thì giữ lời đó, kèm các chỗ chưa rõ
(kết luận warning). */

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::prompts::{self, fill};
use crate::service::ask::contract::{AnswerVerdict, AskStep, FailedCheck, InsightPoint, StepName};
use crate::service::ask::failures::{FailedStep, StepFailure};
use crate::service::ask::judge::judge;
use crate::service::ask::lessons::build_with_lessons;
use crate::service::ask::narrate::{build_check_name, MAX_NARRATE_ATTEMPTS};
use crate::service::ask::repair::build_user_text;
use crate::service::ask::run::build_duration_text;
use crate::service::explain::claims::build_rewrite_prompt;
use crate::service::explain::judge::{build_judge_prompt_with, Verdict};
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

/// Lời đã chọn, cờ "do mô hình viết", kết luận hai cổng và bước cho timeline.
pub struct Narrated {
    pub summary: String,
    pub by_model: bool,
    /// Chỗ mô hình chấm thấy chưa rõ trong lời được dùng; trống khi lời khớp hay summary là câu của code.
    pub warnings: Vec<String>,
    /// Kết luận hai cổng (Insight.summary_verdict); None khi không cổng nào chạy xong.
    pub verdict: Option<AnswerVerdict>,
    /// Bước kiểm chặn lời ở lần viết cuối khi verdict là mismatch (Insight.summary_failed_check); None trong mọi trường
    /// hợp khác.
    pub failed_check: Option<FailedCheck>,
    pub step: AskStep,
    /// Hai cổng chặn lời của mô hình ở mọi lần viết: lý do và lời của lần cuối, cho nhật ký lỗi (#228); None trong mọi
    /// trường hợp khác.
    pub failure: Option<StepFailure>,
}

/// Lời mô hình chấm thấy có chỗ chưa rõ ở một lần viết: giữ lại để dùng khi lần viết sau không khớp hẳn.
struct Unclear {
    text: String,
    warnings: Vec<String>,
    attempt: usize,
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
        let detail = "INSIGHT_TEXT=false nên lời giải thích dùng câu do code viết";
        return build_code_narrated(parts, started, detail.to_string());
    }
    // Bài học đã duyệt nối vào prompt gốc, nên lần viết lại cũng mang theo.
    let first_prompt = build_with_lessons(build_insight_prompt(question, parts), lessons);
    let mut problems: Vec<String> = Vec::new();
    let mut unclear: Option<Unclear> = None;
    let mut last_text: Option<String> = None;
    // Bước kiểm chặn lời ở lần viết gần nhất, để lời báo và báo cáo nói đúng lời trượt ở đâu.
    let mut failed_check = FailedCheck::Number;
    for attempt in 1..=MAX_NARRATE_ATTEMPTS {
        let prompt = if problems.is_empty() {
            first_prompt.clone()
        } else {
            build_rewrite_prompt(&first_prompt, &problems)
        };
        let request = LlmRequest {
            task: LlmTask::Insight,
            question,
            prompt: &prompt,
            json_schema: None,
        };
        // Lỗi gọi mô hình hay quá giờ không phải lỗi của lời, viết lại không sửa được: dừng ngay.
        let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(error)) => {
                let detail = format!(
                    "Mô hình không viết được lời giải thích: {}",
                    build_user_text(&error, "insight")
                );
                return finish_failed(parts, unclear, started, detail);
            }
            Err(_) => {
                let detail = format!(
                    "Mô hình không trả lời trong {}",
                    build_duration_text(timeout)
                );
                return finish_failed(parts, unclear, started, detail);
            }
        };
        let text = reply.trim().to_string();
        last_text = Some(text.clone());
        if let Err(problem) = build_insight_check(&text, parts) {
            tracing::warn!(attempt, %problem, "insight text failed gate 1");
            failed_check = FailedCheck::Number;
            problems = vec![problem];
            continue;
        }
        let rewritten = if attempt > 1 {
            ", sau một lần viết lại"
        } else {
            ""
        };
        // Không chấm được thì không biết lời đúng hay sai, viết lại cũng không giúp: dừng ngay.
        match judge(
            llm,
            question,
            &build_insight_judge_prompt(&text, parts),
            timeout,
        )
        .await
        {
            Ok(Verdict::Match) => {
                let detail = format!(
                    "Đã đối soát với các câu đã tính; mô hình chấm thấy lời khớp{rewritten}"
                );
                return Narrated {
                    summary: text,
                    by_model: true,
                    warnings: Vec::new(),
                    verdict: Some(AnswerVerdict::Match),
                    failed_check: None,
                    step: build_step(started, true, Some(detail)),
                    failure: None,
                };
            }
            // Chỗ chưa rõ cũng gửi lại cho mô hình viết lại như lỗi; lời này được giữ, lần viết lại không khớp hẳn thì
            // dùng lại nó (finish_unclear).
            Ok(Verdict::Warning(found)) => {
                tracing::warn!(
                    attempt,
                    problems = found.len(),
                    "insight text is unclear at gate 2"
                );
                problems = found
                    .iter()
                    .map(|p| format!("Mô hình chấm thấy chưa rõ: {p}"))
                    .collect();
                unclear = Some(Unclear {
                    text,
                    warnings: found,
                    attempt,
                });
            }
            Ok(Verdict::Mismatch(found)) => {
                failed_check = FailedCheck::Meaning;
                tracing::warn!(
                    attempt,
                    problems = found.len(),
                    "insight text failed gate 2"
                );
                problems = found
                    .into_iter()
                    .map(|p| format!("Mô hình chấm: {p}"))
                    .collect();
            }
            Err(detail) => return finish_failed(parts, unclear, started, detail),
        }
    }
    if let Some(unclear) = unclear {
        return finish_unclear(unclear, started);
    }
    let detail = format!(
        "Lời của mô hình không qua {} sau {MAX_NARRATE_ATTEMPTS} lần viết, đã thay bằng câu hệ thống tính: {}",
        build_check_name(failed_check),
        problems.join("; ")
    );
    // Lời trượt mọi lần viết thì không trả cho người dùng: câu của code thay vào, kết luận ghi là mismatch.
    let failure = StepFailure::new(FailedStep::Narrate, &detail, last_text.as_deref());
    Narrated {
        verdict: Some(AnswerVerdict::Mismatch),
        failed_check: Some(failed_check),
        failure: Some(failure),
        ..fallback(parts, started, false, detail)
    }
}

/// Lời có chỗ chưa rõ được dùng: kết luận warning, các chỗ chưa rõ đi kèm lời.
fn finish_unclear(unclear: Unclear, started: Instant) -> Narrated {
    let when = if unclear.attempt > 1 {
        ", sau một lần viết lại"
    } else if MAX_NARRATE_ATTEMPTS > 1 {
        ", lần viết lại không khớp hơn nên giữ lời đầu"
    } else {
        ""
    };
    let detail = format!(
        "Đã đối soát với các câu đã tính; mô hình chấm thấy chỗ chưa rõ{when}: {}",
        unclear.warnings.join("; ")
    );
    Narrated {
        summary: unclear.text,
        by_model: true,
        warnings: unclear.warnings,
        verdict: Some(AnswerVerdict::Warning),
        failed_check: None,
        step: build_step(started, true, Some(detail)),
        failure: None,
    }
}

/// Lần viết hay lần chấm lỗi giữa chừng: còn lời có chỗ chưa rõ của lần trước thì dùng nó, không thì câu của code.
fn finish_failed(
    parts: &InsightParts,
    unclear: Option<Unclear>,
    started: Instant,
    detail: String,
) -> Narrated {
    match unclear {
        Some(unclear) => finish_unclear(unclear, started),
        None => fallback(parts, started, false, detail),
    }
}

/// Prompt cho mô hình chấm: cùng luật với câu "tại sao" (prompts/judge_answer.txt), nghĩa các khóa ở
/// prompts/judge_meaning_plain.txt, JSON là câu chính và các ý code đã viết.
pub fn build_insight_judge_prompt(text: &str, parts: &InsightParts) -> String {
    let points: Vec<serde_json::Value> = parts
        .points
        .iter()
        .map(|p| serde_json::json!({"lead": p.lead, "text": p.text}))
        .collect();
    let facts = serde_json::json!({"title": parts.title, "points": points});
    let json = serde_json::to_string_pretty(&facts).expect("serialize insight facts");
    build_judge_prompt_with(&prompts::JUDGE_MEANING_PLAIN, &json, text)
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
    build_facts_text(&parts.title, &parts.points)
}

/// Câu chính và các ý nối thành một đoạn, mỗi ý "<nhãn>: <câu>".
pub fn build_facts_text(title: &str, points: &[InsightPoint]) -> String {
    let mut text = title.to_string();
    for point in points {
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
        warnings: Vec::new(),
        verdict: None,
        failed_check: None,
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
