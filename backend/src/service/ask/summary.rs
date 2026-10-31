// SPDX-License-Identifier: Apache-2.0
/* service/ask/summary.rs - đoạn trả lời bằng lời cho câu hỏi thường đã answered (AskResponse.answer_text): nhờ mô hình
viết một đến hai câu tiếng Việt từ kết quả đã tính, kiểm mọi số trong câu như bước narrate của câu "tại sao"
(ask/narrate.rs), rơi về câu khuôn do code viết khi cần. ask/mod.rs gọi summarize sau khi dựng thẻ "Đã hiểu là" và
vai trò cột, trước bước record, để bước này có trong timings của lịch sử.

Mô hình chỉ nhận câu hỏi, phần đã hiểu (độ đo, nhóm, lọc), tên cột kết quả, tối đa SUMMARY_ROWS dòng đầu (số viết kiểu
Việt Nam như bảng hiển thị, qua explain/number.rs) và tổng số dòng. Đây là dữ liệu kết quả thật nên có biến
ANSWER_TEXT để tắt (docs/PRIVACY.md); tắt thì dùng câu khuôn, không gọi mô hình.

Bước này dùng lại StepName::Narrate thay vì thêm tên bước mới: hai bước cùng một việc (mô hình viết lời từ số đã tính,
code kiểm số), và giữ nguyên enum thì giao diện cũ vẫn đọc được mọi bước. Như narrate, bước này không bao giờ làm câu
hỏi dừng. Test ở ask/summary_tests.rs. */

use std::collections::HashSet;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::prompts::{self, fill};
use crate::service::ask::contract::{
    AskStep, CellValue, ColumnRole, ResultTable, StepName, Understanding, ValueKind,
};
use crate::service::ask::repair::build_user_text;
use crate::service::ask::run::build_duration_text;
use crate::service::explain::number::{build_number_text, build_number_text_with};
use crate::service::explain::{allow_text, build_text_number_check, has_letter, push_known_text};

/// Số dòng đầu của kết quả gửi cho mô hình. Câu trả lời một hai câu chỉ nêu vài dòng đầu; gửi ít dòng cũng là gửi ít
/// dữ liệu thật ra ngoài.
pub const SUMMARY_ROWS: usize = 10;

/// Số ký tự tối đa của đoạn trả lời; câu của mô hình dài hơn thì bỏ, dùng câu khuôn.
pub const MAX_ANSWER_CHARS: usize = 400;

/// Giá trị ô có chữ dài quá thì cắt khi đưa vào câu khuôn, để đoạn dữ liệu dài không lọt nguyên văn ra giao diện.
const TEMPLATE_CELL_CHARS: usize = 80;

/// Câu trả lời khi mọi ô của kết quả đều trống, thường là tổng hợp trên tập không có dòng nào (hỏi năm 2019 trên dữ liệu
/// tới tháng 1/2018 ra một dòng SUM là null). Câu khuôn "<độ đo> là <giá trị>." khi đó thành "Tổng Sales là ." vì giá trị
/// rỗng, còn lời của mô hình hay bị bước kiểm số loại; cảnh báo all_null (ask/checks.rs) nói thêm khoảng thời gian dữ
/// liệu có khi câu lọc theo thời gian.
pub const NO_DATA_ANSWER: &str =
    "Không có dữ liệu khớp với câu hỏi, nên chưa có con số để trả lời.";

/// Đầu vào của summarize: câu hỏi, thẻ "Đã hiểu là" và bảng kết quả đã có vai trò cột.
pub struct SummaryInput<'a> {
    pub question: &'a str,
    pub understanding: Option<&'a Understanding>,
    pub result: &'a ResultTable,
}

/// Đoạn trả lời, cờ "do mô hình viết" và bước cho timeline.
pub struct SummaryOutcome {
    pub text: String,
    pub by_model: bool,
    pub step: AskStep,
}

