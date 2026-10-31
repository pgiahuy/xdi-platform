// SPDX-License-Identifier: Apache-2.0
/* service/ask/classify.rs - bước phân loại câu hỏi (#197): trước khi viết SQL hay điền form, nhờ mô hình nói câu hỏi
là câu thường hay câu hỏi tìm nguyên nhân. ask/mod.rs gọi classify sau bước context; kết quả quyết định rẽ sang
ask/why.rs hay đi tiếp đường câu thường. Lệnh eval-classify (service/eval_classify.rs) gọi cùng hàm này để đo.

Vì sao nhờ mô hình thay cho so từ khóa: tiếng Việt có nhiều cách hỏi nguyên nhân không chứa cụm nào của luật từ khóa
("Doanh thu giảm do gì?"), còn câu đếm theo một cột tên "lý do" lại chứa đúng cụm đó. Luật từ khóa
(build_question_kind của explain/form.rs) vẫn giữ làm đường dự phòng: mô hình lỗi, quá giờ hay trả JSON không đọc được
thì dùng luật đó, câu hỏi vẫn chạy, và bước classify có ok = false kèm lý do.

Prompt có câu hỏi, metadata đủ của mọi cột của mọi bảng câu hỏi đọc (tên, kiểu, vai trò, đơn vị, cách gộp, mô tả, bí
danh, như prompt SQL, #230; để mô hình nhận ra "lý do" là tên cột), và câu hỏi của các lượt trước trong cuộc trò
chuyện. Không có giá trị nào của cột, không có dòng dữ liệu (docs/PRIVACY.md). Không bỏ cột nào khi prompt dài: chỉ khi
tổng metadata vượt trần (ask/context.rs apply_metadata_cap) thì cột không liên quan rút về dòng gọn. Chữ của prompt ở
backend/prompts/classify_question.txt. Test ở ask/classify_tests.rs.

Mô hình trả thêm `columns`: các cột câu hỏi cần, dạng bảng.cột. Tên không có thật thì bỏ qua. ask/mod.rs dùng chúng để
chọn cột được gửi đủ danh sách giá trị ở lượt viết SQL. Mô hình lỗi hay rơi về luật từ khóa thì columns rỗng.

Database vượt trần metadata (#249, ask/table_pick.rs): prompt có thêm danh sách gọn các bảng chỉ gửi tên
(backend/prompts/classify_other_tables.txt), và mô hình trả thêm `tables`, các bảng trong danh sách đó câu hỏi cần. Tên
không có trong danh sách thì bỏ qua. ask/mod.rs gửi chi tiết các bảng đó ở lượt viết SQL. */

use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

use crate::config::Config;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::prompts::{self, fill};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::{AskStep, StepName};
use crate::service::ask::group_prompt::{CompactTable, TableContext};
use crate::service::ask::prompt::{build_column_metadata, build_compact_tables, PROMPT_MAX_CHARS};
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
    /// Cột của ngữ cảnh (ask/context.rs) của bảng đang chọn. Chỉ đọc metadata; danh sách giá trị không vào prompt.
    pub columns: &'a [ColumnProfile],
    /// Các bảng liên quan câu hỏi đọc được (ask/group_prompt.rs, #213); rỗng khi câu chỉ đọc bảng đang chọn.
    pub related: &'a [TableContext],
    /// Câu hỏi của các lượt trước, cũ trước; rỗng khi câu mở đầu cuộc trò chuyện.
    pub earlier: &'a [String],
    /// Bảng của database chỉ gửi tên (#249); rỗng khi mọi bảng đã có khối cột.
    pub others: &'a [CompactTable],
}

/// Loại câu đã chọn, ai chọn, và bước cho timeline.
pub struct Classified {
    pub kind: QuestionKind,
    /// true khi mô hình chọn; false khi dùng luật từ khóa.
    pub by_model: bool,
    /// Cột mô hình nói câu hỏi cần, cặp (bảng logic, tên cột), đều có thật; rỗng khi dùng luật từ khóa.
    pub columns: Vec<(String, String)>,
    /// Bảng của danh sách gọn mô hình nói câu hỏi cần (tên đúng như trong others); rỗng khi dùng luật từ khóa.
    pub tables: Vec<String>,
    pub step: AskStep,
}

