// SPDX-License-Identifier: Apache-2.0
/* engine/guard/prompt_rules.rs - luật của guard viết cho mô hình ngôn ngữ, đặt vào prompt SQL (service/ask/prompt.rs) để
mô hình viết được câu qua guard ngay lần đầu. Sinh từ đúng các hằng số guard dùng để kiểm (danh sách hàm, các trần kích
thước), nên đổi luật ở guard thì prompt đổi theo; viết tay một bản thứ hai thì hai bên sớm muộn lệch nhau. Khung chữ
của luật nằm ở backend/prompts/guard_rules.txt (engine/prompts.rs); các con số và danh sách hàm điền từ hằng số ở đây.

Chỉ có luật mô hình tự đáp ứng được khi đọc prompt (bước 1, 2 và cách viết LIMIT của bước 4). Phần guard tự thêm sau khi
kiểm không có ở đây: điều kiện lọc dòng theo vai dữ liệu lộ phạm vi dữ liệu của người hỏi, đường dẫn file Parquet lộ cách
lưu trên máy chủ (docs/PRIVACY.md), và mô hình không cần biết cả hai để viết câu. Guard vẫn kiểm lại mọi luật ở đây:
prompt chỉ hướng mô hình, không bảo đảm mô hình làm theo. */

use super::depth::MAX_QUERY_DEPTH;
use super::pieces::MAX_CONCAT_PIECES;
use super::sizes::{MAX_PAD_LENGTH, MAX_TIME_FORMAT_BYTES};
use super::width::{MAX_FROM_SOURCES, MAX_LITERAL_BYTES, MAX_SELECT_ITEMS, MAX_SET_BRANCHES};
use super::{ALLOWED_FUNCTIONS, MAX_SQL_BYTES};
use crate::engine::prompts::{fill, GUARD_RULES};

/// tables: tên bảng logic câu được đọc, như AllowedTable::name.
pub fn build_prompt_rules(tables: &[&str]) -> String {
    let sources = match tables {
        [table] => format!("the table {table}"),
        _ => format!("the tables {}", tables.join(", ")),
    };
    let functions = ALLOWED_FUNCTIONS.join(", ");
    fill(
        &GUARD_RULES,
        &[
            ("max_sql_bytes", &MAX_SQL_BYTES.to_string()),
            ("sources", &sources),
            ("functions", &functions),
            ("max_pad_length", &MAX_PAD_LENGTH.to_string()),
            ("max_time_format_bytes", &MAX_TIME_FORMAT_BYTES.to_string()),
            ("max_query_depth", &MAX_QUERY_DEPTH.to_string()),
            ("max_concat_pieces", &MAX_CONCAT_PIECES.to_string()),
            ("max_literal_bytes", &MAX_LITERAL_BYTES.to_string()),
            ("max_select_items", &MAX_SELECT_ITEMS.to_string()),
            ("max_from_sources", &MAX_FROM_SOURCES.to_string()),
            ("max_set_branches", &MAX_SET_BRANCHES.to_string()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi hàm và mọi trần hiện có của guard đều có trong chữ luật. Chữ viết tay một giá trị thay cho hằng số thì đổi
    /// trần ở guard làm chữ sai, và mô hình bị từ chối vì một luật nó không được biết.
    #[test]
    fn rules_name_every_allowed_function_and_limit() {
        let rules = build_prompt_rules(&["sales"]);
        for function in ALLOWED_FUNCTIONS {
            assert!(
                rules.contains(&format!(" {function},"))
                    || rules.contains(&format!(" {function}.")),
                "{function} missing"
            );
        }
        for limit in [
            MAX_SQL_BYTES,
            MAX_QUERY_DEPTH,
            MAX_CONCAT_PIECES,
            MAX_LITERAL_BYTES,
            MAX_SELECT_ITEMS,
            MAX_FROM_SOURCES,
            MAX_SET_BRANCHES,
            MAX_TIME_FORMAT_BYTES,
        ] {
            assert!(rules.contains(&format!("{limit} ")), "{limit} missing");
        }
        assert!(rules.contains(&format!("at most {MAX_PAD_LENGTH}.")));
        assert!(rules.contains("from the table sales,"));
        assert!(rules.contains("no exceptions"));
    }

    #[test]
    fn several_tables_are_listed() {
        let rules = build_prompt_rules(&["sales", "customers"]);
        assert!(
            rules.contains("from the tables sales, customers,"),
            "{rules}"
        );
    }

    /// Luật không nói gì về phần guard tự thêm: không đường dẫn, không cách lưu file (docs/PRIVACY.md).
    #[test]
    fn rules_say_nothing_about_storage() {
        let rules = build_prompt_rules(&["sales"]);
        for leak in ["parquet", "/data", "DATA_DIR"] {
            assert!(!rules.contains(leak), "{leak} in rules");
        }
    }
}
