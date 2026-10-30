// SPDX-License-Identifier: Apache-2.0
/* service/ask/repeated.rs - chặn câu SQL cộng (SUM) một cột chép từ bảng nối nhiều-một của bản chụp database
(service/sources/, docs/DATA.md mục Nối bảng theo khóa ngoại).

Vì sao: dataset "order_items nối products" lặp giá của một sản phẩm trên mọi dòng hàng của sản phẩm đó; SUM("products.price")
trên bảng cộng giá đó một lần cho mỗi dòng hàng, ra một con số sai mà trông hợp lý. Prompt SQL đã có luật cấm, nhưng mô
hình vẫn có lúc viết (chạy thật với Gemini: "Tổng tín chỉ đăng ký theo vùng" ra SUM("students.studied_credits") trên
bảng bài nộp). Nên ask/run.rs kiểm sau guard: câu cộng cột như vậy ngay trên bảng dataset thì không chạy mà gửi mô hình
sửa, kèm cách viết đúng (giữ một dòng cho mỗi dòng của bảng gốc bằng SELECT DISTINCT theo cột khóa, rồi mới cộng). Câu
sửa vẫn cộng sai thì câu hỏi bị từ chối: thà không có số còn hơn số cộng lặp.

Chỉ xét SELECT đọc thẳng bảng dataset (kể cả trong WITH hay câu con); SUM trên một bảng WITH tự đặt tên (đã DISTINCT)
không bị chặn. So tên cột trong ngoặc kép, không phân biệt hoa thường: tên có dấu chấm nên luôn phải viết trong ngoặc. */

use std::ops::ControlFlow;

use sqlparser::ast::{Expr, Query, Select, SetExpr, TableFactor, Visit, Visitor};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::store::SourceModel;

/// Một cột chép từ bảng nối nhiều-một.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatedColumn {
    pub name: String,
    /// schema.bảng gốc.
    pub table: String,
    /// Cột của dataset xác định một dòng của bảng gốc.
    pub key: Vec<String>,
}

pub fn build_repeated_columns(model: Option<&SourceModel>) -> Vec<RepeatedColumn> {
    let Some(model) = model else {
        return Vec::new();
    };
    model
        .joins
        .iter()
        .flat_map(|join| {
            join.columns.iter().map(|name| RepeatedColumn {
                name: name.clone(),
                table: join.to_table.clone(),
                key: join.key_columns.clone(),
            })
        })
        .collect()
}

/// Lý do (tiếng Anh, gửi mô hình ở vòng sửa) khi câu cộng một cột lặp lại ngay trên bảng dataset; None khi không.
/// Câu không đọc được thì None: guard đã đọc được câu này, và lỗi đọc ở đây không nên chặn câu hỏi.
pub fn find_repeated_sum(sql: &str, table: &str, repeated: &[RepeatedColumn]) -> Option<String> {
    if repeated.is_empty() {
        return None;
    }
    let statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    let column = statements.iter().find_map(|statement| {
        let mut finder = QueryFinder {
            table,
            repeated,
            found: None,
        };
        let _ = statement.visit(&mut finder);
        finder.found
    })?;
    let key = column
        .key
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "The query sums \"{name}\", which was copied from the joined table {source}: its value repeats on every row \
         that links to the same {source} row, so SUM over the rows of {table} counts it many times. Keep one row per \
         {source} row first, then sum: WITH one_row AS (SELECT DISTINCT {key}, \"{name}\" (and any column you group by) \
         FROM {table}) SELECT SUM(\"{name}\") FROM one_row. If the question does not need a total, use MIN, MAX or \
         AVG instead.",
        name = column.name,
        source = column.table,
    ))
}

/// Đi qua mọi câu SELECT (câu ngoài, WITH, câu con): visitor của sqlparser gọi pre_visit_query cho từng Query.
struct QueryFinder<'a> {
    table: &'a str,
    repeated: &'a [RepeatedColumn],
    found: Option<&'a RepeatedColumn>,
}

impl Visitor for QueryFinder<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let SetExpr::Select(select) = query.body.as_ref() {
            if reads_table(select, self.table) {
                self.found = find_in_select(select, self.repeated);
                if self.found.is_some() {
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }
}

fn reads_table(select: &Select, table: &str) -> bool {
    select.from.iter().any(|from| {
        std::iter::once(&from.relation)
            .chain(from.joins.iter().map(|j| &j.relation))
            .any(|factor| match factor {
                TableFactor::Table { name, .. } => name
                    .0
                    .last()
                    .and_then(|part| part.as_ident())
                    .is_some_and(|ident| ident.value.eq_ignore_ascii_case(table)),
                _ => false,
            })
    })
}

/// SUM trong danh sách cột kết quả hay HAVING của một SELECT, có nhắc tên một cột lặp lại.
fn find_in_select<'a>(
    select: &Select,
    repeated: &'a [RepeatedColumn],
) -> Option<&'a RepeatedColumn> {
    let mut finder = SumFinder {
        repeated,
        found: None,
    };
    for item in &select.projection {
        let _ = item.visit(&mut finder);
    }
    if let Some(having) = &select.having {
        let _ = having.visit(&mut finder);
    }
    finder.found
}

struct SumFinder<'a> {
    repeated: &'a [RepeatedColumn],
    found: Option<&'a RepeatedColumn>,
}

impl<'a> Visitor for SumFinder<'a> {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        let Expr::Function(function) = expr else {
            return ControlFlow::Continue(());
        };
        if !function.name.to_string().eq_ignore_ascii_case("sum") {
            return ControlFlow::Continue(());
        }
        let args = function.args.to_string().to_lowercase();
        self.found = self
            .repeated
            .iter()
            .find(|c| args.contains(&format!("\"{}\"", c.name.to_lowercase())));
        if self.found.is_some() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credits() -> Vec<RepeatedColumn> {
        vec![RepeatedColumn {
            name: "students.studied_credits".to_string(),
            table: "edu.students".to_string(),
            key: vec!["code_module".to_string(), "id_student".to_string()],
        }]
    }

    #[test]
    fn sum_of_a_repeated_column_on_the_table_is_caught() {
        let sql =
            r#"SELECT "students.region", sum("students.studied_credits") FROM marks GROUP BY 1"#;
        let reason = find_repeated_sum(sql, "marks", &credits()).unwrap();
        assert!(
            reason.contains("SELECT DISTINCT \"code_module\", \"id_student\""),
            "{reason}"
        );
        // Trong câu con đọc thẳng bảng cũng bị bắt.
        let nested =
            r#"SELECT * FROM (SELECT SUM("Students.Studied_Credits") AS c FROM marks) AS x"#;
        assert!(find_repeated_sum(nested, "marks", &credits()).is_some());
    }

    #[test]
    fn other_aggregates_and_distinct_rows_pass() {
        let ok = [
            r#"SELECT avg("students.studied_credits"), sum("score") FROM marks"#,
            r#"WITH one_row AS (SELECT DISTINCT "code_module", "id_student", "students.studied_credits" FROM marks)
               SELECT SUM("students.studied_credits") FROM one_row"#,
        ];
        for sql in ok {
            assert_eq!(find_repeated_sum(sql, "marks", &credits()), None, "{sql}");
        }
        assert_eq!(
            find_repeated_sum("SELECT sum(x) FROM marks", "marks", &[]),
            None
        );
    }
}
