// SPDX-License-Identifier: Apache-2.0
/* service/ask/lessons_tests.rs - test chọn bài học cho prompt (ask/lessons.rs, #228): bài liên quan theo từ chung hay cột
câu hỏi khớp, tối đa LESSONS_MAX bài, khối riêng trong prompt SQL, prompt form và hai prompt viết lời; không có bài
nào thì prompt không đổi. */

use uuid::Uuid;

use crate::engine::store::{ActiveLesson, ColumnAggregation};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::lessons::{
    build_lessons_section, build_with_lessons, pick_lessons, LESSONS_MAX,
};

fn column(name: &str, matched: bool) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: "DOUBLE".to_string(),
        role: "measure".to_string(),
        time_format: None,
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched,
        unit: None,
        aggregation: ColumnAggregation::Sum,
        join_key: false,
    }
}

fn lesson(text: &str, dataset: bool) -> ActiveLesson {
    ActiveLesson {
        text: text.to_string(),
        dataset_id: dataset.then(Uuid::new_v4),
    }
}

#[test]
fn related_lessons_are_picked_by_words_or_matched_columns() {
    let columns = [column("studied_credits", true), column("score", false)];
    let lessons = [
        lesson("Cột score là điểm của một bài nộp", true),
        lesson("Số tín chỉ lấy trung bình, không cộng", false),
        lesson("Cột studied_credits là số tín chỉ mỗi sinh viên", true),
        lesson("Doanh thu là Order Item Total", true),
    ];
    let picked = pick_lessons("Tín chỉ theo kết quả cuối khóa", &columns, &lessons);
    // Bài nhắc cột câu hỏi khớp và có từ chung đứng đầu; bài chỉ nhắc cột không khớp, hay không chung từ nào, bị bỏ.
    assert_eq!(
        picked,
        [
            "Cột studied_credits là số tín chỉ mỗi sinh viên",
            "Số tín chỉ lấy trung bình, không cộng",
        ]
    );
}

#[test]
fn at_most_a_few_lessons() {
    let lessons: Vec<ActiveLesson> = (0..5)
        .map(|i| lesson(&format!("Doanh thu bài {i}"), true))
        .collect();
    let picked = pick_lessons("Doanh thu theo tháng", &[], &lessons);
    assert_eq!(picked.len(), LESSONS_MAX);
    // Cùng điểm thì giữ thứ tự đọc từ database (mới duyệt trước).
    assert_eq!(picked[0], "Doanh thu bài 0");
}

#[test]
fn lesson_block_is_separate_and_empty_without_lessons() {
    assert_eq!(build_lessons_section(&[]), "");
    assert_eq!(build_with_lessons("prompt".to_string(), &[]), "prompt");
    let lessons = ["Doanh thu là Order Item Total".to_string()];
    let section = build_lessons_section(&lessons);
    assert!(
        section.starts_with("Lessons an admin of the organization wrote"),
        "{section}"
    );
    assert!(
        section.ends_with("\n- Doanh thu là Order Item Total"),
        "{section}"
    );
    let narration = build_with_lessons("Rules\nQuestion: q".to_string(), &lessons);
    assert_eq!(narration, format!("{section}\n\nRules\nQuestion: q"));
}
