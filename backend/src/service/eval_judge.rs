// SPDX-License-Identifier: Apache-2.0
/* service/eval_judge.rs - bộ đo mô hình chấm (cổng 2, #201), chạy ngoài sản phẩm như bộ đánh giá câu hỏi
(service/eval.rs). Mỗi ca là một lời diễn giải câu "tại sao", JudgeFacts (tên độ đo, tên hai bên, các khẳng định đã
qua cổng 1) và kết luận đúng do người viết ca gắn: match, hoặc mismatch kèm loại lỗi cố ý cài vào lời (số gán nhầm
nhóm, sai chiều, thêm lý do, tính sai...). Lệnh `xdi-backend eval-judge` ở cli.rs gọi run_eval_judge: gửi từng ca cho
mô hình chấm qua đúng prompt và hàm bước narrate dùng (explain/judge.rs, ask/judge.rs), rồi đếm bỏ lọt (ca sai mà mô hình nói khớp), ca sai chỉ bị cảnh báo
(warning: lời sai vẫn tới người dùng, kèm cảnh báo), chặn nhầm (ca đúng mà mô hình nói không khớp) và cảnh báo thừa
(ca đúng mà mô hình cảnh báo).

Không cần dữ liệu hay database cho phần chấm: mô hình chấm không nhận bảng số, chỉ nhận lời và các khẳng định, nên
file ca chứa đủ mọi thứ. Với mock thì số đo vô nghĩa: mock chỉ có câu chấm của câu mẫu, các ca khác báo lỗi. */

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::config::Config;
use crate::engine::llm::LlmProvider;
use crate::error::AppError;
use crate::service::ask::judge::{build_judge_timeout, judge};
use crate::service::explain::judge::{build_judge_prompt, JudgeFacts, Verdict};

/// File ca.
#[derive(Debug, Deserialize)]
pub struct JudgeCases {
    pub cases: Vec<JudgeCase>,
}

#[derive(Debug, Deserialize)]
pub struct JudgeCase {
    pub id: String,
    /// "match" hay "mismatch".
    pub expected: String,
    /// Loại lỗi cài vào lời ("swapped_numbers", "extra_reason"...); "none" với ca đúng.
    pub error: String,
    pub text: String,
    pub facts: JudgeFacts,
}

/// Kết quả một ca. got là None khi mô hình chấm không chấm được (lỗi, quá giờ, câu trả lời không đọc được).
pub struct JudgeOutcome {
    pub id: String,
    pub expected: String,
    pub error: String,
    pub got: Option<String>,
    /// Lỗi mô hình chấm nêu, hoặc lý do không chấm được.
    pub detail: String,
    pub duration_ms: u64,
}

pub struct JudgeReport {
    pub total: usize,
    pub correct: usize,
    /// Ca mismatch mà mô hình nói match: lời sai lọt qua cổng 2.
    pub missed: usize,
    /// Ca mismatch mà mô hình nói warning: lời sai vẫn tới người dùng, kèm cảnh báo.
    pub warned_wrong: usize,
    pub mismatch_total: usize,
    /// Ca match mà mô hình nói mismatch: lời đúng bị bắt viết lại hay rơi về câu khuôn.
    pub false_alarms: usize,
    /// Ca match mà mô hình nói warning: lời đúng tới người dùng kèm cảnh báo thừa.
    pub warned_right: usize,
    pub match_total: usize,
    pub unavailable: usize,
    pub median_ms: u64,
    pub cases: Vec<JudgeOutcome>,
}

/// Đọc file ca và kiểm expected của mỗi ca là match hay mismatch. Lỗi là lỗi hạ tầng (mã
/// thoát 2 ở cli).
pub fn load_judge_cases(path: &Path) -> Result<JudgeCases, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    let cases: JudgeCases = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))?;
    for case in &cases.cases {
        if !matches!(case.expected.as_str(), "match" | "mismatch") {
            return Err(AppError::Internal(format!(
                "case {}: expected must be match or mismatch",
                case.id
            )));
        }
    }
    Ok(cases)
}

/// Chấm lần lượt từng ca. pause là khoảng nghỉ giữa hai ca, để không vượt hạn mức số lần gọi mỗi phút của khóa.
pub async fn run_eval_judge(
    llm: &dyn LlmProvider,
    config: &Config,
    cases: &JudgeCases,
    pause: Duration,
) -> JudgeReport {
    let timeout = build_judge_timeout(config);
    let mut outcomes = Vec::new();
    for (index, case) in cases.cases.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(pause).await;
        }
        let started = Instant::now();
        let prompt = build_judge_prompt(&case.text, &case.facts);
        let verdict = judge(llm, &case.id, &prompt, timeout).await;
        let (got, detail) = match verdict {
            Ok(Verdict::Match) => (Some("match".to_string()), String::new()),
            Ok(Verdict::Warning(problems)) => (Some("warning".to_string()), problems.join("; ")),
            Ok(Verdict::Mismatch(problems)) => (Some("mismatch".to_string()), problems.join("; ")),
            Err(reason) => (None, reason),
        };
        outcomes.push(JudgeOutcome {
            id: case.id.clone(),
            expected: case.expected.clone(),
            error: case.error.clone(),
            got,
            detail,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
    }
    build_report(outcomes)
}

pub(super) fn build_report(cases: Vec<JudgeOutcome>) -> JudgeReport {
    let count = |keep: &dyn Fn(&JudgeOutcome) -> bool| cases.iter().filter(|c| keep(c)).count();
    let got = |c: &JudgeOutcome, verdict: &str| c.got.as_deref() == Some(verdict);
    let mut durations: Vec<u64> = cases.iter().map(|c| c.duration_ms).collect();
    durations.sort_unstable();
    JudgeReport {
        total: cases.len(),
        correct: count(&|c| c.got.as_deref() == Some(c.expected.as_str())),
        missed: count(&|c| c.expected == "mismatch" && got(c, "match")),
        warned_wrong: count(&|c| c.expected == "mismatch" && got(c, "warning")),
        mismatch_total: count(&|c| c.expected == "mismatch"),
        false_alarms: count(&|c| c.expected == "match" && got(c, "mismatch")),
        warned_right: count(&|c| c.expected == "match" && got(c, "warning")),
        match_total: count(&|c| c.expected == "match"),
        unavailable: count(&|c| c.got.is_none()),
        median_ms: durations.get(durations.len() / 2).copied().unwrap_or(0),
        cases,
    }
}
