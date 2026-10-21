// SPDX-License-Identifier: Apache-2.0
/* engine/guard/names.rs - chỉ bảng, hàm, cột được phép. Theo danh sách được phép chứ không theo danh sách cấm: DuckDB
có nhiều cách đọc file (read_csv, read_text, glob, FROM 'x.csv'), đọc cấu hình, gọi mạng, và có thêm hàm mới qua từng
bản; danh sách cấm sẽ luôn sót một cách. Thấy tên lạ là từ chối.

Kiểm hai lượt: trình duyệt cây gặp phần SELECT trước phần FROM, nên lượt đầu gom hết bảng, bảng tạm (WITH), bí danh,
lượt sau mới kiểm được tên cột.

Tên cột kiểm trên cả câu, không theo phạm vi từng câu con: một cột có ở câu con này mà dùng ở câu con khác vẫn qua, và
DuckDB báo lỗi khi chạy. Bước này để bắt tên LLM bịa ra và nói rõ cho nó sửa, không phải lớp chặn dữ liệu. */

use std::collections::{BTreeSet, HashSet};
use std::ops::ControlFlow;

use sqlparser::ast::{
    Expr, Ident, ObjectName, Query, Select, SelectItem, TableAlias, TableFactor, Visit, Visitor,
};

use super::{find_table, name_key, rejected, single_ident, AllowedTable};
use crate::error::AppError;

/// Hàm dùng được trong câu hỏi phân tích: tổng hợp, cửa sổ, số, chuỗi, ngày giờ, xử lý null. Viết thường. Cần hàm
/// nào nữa thì thêm vào đây, sau khi chắc hàm đó không đọc file, cấu hình hay mạng.
// Giữ mỗi nhóm trên vài dòng cho dễ đọc; rustfmt sẽ tách mỗi tên một dòng.
#[rustfmt::skip]
const ALLOWED_FUNCTIONS: &[&str] = &[
    // Tổng hợp
    "count", "sum", "avg", "mean", "min", "max", "median", "mode", "stddev", "stddev_samp", "stddev_pop", "variance",
    "var_samp", "var_pop", "quantile_cont", "quantile_disc", "percentile_cont", "percentile_disc",
    "approx_count_distinct", "count_if", "any_value", "arg_max", "arg_min", "max_by", "min_by", "first", "last",
    "string_agg", "bool_and", "bool_or", "corr", "covar_pop", "covar_samp", "product",
    // Cửa sổ
    "row_number", "rank", "dense_rank", "percent_rank", "cume_dist", "ntile", "lag", "lead", "first_value",
    "last_value", "nth_value",
    // Số
    "abs", "round", "ceil", "ceiling", "floor", "trunc", "sqrt", "power", "pow", "exp", "ln", "log", "log10", "log2",
    "sign", "greatest", "least",
    // Null và điều kiện
    "coalesce", "nullif", "ifnull", "if",
    // Chuỗi
    "lower", "upper", "trim", "ltrim", "rtrim", "length", "concat", "concat_ws", "substring", "substr", "replace",
    "left", "right", "strpos", "instr", "starts_with", "ends_with", "contains", "lpad", "rpad", "split_part",
    "regexp_matches", "regexp_replace", "regexp_extract", "strip_accents",
    // Ngày giờ
    "date_trunc", "datetrunc", "date_part", "datepart", "date_diff", "datediff", "date_add", "date_sub", "year",
    "quarter", "month", "week", "weekofyear", "day", "dayofmonth", "dayofweek", "dayofyear", "isodow", "hour",
    "minute", "second", "yearweek", "monthname", "dayname", "last_day", "make_date", "strftime", "strptime",
    "to_timestamp", "epoch", "current_date", "current_timestamp", "now", "today",
];

/// Kiểm mọi bảng, hàm, cột trong câu. Trả tên các bảng tạm (WITH) để bước đổi tên dùng.
pub(super) fn check_names(
    query: &Query,
    tables: &[AllowedTable],
) -> Result<BTreeSet<String>, AppError> {
    let mut names = Names::default();
    let _ = query.visit(&mut Collector {
        tables,
        names: &mut names,
    });
    if let ControlFlow::Break(reason) = query.visit(&mut Checker {
        tables,
        names: &names,
    }) {
        return Err(rejected(reason));
    }
    Ok(names.ctes)
}

#[derive(Default)]
struct Names {
    /// Bảng tạm định nghĩa bằng WITH. BTreeSet để thứ tự cố định: bước đổi tên đánh số theo thứ tự này.
    ctes: BTreeSet<String>,
    /// Tên được đứng trước dấu chấm trong `x.cot`: bảng, bảng tạm, bí danh bảng.
    qualifiers: HashSet<String>,
    /// Cột trong hồ sơ của các bảng câu dùng, cộng tên câu tự đặt (`AS tong`, `t(a, b)`).
    columns: HashSet<String>,
}

/// Lượt đầu: gom tên, chưa kiểm gì.
struct Collector<'a> {
    tables: &'a [AllowedTable],
    names: &'a mut Names,
}

