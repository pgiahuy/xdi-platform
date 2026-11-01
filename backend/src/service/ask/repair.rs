// SPDX-License-Identifier: Apache-2.0
/* service/ask/repair.rs - chữ của vòng sửa: prompt gửi mô hình, lời báo lỗi đã bỏ đường dẫn trên máy chủ, lời cho
người dùng khi mô hình lỗi. ask/run.rs ghép các bước và gọi các hàm ở đây; câu trả lời của lần sửa đọc bằng
ask/reply.rs như lần đầu. */

use std::ops::ControlFlow;

use sqlparser::ast::{visit_expressions, Expr};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::analytics::parquet_glob;
use crate::engine::guard::{build_parent_tables, AllowedTable};
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
/// dòng thứ hai trở đi (DuckDB ghi tên cột gần đúng, vị trí lỗi trong chuỗi ngày ở đó): mô hình cần để sửa đúng. Bảng
/// cha của điều kiện lan theo khóa ngoại (#215) cũng được đọc trong bản runnable, nên đường dẫn của chúng cũng bị thay.
// Lời báo có thể chép một giá trị của dòng dữ liệu ("Could not convert string 'x' to INT32"); giá trị của cột nhạy cảm
// được ẩn trước khi gửi mô hình (build_model_error). Người bị lọc dòng: ô trong lời báo là của dòng họ được thấy, vì
// DuckDB tính biểu thức của câu sau điều kiện lọc của guard; ask/repair_row_filter_tests.rs giữ điều đó khi nâng DuckDB.
pub fn build_safe_error(message: &str, tables: &[AllowedTable]) -> String {
    let mut text = without_sql_echo(message).to_string();
    let parents = build_parent_tables(tables);
    for table in tables.iter().chain(&parents) {
        if let Ok(source) = parquet_glob(&table.parquet_dir) {
            text = text.replace(&source, &table.name);
        }
        if let Some(dir) = table.parquet_dir.to_str() {
            text = text.replace(dir, &table.name);
        }
    }
    text
}

/// Chữ thay cho một giá trị bị ẩn khỏi lời báo gửi mô hình.
pub const HIDDEN_VALUE: &str = "<hidden value>";

/// Lời báo gửi mô hình ở vòng sửa. DuckDB có thể chép một ô của dữ liệu vào lời báo ("Could not convert string
/// 'an@example.com' to INT32"). Câu đã chạy đọc cột đánh dấu nhạy cảm (sensitive là tên các cột đó) thì mọi chuỗi trong
/// nháy đơn hay nháy kép của lời báo bị thay bằng HIDDEN_VALUE, trừ chuỗi có sẵn trong câu SQL (chữ mô hình đã viết) và
/// tên bảng, tên cột của tables (mô hình đã có trong prompt). Câu không đọc cột nhạy cảm thì giữ nguyên, vì mô hình cần
/// ô mẫu để sửa cách đọc ngày ("1/31/2018 22:56").
pub fn build_model_error(
    error: &str,
    sql: &str,
    sensitive: &[String],
    tables: &[AllowedTable],
) -> String {
    if !reads_any_column(sql, sensitive) {
        return error.to_string();
    }
    let mut text = String::new();
    let mut hidden: Vec<&str> = Vec::new();
    let mut rest = error;
    // DuckDB đặt ô trong nháy đơn hay nháy kép tùy loại lỗi: đổi sang số là 'x', đổi sang ngày, giờ, số thập phân và
    // strptime là "x" (invalid date field format: "an@example.com"). Chỉ xét nháy đơn thì ô của ba loại sau lọt tới
    // mô hình. Hai loại nháy đều là một byte ASCII nên cắt chuỗi ở open + 1 không rơi vào giữa một ký tự UTF-8.
    while let Some(open) = rest.find(['\'', '"']) {
        let quote = &rest[open..=open];
        let after = &rest[open + 1..];
        let Some(close) = after.find(quote) else {
            break;
        };
        let value = &after[..close];
        text.push_str(&rest[..=open]);
        if value.is_empty() || sql.contains(value) || is_known_name(value, tables) {
            text.push_str(value);
        } else {
            text.push_str(HIDDEN_VALUE);
            hidden.push(value);
        }
        text.push_str(quote);
        rest = &after[close + 1..];
    }
    text.push_str(rest);
    // strptime chép lại ô ở dòng riêng, không nháy, trên dấu ^ chỉ chỗ lỗi; dòng đó cũng thay.
    let lines: Vec<&str> = text
        .lines()
        .map(|line| {
            if hidden.contains(&line) {
                HIDDEN_VALUE
            } else {
                line
            }
        })
        .collect();
    lines.join("\n")
}

// Tên bảng hay tên cột của các bảng được phép, kể cả dạng bang.cot của lời báo Binder Error ("Candidate bindings:
// "customers.amount""): giữ để mô hình sửa đúng tên cột. So không phân biệt hoa thường như DuckDB.
fn is_known_name(value: &str, tables: &[AllowedTable]) -> bool {
    value.split('.').all(|part| {
        tables.iter().any(|table| {
            table.name.eq_ignore_ascii_case(part)
                || table.columns.iter().any(|c| c.eq_ignore_ascii_case(part))
        })
    })
}

// Câu có nhắc một cột trong names (so không phân biệt hoa thường). Câu không đọc được thì so theo chữ: thà ẩn thừa.
fn reads_any_column(sql: &str, names: &[String]) -> bool {
    if names.is_empty() {
        return false;
    }
    let Ok(statements) = Parser::parse_sql(&DuckDbDialect {}, sql) else {
        let lower = sql.to_lowercase();
        return names.iter().any(|n| lower.contains(&n.to_lowercase()));
    };
    let mut found = false;
    let _ = visit_expressions(&statements, |e| {
        let name = match e {
            Expr::Identifier(ident) => Some(&ident.value),
            Expr::CompoundIdentifier(parts) => parts.last().map(|p| &p.value),
            _ => None,
        };
        if name.is_some_and(|n| names.iter().any(|s| s.eq_ignore_ascii_case(n))) {
            found = true;
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });
    found
}

/// Lời báo nói về chính file Parquet của dataset (file hỏng: "Invalid Input Error: No magic bytes found at end of file
/// '<đường dẫn>'"). DuckDB xếp lỗi này cùng loại với lỗi của câu, nhưng mô hình không sửa được: đây là lỗi máy chủ.
/// Dựa vào đường dẫn vì guard không cho câu tự viết đường dẫn file, và lỗi của câu không nhắc tới file (phần LINE chép
/// lại câu đã bị bỏ trước khi so).
pub(crate) fn mentions_data_files(message: &str, tables: &[AllowedTable]) -> bool {
    let text = without_sql_echo(message);
    let parents = build_parent_tables(tables);
    tables.iter().chain(&parents).any(|table| {
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
pub(crate) fn build_user_text(error: &AppError, step: &str) -> String {
    match error {
        AppError::BadRequest(text)
        | AppError::Upstream(text)
        | AppError::Unavailable(text)
        | AppError::Unauthorized(text)
        | AppError::Forbidden(text)
        | AppError::TooLarge(text)
        | AppError::NotFound(text)
        | AppError::InvalidField { message: text, .. } => text.clone(),
        AppError::NotImplemented(feature) => format!("Chưa cài đặt: {feature}"),
        AppError::Internal(text) => {
            tracing::error!(error = %text, "model failed during {step}");
            "Lỗi nội bộ, xem log của backend".to_string()
        }
    }
}