/// None khi kết quả không có dòng nào: thẻ cảnh báo kết quả rỗng (ask/checks.rs) đã nói điều đó. Mọi ô đều trống thì
/// trả NO_DATA_ANSWER, không gọi mô hình: không có số nào để mô hình kể. enabled là Config::answer_text; false thì không
/// gọi mô hình.
pub async fn summarize(
    llm: &dyn LlmProvider,
    input: &SummaryInput<'_>,
    enabled: bool,
    timeout: Duration,
) -> Option<SummaryOutcome> {
    if input.result.rows.is_empty() {
        return None;
    }
    let started = Instant::now();
    if input.result.rows.iter().flatten().all(Option::is_none) {
        let detail = "Mọi ô của kết quả đều trống nên không gọi mô hình".to_string();
        return Some(SummaryOutcome {
            text: NO_DATA_ANSWER.to_string(),
            by_model: false,
            step: build_step(started, true, Some(detail)),
        });
    }
    if !enabled {
        let detail = "ANSWER_TEXT=false nên không gọi mô hình".to_string();
        return Some(fallback(input, build_step(started, true, Some(detail))));
    }
    let prompt = build_summary_prompt(input);
    let request = LlmRequest {
        task: LlmTask::Summary,
        question: input.question,
        prompt: &prompt,
        json_schema: None,
    };
    let reply = match tokio::time::timeout(timeout, llm.complete(&request)).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => {
            let detail = format!(
                "Mô hình không viết được đoạn trả lời: {}",
                build_user_text(&error, "summary")
            );
            return Some(fallback(input, build_step(started, false, Some(detail))));
        }
        Err(_) => {
            let detail = format!(
                "Mô hình không trả lời trong {}",
                build_duration_text(timeout)
            );
            return Some(fallback(input, build_step(started, false, Some(detail))));
        }
    };
    let text = reply.trim();
    let problem = if text.is_empty() {
        Some("Mô hình trả về câu trống".to_string())
    } else if text.chars().count() > MAX_ANSWER_CHARS {
        Some(format!("Câu của mô hình dài hơn {MAX_ANSWER_CHARS} ký tự"))
    } else {
        build_answer_check(text, input).err().map(|wrong| {
            tracing::warn!(numbers = ?wrong, "answer text has numbers outside the sent rows");
            format!(
                "Câu của mô hình có số không nằm trong kết quả đã gửi: {}",
                wrong.join(", ")
            )
        })
    };
    if let Some(detail) = problem {
        return Some(fallback(input, build_step(started, false, Some(detail))));
    }
    Some(SummaryOutcome {
        text: text.to_string(),
        by_model: true,
        step: build_step(started, true, None),
    })
}

/// Prompt: luật viết tiếng Anh như prompt narrate (chữ ở backend/prompts/summary.txt), dữ liệu dạng JSON. Chỉ
/// SUMMARY_ROWS dòng đầu; ô số viết như bảng hiển thị để số mô hình chép khớp đúng chuỗi bước kiểm chờ.
pub fn build_summary_prompt(input: &SummaryInput<'_>) -> String {
    let result = input.result;
    let columns: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    let rows: Vec<Vec<String>> = result
        .rows
        .iter()
        .take(SUMMARY_ROWS)
        .map(|row| build_row_texts(result, row))
        .collect();
    let data = serde_json::json!({
        "understood": build_understood_json(input.understanding),
        "columns": columns,
        "rows": rows,
        "total_rows": build_number_text(&result.total_rows.to_string()),
    });
    let json = serde_json::to_string_pretty(&data).expect("serialize summary data");
    fill(
        &prompts::SUMMARY,
        &[
            ("max_answer_chars", &MAX_ANSWER_CHARS.to_string()),
            ("question", input.question),
            ("result", &json),
        ],
    )
}

