// SPDX-License-Identifier: Apache-2.0
/* service/ask/narrate.rs - bước narrate của câu "tại sao" (#41): nhờ mô hình viết insight (câu mở đầu và các ý
"Nguyên nhân chính") từ số code đã tính trên bảng phân rã (Explanation), kèm danh sách khẳng định. Code không đọc lời; lời qua cổng 1 (#201, explain/claims.rs): code so
từng khẳng định với đúng ô của bảng; rồi qua cổng 2 (ask/judge.rs): một lần gọi mô hình riêng so nghĩa của lời với các
khẳng định và tên hai bên. Trượt thì nhờ mô hình viết lại một lần, đưa kèm lý do trượt; vẫn trượt thì dùng câu khuôn.
Mô hình chấm thấy chỗ chưa rõ (warning) thì cũng viết lại một lần kèm các chỗ đó; lần viết lại không khớp hẳn thì giữ
lời có chỗ chưa rõ, kèm các chỗ đó, không rơi về câu khuôn. Prompt, cổng 1 và câu khuôn là hàm thuần
ở explain/; file này ghép các bước và dựng AskStep, như ask/run.rs ghép guard, run, repair. #33/#38 gọi narrate sau
bước explain rồi đưa summary, summary_by_model, summary_warnings, summary_verdict vào Explanation và step vào danh sách
bước. Bài học đã duyệt (#235) nối vào prompt viết lời; lời bị chặn ở mọi lần viết ghi vào nhật ký lỗi (#228).

narrate không bao giờ làm câu hỏi dừng: mô hình viết lời lỗi hay quá giờ, mô hình chấm không chấm được, hay lời trượt
cả hai lần thì dùng câu khuôn của build_template_summary, câu hỏi vẫn answered, bước narrate có ok = false và detail
nói lý do (hợp đồng ở AskStep). Qua hai cổng thì detail nói đã đối soát bao nhiêu khẳng định. Chênh lệch bằng 0 thì
không gọi mô hình, vì không có gì để giải thích. Test ở ask/narrate_tests.rs. */

use std::time::{Duration, Instant};

use rust_decimal::Decimal;

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::service::ask::contract::{
    AnswerVerdict, AskStep, Comparison, Explanation, FailedCheck, StepName, SummaryPoint,
    SummaryPointKind,
};
use crate::service::ask::failures::{FailedStep, StepFailure};
use crate::service::ask::judge::judge;
use crate::service::ask::lessons::build_with_lessons;
use crate::service::ask::narrate_text::{build_tidy_text, upper_first};
use crate::service::ask::repair::build_user_text;
use crate::service::explain::claims::{
    build_claim_check, build_narrate_schema, build_rewrite_prompt, parse_narrate_reply, Claim,
};
use crate::service::explain::judge::{build_judge_facts, build_judge_prompt, Verdict};
use crate::service::explain::{
    build_narrate_prompt, build_template_summary, MAX_REASONS, MAX_SUMMARY_CHARS,
};

/// Số lần nhờ mô hình viết lời: lần đầu và một lần viết lại khi câu trả lời trượt cổng 1 hay cổng 2. Mỗi lần thêm là
/// người hỏi chờ thêm hai lượt gọi mô hình (viết và chấm); chưa đo lần viết thứ ba sửa được thêm bao nhiêu.
pub const MAX_NARRATE_ATTEMPTS: usize = 2;

