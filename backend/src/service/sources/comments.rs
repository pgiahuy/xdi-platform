// SPDX-License-Identifier: Apache-2.0
/* service/sources/comments.rs - mô tả bảng và mô tả cột mà tổ chức đã ghi trong database nguồn (COMMENT ON TABLE,
COMMENT ON COLUMN, #245). snapshot.rs đọc chúng trong cùng giao dịch chỉ đọc với lúc chép bảng, kiểu cột và khóa ngoại,
rồi đưa qua service/ingest.rs (SourceDescriptions): mô tả cột thành mô tả trong hồ sơ cột với nguồn database, mô tả
bảng ghi trên phiên bản.

Vì sao dùng: mô tả do người trong tổ chức viết đúng nghĩa hơn mô tả mô hình đoán từ tên cột và vài giá trị, và lớp ngữ
nghĩa (service/semantic/) không phải viết lại chúng. Chỉ đọc catalog (pg_description qua obj_description,
col_description), không đọc dòng dữ liệu nào.

Làm sạch trước khi dùng: mô tả nằm trên một dòng của prompt và của giao diện, nên mọi khoảng trắng (cả xuống dòng)
gộp thành một dấu cách, ký tự vô hình bị bỏ, comment rỗng hay chỉ có khoảng trắng coi như không có, và cắt còn
DESCRIPTION_MAX_CHARS ký tự như mô tả mô hình viết, để một comment rất dài không chiếm chỗ prompt của mọi câu hỏi. */

use sqlx::postgres::PgConnection;

use super::postgres::{query_error, SourceTable};
use crate::error::AppError;
use crate::service::ingest::SourceDescriptions;
use crate::service::semantic::{is_hidden_char, DESCRIPTION_MAX_CHARS};

/// Dấu ghi ở cuối mô tả bị cắt bớt, để người đọc và mô hình biết câu chưa hết.
const CUT_MARK: &str = "...";

/// Comment của bảng và của từng cột có comment, đã làm sạch. Bảng không có comment nào thì ra SourceDescriptions rỗng.
pub async fn read_descriptions(
    c: &mut PgConnection,
    table: &SourceTable,
) -> Result<SourceDescriptions, AppError> {
    let table_comment: Option<Option<String>> = sqlx::query_scalar(
        "SELECT obj_description(c.oid, 'pg_class') FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND c.relname = $2",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_optional(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    let column_comments: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT a.attname::text, col_description(a.attrelid, a.attnum) FROM pg_attribute a
         JOIN pg_class c ON c.oid = a.attrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped
         ORDER BY a.attnum",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    Ok(build_descriptions(table_comment.flatten(), column_comments))
}

/// Ghép comment đọc từ catalog thành SourceDescriptions: bỏ cột không có comment hay comment rỗng.
pub fn build_descriptions(
    table_comment: Option<String>,
    column_comments: Vec<(String, Option<String>)>,
) -> SourceDescriptions {
    SourceDescriptions {
        table: table_comment.as_deref().and_then(build_clean_comment),
        columns: column_comments
            .into_iter()
            .filter_map(|(name, comment)| Some((name, build_clean_comment(comment.as_deref()?)?)))
            .collect(),
    }
}

/// Một comment sau khi làm sạch; None khi không còn chữ nào. Quá DESCRIPTION_MAX_CHARS ký tự thì cắt ở khoảng trắng
/// cuối cùng trước giới hạn (không gãy giữa một chữ) và thêm CUT_MARK.
pub fn build_clean_comment(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| word.chars().filter(|c| !is_hidden_char(*c)).collect())
        .filter(|word: &String| !word.is_empty())
        .collect();
    let joined = words.join(" ");
    if joined.is_empty() {
        return None;
    }
    if joined.chars().count() <= DESCRIPTION_MAX_CHARS {
        return Some(joined);
    }
    let room = DESCRIPTION_MAX_CHARS - CUT_MARK.len();
    let cut: String = joined.chars().take(room).collect();
    let cut = match cut.rfind(' ') {
        Some(end) if end > 0 => &cut[..end],
        _ => cut.as_str(),
    };
    Some(format!("{}{CUT_MARK}", cut.trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_are_cleaned_and_empty_ones_dropped() {
        assert_eq!(
            build_clean_comment("  Mã đơn hàng\n\tduy nhất  ").as_deref(),
            Some("Mã đơn hàng duy nhất")
        );
        assert_eq!(build_clean_comment(""), None);
        assert_eq!(build_clean_comment("   \n\t "), None);
        // Ký tự vô hình và điều hướng hai chiều bị bỏ, chữ còn lại giữ.
        assert_eq!(
            build_clean_comment("Doanh\u{200b}thu \u{202e}gộp").as_deref(),
            Some("Doanhthu gộp")
        );
        assert_eq!(build_clean_comment("\u{feff}\u{200b}"), None);
    }

    #[test]
    fn long_comments_are_cut_at_a_space() {
        let long = "chữ ".repeat(200);
        let cut = build_clean_comment(&long).unwrap();
        assert!(cut.chars().count() <= DESCRIPTION_MAX_CHARS, "{cut}");
        assert!(cut.ends_with("chữ..."), "{cut}");
        // Một chữ dài không có khoảng trắng: cắt đúng ở giới hạn.
        let word = "a".repeat(DESCRIPTION_MAX_CHARS + 10);
        let cut = build_clean_comment(&word).unwrap();
        assert_eq!(cut.chars().count(), DESCRIPTION_MAX_CHARS);
    }

    #[test]
    fn columns_without_comment_are_left_out() {
        let built = build_descriptions(
            Some("  Đơn hàng bán ra ".to_string()),
            vec![
                ("order_id".to_string(), Some("Mã đơn hàng".to_string())),
                ("status".to_string(), None),
                ("note".to_string(), Some("  ".to_string())),
            ],
        );
        assert_eq!(built.table.as_deref(), Some("Đơn hàng bán ra"));
        assert_eq!(
            built.columns,
            vec![("order_id".to_string(), "Mã đơn hàng".to_string())]
        );
        let none = build_descriptions(None, vec![("a".to_string(), None)]);
        assert_eq!(none, SourceDescriptions::default());
    }
}
