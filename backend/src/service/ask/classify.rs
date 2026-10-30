// SPDX-License-Identifier: Apache-2.0
/* service/ask/classify.rs - bước phân loại câu hỏi (#197): trước khi viết SQL hay điền form, nhờ mô hình nói câu hỏi
là câu thường hay câu hỏi tìm nguyên nhân. ask/mod.rs gọi classify sau bước context; kết quả quyết định rẽ sang
ask/why.rs hay đi tiếp đường câu thường. Lệnh eval-classify (service/eval_classify.rs) gọi cùng hàm này để đo.

Vì sao nhờ mô hình thay cho so từ khóa: tiếng Việt có nhiều cách hỏi nguyên nhân không chứa cụm nào của luật từ khóa
("Doanh thu giảm do gì?"), còn câu đếm theo một cột tên "lý do" lại chứa đúng cụm đó. Luật từ khóa
(build_question_kind của explain/form.rs) vẫn giữ làm đường dự phòng: mô hình lỗi, quá giờ hay trả JSON không đọc được
thì dùng luật đó, câu hỏi vẫn chạy, và bước classify có ok = false kèm lý do.

Prompt chỉ có câu hỏi, tên cột kèm bí danh và mô tả (để mô hình nhận ra "lý do" là tên cột), và câu hỏi của các lượt
trước trong cuộc trò chuyện. Không có giá trị nào của cột, không có dòng dữ liệu (docs/PRIVACY.md). Chữ của prompt ở
backend/prompts/classify_question.txt. Test ở ask/classify_tests.rs. */

use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

use crate::config::Config;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::prompts::{self, fill};
use crate::engine::store::ProfileRole;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{AskStep, StepName};
use crate::service::ask::prompt::{build_within_limit, Detail};
use crate::service::ask::repair::build_user_text;
use crate::service::ask::run::build_duration_text;
use crate::service::ask::why::build_narrate_timeout;
use crate::service::explain::form::{build_question_kind, QuestionKind};

/// Số ký tự tối đa của lý do hiện ở bước classify; lý do dài hơn thì cắt, để chữ mô hình viết không chiếm cả tab.
pub const MAX_REASON_CHARS: usize = 160;

/// Đầu vào của bước phân loại.
pub struct ClassifyInput<'a> {
    /// Bảng logic của dataset, như trong prompt SQL.
    pub table: &'a str,
    pub question: &'a str,
    /// Cột của ngữ cảnh (ask/context.rs). Chỉ đọc tên, bí danh và mô tả; danh sách giá trị không vào prompt.
    pub columns: &'a [ColumnProfile],
    /// Câu hỏi của các lượt trước, cũ trước; rỗng khi câu mở đầu cuộc trò chuyện.
    pub earlier: &'a [String],
}

/// Loại câu đã chọn, ai chọn, và bước cho timeline.
pub struct Classified {
    pub kind: QuestionKind,
    /// true khi mô hình chọn; false khi dùng luật từ khóa.
    pub by_model: bool,
    pub step: AskStep,
}

/// Câu trả lời của mô hình theo build_classify_schema.
#[derive(Debug, Deserialize)]
struct ClassifyReply {
    kind: ReplyKind,
    reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReplyKind {
    Plain,
    Cause,
}

/// Không bao giờ trả lỗi: mô hình hỏng thì dùng luật từ khóa. timeout là trần của riêng lần gọi này
/// (build_classify_timeout), ngắn hơn timeout của nhà cung cấp: quá mức đó thì dùng luật từ khóa và chi tiết của bước
/// ghi là mô hình không trả lời kịp.
pub async fn classify(
    llm: &dyn LlmProvider,
    input: &ClassifyInput<'_>,
    timeout: Duration,
) -> Classified {
    let started = Instant::now();
    let prompt = build_classify_prompt(input);
    let request = LlmRequest {
        task: LlmTask::Classify,
        question: input.question,
        prompt: &prompt,
        json_schema: Some(build_classify_schema()),
    };
    let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            let why = format!("mô hình lỗi ({})", build_user_text(&error, "classify"));
            return fallback(input.question, started, &why);
        }
        Err(_) => {
            let why = format!(
                "mô hình không trả lời trong {}",
                build_duration_text(timeout)
            );
            return fallback(input.question, started, &why);
        }
    };
    match build_reply(&reply) {
        Ok(parsed) => {
            let kind = match parsed.kind {
                ReplyKind::Plain => QuestionKind::Plain,
                ReplyKind::Cause => QuestionKind::Why,
            };
            let reason: String = parsed
                .reason
                .trim()
                .chars()
                .take(MAX_REASON_CHARS)
                .collect();
            let detail = format!("{}: {reason}", build_kind_text(kind));
            Classified {
                kind,
                by_model: true,
                step: build_step(started, true, detail),
            }
        }
        Err(problem) => {
            tracing::warn!(%problem, "classify reply is unreadable");
            fallback(
                input.question,
                started,
                "câu trả lời của mô hình không đọc được",
            )
        }
    }
}