/// Kết quả bước narrate: lời diễn giải (của mô hình hoặc câu khuôn), cờ "do mô hình viết" và bước cho timeline.
pub struct NarrateOutcome {
    pub summary: String,
    /// False khi summary là câu khuôn: mô hình lỗi, quá giờ, trả câu không dùng được, hoặc không được gọi.
    pub summary_by_model: bool,
    /// Câu mở đầu và các ý "Nguyên nhân chính" của mô hình khi summary_by_model; None thì bên gọi giữ câu chính và các
    /// ý của câu khuôn đã có trong Explanation.
    pub insight: Option<Insight>,
    /// Các chỗ mô hình chấm thấy chưa rõ nghĩa trong lời của mô hình; trống khi lời khớp hay summary là câu khuôn.
    pub warnings: Vec<String>,
    /// Kết luận của hai cổng (Explanation.summary_verdict); None khi không cổng nào chạy xong.
    pub verdict: Option<AnswerVerdict>,
    /// Bước kiểm chặn lời ở lần viết cuối khi verdict là mismatch (Explanation.summary_failed_check); None trong mọi
    /// trường hợp khác.
    pub failed_check: Option<FailedCheck>,
    pub step: AskStep,
    /// Lời mô hình viết bị hai cổng chặn ở mọi lần viết: lý do và lời của lần cuối, cho nhật ký lỗi (#228). None khi lời
    /// được dùng, hay khi rơi về câu khuôn vì lý do khác (mô hình lỗi, quá giờ, không chấm được).
    pub failure: Option<StepFailure>,
}

/// Insight của mô hình đã qua hai cổng, theo khuôn summary_title và summary_points của Explanation.
pub struct Insight {
    pub title: String,
    pub points: Vec<SummaryPoint>,
}

/// Câu trả lời đã đọc và qua cổng 1.
struct Checked {
    text: String,
    insight: Insight,
    claims: Vec<Claim>,
}

/// Lời mô hình chấm thấy có chỗ chưa rõ ở một lần viết: giữ lại để dùng khi lần viết sau không khớp hẳn.
struct Unclear {
    summary: String,
    insight: Insight,
    warnings: Vec<String>,
    claims: usize,
    attempt: usize,
}

