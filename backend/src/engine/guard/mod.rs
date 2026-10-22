// SPDX-License-Identifier: Apache-2.0
/* engine/guard/mod.rs - kiểm SQL do LLM sinh trước khi chạy, theo bốn bước trong build_guarded_sql: chỉ nhận một câu
SELECT chỉ đọc; kiểm tên bảng, hàm, cột; thêm LIMIT; đổi bảng logic sang file Parquet. Test ở guard/tests.rs.

Kiểm trên cây cú pháp vì dò chữ "DROP" thì lọt khi viết hoa thường lẫn lộn hay chen chú thích. Hạn chế: cây do
sqlparser dựng, không phải DuckDB, nên cú pháp hiếm có thể bị hai bên hiểu khác nhau.

Lý do từ chối được gửi lại cho LLM tự sửa, nên phải chỉ đúng chỗ sai. Guard nhận hồ sơ bảng từ service thay vì tự đọc
Postgres, để test không cần cơ sở dữ liệu. */

use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use sqlparser::ast::{
    Expr, Function, Ident, LimitClause, ObjectName, Query, Select, SelectItem, SetExpr, Statement,
    TableAlias, TableFactor, Value, Visit, VisitMut, Visitor, VisitorMut,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::analytics::parquet_glob;
use crate::error::AppError;

/// Bảng logic mà SQL của LLM được phép đọc, ví dụ "sales".
pub struct AllowedTable {
    /// Cột datasets.logical_table.
    pub name: String,
    /// Lấy từ bảng columns.
    pub columns: Vec<String>,
    pub parquet_dir: PathBuf,
}

/// Hai bản của cùng một câu, vì bản chạy chứa đường dẫn file trên máy chủ, không được lộ ra cho người dùng.
#[derive(Debug)]
pub struct GuardedSql {
    /// Hiện cho người dùng và lưu vào queries.sql_guarded.
    pub shown: String,
    /// Chỉ đưa cho DuckDB.
    pub runnable: String,
}

/// Cả hai bản in lại từ cây đã kiểm, không dùng câu gốc của LLM: phần guard bỏ qua khi đọc (chú thích) không tới được
/// DuckDB.
pub fn build_guarded_sql(
    sql: &str,
    tables: &[AllowedTable],
    max_rows: u64,
) -> Result<GuardedSql, AppError> {
    let mut query = parse_single_select(sql)?;
    let cte_names = check_names(&query, tables)?;
    // Chỗ cho #31 tiêm điều kiện phân quyền. Sau check_names để mọi bảng gặp được đều có trong danh sách; trước
    // build_runnable vì sau đó `sales s` thành `read_parquet(...) AS s`, mất tên bảng. `shown` sẽ chứa cả điều kiện.
    apply_row_limit(&mut query, max_rows)?;
    let shown = query.to_string();
    let runnable = build_runnable(query, tables, &cte_names)?.to_string();
    Ok(GuardedSql { shown, runnable })
}

// Bước 1: chỉ nhận một câu SELECT chỉ đọc.

fn parse_single_select(sql: &str) -> Result<Query, AppError> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql)
        .map_err(|e| rejected(format!("Không đọc được câu SQL: {e}")))?;
    if statements.is_empty() {
        return Err(rejected("Câu SQL đang trống"));
    }
    // Nhận nhiều câu thì câu SELECT hợp lệ ở đầu kéo theo được câu ghi phía sau: "SELECT 1; DROP TABLE x".
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

/// Kiểm riêng câu ngoài cùng là chưa đủ: cú pháp SQL cho đặt DELETE trong WITH, và SELECT INTO tạo bảng mới.
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

fn is_select_body(body: &SetExpr) -> bool {
    match body {
        SetExpr::Select(_) | SetExpr::Query(_) => true,
        SetExpr::SetOperation { left, right, .. } => is_select_body(left) && is_select_body(right),
        // "TABLE t" đọc cả bảng mà không qua FROM nên lọt bước kiểm bảng. VALUES bị từ chối vì câu hỏi dữ liệu không
        // cần. Còn lại là câu ghi.
        _ => false,
    }
}

