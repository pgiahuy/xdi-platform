// SPDX-License-Identifier: Apache-2.0
/* service/conversation/history.rs - khối "Hội thoại trước" gửi mô hình (#164) và phần "Đã hiểu là" dạng chữ ngắn lưu
cùng mỗi lượt (queries.understood).

Mỗi lượt chỉ có câu hỏi, trạng thái, câu SQL đã chạy và phần đã hiểu (độ đo, bộ lọc, nhóm, kỳ). Không có dòng dữ liệu,
giá trị kết quả hay lời báo lỗi (lời lỗi của DuckDB có thể trích giá trị trong dữ liệu), đúng như docs/PRIVACY.md.
Khối đặt ngay trước câu hỏi trong prompt câu thường (ask/prompt.rs) và prompt form câu "tại sao" (ask/why_prompt.rs).
Phần luật viết tiếng Anh như các phần khác của prompt. Test ở conversation/history_tests.rs. */

use crate::engine::store::StoredTurn;
use crate::service::ask::contract::{Understanding, UnderstoodItem};

/// Số lượt gần nhất gửi mô hình. Câu tiếp nối thường chỉ dựa vào một hai lượt trước; gửi nhiều hơn thì prompt dài mà
/// mô hình dễ lấy nhầm bộ lọc của lượt cũ.
pub const HISTORY_TURNS: i64 = 4;

/// Trần độ dài của khối, tính theo ký tự. Prompt có trần chung PROMPT_MAX_CHARS (ask/prompt.rs) và chỉ biết hạ mức
/// chi tiết của danh sách cột; giữ khối lịch sử nhỏ để nó không đẩy phần cột ra ngoài.
pub const HISTORY_MAX_CHARS: usize = 4_000;

/// Đầu nhãn của điều kiện phân quyền guard thêm (understanding/labels.rs, build_row_filter_label). Phần này không đưa
/// vào lịch sử: guard tự thêm lại ở mỗi lượt, còn mô hình thấy nó thì có thể chép thành bộ lọc của câu hỏi.
const ROW_FILTER_LABEL_PREFIX: &str = "Phân quyền:";

const HISTORY_RULES: &str = "Earlier turns of this conversation, oldest first. They hold the questions and how they \
were understood, never data rows. If the question below follows on from them (for example it only names another \
value, another grouping or another period, or asks why), start from the last answered turn and change only what the \
question changes: a filter, a grouping or a period. If the question is complete on its own, ignore these turns.";

/// Khối "Hội thoại trước"; chuỗi rỗng khi chưa có lượt nào. turns theo thứ tự hỏi (Store::list_turns). Vượt
/// HISTORY_MAX_CHARS thì bỏ dần lượt cũ nhất; còn một lượt mà vẫn dài thì cắt phần cuối của nó.
pub fn build_history_section(turns: &[StoredTurn]) -> String {
    let start = turns.len().saturating_sub(HISTORY_TURNS as usize);
    let mut blocks: Vec<String> = turns[start..]
        .iter()
        .enumerate()
        .map(|(i, turn)| build_turn_text(i + 1, turn))
        .collect();
    while !blocks.is_empty() {
        let section = format!("{HISTORY_RULES}\n{}", blocks.join("\n"));
        if section.chars().count() <= HISTORY_MAX_CHARS {
            return section;
        }
        if blocks.len() == 1 {
            return section.chars().take(HISTORY_MAX_CHARS).collect();
        }
        blocks.remove(0);
    }
    String::new()
}

fn build_turn_text(number: usize, turn: &StoredTurn) -> String {
    let mut text = format!(
        "Turn {number}: {}\n  Status: {}",
        turn.question.trim(),
        build_status_text(&turn.status)
    );
    // SQL mô hình viết trước, bản sau guard khi không có (câu "tại sao" chạy như câu thường vẫn có cả hai): bản sau guard
    // có LIMIT và hộp phân quyền, mô hình chép lại thì câu mới dài và khó đọc hơn mà không đổi kết quả.
    if let Some(sql) = turn
        .sql_generated
        .as_deref()
        .or(turn.sql_guarded.as_deref())
    {
        text.push_str(&format!("\n  SQL: {}", sql.trim()));
    }
    if let Some(understood) = &turn.understood {
        text.push_str(&format!("\n  Understood as: {understood}"));
    }
    text
}

/// Trạng thái lưu trong queries (QueryStatus::as_str) viết theo tên của AskStatus, tên mô hình dễ hiểu hơn.
fn build_status_text(status: &str) -> &str {
    match status {
        "ok" => "answered",
        "error" => "failed",
        other => other,
    }
}

/// Phần "Đã hiểu là" dạng một dòng chữ: độ đo, bộ lọc (trừ điều kiện phân quyền), nhóm, kỳ. Chỉ có nhãn, nhãn dựng từ
/// câu SQL và hồ sơ cột, không từ kết quả. None khi thẻ không có mục nào.
pub fn build_understood_text(understanding: &Understanding) -> Option<String> {
    let filters: Vec<&UnderstoodItem> = understanding
        .filters
        .iter()
        .filter(|item| !item.label.starts_with(ROW_FILTER_LABEL_PREFIX))
        .collect();
    let parts = [
        build_part("measures", understanding.measures.iter().collect()),
        build_part("filters", filters),
        build_part("grouped by", understanding.group_by.iter().collect()),
        build_part("period", understanding.time_range.iter().collect()),
    ];
    let parts: Vec<String> = parts.into_iter().flatten().collect();
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("; "))
}

fn build_part(name: &str, items: Vec<&UnderstoodItem>) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    Some(format!("{name}: {}", labels.join(", ")))
}
