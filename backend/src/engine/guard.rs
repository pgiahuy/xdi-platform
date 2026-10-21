// SPDX-License-Identifier: Apache-2.0
/* engine/guard.rs - kiểm SQL do LLM sinh trước khi chạy. Các bước theo thứ tự: đọc thành cây cú pháp và chỉ nhận
đúng một câu SELECT (file này); chỉ bảng, hàm, cột được phép (names.rs); thêm giới hạn dòng, đổi bảng logic sang file
Parquet (rewrite.rs). Tiêm điều kiện phân quyền theo vai là issue riêng (#31), chen vào giữa hai bước sau.

Guard không đọc Postgres hay DuckDB: service đưa hồ sơ bảng vào, nên test được mà không cần cơ sở dữ liệu.

Kiểm trên cây cú pháp, không dò chữ trong câu: dò chữ "DROP" thì lọt khi viết hoa thường lẫn lộn hay chen chú thích vào
giữa, còn cây cú pháp là đúng thứ DuckDB sẽ hiểu. Lý do từ chối hiện cho người dùng và gửi lại cho LLM để tự sửa một
vòng, nên phải nói rõ cái gì sai. */

mod names;
mod rewrite;
#[cfg(test)]
mod tests;

use std::ops::ControlFlow;
use std::path::PathBuf;

use sqlparser::ast::{Ident, ObjectName, Query, Select, SetExpr, Statement, Visit, Visitor};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::error::AppError;

/// Một bảng câu hỏi được dùng, lấy từ hồ sơ của phiên bản dataset đang dùng.
pub struct AllowedTable {
    /// Tên bảng logic mà LLM viết trong SQL, ví dụ "sales" (cột datasets.logical_table).
    pub name: String,
    /// Tên cột trong hồ sơ (bảng columns).
    pub columns: Vec<String>,
    /// Thư mục chứa các file Parquet của phiên bản.
    pub parquet_dir: PathBuf,
}

#[derive(Debug)]
pub struct GuardedSql {
    /// Còn tên bảng logic: hiện cho người dùng và lưu vào queries.sql_guarded. Không lộ đường dẫn trên máy chủ.
    pub shown: String,
    /// Đã đổi bảng sang file Parquet: chỉ đưa cho DuckDB chạy.
    pub runnable: String,
}

/// Kiểm câu SQL của LLM. Trả câu đã kiểm ở hai dạng, hoặc BadRequest kèm lý do từ chối.
pub fn build_guarded_sql(
    sql: &str,
    tables: &[AllowedTable],
    max_rows: u64,
) -> Result<GuardedSql, AppError> {
    let mut query = parse_single_select(sql)?;
    let cte_names = names::check_names(&query, tables)?;
    // Tiêm điều kiện phân quyền (#31) đặt ở đây: sau khi tên đã được kiểm, trước khi bảng bị đổi sang file.
    rewrite::apply_row_limit(&mut query, max_rows)?;
    let shown = query.to_string();
    let runnable = rewrite::build_runnable(query, tables, &cte_names)?.to_string();
    Ok(GuardedSql { shown, runnable })
}

/// Đọc câu SQL, chỉ nhận đúng một câu SELECT. Trả cây cú pháp cho các bước kiểm sau.
fn parse_single_select(sql: &str) -> Result<Query, AppError> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql)
        .map_err(|e| rejected(format!("Không đọc được câu SQL: {e}")))?;
    if statements.is_empty() {
        return Err(rejected("Câu SQL đang trống"));
    }
    // "SELECT 1; DROP TABLE x": câu đầu hợp lệ không được kéo theo câu sau.
    if statements.len() > 1 {
        return Err(rejected("Chỉ nhận một câu SQL mỗi lần"));
    }
    let Statement::Query(query) = statements.remove(0) else {
        return Err(rejected("Chỉ cho phép câu SELECT để đọc dữ liệu"));
    };
    if let ControlFlow::Break(reason) = query.visit(&mut ReadOnly) {
        return Err(rejected(reason));
    }
    Ok(*query)
}

/// Đi khắp cây (câu con, WITH, UNION) để chắc mọi phần chỉ đọc. Kiểm riêng câu ngoài cùng là chưa đủ: cú pháp SQL cho
/// đặt DELETE trong WITH, và SELECT INTO tạo bảng mới.
struct ReadOnly;

impl Visitor for ReadOnly {
    type Break = &'static str;

    // Câu ngoài cùng đã được kiểm là SELECT và không đi qua đây, nên gặp câu lệnh nào là câu lồng bên trong.
    fn pre_visit_statement(&mut self, _statement: &Statement) -> ControlFlow<Self::Break> {
        ControlFlow::Break("Không cho câu lệnh lồng trong câu SELECT")
    }

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
        if is_select_body(&query.body) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break("Chỉ cho phép câu SELECT để đọc dữ liệu")
        }
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<Self::Break> {
        if select.into.is_some() {
            ControlFlow::Break("Không cho SELECT INTO vì nó tạo bảng mới")
        } else {
            ControlFlow::Continue(())
        }
    }
}

/// SELECT, câu trong ngoặc, hoặc các câu SELECT nối bằng UNION, INTERSECT, EXCEPT.
fn is_select_body(body: &SetExpr) -> bool {
    match body {
        SetExpr::Select(_) | SetExpr::Query(_) => true,
        SetExpr::SetOperation { left, right, .. } => is_select_body(left) && is_select_body(right),
        // "TABLE t" đọc cả bảng mà không có FROM, nên bước kiểm bảng sẽ không thấy nó. VALUES không cần cho câu hỏi
        // dữ liệu. Còn lại là câu ghi (INSERT, UPDATE, DELETE, MERGE).
        _ => false,
    }
}

/// Tên một phần như `sales` hay `"Sales"`. None khi có schema đi kèm (`main.sales`, `information_schema.tables`) hay
/// viết trong nháy đơn: DuckDB hiểu `FROM 'x.csv'` là đọc file x.csv.
fn single_ident(name: &ObjectName) -> Option<&Ident> {
    match name.0.as_slice() {
        [part] => part
            .as_ident()
            .filter(|ident| ident.quote_style != Some('\'')),
        _ => None,
    }
}

/// So tên không phân biệt hoa thường, như DuckDB: LLM có thể viết Sales hay sales, kể cả trong nháy kép.
fn name_key(ident: &Ident) -> String {
    ident.value.to_lowercase()
}

fn find_table<'a>(tables: &'a [AllowedTable], key: &str) -> Option<&'a AllowedTable> {
    tables.iter().find(|t| t.name.to_lowercase() == key)
}

fn rejected(reason: impl Into<String>) -> AppError {
    AppError::BadRequest(reason.into())
}