// Bước 2: chỉ nhận bảng, hàm, cột có trong danh sách được phép. Không dùng danh sách cấm vì DuckDB có nhiều cách đọc
// file (read_csv, glob, FROM 'x.csv'), đọc cấu hình, gọi mạng, và thêm hàm mới qua từng bản: danh sách cấm sẽ luôn sót.
//
// Đi cây ba lượt vì Visitor gặp danh sách cột của SELECT trước phần FROM: Collector gom tên, TableChecker kiểm bảng,
// ExprChecker kiểm hàm và cột. Kiểm bảng trước cột để `SELECT region FROM orders` báo sai bảng orders; báo "không có
// cột region" thì LLM sẽ sửa cột.
//
// Hạn chế: cột kiểm trên cả câu, không theo phạm vi từng câu con, nên cột của câu con này dùng ở câu con khác vẫn qua
// guard và chỉ bị DuckDB báo lỗi lúc chạy. Kiểm cột để LLM sửa tên bịa, không phải để chặn dữ liệu.

/// Viết thường vì tên trong câu được hạ thường trước khi so. Chỉ thêm hàm sau khi chắc nó không đọc file, cấu hình
/// hay mạng.
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

/// Trả tên các bảng tạm (WITH) cho build_runnable đổi tên.
fn check_names(query: &Query, tables: &[AllowedTable]) -> Result<BTreeSet<String>, AppError> {
    let mut names = Names::default();
    let _ = query.visit(&mut Collector {
        tables,
        names: &mut names,
    });
    if let ControlFlow::Break(reason) = query.visit(&mut TableChecker {
        tables,
        names: &names,
    }) {
        return Err(rejected(reason));
    }
    if let ControlFlow::Break(reason) = query.visit(&mut ExprChecker { names: &names }) {
        return Err(rejected(reason));
    }
    Ok(names.ctes)
}

#[derive(Default)]
struct Names {
    /// BTreeSet vì build_runnable đánh số guard_cte_N theo thứ tự duyệt tập này; HashSet duyệt ngẫu nhiên thì cùng một
    /// câu ra bản chạy khác nhau.
    ctes: BTreeSet<String>,
    /// Tên được đứng trước dấu chấm trong `x.cot`: bảng, bảng tạm, bí danh bảng.
    qualifiers: HashSet<String>,
    /// Cột trong hồ sơ của các bảng câu dùng, cộng tên câu tự đặt (`AS tong`, `t(a, b)`).
    columns: HashSet<String>,
}

/// Không từ chối gì: phải gom hết mới biết một tên là lạ hay được định nghĩa ở chỗ duyệt sau.
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

struct TableChecker<'a> {
    tables: &'a [AllowedTable],
    names: &'a Names,
}

impl TableChecker<'_> {
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
}

impl Visitor for TableChecker<'_> {
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
            // Nhận thì build_runnable đổi mọi `FROM sales` sang file Parquet: câu đọc bảng dữ liệu thay vì bảng tạm,
            // ra kết quả sai mà không báo lỗi.
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
            // Chặn mọi hàm sinh bảng thay vì lọc từng hàm: read_csv, glob đọc được file ngoài dữ liệu đã nạp; range
            // vô hại nhưng câu hỏi dữ liệu không cần.
            TableFactor::Table {
                name,
                args: Some(_),
                ..
            } => ControlFlow::Break(format!(
                "Không cho gọi hàm {name} để lấy dữ liệu. Chỉ đọc từ các bảng: {}",
                self.table_list()
            )),
            TableFactor::Table { name, .. } => self.check_table(name),
            // Phần bên trong câu con và JOIN trong ngoặc được Visitor duyệt và kiểm tiếp.
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                ControlFlow::Continue(())
            }
            // UNNEST, PIVOT và các dạng khác: chưa cần; muốn nhận thì phải xét dạng đó có đọc được nguồn ngoài không.
            other => ControlFlow::Break(format!("Không hỗ trợ cách lấy dữ liệu này: {other}")),
        }
    }
}

struct ExprChecker<'a> {
    names: &'a Names,
}