/// Câu khuôn: kết quả một ô là "<độ đo> là <số>."; nhiều dòng có cột số đo xếp giảm dần và một cột nhóm là "Có N
/// dòng; cao nhất là <nhóm> với <số>."; còn lại "Có N dòng kết quả.".
pub fn build_template_answer(input: &SummaryInput<'_>) -> String {
    let result = input.result;
    let total = build_number_text(&result.total_rows.to_string());
    if result.columns.len() == 1 && result.total_rows == 1 {
        let measure = input
            .understanding
            .and_then(|u| u.measures.first())
            .map_or(result.columns[0].name.as_str(), |m| m.label.as_str());
        let value = build_row_texts(result, &result.rows[0]).remove(0);
        return format!("{measure} là {value}.");
    }
    if let Some((group, value)) = build_top_group(result) {
        return format!("Có {total} dòng; cao nhất là {group} với {value}.");
    }
    format!("Có {total} dòng kết quả.")
}

/// Mọi số trong câu phải có nguyên văn trong các ô đã gửi, trong tổng số dòng, trong một nhãn của thẻ "Đã hiểu là" (độ
/// đo, nhóm, điều kiện lọc, khoảng thời gian, giới hạn số dòng), hoặc nằm trong một đoạn chữ dữ liệu (ô có chữ, tên
/// cột); số kèm chữ tỷ lệ luôn sai. Luật quét số ở explain/mod.rs.
pub fn build_answer_check(text: &str, input: &SummaryInput<'_>) -> Result<(), Vec<String>> {
    let result = input.result;
    let mut allowed = HashSet::new();
    let mut known = Vec::new();
    for row in result.rows.iter().take(SUMMARY_ROWS) {
        for cell in build_row_texts(result, row) {
            if has_letter(&cell) {
                push_known_text(&mut known, &cell);
            } else if let Some(abs) = cell.strip_prefix('-') {
                allowed.insert(abs.to_string());
                allowed.insert(cell);
            } else {
                // Ô ngày ("31/01/2018") góp từng dãy chữ số; ô số góp chính nó.
                allow_text(&mut allowed, &cell);
                allowed.insert(cell);
            }
        }
    }
    allowed.insert(build_number_text(&result.total_rows.to_string()));
    allowed.insert(result.total_rows.to_string());
    for column in &result.columns {
        push_known_text(&mut known, &column.name);
        allow_text(&mut allowed, &column.name);
    }
    let labels = build_labels(input.understanding);
    for label in &labels {
        push_known_text(&mut known, label);
        // Số trong nhãn của thẻ là số của câu hỏi (năm 2019, top 5), không phải số mô hình bịa: mô hình được nhắc lại
        // ở bất kỳ đâu, kể cả không chép nguyên nhãn ("năm 2019" thay cho nhãn "Năm 2019"). Số kèm chữ tỷ lệ vẫn bị
        // loại (build_text_number_check).
        allow_text(&mut allowed, label);
    }
    build_text_number_check(text, &allowed, &known, &labels.join(" "))
}

/// Chữ của từng ô trong một dòng, như giao diện hiện (cellFormat.ts): số kiểu Việt Nam, ngày dd/mm/yyyy, ô trống là
/// chuỗi rỗng.
fn build_row_texts(result: &ResultTable, row: &[Option<CellValue>]) -> Vec<String> {
    row.iter()
        .zip(&result.columns)
        .map(|(cell, column)| match cell {
            None => String::new(),
            Some(cell) => build_cell_text(cell, column.kind),
        })
        .collect()
}

pub(crate) fn build_cell_text(cell: &CellValue, kind: ValueKind) -> String {
    match (cell, kind) {
        (CellValue::Number(number), _) => build_float_text(*number),
        (CellValue::String(text), ValueKind::Decimal) => build_number_text_with(text, u32::MAX),
        (CellValue::String(text), ValueKind::Date) => build_date_text(text),
        (CellValue::String(text), _) => text.clone(),
        (CellValue::Boolean(true), _) => "Đúng".to_string(),
        (CellValue::Boolean(false), _) => "Sai".to_string(),
    }
}