pub async fn narrate(
    llm: &dyn LlmProvider,
    question: &str,
    explanation: &Explanation,
    timeout: Duration,
    lessons: &[String],
) -> NarrateOutcome {
    let started = Instant::now();
    // change không đọc được thì cứ gọi mô hình: bảng phân rã do code dựng nên nhánh này chỉ là phòng hờ.
    if explanation.change.parse::<Decimal>() == Ok(Decimal::ZERO) {
        let detail = "Chênh lệch bằng 0 nên không gọi mô hình".to_string();
        return fallback(explanation, build_step(started, true, Some(detail)));
    }
    // So hai nhóm cũng do mô hình viết. Trước #201 so hai nhóm luôn dùng câu khuôn: thử với Gemini, mô hình hay đảo nhóm
    // nào cao hơn ("Pass thấp hơn Withdrawn" khi Pass cao hơn) mà không bước nào bắt được. Giờ mô hình chấm nhận tên hai
    // bên và kiểu so sánh (explain/judge.rs) cùng số có dấu, nên lời đảo nhóm trượt cổng 2 như lời đảo kỳ.
    // Bài học đã duyệt (#235) nối vào prompt gốc, nên lần viết lại cũng mang theo.
    let first_prompt = build_with_lessons(build_narrate_prompt(question, explanation), lessons);
    let mut problems: Vec<String> = Vec::new();
    // Lời có chỗ chưa rõ của lần viết trước. Lần viết lại không khớp hẳn (sai, lỗi, quá giờ, không chấm được) thì dùng
    // lại lời này kèm các chỗ chưa rõ: nó đã qua cổng 1 và không bị chấm là sai, tốt hơn câu khuôn.
    let mut unclear: Option<Unclear> = None;
    // Lời của lần viết gần nhất, ghi vào nhật ký lỗi khi mọi lần viết đều bị chặn.
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
            task: LlmTask::Narrate,
            question,
            prompt: &prompt,
            json_schema: Some(build_narrate_schema()),
        };
        // Lỗi gọi mô hình hay quá giờ không phải lỗi của lời, viết lại không sửa được: dừng ngay.
        let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(error)) => {
                let detail = format!(
                    "Mô hình không viết được lời diễn giải: {}",
                    build_user_text(&error, "narrate")
                );
                return finish_failed(explanation, unclear, started, detail);
            }
            Err(_) => {
                let detail = format!(
                    "Mô hình không trả lời trong {}",
                    build_duration_text(timeout)
                );
                return finish_failed(explanation, unclear, started, detail);
            }
        };
        let Checked {
            text: summary,
            insight,
            claims,
        } = match check_reply(&reply, explanation) {
            Ok(checked) => checked,
            Err(found) => {
                last_text = Some(reply.trim().to_string());
                // Không ghi câu hỏi hay lời vào log: câu hỏi là chữ người dùng gõ, cả backend không log nó ở đâu.
                tracing::warn!(
                    attempt,
                    problems = found.len(),
                    "narrate reply failed gate 1"
                );
                problems = found;
                failed_check = FailedCheck::Number;
                continue;
            }
        };
        let rewritten = if attempt > 1 {
            ", sau một lần viết lại"
        } else {
            ""
        };
        last_text = Some(summary.clone());
        let facts = build_judge_facts(explanation, &claims);
        // Không chấm được thì không biết lời đúng hay sai, viết lại cũng không giúp: dùng câu khuôn ngay.
        let judge_prompt = build_judge_prompt(&summary, &facts);
        match judge(llm, question, &judge_prompt, timeout).await {
            Ok(Verdict::Match) => {
                let detail = format!(
                    "Đã đối soát {} khẳng định với bảng phân rã; mô hình chấm thấy lời khớp các khẳng định{rewritten}",
                    claims.len()
                );
                return NarrateOutcome {
                    summary,
                    summary_by_model: true,
                    insight: Some(insight),
                    warnings: Vec::new(),
                    verdict: Some(AnswerVerdict::Match),
                    failed_check: None,
                    step: build_step(started, true, Some(detail)),
                    failure: None,
                };
            }
            // Chỗ chưa rõ cũng gửi lại cho mô hình viết lại như lỗi, để nó biết sửa chỗ nào; lời này được giữ, lần viết
            // lại không khớp hẳn thì dùng lại nó (finish_unclear).
            Ok(Verdict::Warning(found)) => {
                tracing::warn!(
                    attempt,
                    problems = found.len(),
                    "narrate reply is unclear at gate 2"
                );
                problems = found
                    .iter()
                    .map(|p| format!("Mô hình chấm thấy chưa rõ: {p}"))
                    .collect();
                unclear = Some(Unclear {
                    summary,
                    insight,
                    warnings: found,
                    claims: claims.len(),
                    attempt,
                });
            }
            Ok(Verdict::Mismatch(found)) => {
                failed_check = FailedCheck::Meaning;
                tracing::warn!(
                    attempt,
                    problems = found.len(),
                    "narrate reply failed gate 2"
                );
                problems = found
                    .into_iter()
                    .map(|p| format!("Mô hình chấm: {p}"))
                    .collect();
            }
            Err(detail) => {
                return finish_failed(explanation, unclear, started, detail);
            }
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
    // Lời trượt mọi lần viết thì không trả cho người dùng: câu khuôn thay vào, kết luận ghi là mismatch.
    let failure = StepFailure::new(FailedStep::Narrate, &detail, last_text.as_deref());
    NarrateOutcome {
        verdict: Some(AnswerVerdict::Mismatch),
        failed_check: Some(failed_check),
        failure: Some(failure),
        ..fallback(explanation, build_step(started, false, Some(detail)))
    }
}