impl ExprChecker<'_> {
    /// `main.sum` bị từ chối dù sum có trong danh sách: danh sách chỉ ghi tên trơn, không xét schema nào chứa gì.
    fn check_function(&self, function: &Function) -> ControlFlow<String> {
        let allowed = single_ident(&function.name)
            .is_some_and(|ident| ALLOWED_FUNCTIONS.contains(&name_key(ident).as_str()));
        if allowed {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(format!("Không cho dùng hàm {}", function.name))
        }
    }

    fn check_column(&self, column: &Ident) -> ControlFlow<String> {
        if self.names.columns.contains(&name_key(column)) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(format!("Không có cột {column} trong các bảng đang dùng"))
        }
    }

    /// Hạn chế: trường của cột kiểu struct (`cot.truong`) cũng bị từ chối, vì guard hiểu `cot` là tên bảng.
    fn check_qualified_column(&self, expr: &Expr, parts: &[Ident]) -> ControlFlow<String> {
        let [qualifier, column] = parts else {
            return ControlFlow::Break(format!("Tên cột chỉ viết dạng cot hoặc bang.cot: {expr}"));
        };
        if !self.names.qualifiers.contains(&name_key(qualifier)) {
            return ControlFlow::Break(format!("Không có bảng hay bí danh {qualifier}"));
        }
        self.check_column(column)
    }
}

impl Visitor for ExprChecker<'_> {
    type Break = String;

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<String> {
        match expr {
            Expr::Function(function) => self.check_function(function),
            Expr::Identifier(column) => self.check_column(column),
            Expr::CompoundIdentifier(parts) => self.check_qualified_column(expr, parts),
            _ => ControlFlow::Continue(()),
        }
    }
}

// Bước 3: giới hạn số dòng.

/// Hạn chế: chỉ giới hạn số dòng trả về, không giới hạn thời gian chạy hay bộ nhớ của câu.
fn apply_row_limit(query: &mut Query, max_rows: u64) -> Result<(), AppError> {
    // Từ chối thay vì sửa: FETCH FIRST làm cùng việc với LIMIT, LLM đọc lý do rồi viết lại bằng LIMIT.
    if query.fetch.is_some() {
        return Err(rejected("Dùng LIMIT thay cho FETCH FIRST"));
    }
    let max = Expr::value(Value::Number(max_rows.to_string(), false));
    match &mut query.limit_clause {
        None => {
            query.limit_clause = Some(LimitClause::LimitOffset {
                limit: Some(max),
                offset: None,
                limit_by: vec![],
            });
        }
        Some(LimitClause::LimitOffset {
            limit, limit_by, ..
        }) if limit_by.is_empty() => {
            // Biểu thức hay câu con thì không biết trước số dòng nên thay bằng max_rows, kể cả khi nó ra số nhỏ hơn.
            if !limit
                .as_ref()
                .is_some_and(|l| is_number_at_most(l, max_rows))
            {
                *limit = Some(max);
            }
        }
        Some(_) => return Err(rejected("Chỉ hỗ trợ LIMIT n hoặc LIMIT n OFFSET m")),
    }
    Ok(())
}

fn is_number_at_most(expr: &Expr, max: u64) -> bool {
    match expr {
        Expr::Value(v) => {
            matches!(&v.value, Value::Number(n, _) if n.parse::<u64>().is_ok_and(|n| n <= max))
        }
        _ => false,
    }
}

// Bước 4: tạo bản cho DuckDB chạy: bảng logic thành read_parquet(...), bảng tạm (WITH) thành guard_cte_N. Làm sau bước
// 2 vì read_parquet do guard thay vào là đúng loại hàm bước 2 chặn.
//
// Đổi tên bảng tạm vì bước 2 nhận tên bảng tạm ở mọi chỗ, không theo phạm vi: bảng tạm tên "x.csv" gọi ở ngoài chỗ
// định nghĩa sẽ thành đọc file x.csv. Đổi tên rồi thì chỗ gọi đó chỉ là tên không tồn tại, DuckDB báo lỗi.