/// Số nguyên giữ mọi chữ số; số lẻ từ 1 trở lên đúng hai chữ số lẻ; số lẻ dưới 1 giữ hai chữ số có nghĩa, như
/// formatNumber của cellFormat.ts.
fn build_float_text(number: f64) -> String {
    let Ok(value) = number.to_string().parse::<Decimal>() else {
        return number.to_string();
    };
    let shown = if value.fract().is_zero() {
        value.trunc()
    } else if value.abs() < Decimal::ONE {
        value.round_sf(2).unwrap_or(value).normalize()
    } else {
        let mut rounded = value.round_dp(2);
        rounded.rescale(2);
        rounded
    };
    build_number_text_with(&shown.to_string(), u32::MAX)
}

// "2018-01-31" thành "31/01/2018"; chuỗi khác dạng thì giữ nguyên.
fn build_date_text(text: &str) -> String {
    let parts: Vec<&str> = text.split('-').collect();
    match parts.as_slice() {
        [year, month, day] => format!("{day}/{month}/{year}"),
        _ => text.to_string(),
    }
}

/// Nhóm đứng đầu và số đo của nó, khi bảng có đúng một cột số đo, ít nhất một cột nhóm (hay thời gian) và cột số đo
/// xếp giảm dần trên các dòng đã có. Không xếp giảm dần thì dòng đầu không phải "cao nhất", nên không nói.
fn build_top_group(result: &ResultTable) -> Option<(String, String)> {
    let measures: Vec<usize> = indexes_with(result, |role| role == ColumnRole::Measure);
    let groups: Vec<usize> = indexes_with(result, |role| {
        matches!(role, ColumnRole::Dimension | ColumnRole::Time)
    });
    let (&[measure], Some(&group)) = (measures.as_slice(), groups.first()) else {
        return None;
    };
    let values: Option<Vec<Decimal>> = result
        .rows
        .iter()
        .map(|row| build_decimal(row[measure].as_ref()?))
        .collect();
    let values = values?;
    if values.len() < 2 || values.windows(2).any(|pair| pair[0] < pair[1]) {
        return None;
    }
    let texts = build_row_texts(result, &result.rows[0]);
    let name: String = texts[group].chars().take(TEMPLATE_CELL_CHARS).collect();
    if name.is_empty() {
        return None;
    }
    Some((name, texts[measure].clone()))
}

fn indexes_with(result: &ResultTable, wanted: impl Fn(ColumnRole) -> bool) -> Vec<usize> {
    result
        .columns
        .iter()
        .enumerate()
        .filter(|(_, column)| wanted(column.role))
        .map(|(i, _)| i)
        .collect()
}

fn build_decimal(cell: &CellValue) -> Option<Decimal> {
    match cell {
        CellValue::Number(number) => number.to_string().parse().ok(),
        CellValue::String(text) => text.parse().ok(),
        CellValue::Boolean(_) => None,
    }
}

// Nhãn độ đo, nhóm, lọc của thẻ "Đã hiểu là" gửi mô hình: đúng ba phần câu trả lời cần, không gửi SQL.
fn build_understood_json(understanding: Option<&Understanding>) -> serde_json::Value {
    let Some(u) = understanding else {
        return serde_json::Value::Null;
    };
    let labels = |items: &[crate::service::ask::contract::UnderstoodItem]| -> Vec<String> {
        items.iter().map(|item| item.label.clone()).collect()
    };
    serde_json::json!({
        "measures": labels(&u.measures),
        "group_by": labels(&u.group_by),
        "filters": labels(&u.filters),
    })
}

fn build_labels(understanding: Option<&Understanding>) -> Vec<String> {
    let Some(u) = understanding else {
        return Vec::new();
    };
    u.measures
        .iter()
        .chain(&u.group_by)
        .chain(&u.filters)
        .chain(&u.time_range)
        .chain(&u.limit)
        .map(|item| item.label.clone())
        .collect()
}

/// Câu khuôn thay cho câu của mô hình; step do bên gọi dựng vì ok và detail tùy lý do.
fn fallback(input: &SummaryInput<'_>, step: AskStep) -> SummaryOutcome {
    SummaryOutcome {
        text: build_template_answer(input),
        by_model: false,
        step,
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
