// SPDX-License-Identifier: Apache-2.0
/* engine/guard/prompt_rules.rs - luật của guard viết cho mô hình ngôn ngữ, đặt vào prompt SQL (service/ask/prompt.rs) để
mô hình viết được câu qua guard ngay lần đầu. Sinh từ đúng các hằng số guard dùng để kiểm (danh sách hàm, các trần kích
thước), nên đổi luật ở guard thì prompt đổi theo; viết tay một bản thứ hai thì hai bên sớm muộn lệch nhau.

Chỉ có luật mô hình tự đáp ứng được khi đọc prompt (bước 1, 2 và cách viết LIMIT của bước 4). Phần guard tự thêm sau khi
kiểm không có ở đây: điều kiện lọc dòng theo vai dữ liệu lộ phạm vi dữ liệu của người hỏi, đường dẫn file Parquet lộ cách
lưu trên máy chủ (docs/PRIVACY.md), và mô hình không cần biết cả hai để viết câu. Guard vẫn kiểm lại mọi luật ở đây:
prompt chỉ hướng mô hình, không bảo đảm mô hình làm theo. */

use super::depth::MAX_QUERY_DEPTH;
use super::pieces::MAX_CONCAT_PIECES;
use super::sizes::{MAX_PAD_LENGTH, MAX_TIME_FORMAT_BYTES};
use super::width::{MAX_FROM_SOURCES, MAX_LITERAL_BYTES, MAX_SELECT_ITEMS, MAX_SET_BRANCHES};
use super::{ALLOWED_FUNCTIONS, MAX_SQL_BYTES};

/// tables: tên bảng logic câu được đọc, như AllowedTable::name.
pub fn build_prompt_rules(tables: &[&str]) -> String {
    let sources = match tables {
        [table] => format!("the table {table}"),
        _ => format!("the tables {}", tables.join(", ")),
    };
    let functions = ALLOWED_FUNCTIONS.join(", ");
    format!(
        "SQL rules. They have no exceptions: a query that breaks any of them is refused and never runs.\n\
         - Write one read-only SELECT statement (WITH, UNION, INTERSECT and EXCEPT are allowed), at most \
         {MAX_SQL_BYTES} bytes. No other statement: no INSERT, UPDATE, DELETE, CREATE, DROP, COPY, ATTACH, PRAGMA, SET \
         or CALL, and no SELECT INTO, TABLE, VALUES, FOR UPDATE or FOR SHARE.\n\
         - Read rows only from {sources}, or from a WITH table you define with a name of its own. These are already \
         tables: never read a file, never put a function in FROM (such as read_csv_auto, glob or range), never write a \
         schema before a table name, never read system tables, and never use UNNEST, PIVOT or WITH RECURSIVE.\n\
         - Use only the listed columns and the names the query itself defines with AS, written as \"column\" or \
         table.\"column\", never with three parts.\n\
         - Call only these functions, written as f(x) and never as x.f(): {functions}. CAST and CASE are allowed.\n\
         - lpad and rpad: the length is a number written out, at most {MAX_PAD_LENGTH}. strftime(value, 'format'): \
         the format is written out, at most {MAX_TIME_FORMAT_BYTES} bytes. replace: the searched text and the \
         replacement are written out, and the replacement is not longer than the searched text. regexp_replace: the \
         pattern and the replacement are written out; the replacement is empty, or one ASCII character when the \
         pattern cannot match an empty string.\n\
         - At most {MAX_QUERY_DEPTH} query levels counting the outer query, every subquery and every WITH table; at \
         most {MAX_CONCAT_PIECES} pieces joined with ||, concat or concat_ws in the whole query; at most \
         {MAX_LITERAL_BYTES} bytes in one string literal; at most {MAX_SELECT_ITEMS} columns in one SELECT; at most \
         {MAX_FROM_SOURCES} sources in one FROM, joins included; at most {MAX_SET_BRANCHES} UNION, INTERSECT or EXCEPT \
         branches in the whole query.\n\
         - Write LIMIT n only when the question asks for a number of rows, and never FETCH FIRST or LIMIT BY: the system \
         caps the rows itself. Never filter rows for access rights: the system does that too."
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