fn build_runnable(
    mut query: Query,
    tables: &[AllowedTable],
    cte_names: &BTreeSet<String>,
) -> Result<Query, AppError> {
    let mut sources = HashMap::new();
    for table in tables {
        sources.insert(
            table.name.to_lowercase(),
            parquet_source(&table.parquet_dir)?,
        );
    }
    let renames = cte_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), format!("guard_cte_{}", i + 1)))
        .collect();
    // Visit và VisitMut cùng có hàm visit; gọi rõ VisitMut vì Resolver sửa cây.
    let _ = VisitMut::visit(&mut query, &mut Resolver { sources, renames });
    Ok(query)
}

/// Đọc lại chuỗi của analytics::parquet_glob thành cây, để cách đọc Parquet (union_by_name) chỉ viết ở một chỗ.
fn parquet_source(dir: &Path) -> Result<TableFactor, AppError> {
    let sql = format!("SELECT * FROM {}", parquet_glob(dir)?);
    let statement = Parser::parse_sql(&DuckDbDialect {}, &sql)
        .ok()
        .and_then(|mut statements| statements.pop());
    if let Some(Statement::Query(query)) = statement {
        if let SetExpr::Select(select) = *query.body {
            if let Some(from) = select.from.into_iter().next() {
                return Ok(from.relation);
            }
        }
    }
    Err(AppError::Internal(format!(
        "cannot build parquet source from {sql}"
    )))
}

struct Resolver {
    /// Tên bảng logic viết thường, và phần FROM đọc Parquet thay vào.
    sources: HashMap<String, TableFactor>,
    /// Tên bảng tạm viết thường, và tên mới.
    renames: HashMap<String, String>,
}

impl VisitorMut for Resolver {
    type Break = Infallible;

    fn pre_visit_query(&mut self, query: &mut Query) -> ControlFlow<Infallible> {
        for cte in query.with.iter_mut().flat_map(|with| &mut with.cte_tables) {
            if let Some(new_name) = self.renames.get(&name_key(&cte.alias.name)) {
                cte.alias.name = Ident::new(new_name);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &mut TableFactor) -> ControlFlow<Infallible> {
        let TableFactor::Table { name, alias, .. } = factor else {
            return ControlFlow::Continue(());
        };
        let Some(ident) = single_ident(name).cloned() else {
            return ControlFlow::Continue(());
        };
        let key = name_key(&ident);
        // Tên cũ thành bí danh khi câu chưa đặt, để `sales.amount` hay `t.region` vẫn trỏ đúng bảng.
        let kept_alias = alias.clone().unwrap_or(TableAlias {
            explicit: true,
            name: ident,
            columns: vec![],
            at: None,
        });
        if let Some(source) = self.sources.get(&key) {
            let mut source = source.clone();
            if let TableFactor::Table { alias, .. } = &mut source {
                *alias = Some(kept_alias);
            }
            *factor = source;
        } else if let Some(new_name) = self.renames.get(&key) {
            *name = ObjectName::from(vec![Ident::new(new_name)]);
            *alias = Some(kept_alias);
        }
        ControlFlow::Continue(())
    }
}

// Hàm dùng chung cho các bước.

/// Tên kèm schema (`main.sales`, `information_schema.tables`) trỏ được tới bảng hệ thống, và DuckDB hiểu
/// `FROM 'x.csv'` là đọc file, nên hai dạng này trả None như tên không có.
fn single_ident(name: &ObjectName) -> Option<&Ident> {
    match name.0.as_slice() {
        [part] => part
            .as_ident()
            .filter(|ident| ident.quote_style != Some('\'')),
        _ => None,
    }
}

/// Không phân biệt hoa thường, kể cả trong nháy kép, vì DuckDB so tên như vậy.
fn name_key(ident: &Ident) -> String {
    ident.value.to_lowercase()
}

fn find_table<'a>(tables: &'a [AllowedTable], key: &str) -> Option<&'a AllowedTable> {
    tables.iter().find(|t| t.name.to_lowercase() == key)
}

fn rejected(reason: impl Into<String>) -> AppError {
    AppError::BadRequest(reason.into())
}

#[cfg(test)]
mod tests;