/// Trần thời gian của lần gọi: CLASSIFY_TIMEOUT_SECS (mặc định 15 giây; lần đo ngày 28/10/2026 với Gemini lâu nhất
/// 12,8 giây), không quá timeout của nhà cung cấp. Bước này chạy trước lần gọi viết SQL hay điền form, nên nếu dùng
/// đúng timeout của nhà cung cấp thì mô hình treo bắt người hỏi chờ gần gấp đôi trước khi nhận cùng một lỗi. Hạn chế:
/// Ollama lần gọi đầu còn nạp mô hình vào RAM, có thể quá mức này, khi đó câu hỏi đó dùng luật từ khóa.
pub fn build_classify_timeout(config: &Config) -> Duration {
    config.classify_timeout.min(build_narrate_timeout(config))
}

/// Prompt phân loại: luật ở backend/prompts/classify_question.txt, rồi danh sách cột, câu hỏi trước và câu hỏi. Cùng
/// trần PROMPT_MAX_CHARS với prompt SQL (build_within_limit): dài hơn thì chỉ giữ cột khớp câu hỏi và cột thời gian.
/// Prompt này không có danh sách giá trị nên ngắn hơn prompt SQL nhiều, ít khi chạm trần.
pub fn build_classify_prompt(input: &ClassifyInput<'_>) -> String {
    build_within_limit(|detail| build_prompt_text(input, detail))
}

fn build_prompt_text(input: &ClassifyInput<'_>, detail: Detail) -> String {
    let columns: Vec<String> = input
        .columns
        .iter()
        .filter(|c| {
            detail < Detail::MatchedColumns || c.matched || c.role == ProfileRole::Time.as_str()
        })
        .map(build_column_line)
        .collect();
    let earlier = if input.earlier.is_empty() {
        "(none)".to_string()
    } else {
        let lines: Vec<String> = input
            .earlier
            .iter()
            .map(|q| format!("- {}", q.trim()))
            .collect();
        lines.join("\n")
    };
    fill(
        &prompts::CLASSIFY_QUESTION,
        &[
            ("table", input.table),
            ("max_reason_chars", &MAX_REASON_CHARS.to_string()),
            ("columns", &columns.join("\n")),
            ("earlier", &earlier),
            ("question", input.question.trim()),
        ],
    )
}

/// Một dòng cột: tên trong nháy kép như prompt SQL, bí danh, mô tả. Không có vai trò hay giá trị: mô hình chỉ cần biết
/// tên nào là tên cột.
fn build_column_line(column: &ColumnProfile) -> String {
    let mut line = format!("- \"{}\"", column.name);
    if !column.aliases.is_empty() {
        line.push_str(&format!(" (aliases: {})", column.aliases.join(", ")));
    }
    if let Some(description) = column.description.as_deref().map(str::trim) {
        if !description.is_empty() {
            line.push_str(&format!(": {description}"));
        }
    }
    line
}

/// JSON Schema của câu trả lời, gửi kèm request (LlmRequest::json_schema).
pub fn build_classify_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": {
                "type": "string",
                "enum": ["plain", "cause"],
                "description": "cause when the question asks what caused a number, a change or a gap; plain otherwise"
            },
            "reason": {
                "type": "string",
                "description": "One short Vietnamese sentence saying why"
            }
        },
        "required": ["kind", "reason"]
    })
}

/// Đọc object JSON đầu tiên của câu trả lời, như build_form_from_reply (explain/form.rs): mô hình không bị ép bằng
/// schema (mock, Ollama cũ) có thể viết thêm chữ trước sau hay bọc object trong khối ```json.
fn build_reply(reply: &str) -> Result<ClassifyReply, String> {
    let start = reply.find('{').ok_or("the reply contains no JSON object")?;
    let mut objects =
        serde_json::Deserializer::from_str(&reply[start..]).into_iter::<ClassifyReply>();
    match objects.next() {
        Some(Ok(parsed)) => Ok(parsed),
        Some(Err(e)) => Err(format!("the reply is not a classify object: {e}")),
        None => Err("the reply contains no JSON object".to_string()),
    }
}

/// Đường dự phòng: luật từ khóa, kèm lý do không dùng được mô hình.
fn fallback(question: &str, started: Instant, why: &str) -> Classified {
    let kind = build_question_kind(question);
    let detail = format!(
        "Dùng luật từ khóa vì {why}. Kết quả: {}.",
        build_kind_text(kind).to_lowercase()
    );
    Classified {
        kind,
        by_model: false,
        step: build_step(started, false, detail),
    }
}

fn build_kind_text(kind: QuestionKind) -> &'static str {
    match kind {
        QuestionKind::Why => "Câu hỏi tìm nguyên nhân",
        QuestionKind::Plain => "Câu hỏi thường",
    }
}

fn build_step(started: Instant, ok: bool, detail: String) -> AskStep {
    AskStep {
        name: StepName::Classify,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        ok,
        detail: Some(detail),
    }
}