/// Câu trả lời của mô hình theo build_classify_schema.
#[derive(Debug, Deserialize)]
struct ClassifyReply {
    kind: ReplyKind,
    reason: String,
    /// Mock và mô hình không theo schema có thể bỏ trường này.
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    tables: Vec<String>,
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
            let columns = build_reply_columns(input, &parsed.columns);
            let tables = build_reply_tables(input.others, &parsed.tables);
            let mut detail = format!("{}: {reason}", build_kind_text(kind));
            if !columns.is_empty() {
                let names: Vec<String> = columns.iter().map(|(t, c)| format!("{t}.{c}")).collect();
                detail.push_str(&format!(" Cột cần: {}.", names.join(", ")));
            }
            if !tables.is_empty() {
                detail.push_str(&format!(" Bảng cần thêm: {}.", tables.join(", ")));
            }
            Classified {
                kind,
                by_model: true,
                columns,
                tables,
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

/// Prompt phân loại: luật ở backend/prompts/classify_question.txt, rồi metadata cột của từng bảng, câu hỏi trước và câu
/// hỏi. Không hạ mức chi tiết theo PROMPT_MAX_CHARS như prompt SQL: prompt này không có giá trị nào để bỏ, và bỏ cột
/// là điều #230 không cho; vượt trần thì chỉ ghi log.
pub fn build_classify_prompt(input: &ClassifyInput<'_>) -> String {
    let mut blocks = vec![build_table_lines(input.table, input.columns)];
    for table in input.related {
        blocks.push(build_table_lines(&table.table, &table.columns));
    }
    // Danh sách gọn nằm cùng chỗ trống columns: câu không có bảng nào chỉ gửi tên thì prompt giữ nguyên chữ như trước.
    if !input.others.is_empty() {
        blocks.push(fill(
            &prompts::CLASSIFY_OTHER_TABLES,
            &[("tables", &build_compact_tables(input.others))],
        ));
    }
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
    let prompt = fill(
        &prompts::CLASSIFY_QUESTION,
        &[
            ("table", input.table),
            ("max_reason_chars", &MAX_REASON_CHARS.to_string()),
            ("columns", &blocks.join("\n\n")),
            ("earlier", &earlier),
            ("question", input.question.trim()),
        ],
    );
    if prompt.chars().count() > PROMPT_MAX_CHARS {
        tracing::warn!(
            chars = prompt.chars().count(),
            "classify prompt is over the limit; sending it anyway"
        );
    }
    prompt
}

/// Khối cột của một bảng: dòng tên bảng, rồi mỗi cột một dòng metadata như prompt SQL (không giá trị). Cột gọn chỉ
/// có tên, kiểu, vai trò.
fn build_table_lines(table: &str, columns: &[ColumnProfile]) -> String {
    let mut lines = vec![format!("Table {table}:")];
    for column in columns {
        if column.brief {
            lines.push(format!(
                "- \"{}\" ({}, {})",
                column.name, column.data_type, column.role
            ));
        } else {
            lines.push(build_column_metadata(column));
        }
    }
    lines.join("\n")
}

/// Cột mô hình trả, chỉ giữ cột có thật: "bảng.cột" với bảng là một bảng của câu hỏi, hay tên cột trơn của bảng đang
/// chọn. Nháy kép quanh tên được bỏ. Mỗi cột một lần, giữ thứ tự mô hình viết.
fn build_reply_columns(input: &ClassifyInput<'_>, names: &[String]) -> Vec<(String, String)> {
    let mut tables: Vec<(&str, &[ColumnProfile])> = vec![(input.table, input.columns)];
    tables.extend(
        input
            .related
            .iter()
            .map(|t| (t.table.as_str(), t.columns.as_slice())),
    );
    let exists = |table: &str, column: &str| {
        tables
            .iter()
            .any(|(t, columns)| *t == table && columns.iter().any(|c| c.name == column))
    };
    let unquote = |text: &str| text.trim().trim_matches('"').to_string();
    let mut found: Vec<(String, String)> = Vec::new();
    for name in names {
        let name = name.trim();
        let qualified = name.split_once('.').and_then(|(t, c)| {
            let (t, c) = (unquote(t), unquote(c));
            exists(&t, &c).then_some((t, c))
        });
        let plain = || {
            let c = unquote(name);
            exists(input.table, &c).then(|| (input.table.to_string(), c))
        };
        if let Some(pair) = qualified.or_else(plain) {
            if !found.contains(&pair) {
                found.push(pair);
            }
        }
    }
    found
}

/// Bảng mô hình trả, chỉ giữ bảng đã chụp có trong danh sách gọn (so không phân biệt hoa thường, bỏ schema và nháy
/// kép), mỗi bảng một lần, theo thứ tự mô hình viết.
fn build_reply_tables(others: &[CompactTable], names: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim().trim_matches('"');
        let bare = name.rsplit_once('.').map(|(_, t)| t).unwrap_or(name);
        let table = others
            .iter()
            .find(|t| !t.unsnapshotted && t.table.eq_ignore_ascii_case(bare));
        if let Some(table) = table {
            if !found.contains(&table.table) {
                found.push(table.table.clone());
            }
        }
    }
    found
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
            },
            "columns": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Columns the question needs, each as table.column with names exactly as listed; empty when unsure"
            },
            "tables": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Only when other tables are listed by name: those the question needs; empty otherwise"
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
        columns: Vec::new(),
        tables: Vec::new(),
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