/// Đọc và kiểm một câu trả lời: đúng khuôn JSON, có câu mở đầu, từ 1 tới MAX_REASONS ý không trống, cả insight không
/// quá dài, rồi cổng 1 (khẳng định so với bảng). Ok là insight và các khẳng định đã khớp bảng; Err là mọi lý do trượt,
/// đưa vào prompt viết lại và vào detail của bước.
fn check_reply(reply: &str, explanation: &Explanation) -> Result<Checked, Vec<String>> {
    let mut parsed = parse_narrate_reply(reply).map_err(|problem| vec![problem])?;
    let labels = build_column_labels(explanation);
    parsed.headline = upper_first(&build_tidy_text(parsed.headline.trim(), &labels));
    for reason in &mut parsed.reasons {
        reason.lead = upper_first(&build_tidy_text(reason.lead.trim(), &labels));
        let text = build_text_without_lead(&reason.lead, &reason.text);
        reason.text = build_tidy_text(&text, &labels);
    }
    if parsed.headline.trim().is_empty() {
        return Err(vec!["Câu mở đầu trống".to_string()]);
    }
    if parsed.reasons.is_empty() || parsed.reasons.len() > MAX_REASONS {
        return Err(vec![format!(
            "Cần từ 1 tới {MAX_REASONS} ý nguyên nhân chính, câu trả lời có {}",
            parsed.reasons.len()
        )]);
    }
    if parsed
        .reasons
        .iter()
        .any(|r| r.lead.trim().is_empty() || r.text.trim().is_empty())
    {
        return Err(vec!["Có ý trống đầu ý hay trống câu".to_string()]);
    }
    let text = parsed.build_text();
    if text.chars().count() > MAX_SUMMARY_CHARS {
        return Err(vec![format!(
            "Lời giải thích dài hơn {MAX_SUMMARY_CHARS} ký tự"
        )]);
    }
    if explanation.comparison == Comparison::Groups {
        if let Some(phrase) = find_change_phrase(&text) {
            return Err(vec![format!(
                "So hai nhóm thì không có tăng hay giảm: thay \"{phrase}\" bằng \"mức chênh\", \"cao hơn\" hay \
                 \"thấp hơn\""
            )]);
        }
    }
    build_claim_check(&parsed.claims, explanation)?;
    let points = parsed
        .reasons
        .iter()
        .map(|r| SummaryPoint {
            // Mọi ý của mô hình là ý nguyên nhân; kind chỉ để phân loại ý của câu khuôn.
            kind: SummaryPointKind::Reason,
            lead: r.lead.trim().to_string(),
            text: r.text.trim().to_string(),
        })
        .collect();
    Ok(Checked {
        text,
        insight: Insight {
            title: parsed.headline.trim().to_string(),
            points,
        },
        claims: parsed.claims,
    })
}

/// Tên bước kiểm trong lời cho người dùng: "kiểm số" hay "kiểm nghĩa" (thuật ngữ ở docs/ARCHITECTURE.md).
pub(crate) fn build_check_name(check: FailedCheck) -> &'static str {
    match check {
        FailedCheck::Number => "kiểm số",
        FailedCheck::Meaning => "kiểm nghĩa",
    }
}

/// Nhãn cột của bảng phân rã và của cột hai nhóm đang so, cho build_tidy_text.
fn build_column_labels(explanation: &Explanation) -> Vec<&str> {
    explanation
        .dimensions
        .iter()
        .filter_map(|d| d.label.as_deref())
        .chain(explanation.slice_column_label.as_deref())
        .collect()
}

/// Câu của một ý bỏ phần lặp lại đầu ý: mô hình hay viết "Hiệu ứng tỷ suất tăng" làm đầu ý rồi mở câu bằng đúng cụm đó,
/// giao diện in "Hiệu ứng tỷ suất tăng: Hiệu ứng tỷ suất tăng 0,1081...". Chỉ bỏ khi đầu ý có từ hai từ; bỏ phần lặp mà câu
/// còn trống thì giữ nguyên.
pub(super) fn build_text_without_lead(lead: &str, text: &str) -> String {
    let (lead, text) = (lead.trim(), text.trim());
    let lead_chars = lead.chars().count();
    // Đầu ý một từ ("Giảm") lặp ở đầu câu thường là chữ của câu ("Giảm giá ..."), không phải lặp đầu ý.
    let starts_with_lead = lead.split_whitespace().count() >= 2
        && text.chars().count() > lead_chars
        && text
            .chars()
            .take(lead_chars)
            .collect::<String>()
            .to_lowercase()
            == lead.to_lowercase();
    if !starts_with_lead {
        return text.to_string();
    }
    let rest: String = text.chars().skip(lead_chars).collect();
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == ',');
    // Phần lặp chỉ là một phần của từ ("Giảm" trong "Giảm giá...") thì không bỏ: chữ ngay sau phải là ranh giới từ.
    let boundary = text
        .chars()
        .nth(lead_chars)
        .is_none_or(|c| !c.is_alphanumeric());
    if rest.is_empty() || !boundary {
        return text.to_string();
    }
    rest.to_string()
}