impl Collector<'_> {
    fn add_alias(&mut self, alias: Option<&TableAlias>) {
        if let Some(alias) = alias {
            self.names.qualifiers.insert(name_key(&alias.name));
            let columns = alias.columns.iter().map(|c| name_key(&c.name));
            self.names.columns.extend(columns);
        }
    }
}

impl Visitor for Collector<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        for cte in query.with.iter().flat_map(|with| &with.cte_tables) {
            self.names.ctes.insert(name_key(&cte.alias.name));
            self.add_alias(Some(&cte.alias));
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        match factor {
            TableFactor::Table { name, alias, .. } => {
                if let Some(ident) = single_ident(name) {
                    let key = name_key(ident);
                    if let Some(table) = find_table(self.tables, &key) {
                        let columns = table.columns.iter().map(|c| c.to_lowercase());
                        self.names.columns.extend(columns);
                    }
                    self.names.qualifiers.insert(key);
                }
                self.add_alias(alias.as_ref());
            }
            TableFactor::Derived { alias, .. } => self.add_alias(alias.as_ref()),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<()> {
        for item in &select.projection {
            match item {
                SelectItem::ExprWithAlias { alias, .. } => {
                    self.names.columns.insert(name_key(alias));
                }
                SelectItem::ExprWithAliases { aliases, .. } => {
                    self.names.columns.extend(aliases.iter().map(name_key));
                }
                _ => {}
            }
        }
        ControlFlow::Continue(())
    }
}

/// Lượt sau: kiểm từng bảng, hàm, cột theo tên đã gom.
struct Checker<'a> {
    tables: &'a [AllowedTable],
    names: &'a Names,
}

impl Checker<'_> {
    /// Kèm danh sách bảng vào lý do từ chối, để LLM sửa đúng tên ở vòng sau.
    fn table_list(&self) -> String {
        let names: Vec<&str> = self.tables.iter().map(|t| t.name.as_str()).collect();
        names.join(", ")
    }

    fn check_table(&self, name: &ObjectName) -> ControlFlow<String> {
        if let Some(ident) = single_ident(name) {
            let key = name_key(ident);
            if find_table(self.tables, &key).is_some() || self.names.ctes.contains(&key) {
                return ControlFlow::Continue(());
            }
        }
        ControlFlow::Break(format!(
            "Không có bảng {name}. Các bảng dùng được: {}",
            self.table_list()
        ))
    }

    fn check_column(&self, expr: &Expr) -> ControlFlow<String> {
        let known = |ident: &Ident| self.names.columns.contains(&name_key(ident));
        match expr {
            Expr::Identifier(column) if known(column) => ControlFlow::Continue(()),
            Expr::Identifier(column) => {
                ControlFlow::Break(format!("Không có cột {column} trong các bảng đang dùng"))
            }
            Expr::CompoundIdentifier(parts) => match parts.as_slice() {
                [table, _] if !self.names.qualifiers.contains(&name_key(table)) => {
                    ControlFlow::Break(format!("Không có bảng hay bí danh {table}"))
                }
                [_, column] if known(column) => ControlFlow::Continue(()),
                [_, column] => {
                    ControlFlow::Break(format!("Không có cột {column} trong các bảng đang dùng"))
                }
                _ => ControlFlow::Break(format!("Tên cột chỉ viết dạng cot hoặc bang.cot: {expr}")),
            },
            _ => ControlFlow::Continue(()),
        }
    }
}

impl Visitor for Checker<'_> {
    type Break = String;

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<String> {
        let Some(with) = &query.with else {
            return ControlFlow::Continue(());
        };
        // Câu đệ quy có thể chạy không dừng, và câu hỏi phân tích không cần nó.
        if with.recursive {
            return ControlFlow::Break("Không cho WITH RECURSIVE".to_string());
        }
        for cte in &with.cte_tables {
            // Trùng tên thì không rõ "FROM sales" là bảng tạm hay bảng dữ liệu.
            if find_table(self.tables, &name_key(&cte.alias.name)).is_some() {
                return ControlFlow::Break(format!(
                    "Bảng tạm {} trùng tên bảng dữ liệu, hãy đặt tên khác",
                    cte.alias.name
                ));
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<String> {
        match factor {
            // read_csv('...'), glob('...'), range(...): hàm sinh bảng, đọc được file ngoài dữ liệu đã nạp.
            TableFactor::Table {
                name,
                args: Some(_),
                ..
            } => ControlFlow::Break(format!(
                "Không cho gọi hàm {name} để lấy dữ liệu. Chỉ đọc từ các bảng: {}",
                self.table_list()
            )),
            TableFactor::Table { name, .. } => self.check_table(name),
            // Câu con trong FROM và JOIN lồng trong ngoặc: phần bên trong được duyệt và kiểm tiếp.
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                ControlFlow::Continue(())
            }
            other => ControlFlow::Break(format!("Không hỗ trợ cách lấy dữ liệu này: {other}")),
        }
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<String> {
        if let Expr::Function(function) = expr {
            let allowed = single_ident(&function.name)
                .is_some_and(|ident| ALLOWED_FUNCTIONS.contains(&name_key(ident).as_str()));
            if !allowed {
                return ControlFlow::Break(format!("Không cho dùng hàm {}", function.name));
            }
        }
        self.check_column(expr)
    }
}
