// SPDX-License-Identifier: Apache-2.0
/* service/ask/lessons.rs - chọn bài học đưa vào prompt (#228). ask/mod.rs đọc bài học đã duyệt, đang bật của dataset và
của cả workspace (Store::list_active_lessons), pick_lessons giữ tối đa LESSONS_MAX bài liên quan tới câu đang hỏi, rồi
prompt SQL, prompt form câu "tại sao" (ask/prompt.rs, ask/why_prompt.rs) và hai prompt viết lời (insight/narrate.rs,
ask/narrate.rs) nhận đúng các bài đó trong một khối riêng, chữ ở backend/prompts/lessons.txt.

Chỉ bài admin đã duyệt mới tới đây: chữ người dùng gõ (ghi chú "Chưa đạt", câu hỏi trong nhật ký) không bao giờ đi
thẳng vào prompt. Bài không liên quan thì bỏ: mỗi bài là một luật thêm cho mô hình, luật thừa kéo mô hình lệch ở câu
không cần nó. Test ở ask/lessons_tests.rs. */

use std::collections::HashSet;

use crate::engine::prompts::{self, fill};
use crate::engine::store::ActiveLesson;
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::phrases::build_phrases;

/// Số bài học tối đa trong một prompt.
pub const LESSONS_MAX: usize = 3;

/// Các bài liên quan tới câu hỏi, liên quan nhất trước: có chung từ (không tính từ dừng) với câu hỏi, hay nhắc tên một
/// cột câu hỏi khớp (ColumnProfile::matched). Cùng điểm thì giữ thứ tự đọc từ database (mới duyệt trước).
pub fn pick_lessons(
    question: &str,
    columns: &[ColumnProfile],
    lessons: &[ActiveLesson],
) -> Vec<String> {
    let words = build_words(question);
    let matched: Vec<String> = columns
        .iter()
        .filter(|c| c.matched)
        .map(|c| c.name.to_lowercase())
        .collect();
    let mut scored: Vec<(usize, &ActiveLesson)> = lessons
        .iter()
        .filter_map(|lesson| {
            let shared = build_words(&lesson.text).intersection(&words).count();
            let text = lesson.text.to_lowercase();
            let column = matched.iter().any(|name| text.contains(name.as_str()));
            if shared == 0 && !column {
                return None;
            }
            // Nhắc đúng cột của câu hỏi chắc hơn một từ chung; bài của riêng dataset này hơn bài của cả workspace.
            let score = shared + 2 * usize::from(column) + usize::from(lesson.dataset_id.is_some());
            Some((score, lesson))
        })
        .collect();
    // Sắp giảm dần theo điểm (Reverse); sort_by_key của Rust ổn định nên cùng điểm thì giữ thứ tự cũ.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored
        .into_iter()
        .take(LESSONS_MAX)
        .map(|(_, lesson)| lesson.text.clone())
        .collect()
}

/// Khối bài học của prompt; chuỗi rỗng khi không có bài nào, để prompt không đổi một byte.
pub fn build_lessons_section(lessons: &[String]) -> String {
    if lessons.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = lessons.iter().map(|l| format!("- {l}")).collect();
    fill(&prompts::LESSONS, &[("lessons", &lines.join("\n"))])
}

/// Prompt viết lời (đoạn trả lời, lời diễn giải) với khối bài học đứng trước: hai prompt đó là một file khuôn kết thúc
/// bằng dữ liệu kết quả, chèn giữa khuôn thì phải đổi file mẫu kể cả khi không có bài nào.
pub fn build_with_lessons(prompt: String, lessons: &[String]) -> String {
    let section = build_lessons_section(lessons);
    if section.is_empty() {
        prompt
    } else {
        format!("{section}\n\n{prompt}")
    }
}

/// Từ đơn không phải từ dừng, dạng so được, như cách prompt chọn câu mẫu (ask/prompt.rs).
fn build_words(text: &str) -> HashSet<String> {
    build_phrases(text)
        .into_iter()
        .filter(|p| p.len == 1 && !p.has_stop)
        .map(|p| p.key)
        .collect()
}
