// SPDX-License-Identifier: Apache-2.0
/* service/ask_repair.rs - chữ của vòng sửa: prompt gửi mô hình, lời báo lỗi đã bỏ đường dẫn trên máy chủ, đọc câu SQL
mô hình trả về, lời cho người dùng khi mô hình lỗi. ask_run.rs ghép các bước và gọi các hàm ở đây. */

use crate::engine::analytics::parquet_glob;
use crate::engine::guard::AllowedTable;
use crate::error::AppError;

/// Prompt của bước repair. Phần luật viết tiếng Anh như prompt sinh SQL (#29); prompt gốc đã có hồ sơ bảng và câu hỏi.
pub fn build_repair_prompt(prompt: &str, sql: &str, error: &str) -> String {
    format!(
        "{prompt}\n\n\
         The SQL you wrote for this question could not run.\n\
         SQL:\n{sql}\n\n\
         Error:\n{error}\n\n\
         Fix the SQL so that it answers the same question. Use only the tables and columns given above. \
         Reply with the fixed SQL only: one SELECT statement, no explanation."
    )
}

/// Lời báo của DuckDB không lộ đường dẫn trên máy chủ: bỏ phần "LINE n:" DuckDB chép lại cả câu đã chạy (bản runnable,
/// có read_parquet), rồi thay mọi đường dẫn thư mục Parquet còn lại bằng tên bảng logic. Phần còn lại giữ nguyên, kể cả
/// dòng thứ hai trở đi (DuckDB ghi tên cột gần đúng, vị trí lỗi trong chuỗi ngày ở đó): mô hình cần để sửa đúng.
pub fn build_safe_error(message: &str, tables: &[AllowedTable]) -> String {
    let without_sql = message
        .find("\nLINE ")
        .map_or(message, |end| &message[..end]);
    let mut text = without_sql.trim_end().to_string();
    for table in tables {
        if let Ok(source) = parquet_glob(&table.parquet_dir) {
            text = text.replace(&source, &table.name);
        }
        if let Some(dir) = table.parquet_dir.to_str() {
            text = text.replace(dir, &table.name);
        }
    }
    text
}

/// Dòng đầu của lời báo cho người dùng và cho chi tiết bước; mô hình nhận cả lời báo.
pub(super) fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

/// Lấy câu SQL trong câu trả lời: mô hình hay bọc SQL trong khối ```sql ... ```. Đọc chữ thừa trước và sau khối là việc
/// của #29 (build_sql_from_reply); khi có hàm đó thì dùng chung.
pub(super) fn read_sql_reply(reply: &str) -> String {
    let text = reply.trim();
    let Some(start) = text.find("```") else {
        return text.to_string();
    };
    let body = &text[start + 3..];
    let body = body
        .strip_prefix("sql")
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .unwrap_or(body);
    let end = body.find("```").unwrap_or(body.len());
    body[..end].trim().to_string()
}

/// Lời cho người dùng từ lỗi của mô hình: các loại lỗi của LlmProvider đã viết lời tiếng Việt (engine/llm/mod.rs). Lỗi
/// nội bộ chỉ ghi chi tiết vào log, như AppError::Internal ở error.rs.
pub(super) fn build_user_text(error: &AppError) -> String {
    match error {
        AppError::BadRequest(text)
        | AppError::Upstream(text)
        | AppError::Unavailable(text)
        | AppError::Unauthorized(text)
        | AppError::Forbidden(text)
        | AppError::TooLarge(text)
        | AppError::NotFound(text) => text.clone(),
        AppError::NotImplemented(feature) => format!("Chưa cài đặt: {feature}"),
        AppError::Internal(text) => {
            tracing::error!(error = %text, "model failed during repair");
            "Lỗi nội bộ, xem log của backend".to_string()
        }
    }
}