/// Cụm nói chênh lệch giữa hai nhóm như một lần tăng hay giảm ("tạo ra phần lớn mức giảm"): hai nhóm cùng lúc không có
/// trước sau nên chênh lệch là "mức chênh". Chỉ bắt cụm danh từ, không bắt "tăng", "giảm" đứng một mình, vì tên độ đo hay
/// tên nhóm có thể chứa chúng ("thành tiền sau giảm giá").
pub(super) fn find_change_phrase(text: &str) -> Option<&'static str> {
    const PHRASES: [&str; 6] = [
        "mức tăng",
        "mức giảm",
        "phần tăng",
        "phần giảm",
        "sự tăng",
        "sự giảm",
    ];
    let lower = format!(" {} ", text.to_lowercase());
    PHRASES.into_iter().find(|phrase| {
        lower.match_indices(phrase).any(|(at, _)| {
            let before = lower[..at].chars().next_back();
            let after = lower[at + phrase.len()..].chars().next();
            before.is_some_and(|c| !c.is_alphanumeric())
                && after.is_some_and(|c| !c.is_alphanumeric())
        })
    })
}

/// Lời có chỗ chưa rõ được dùng: kết luận warning, các chỗ chưa rõ đi kèm lời.
fn finish_unclear(unclear: Unclear, started: Instant) -> NarrateOutcome {
    let when = if unclear.attempt > 1 {
        ", sau một lần viết lại"
    } else if MAX_NARRATE_ATTEMPTS > 1 {
        ", lần viết lại không khớp hơn nên giữ lời đầu"
    } else {
        ""
    };
    let detail = format!(
        "Đã đối soát {} khẳng định với bảng phân rã; mô hình chấm thấy chỗ chưa rõ{when}: {}",
        unclear.claims,
        unclear.warnings.join("; ")
    );
    NarrateOutcome {
        summary: unclear.summary,
        summary_by_model: true,
        insight: Some(unclear.insight),
        warnings: unclear.warnings,
        verdict: Some(AnswerVerdict::Warning),
        failed_check: None,
        step: build_step(started, true, Some(detail)),
        failure: None,
    }
}

/// Lần viết hay lần chấm lỗi giữa chừng: còn lời có chỗ chưa rõ của lần trước thì dùng nó, không thì câu khuôn.
fn finish_failed(
    explanation: &Explanation,
    unclear: Option<Unclear>,
    started: Instant,
    detail: String,
) -> NarrateOutcome {
    match unclear {
        Some(unclear) => finish_unclear(unclear, started),
        None => fallback(explanation, build_step(started, false, Some(detail))),
    }
}

/// Câu khuôn thay cho lời của mô hình; step do bên gọi dựng vì ok và detail tùy lý do rơi về khuôn.
fn fallback(explanation: &Explanation, step: AskStep) -> NarrateOutcome {
    NarrateOutcome {
        summary: build_template_summary(explanation),
        summary_by_model: false,
        insight: None,
        warnings: Vec::new(),
        verdict: None,
        failed_check: None,
        step,
        failure: None,
    }
}

/// "30 giây"; dưới một giây thì mili giây ("300 mili giây"), không in "0 giây". Như build_duration_text của
/// ask/run.rs.
fn build_duration_text(duration: Duration) -> String {
    if duration < Duration::from_secs(1) {
        format!("{} mili giây", duration.as_millis())
    } else {
        format!("{} giây", duration.as_secs())
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
