// SPDX-License-Identifier: Apache-2.0
/* service/ask_narrate.rs - bước narrate của câu "tại sao" (#41): nhờ mô hình viết lời diễn giải từ bảng phân rã
(Explanation) đã tính, kiểm mọi số trong lời đó có trong bảng không, rơi về câu khuôn khi cần. Prompt, tập số được
phép, bước kiểm và câu khuôn là hàm thuần ở explain.rs; file này ghép các bước và dựng AskStep, như ask_run.rs ghép
guard, run, repair. #33/#38 gọi narrate sau bước explain rồi đưa summary, summary_by_model vào Explanation và step
vào danh sách bước.

narrate không bao giờ làm câu hỏi dừng: mô hình lỗi, quá giờ hay viết sai số thì dùng câu khuôn của
build_template_summary, câu hỏi vẫn answered, bước narrate có ok = false và detail nói lý do (hợp đồng ở AskStep).
Chênh lệch bằng 0 thì không gọi mô hình, vì không có gì để giải thích. Test ở ask_narrate_tests.rs. */

use std::time::{Duration, Instant};

use rust_decimal::Decimal;

use super::ask_contract::{AskStep, Explanation, StepName};
use super::ask_repair::build_user_text;
use super::explain::{
    build_allowed_numbers, build_narrate_prompt, build_number_check, build_template_summary,
    MAX_SUMMARY_CHARS,
};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};

/// Kết quả bước narrate: lời diễn giải (của mô hình hoặc câu khuôn), cờ "do mô hình viết" và bước cho timeline.
pub struct NarrateOutcome {
    pub summary: String,
    /// False khi summary là câu khuôn: mô hình lỗi, quá giờ, trả câu không dùng được, hoặc không được gọi.
    pub summary_by_model: bool,
    pub step: AskStep,
}

pub async fn narrate(
    llm: &dyn LlmProvider,
    question: &str,
    explanation: &Explanation,
    timeout: Duration,
) -> NarrateOutcome {
    let started = Instant::now();
    // change không đọc được thì cứ gọi mô hình: bảng phân rã do code dựng nên nhánh này chỉ là phòng hờ.
    if explanation.change.parse::<Decimal>() == Ok(Decimal::ZERO) {
        let detail = "Chênh lệch bằng 0 nên không gọi mô hình".to_string();
        return fallback(explanation, build_step(started, true, Some(detail)));
    }
    let prompt = build_narrate_prompt(question, explanation);
    let request = LlmRequest {
        task: LlmTask::Narrate,
        question,
        prompt: &prompt,
    };
    let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            let detail = format!(
                "Mô hình không viết được lời diễn giải: {}",
                build_user_text(&error)
            );
            return fallback(explanation, build_step(started, false, Some(detail)));
        }
        Err(_) => {
            let detail = format!("Mô hình trả lời quá {}", build_duration_text(timeout));
            return fallback(explanation, build_step(started, false, Some(detail)));
        }
    };
    let summary = reply.trim();
    if summary.is_empty() {
        let detail = "Mô hình trả về câu trống".to_string();
        return fallback(explanation, build_step(started, false, Some(detail)));
    }
    if summary.chars().count() > MAX_SUMMARY_CHARS {
        let detail = format!("Câu của mô hình dài hơn {MAX_SUMMARY_CHARS} ký tự");
        return fallback(explanation, build_step(started, false, Some(detail)));
    }
    let allowed = build_allowed_numbers(question, explanation);
    match build_number_check(summary, &allowed) {
        Ok(()) => NarrateOutcome {
            summary: summary.to_string(),
            summary_by_model: true,
            step: build_step(started, true, None),
        },
        Err(wrong) => {
            tracing::warn!(question, numbers = ?wrong, "narrate summary has numbers outside the breakdown");
            let detail = format!(
                "Câu của mô hình có số không nằm trong bảng phân rã: {}",
                wrong.join(", ")
            );
            fallback(explanation, build_step(started, false, Some(detail)))
        }
    }
}

/// Câu khuôn thay cho lời của mô hình; step do bên gọi dựng vì ok và detail tùy lý do rơi về khuôn.
fn fallback(explanation: &Explanation, step: AskStep) -> NarrateOutcome {
    NarrateOutcome {
        summary: build_template_summary(explanation),
        summary_by_model: false,
        step,
    }
}

/// "30 giây"; dưới một giây thì mili giây ("300 mili giây"), không in "0 giây". Như build_duration_text của
/// ask_run.rs.
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
