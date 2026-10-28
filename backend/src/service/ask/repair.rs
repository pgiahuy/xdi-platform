// SPDX-License-Identifier: Apache-2.0
/* service/ask/repair.rs - chữ của vòng sửa: prompt gửi mô hình, lời báo lỗi đã bỏ đường dẫn trên máy chủ, lời cho
người dùng khi mô hình lỗi. ask/run.rs ghép các bước và gọi các hàm ở đây; câu trả lời của lần sửa đọc bằng
ask/reply.rs như lần đầu. */

use crate::engine::analytics::parquet_glob;
use crate::engine::guard::AllowedTable;
use crate::engine::prompts::{self, fill};
use crate::error::AppError;

/// Prompt của bước repair, chữ ở backend/prompts/sql_repair*.txt. Phần luật viết tiếng Anh như prompt sinh SQL (#29);
/// prompt gốc đã có hồ sơ bảng và câu hỏi.
/// sql là câu SQL của lần đầu; None khi câu trả lời lần đầu không đọc được, error khi đó là lý do của ask/reply.rs.
/// Câu sửa phải cùng dạng JSON như lần đầu nhưng không được hỏi lại: hết vòng sửa thì không còn lượt nào để dùng câu
/// trả lời của người dùng.
pub fn build_repair_prompt(prompt: &str, sql: Option<&str>, error: &str) -> String {
    let problem = match sql {
        Some(sql) => fill(&prompts::SQL_REPAIR_FAILED_SQL, &[("sql", sql)]),
        None => fill(&prompts::SQL_REPAIR_UNREADABLE_REPLY, &[]),
    };
    fill(
        &prompts::SQL_REPAIR,
        &[("prompt", prompt), ("problem", &problem), ("error", error)],
    )
}

/// Lời báo của DuckDB không lộ đường dẫn trên máy chủ: bỏ phần "LINE n:" DuckDB chép lại cả câu đã chạy (bản runnable,
/// có read_parquet), rồi thay mọi đường dẫn thư mục Parquet còn lại bằng tên bảng logic. Phần còn lại giữ nguyên, kể cả
/// dòng thứ hai trở đi (DuckDB ghi tên cột gần đúng, vị trí lỗi trong chuỗi ngày ở đó): mô hình cần để sửa đúng.
// Lời báo có thể chép một giá trị của dòng dữ liệu ("Could not convert string 'x' to INT32"). Khi #31 tiêm điều kiện
// phân quyền, cần thêm test: câu chạy lỗi không đưa vào lời báo giá trị của dòng mà người hỏi không được thấy.
pub fn build_safe_error(message: &str, tables: &[AllowedTable]) -> String {
    let mut text = without_sql_echo(message).to_string();
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

/// Lời báo nói về chính file Parquet của dataset (file hỏng: "Invalid Input Error: No magic bytes found at end of file
/// '<đường dẫn>'"). DuckDB xếp lỗi này cùng loại với lỗi của câu, nhưng mô hình không sửa được: đây là lỗi máy chủ.
/// Dựa vào đường dẫn vì guard không cho câu tự viết đường dẫn file, và lỗi của câu không nhắc tới file (phần LINE chép
/// lại câu đã bị bỏ trước khi so).
pub(crate) fn mentions_data_files(message: &str, tables: &[AllowedTable]) -> bool {
    let text = without_sql_echo(message);
    tables.iter().any(|table| {
        table
            .parquet_dir
            .to_str()
            .is_some_and(|dir| text.contains(dir))
    })
}

fn without_sql_echo(message: &str) -> &str {
    let end = message.find("\nLINE ").unwrap_or(message.len());
    message[..end].trim_end()
}

/// Lời cho người dùng và cho chi tiết bước: dòng đầu, bỏ loại lỗi tiếng Anh của DuckDB ở đầu ("Conversion Error: ",
/// "Binder Error: "), như mẫu failed ở ask/examples.rs. Mô hình nhận cả lời báo, kể cả loại lỗi.
pub(crate) fn build_user_error(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim_end();
    let without_kind = match line.split_once(": ") {
        Some((kind, rest))
            if kind.ends_with(" Error")
                && kind.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') =>
        {
            rest
        }
        _ => line,
    };
    without_kind.to_string()
}

/// Lời cho người dùng từ lỗi của mô hình: các loại lỗi của LlmProvider đã viết lời tiếng Việt (engine/llm/mod.rs). Lỗi
/// nội bộ chỉ ghi chi tiết vào log, như AppError::Internal ở error.rs. `step` là tên bước đang gọi mô hình ("repair",
/// "narrate"), chỉ để dòng log của lỗi nội bộ nói đúng bước.
pub(super) fn build_user_text(error: &AppError, step: &str) -> String {
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
            tracing::error!(error = %text, "model failed during {step}");
            "Lỗi nội bộ, xem log của backend".to_string()
        }
    }
}
