// SPDX-License-Identifier: Apache-2.0
/* service/ask/unsnapshotted.rs - lời báo khi câu hỏi cần một bảng của database nguồn mới có metadata, chưa có dữ liệu
(#247: bảng quá SOURCE_MAX_ROWS hay trần dung lượng lúc chụp, engine/store/source_tables.rs). ask/mod.rs gọi khi câu
không trả lời được (mô hình báo không có thông tin, guard từ chối, chạy lỗi): nếu một bảng như vậy của cùng nguồn khớp
câu hỏi thì lời nêu đúng tên bảng và cách chụp nó, thay cho lời chung "không có thông tin". Câu đã trả lời được thì
không gọi: câu hỏi chạm từ "sinh viên" không có nghĩa là cần bảng lượt bấm của sinh viên.

Khớp theo hai cách, cách nào chắc hơn trước: câu SQL mô hình viết có tên bảng (mô hình biết bảng từ câu mẫu hay từ câu
hỏi gõ tên bảng, guard chặn vì bảng không có trong danh sách); hay một cụm từ của câu hỏi có trong tên bảng, mô tả bảng,
tên hay mô tả cột của bảng đó (bỏ dấu, so trọn từ). Không khớp bảng nào thì giữ lời cũ. Hàm thuần, test ở cuối file. */

use crate::engine::store::MetadataTableRow;
use crate::service::ask::phrases::{build_phrases, fold};

/// Bảng chưa có dữ liệu mà câu hỏi (hay câu SQL mô hình viết cho nó) cần; None khi không bảng nào khớp.
pub fn find_needed_table<'a>(
    question: &str,
    sql: Option<&str>,
    tables: &'a [MetadataTableRow],
) -> Option<&'a MetadataTableRow> {
    if let Some(sql) = sql {
        let lower = sql.to_lowercase();
        let words = build_word_list(&lower);
        let named = tables.iter().find(|t| {
            let name = bare_name(&t.table_name).to_lowercase();
            words.iter().any(|w| *w == name)
        });
        if named.is_some() {
            return named;
        }
    }
    let phrases: Vec<String> = build_phrases(question)
        .into_iter()
        .filter(|p| !p.has_stop)
        .map(|p| p.folded)
        .collect();
    // Bảng có nhiều cụm khớp nhất; bằng nhau thì bảng xếp trước theo tên.
    tables
        .iter()
        .map(|t| {
            let text = build_table_text(t);
            let score = phrases.iter().filter(|p| contains_words(&text, p)).count();
            (score, t)
        })
        .filter(|(score, _)| *score > 0)
        .min_by_key(|(score, t)| (std::cmp::Reverse(*score), t.table_name.clone()))
        .map(|(_, t)| t)
}

/// "Câu này cần bảng edu.clicks: ..." kèm lý do lưu lúc chụp.
pub fn build_needed_table_message(table: &MetadataTableRow) -> String {
    format!(
        "Câu này cần dữ liệu của bảng {}: bảng có trong database nguồn nhưng chưa được chụp, mới có cột, kiểu và mô tả. \
         Lý do: {}. Admin chụp bảng này ở trang Dữ liệu (nút chụp bảng) rồi hỏi lại.",
        table.table_name,
        table.reason.trim_end_matches('.')
    )
}

/// Tên bảng không kèm schema: mô hình viết SQL theo tên bảng logic, không có schema.
fn bare_name(full: &str) -> &str {
    full.rsplit_once('.').map(|(_, name)| name).unwrap_or(full)
}

/// Từ của một câu SQL: chữ, số, gạch dưới; dấu ngoặc kép và dấu chấm tách từ.
fn build_word_list(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Chữ để so của một bảng: tên bảng (gạch dưới thành dấu cách), mô tả bảng, tên và mô tả cột, đã bỏ dấu.
fn build_table_text(table: &MetadataTableRow) -> String {
    let mut parts = vec![bare_name(&table.table_name).replace('_', " ")];
    parts.extend(table.description.clone());
    for c in table.build_columns() {
        parts.push(c.name.replace('_', " "));
        parts.extend(c.description);
    }
    format!(" {} ", fold(&parts.join(" ")))
}

/// Cụm có trong chữ như trọn từ (cả hai đã bỏ dấu, từ cách nhau một dấu cách).
fn contains_words(text: &str, phrase: &str) -> bool {
    text.contains(&format!(" {phrase} "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(table: &str, description: &str, columns: serde_json::Value) -> MetadataTableRow {
        MetadataTableRow {
            source_id: uuid::Uuid::nil(),
            table_name: table.to_string(),
            cause: "row_limit".to_string(),
            row_count: Some(1_808_119),
            size_bytes: None,
            reason: "Bảng có 1808119 dòng, quá giới hạn 1000000 dòng (SOURCE_MAX_ROWS)."
                .to_string(),
            description: Some(description.to_string()),
            columns,
        }
    }

    fn tables() -> Vec<MetadataTableRow> {
        vec![
            row(
                "edu.student_daily_clicks",
                "Số lần bấm trên hệ thống học trực tuyến của sinh viên theo ngày",
                json!([{"name": "sum_click", "data_type": "integer", "nullable": false, "primary_key": false,
                        "description": "Tổng số lần bấm trong ngày"}]),
            ),
            row("sales.audit_log", "Nhật ký thao tác", json!([])),
        ]
    }

    #[test]
    fn sql_naming_the_table_wins() {
        let tables = tables();
        let sql =
            "SELECT SUM(sum_click) FROM student_daily_clicks JOIN students USING (id_student)";
        let found = find_needed_table("Số đơn theo tháng", Some(sql), &tables).unwrap();
        assert_eq!(found.table_name, "edu.student_daily_clicks");
    }

    #[test]
    fn question_words_match_comments_without_accents() {
        let tables = tables();
        let found =
            find_needed_table("Trung bình số lan bam mỗi sinh viên", None, &tables).unwrap();
        assert_eq!(found.table_name, "edu.student_daily_clicks");
        let message = build_needed_table_message(found);
        assert!(
            message.contains("bảng edu.student_daily_clicks"),
            "{message}"
        );
        assert!(message.contains("SOURCE_MAX_ROWS)."), "{message}");
    }

    #[test]
    fn no_match_keeps_the_old_message() {
        let tables = tables();
        assert!(
            find_needed_table("Doanh thu theo vùng", Some("SELECT 1 FROM orders"), &tables)
                .is_none()
        );
        assert!(find_needed_table("Doanh thu theo vùng", None, &[]).is_none());
    }
}
