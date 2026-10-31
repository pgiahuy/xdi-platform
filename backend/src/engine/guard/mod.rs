// SPDX-License-Identifier: Apache-2.0
/* engine/guard/mod.rs - kiểm SQL do LLM sinh trước khi chạy, theo năm bước trong build_guarded_sql: chỉ nhận một câu
SELECT chỉ đọc; kiểm tên bảng, hàm, cột (luật kích thước của vài hàm ở guard/sizes.rs), số tầng câu lồng nhau
(guard/depth.rs), số mảnh ghép chuỗi (guard/pieces.rs), độ dài chuỗi hằng và số cột (guard/width.rs), nối bảng đúng
khóa ngoại (guard/joins.rs); lọc dòng theo
vai dữ liệu của người hỏi; thêm LIMIT; đổi bảng logic sang file Parquet. Test ở guard/tests.rs.

Kiểm trên cây cú pháp vì dò chữ "DROP" thì lọt khi viết hoa thường lẫn lộn hay chen chú thích. Hạn chế: cây do
sqlparser dựng, không phải DuckDB, nên cú pháp hiếm có thể bị hai bên hiểu khác nhau.

Lý do từ chối được gửi lại cho LLM tự sửa, nên phải chỉ đúng chỗ sai. Luồng hỏi (service/ask/run.rs) gửi mọi lý do
từ chối, trừ lỗi máy chủ (GuardError::Internal), cho mô hình sửa một lần; loại GuardError cho biết lỗi thuộc nhóm nào.
Các luật mô hình tự đáp ứng được cũng được viết vào prompt (guard/prompt_rules.rs), để mô hình ít bị từ chối ngay từ
lần đầu. Guard nhận hồ sơ bảng và chính sách lọc dòng từ service thay vì tự đọc Postgres, để test không cần cơ sở dữ
liệu. */

use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use sqlparser::ast::{
    AccessExpr, BinaryOperator, Expr, Function, Ident, LimitClause, ObjectName, Query, Select,
    SelectItem, SetExpr, Statement, TableAlias, TableFactor, Value, Visit, VisitMut, Visitor,
    VisitorMut,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::analytics::parquet_glob;
use crate::error::AppError;

pub use joins::JoinLink;
pub use prompt_rules::build_prompt_rules;

/// Bảng logic mà SQL của LLM được phép đọc, ví dụ "sales". Cũng là bảng cha trong RowFilter::Parent: khi đó mô hình
/// không đọc được nó, guard chỉ dùng nó trong điều kiện lọc của bảng con.
#[derive(Debug, Clone)]
pub struct AllowedTable {
    /// Cột datasets.logical_table.
    pub name: String,
    /// Lấy từ bảng columns.
    pub columns: Vec<String>,
    pub parquet_dir: PathBuf,
    /// Lấy từ rls_policies theo data_role của người hỏi, cộng điều kiện lan theo khóa ngoại. Rỗng thì không lọc:
    /// service quyết định ai được thấy hết.
    pub row_filters: Vec<RowFilter>,
}

/// Một điều kiện lọc dòng. Mọi điều kiện của một bảng nối bằng AND.
#[derive(Debug, Clone)]
pub enum RowFilter {
    /// Một dòng của rls_policies: chỉ thấy dòng có `column` nằm trong `allowed_values`. Dòng có `column` rỗng (NULL)
    /// cũng bị ẩn, vì NULL không khớp IN.
    Column {
        column: String,
        allowed_values: Vec<String>,
    },
    /// Điều kiện lan theo khóa ngoại (#215), do service dựng từ dataset_relations, không do mô hình viết: chỉ thấy dòng
    /// mà khóa ngoại `columns` trỏ tới một dòng của `parent` người hỏi được thấy. Thành
    /// `("columns") IN (SELECT "parent_columns" FROM parent WHERE <điều kiện của parent>)`. Luật NULL khi bảng có
    /// nhiều đường lan ở build_parent_conditions.
    Parent {
        /// Cột khóa ngoại của bảng được lọc, cùng thứ tự với parent_columns.
        columns: Vec<String>,
        /// Cột khóa chính hay UNIQUE của bảng cha mà khóa ngoại trỏ tới.
        parent_columns: Vec<String>,
        /// Bảng cha kèm điều kiện lọc của chính nó với người hỏi, có thể lại gồm RowFilter::Parent (lan nhiều bước).
        parent: AllowedTable,
    },
}

/// Hai bản của cùng một câu, vì bản chạy chứa đường dẫn file trên máy chủ, không được lộ ra cho người dùng.
#[derive(Debug)]
pub struct GuardedSql {
    /// Hiện cho người dùng và lưu vào queries.sql_guarded.
    pub shown: String,
    /// Chỉ đưa cho DuckDB.
    pub runnable: String,
    /// Guard đã đổi LIMIT thế nào, chữ cho người dùng ("Thêm LIMIT 100000"); None khi giữ LIMIT của câu.
    pub limit_note: Option<String>,
    /// Điều kiện lọc dòng guard đã thêm, in đúng như trong `shown`, để thẻ "Đã hiểu là" (#35) gắn nhãn điều kiện nào do
    /// phân quyền chứ không do câu hỏi. Mỗi điều kiện một lần dù bảng được đọc ở nhiều chỗ; rỗng khi không lọc.
    pub row_filters: Vec<String>,
    /// Tên các bảng câu thật sự đọc (AllowedTable::name), theo thứ tự gặp, mỗi bảng một lần. Luồng hỏi (#213) ghi phiên
    /// bản của đúng các bảng này vào lịch sử, không phải mọi bảng được phép.
    pub tables_read: Vec<String>,
}

/// Vì sao guard không nhận câu. Lý do là chữ tiếng Việt chỉ đúng chỗ sai, đọc được cả với mô hình lẫn người dùng.
#[derive(Debug)]
pub enum GuardError {
    /// Câu dùng bảng, bí danh hay cột không có trong hồ sơ. Mô hình hay viết sai tên kiểu này và sửa được khi đọc lý do
    /// (lý do sai bảng kèm danh sách bảng dùng được), nên service cho vào vòng sửa.
    UnknownName(String),
    /// Câu có thể dựng giá trị quá lớn: hàm có tham số kích thước vượt luật của guard/sizes.rs, lồng quá số tầng của
    /// guard/depth.rs, ghép quá số mảnh của guard/pieces.rs, chuỗi hằng quá dài hay quá nhiều cột (guard/width.rs).
    /// Mô hình viết lại được câu gọn hơn cho cùng câu hỏi, nên service cho vào vòng sửa.
    Oversized(String),
    /// Câu bị chặn vì lý do khác: không đọc được, không phải một câu SELECT chỉ đọc, hàm ở chỗ bảng hay hàm không cho
    /// dùng, quá dài. Luồng hỏi vẫn gửi lý do cho mô hình sửa một lần: câu viết lại theo đúng luật có thể qua.
    Rejected(String),
    /// Lỗi của máy chủ khi dựng bản chạy, không phải lỗi của câu.
    Internal(AppError),
}

/// Cả hai bản in lại từ cây đã kiểm, không dùng câu gốc của LLM: phần guard bỏ qua khi đọc (chú thích) không tới được
/// DuckDB.
///
/// links là các quan hệ khóa ngoại giữa các bảng (rỗng với dataset nạp từ file): mọi phép nối bảng phải đi đúng một
/// quan hệ trong đó (guard/joins.rs).
pub fn build_guarded_sql(
    sql: &str,
    tables: &[AllowedTable],
    links: &[JoinLink],
    max_rows: u64,
) -> Result<GuardedSql, GuardError> {
    let mut query = parse_single_select(sql)?;
    let cte_names = check_names(&query, tables)?;
    depth::check_query_depth(&query).map_err(GuardError::Oversized)?;
    pieces::check_concat_pieces(&query).map_err(GuardError::Oversized)?;
    width::check_query_width(&query, tables).map_err(GuardError::Oversized)?;
    // Trước bước lọc dòng: hộp lọc guard thêm có nửa nối theo khóa ngoại do code dựng, không cần kiểm lại.
    joins::check_joins(&query, tables, links)?;
    // Sau check_names để mọi chỗ đọc dữ liệu đã chắc là bảng trong danh sách; trước build_runnable vì sau đó `sales s`
    // thành `read_parquet(...) AS s`, mất tên bảng. `shown` chứa cả điều kiện để người hỏi thấy phạm vi dữ liệu của mình.
    // Sau các luật kích thước: hộp lọc thêm một tầng câu nhưng chỉ là SELECT * kèm WHERE, không ghép thêm giá trị, nên
    // không đếm vào số tầng; đếm thì câu 4 tầng hợp lệ của người bị lọc thành 5 tầng và bị từ chối.
    let (row_filters, tables_read) =
        apply_row_filters(&mut query, tables, &cte_names).map_err(GuardError::Internal)?;
    let limit_note = apply_row_limit(&mut query, max_rows)?;
    let shown = query.to_string();
    let runnable = build_runnable(query, tables, &cte_names)
        .map_err(GuardError::Internal)?
        .to_string();
    Ok(GuardedSql {
        shown,
        runnable,
        limit_note,
        row_filters,
        tables_read,
    })
}

// Bước 1: chỉ nhận một câu SELECT chỉ đọc.

/// Độ dài tối đa của câu SQL nhận vào, tính bằng byte. Chuỗi UNION rất dài (khoảng 20.000 vế, 700 KB) đọc được thành
/// cây, nhưng duyệt và in lại cây thì tràn stack: tiến trình bị hủy, mọi người đang dùng mất kết nối, không chỉ request
/// đó. Câu do mô hình viết thường chỉ vài KB; 32 KB vẫn rộng, và dưới mức đã thử chạy được trên stack 2 MB của luồng.
const MAX_SQL_BYTES: usize = 32 * 1024;

fn parse_single_select(sql: &str) -> Result<Query, GuardError> {
    if sql.len() > MAX_SQL_BYTES {
        return Err(rejected(format!(
            "Câu SQL quá dài ({} KB), tối đa {} KB",
            sql.len() / 1024,
            MAX_SQL_BYTES / 1024
        )));
    }
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
        if !is_select_body(&query.body) {
            return ControlFlow::Break("Chỉ cho phép câu SELECT để đọc dữ liệu");
        }
        // `SELECT ... FOR UPDATE`, `FOR SHARE` xin khóa dòng để ghi sau: câu hỏi dữ liệu chỉ đọc, không cần khóa. Kiểm ở
        // mọi câu, kể cả câu con.
        if !query.locks.is_empty() {
            return ControlFlow::Break(
                "Không cho FOR UPDATE, FOR SHARE: câu đọc dữ liệu không cần khóa dòng",
            );
        }
        ControlFlow::Continue(())
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
fn check_names(query: &Query, tables: &[AllowedTable]) -> Result<BTreeSet<String>, GuardError> {
    let mut names = Names::default();
    let _ = query.visit(&mut Collector {
        tables,
        names: &mut names,
    });
    let parents = build_parent_tables(tables);
    if let ControlFlow::Break(error) = query.visit(&mut TableChecker {
        tables,
        parents: &parents,
        names: &names,
    }) {
        return Err(error);
    }
    if let ControlFlow::Break(error) = query.visit(&mut ExprChecker { names: &names }) {
        return Err(error);
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
    /// Bảng cha của điều kiện lan theo khóa ngoại (build_parent_tables): mô hình không đọc được, nhưng tên vẫn bị giữ.
    parents: &'a [AllowedTable],
    names: &'a Names,
}

impl TableChecker<'_> {
    /// Kèm danh sách bảng vào lý do từ chối, để LLM sửa đúng tên ở vòng sau.
    fn table_list(&self) -> String {
        let names: Vec<&str> = self.tables.iter().map(|t| t.name.as_str()).collect();
        names.join(", ")
    }

    fn check_table(&self, name: &ObjectName) -> ControlFlow<GuardError> {
        if let Some(ident) = single_ident(name) {
            let key = name_key(ident);
            if find_table(self.tables, &key).is_some() || self.names.ctes.contains(&key) {
                return ControlFlow::Continue(());
            }
        }
        ControlFlow::Break(unknown_name(format!(
            "Không có bảng {name}. Các bảng dùng được: {}",
            self.table_list()
        )))
    }
}

impl Visitor for TableChecker<'_> {
    type Break = GuardError;

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<GuardError> {
        let Some(with) = &query.with else {
            return ControlFlow::Continue(());
        };
        // Câu đệ quy có thể chạy không dừng, và câu hỏi phân tích không cần nó.
        if with.recursive {
            return ControlFlow::Break(rejected("Không cho WITH RECURSIVE"));
        }
        for cte in &with.cte_tables {
            // Nhận thì build_runnable đổi mọi `FROM sales` sang file Parquet: câu đọc bảng dữ liệu thay vì bảng tạm,
            // ra kết quả sai mà không báo lỗi. Tên bảng cha cũng vậy, và còn tệ hơn: bảng tạm `customers` của câu thành
            // đọc thẳng file của customers, không qua hộp lọc nào.
            let key = name_key(&cte.alias.name);
            if find_table(self.tables, &key).is_some() || find_table(self.parents, &key).is_some() {
                return ControlFlow::Break(rejected(format!(
                    "Bảng tạm {} trùng tên bảng dữ liệu, hãy đặt tên khác",
                    cte.alias.name
                )));
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<GuardError> {
        match factor {
            // Chặn mọi hàm sinh bảng thay vì lọc từng hàm: read_csv, glob đọc được file ngoài dữ liệu đã nạp; range
            // vô hại nhưng câu hỏi dữ liệu không cần.
            TableFactor::Table {
                name,
                args: Some(_),
                ..
            } => ControlFlow::Break(rejected(format!(
                "Không cho gọi hàm {name} để lấy dữ liệu. Chỉ đọc từ các bảng: {}",
                self.table_list()
            ))),
            TableFactor::Table { name, .. } => self.check_table(name),
            // Phần bên trong câu con và JOIN trong ngoặc được Visitor duyệt và kiểm tiếp.
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                ControlFlow::Continue(())
            }
            // UNNEST, PIVOT và các dạng khác: chưa cần; muốn nhận thì phải xét dạng đó có đọc được nguồn ngoài không.
            other => ControlFlow::Break(rejected(format!(
                "Không hỗ trợ cách lấy dữ liệu này: {other}"
            ))),
        }
    }
}

struct ExprChecker<'a> {
    names: &'a Names,
}

impl ExprChecker<'_> {
    /// Tên nhiều phần mà phần cuối là hàm được phép (`x.lpad(3, '0')`, `main.sum(a)`): DuckDB đọc dạng này là gọi hàm
    /// trên x như phương thức, x thành tham số đầu, nên các luật đếm tham số ở sizes.rs và pieces.rs đếm thiếu. Từ chối
    /// loại vào vòng sửa, lý do chỉ cách viết lại dạng hàm thường.
    fn check_function(&self, function: &Function) -> ControlFlow<GuardError> {
        let last = function.name.0.last().and_then(|part| part.as_ident());
        let allowed = |ident: &Ident| ALLOWED_FUNCTIONS.contains(&name_key(ident).as_str());
        let Some(name) = single_ident(&function.name).filter(|ident| allowed(ident)) else {
            if function.name.0.len() > 1 && last.is_some_and(allowed) {
                return ControlFlow::Break(method_call(&function.name.to_string()));
            }
            return ControlFlow::Break(rejected(format!("Không cho dùng hàm {}", function.name)));
        };
        let name = name_key(name);
        match sizes::check_function_size(&name, function) {
            Ok(()) => ControlFlow::Continue(()),
            Err(reason) => ControlFlow::Break(GuardError::Oversized(reason)),
        }
    }

    fn check_column(&self, column: &Ident) -> ControlFlow<GuardError> {
        if self.names.columns.contains(&name_key(column)) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(unknown_name(format!(
                "Không có cột {column} trong các bảng đang dùng"
            )))
        }
    }

    /// Hạn chế: trường của cột kiểu struct (`cot.truong`) cũng bị từ chối, vì guard hiểu `cot` là tên bảng.
    fn check_qualified_column(&self, expr: &Expr, parts: &[Ident]) -> ControlFlow<GuardError> {
        let [qualifier, column] = parts else {
            return ControlFlow::Break(rejected(format!(
                "Tên cột chỉ viết dạng cot hoặc bang.cot: {expr}"
            )));
        };
        if !self.names.qualifiers.contains(&name_key(qualifier)) {
            return ControlFlow::Break(unknown_name(format!(
                "Không có bảng hay bí danh {qualifier}"
            )));
        }
        self.check_column(column)
    }
}

impl Visitor for ExprChecker<'_> {
    type Break = GuardError;

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<GuardError> {
        match expr {
            Expr::Function(function) => self.check_function(function),
            // `(a).concat(b)`, `'ab'.concat(...)`: như tên nhiều phần ở check_function, biểu thức trước dấu chấm là
            // tham số đầu mà các luật đếm không thấy.
            Expr::CompoundFieldAccess { access_chain, .. }
                if access_chain
                    .iter()
                    .any(|access| matches!(access, AccessExpr::Dot(Expr::Function(_)))) =>
            {
                ControlFlow::Break(method_call(&expr.to_string()))
            }
            Expr::Identifier(column) => self.check_column(column),
            Expr::CompoundIdentifier(parts) => self.check_qualified_column(expr, parts),
            _ => ControlFlow::Continue(()),
        }
    }
}

// Bước 3: lọc dòng theo vai dữ liệu (data_role) của người hỏi.
//
// Bọc chỗ đọc bảng thành câu con: `sales s` thành `(SELECT * FROM sales WHERE "region" IN ('South')) s`. Điều kiện lan
// theo khóa ngoại (RowFilter::Parent, #215) nằm trong cùng hộp đó, dạng `"customer_id" IN (SELECT "id" FROM
// "customers" WHERE ...)`, nên câu chỉ đọc orders cũng chỉ thấy đơn của khách được thấy. Không thêm
// điều kiện vào WHERE của câu LLM viết vì điều kiện trong câu con chạy trước mọi JOIN, WHERE, OR của câu ngoài: viết
// khéo cỡ nào cũng không gỡ được nó. Chỉ cần bọc chỗ đọc bảng vì bước 2 đã từ chối mọi nguồn khác: hàm đọc file, bảng
// hệ thống, tên kèm schema, bảng tạm trùng tên bảng dữ liệu.
//
// Bước này vẫn không tin hẳn bước 2: gặp chỗ đọc dữ liệu nào không nhận ra thì chặn cả câu thay vì bỏ qua. Bỏ qua thì
// một dạng bước 2 sót, nay hay sau này, thành chỗ đọc dữ liệu không qua lọc.

/// Trả các điều kiện đã thêm (GuardedSql::row_filters) và các bảng đã đọc (GuardedSql::tables_read). Bước này đi qua
/// mọi chỗ đọc bảng dữ liệu nên cũng là chỗ gom tên bảng chắc nhất.
fn apply_row_filters(
    query: &mut Query,
    tables: &[AllowedTable],
    cte_names: &BTreeSet<String>,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let mut injector = RowFilterInjector {
        tables,
        cte_names,
        added: Vec::new(),
        read: Vec::new(),
    };
    if let ControlFlow::Break(error) = VisitMut::visit(query, &mut injector) {
        return Err(error);
    }
    Ok((injector.added, injector.read))
}

struct RowFilterInjector<'a> {
    tables: &'a [AllowedTable],
    cte_names: &'a BTreeSet<String>,
    /// Điều kiện đã thêm, in như trong `shown`, theo thứ tự gặp và không lặp.
    added: Vec<String>,
    /// Tên bảng dữ liệu đã gặp, theo thứ tự gặp và không lặp.
    read: Vec<String>,
}

impl RowFilterInjector<'_> {
    fn note_added(&mut self, conditions: &[Expr]) {
        for condition in conditions {
            let text = condition.to_string();
            if !self.added.contains(&text) {
                self.added.push(text);
            }
        }
    }
}

impl VisitorMut for RowFilterInjector<'_> {
    type Break = AppError;

    // Bọc ở post_visit vì Visitor đi vào bên trong một nút sau pre_visit: bọc ở pre_visit thì nó đi tiếp vào câu con
    // vừa tạo, gặp lại `sales` và bọc mãi không dừng.
    fn post_visit_table_factor(&mut self, factor: &mut TableFactor) -> ControlFlow<AppError> {
        let TableFactor::Table {
            name, alias, args, ..
        } = factor
        else {
            return match factor {
                // Phần bên trong câu con và JOIN trong ngoặc đã được duyệt và bọc trước khi tới đây.
                TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                    ControlFlow::Continue(())
                }
                other => ControlFlow::Break(unfilterable_read(other)),
            };
        };
        let ident = match (args, single_ident(name)) {
            (None, Some(ident)) => ident.clone(),
            _ => return ControlFlow::Break(unfilterable_read(name)),
        };
        let key = name_key(&ident);
        let Some(table) = find_table(self.tables, &key) else {
            // Bảng tạm (WITH) không phải bảng dữ liệu nên không bọc; bảng dữ liệu bên trong nó được bọc ở chỗ đọc.
            if self.cte_names.contains(&key) {
                return ControlFlow::Continue(());
            }
            return ControlFlow::Break(unfilterable_read(&ident));
        };
        if !self.read.contains(&table.name) {
            self.read.push(table.name.clone());
        }
        let conditions = match build_row_conditions(table) {
            Ok(conditions) => conditions,
            Err(error) => return ControlFlow::Break(error),
        };
        // Bảng không có chính sách thì giữ nguyên chỗ đọc: shown và runnable như khi chưa có bước này.
        let Some(condition) = build_all_of(&conditions) else {
            return ControlFlow::Continue(());
        };
        self.note_added(&conditions);
        // Bí danh chuyển ra ngoài câu con để `s.region` hay `sales.region` của câu vẫn trỏ đúng bảng.
        let outer_alias = alias.take().unwrap_or(TableAlias {
            explicit: true,
            name: ident,
            columns: vec![],
            at: None,
        });
        match build_filtered_read(factor.clone(), Some(condition)) {
            Ok(subquery) => {
                *factor = TableFactor::Derived {
                    lateral: false,
                    subquery: Box::new(subquery),
                    alias: Some(outer_alias),
                    sample: None,
                };
                ControlFlow::Continue(())
            }
            // Không dựng được câu con thì dừng cả câu, không để câu chạy mà thiếu điều kiện lọc.
            Err(error) => ControlFlow::Break(error),
        }
    }
}

/// Chỗ đọc dữ liệu bước 3 không biết lọc thế nào. Bước 2 phải đã từ chối nó, nên gặp ở đây là lỗi của guard.
fn unfilterable_read(read: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("row filter cannot filter the data source {read}"))
}

/// Mỗi chính sách một điều kiện; rỗng khi bảng không có chính sách. Cột phải có trong hồ sơ bảng: cột không có thì
/// DuckDB lấy cột cùng tên của bảng khác ở câu ngoài (câu con trong FROM được nối như LATERAL), điều kiện thành lọc
/// bảng khác và mọi dòng của bảng này lọt ra. Cùng lý do, cột của bảng cha trong câu con của RowFilter::Parent cũng
/// phải có trong hồ sơ bảng cha.
fn build_row_conditions(table: &AllowedTable) -> Result<Vec<Expr>, AppError> {
    let mut conditions = Vec::new();
    for filter in &table.row_filters {
        if let RowFilter::Column {
            column,
            allowed_values,
        } = filter
        {
            check_filter_columns(table, std::slice::from_ref(column))?;
            conditions.push(build_column_condition(column, allowed_values));
        }
    }
    conditions.extend(build_parent_conditions(table)?);
    Ok(conditions)
}

/// Điều kiện của các RowFilter::Parent (các đường lan) của một bảng.
///
/// Một đường: `fk IN (SELECT ...)`. Khóa ngoại rỗng thì dòng bị ẩn: dòng không nối với dòng nào được thấy của bảng cha.
///
/// Nhiều đường (order_items.order_id qua orders, order_items.gift_customer_id tới customers): mỗi đường thành
/// `(fk IS NULL OR fk IN (SELECT ...))`, nối AND, cộng một điều kiện "ít nhất một khóa ngoại không rỗng". Lý do: khóa
/// ngoại tùy chọn (gift_customer_id) thường rỗng; ẩn dòng vì nó rỗng thì người bị lọc không thấy dòng hàng nào, kể cả
/// dòng hàng của đơn trong vùng mình. Khóa rỗng nghĩa là đường đó không nối tới đâu, nên không cần qua đường đó; còn
/// dòng mà mọi đường đều rỗng thì không nối với dòng nào được thấy, nên vẫn ẩn. Khóa nhiều cột chỉ tính là rỗng khi mọi
/// cột đều rỗng; rỗng một phần thì không khớp IN (luật chuẩn) và dòng bị ẩn.
///
/// Một đường cũng là ca riêng của luật trên: `(fk IS NULL OR fk IN ...) AND fk IS NOT NULL` bằng `fk IN ...`, nên viết
/// gọn cho dễ đọc.
fn build_parent_conditions(table: &AllowedTable) -> Result<Vec<Expr>, AppError> {
    let mut paths = Vec::new();
    for filter in &table.row_filters {
        let RowFilter::Parent {
            columns,
            parent_columns,
            parent,
        } = filter
        else {
            continue;
        };
        if columns.is_empty() || columns.len() != parent_columns.len() {
            return Err(AppError::Internal(format!(
                "row filter of table {} has {} key columns for {} columns of {}",
                table.name,
                columns.len(),
                parent_columns.len(),
                parent.name
            )));
        }
        check_filter_columns(table, columns)?;
        check_filter_columns(parent, parent_columns)?;
        let condition = build_parent_condition(columns, parent_columns, parent)?;
        paths.push((columns, condition));
    }
    if paths.len() < 2 {
        return Ok(paths.into_iter().map(|(_, condition)| condition).collect());
    }
    let mut conditions = Vec::new();
    let mut key_columns: Vec<&String> = Vec::new();
    for (columns, condition) in paths {
        let all_null = build_all_of(
            &columns
                .iter()
                .map(|c| is_null(c, false))
                .collect::<Vec<_>>(),
        );
        if let Some(all_null) = all_null {
            // Bọc ngoặc: các điều kiện được nối bằng AND, mà AND tính trước OR.
            conditions.push(Expr::Nested(Box::new(or(nested(all_null), condition))));
        }
        for column in columns {
            if !key_columns.contains(&column) {
                key_columns.push(column);
            }
        }
    }
    let linked = key_columns
        .iter()
        .map(|c| is_null(c, true))
        .reduce(or)
        .map(|any| Expr::Nested(Box::new(any)));
    conditions.extend(linked);
    Ok(conditions)
}

/// `"cot" IS NULL`, hay `"cot" IS NOT NULL` khi negated.
fn is_null(column: &str, negated: bool) -> Expr {
    let column = Box::new(quoted_column(column));
    if negated {
        Expr::IsNotNull(column)
    } else {
        Expr::IsNull(column)
    }
}

fn or(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: BinaryOperator::Or,
        right: Box::new(right),
    }
}

/// Bọc ngoặc khi là phép AND của nhiều vế (khóa nhiều cột), để đứng cạnh OR không đổi nghĩa.
fn nested(expr: Expr) -> Expr {
    match expr {
        Expr::BinaryOp { .. } => Expr::Nested(Box::new(expr)),
        other => other,
    }
}

fn check_filter_columns(table: &AllowedTable, names: &[String]) -> Result<(), AppError> {
    for name in names {
        let key = name.to_lowercase();
        if !table
            .columns
            .iter()
            .any(|column| column.to_lowercase() == key)
        {
            return Err(AppError::Internal(format!(
                "row filter column {name} is not a column of table {}",
                table.name
            )));
        }
    }
    Ok(())
}

/// None khi không có điều kiện. Nhiều chính sách trên một bảng (mỗi cột một dòng) thì dòng phải qua tất cả.
fn build_all_of(conditions: &[Expr]) -> Option<Expr> {
    conditions
        .iter()
        .cloned()
        .reduce(|left, right| Expr::BinaryOp {
            left: Box::new(left),
            op: BinaryOperator::And,
            right: Box::new(right),
        })
}

fn build_column_condition(column: &str, allowed_values: &[String]) -> Expr {
    // Danh sách rỗng nghĩa là không được thấy dòng nào; `IN ()` là lỗi cú pháp nên viết thẳng FALSE.
    if allowed_values.is_empty() {
        return Expr::value(Value::Boolean(false));
    }
    let list = allowed_values
        .iter()
        .map(|value| Expr::value(Value::SingleQuotedString(value.clone())))
        .collect();
    Expr::InList {
        expr: Box::new(quoted_column(column)),
        list,
        negated: false,
    }
}

/// Nháy kép vì tên cột lấy từ file nạp vào, có thể có dấu cách hay chữ hoa như "Order Item Total".
fn quoted_column(column: &str) -> Expr {
    Expr::Identifier(Ident::with_quote('"', column))
}

/// `"customer_id" IN (SELECT "id" FROM "customers" WHERE "region" IN ('South'))`; khóa nhiều cột thì
/// `("maker", "sku") IN (SELECT "maker", "sku" FROM "products" WHERE ...)`.
///
/// Nửa nối (IN câu con) chứ không JOIN bảng cha vào hộp lọc: JOIN nhân dòng khi khóa bên cha lặp (cột UNIQUE có thể
/// có nhiều NULL, hay hồ sơ ghi khóa sai), và thêm cột của bảng cha vào `SELECT *` của hộp, làm câu ngoài thấy cột lạ.
/// IN chỉ giữ hay bỏ từng dòng của bảng con, không đổi số dòng hay cột. IN nhiều cột của DuckDB theo luật chuẩn: một ô
/// khóa ngoại rỗng thì không khớp dòng nào (build_parent_conditions quyết định khi nào khóa rỗng vẫn cho qua). Không
/// dùng so cả bộ dạng `(a, b)` với câu con trả một cột kiểu STRUCT: DuckDB coi NULL bằng NULL khi so STRUCT, dòng có ô
/// rỗng sẽ khớp dòng cha có ô rỗng.
///
/// Điều kiện của bảng cha nằm trong câu con, dựng bằng chính build_row_conditions, nên lan được nhiều bước
/// (order_items qua orders tới customers). Tên cột trong câu con không cần tên bảng đi trước: check_filter_columns đã
/// chắc chúng là cột của bảng cha, nên DuckDB gắn chúng với bảng gần nhất là bảng cha. Bước 5 đổi tên bảng cha sang
/// file Parquet như mọi bảng khác (build_parent_tables).
fn build_parent_condition(
    columns: &[String],
    parent_columns: &[String],
    parent: &AllowedTable,
) -> Result<Expr, AppError> {
    let parent_conditions = build_row_conditions(parent)?;
    let mut subquery = build_filtered_read(
        build_table_read(&parent.name)?,
        build_all_of(&parent_conditions),
    )?;
    if let SetExpr::Select(select) = subquery.body.as_mut() {
        select.projection = parent_columns
            .iter()
            .map(|column| SelectItem::UnnamedExpr(quoted_column(column)))
            .collect();
    }
    let keys: Vec<Expr> = columns.iter().map(|column| quoted_column(column)).collect();
    let left = match <[Expr; 1]>::try_from(keys) {
        Ok([key]) => key,
        Err(keys) => Expr::Tuple(keys),
    };
    Ok(Expr::InSubquery {
        expr: Box::new(left),
        subquery: Box::new(subquery),
        negated: false,
    })
}

/// Chỗ đọc `"name"` dạng cây, lấy từ khung `SELECT * FROM t`. Nháy kép để tên trùng từ khóa SQL vẫn đọc lại được
/// (truy dòng nguồn đọc lại bản shown).
fn build_table_read(name: &str) -> Result<TableFactor, AppError> {
    if let SetExpr::Select(mut select) = *parse_select_template()?.body {
        if let Some(from) = select.from.pop() {
            let mut read = from.relation;
            if let TableFactor::Table { name: table, .. } = &mut read {
                *table = ObjectName::from(vec![Ident::with_quote('"', name)]);
                return Ok(read);
            }
        }
    }
    Err(AppError::Internal(format!(
        "cannot build a table read for {name}"
    )))
}

/// Lấy khung `SELECT * FROM t` rồi gắn bảng và điều kiện vào cây, không ghép chuỗi SQL: giá trị như "O'Brien" nằm
/// trong cây dạng chuỗi, sqlparser tự thoát dấu nháy khi in.
/// condition None thì không có WHERE (bảng cha không bị lọc gì, chỉ cần khóa có thật).
fn build_filtered_read(table: TableFactor, condition: Option<Expr>) -> Result<Query, AppError> {
    let mut query = parse_select_template()?;
    if let SetExpr::Select(select) = query.body.as_mut() {
        if let Some(from) = select.from.first_mut() {
            from.relation = table;
            select.selection = condition;
            return Ok(query);
        }
    }
    Err(AppError::Internal(
        "cannot build row filter from the select template".to_string(),
    ))
}

fn parse_select_template() -> Result<Query, AppError> {
    let template = "SELECT * FROM t";
    let statement = Parser::parse_sql(&DuckDbDialect {}, template)
        .ok()
        .and_then(|mut statements| statements.pop());
    match statement {
        Some(Statement::Query(query)) => Ok(*query),
        _ => Err(AppError::Internal(format!("cannot parse {template}"))),
    }
}

// Bước 4: giới hạn số dòng.

/// Chỉ giới hạn số dòng trả về; thời gian chạy và bộ nhớ của câu giới hạn lúc chạy, ở engine/query.rs. Trả ghi chú
/// cho GuardedSql::limit_note.
fn apply_row_limit(query: &mut Query, max_rows: u64) -> Result<Option<String>, GuardError> {
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
            Ok(Some(format!("Thêm LIMIT {max_rows}")))
        }
        Some(LimitClause::LimitOffset {
            limit, limit_by, ..
        }) if limit_by.is_empty() => match limit {
            // Chỉ có OFFSET.
            None => {
                *limit = Some(max);
                Ok(Some(format!("Thêm LIMIT {max_rows}")))
            }
            Some(l) if is_number_at_most(l, max_rows) => Ok(None),
            // Biểu thức hay câu con thì không biết trước số dòng nên thay bằng max_rows, kể cả khi nó ra số nhỏ hơn.
            Some(_) => {
                *limit = Some(max);
                Ok(Some(format!("Hạ LIMIT xuống {max_rows}")))
            }
        },
        Some(_) => Err(rejected("Chỉ hỗ trợ LIMIT n hoặc LIMIT n OFFSET m")),
    }
}

/// DuckDB nhận số viết có gạch dưới (`1_000`); không bỏ gạch thì `LIMIT 1_000` không đọc được và bị nâng thành max.
fn is_number_at_most(expr: &Expr, max: u64) -> bool {
    match expr {
        Expr::Value(v) => matches!(
            &v.value,
            Value::Number(n, _) if n.replace('_', "").parse::<u64>().is_ok_and(|n| n <= max)
        ),
        _ => false,
    }
}

// Bước 5: tạo bản cho DuckDB chạy: bảng logic thành read_parquet(...), bảng tạm (WITH) thành guard_cte_N. Làm sau bước
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
    // Bảng cha chỉ xuất hiện trong câu con của điều kiện lan theo khóa ngoại (bước 3), nhưng cũng phải đọc từ Parquet.
    let parents = build_parent_tables(tables);
    for table in tables.iter().chain(&parents) {
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

/// Mọi bảng cha trong RowFilter::Parent của các bảng, kể cả cha của cha, mỗi tên một lần, trừ tên đã có trong tables.
/// Bước 2 giữ tên của chúng (không cho đặt bảng tạm trùng tên), bước 5 đổi chúng sang file Parquet. Truy dòng nguồn
/// (service/drill/mod.rs) thêm chúng vào danh sách bảng, vì câu đã lưu có hộp lọc nhắc tới chúng.
pub fn build_parent_tables(tables: &[AllowedTable]) -> Vec<AllowedTable> {
    let mut found: Vec<AllowedTable> = Vec::new();
    let mut pending: Vec<&AllowedTable> = tables.iter().collect();
    while let Some(table) = pending.pop() {
        for filter in &table.row_filters {
            let RowFilter::Parent { parent, .. } = filter else {
                continue;
            };
            let key = parent.name.to_lowercase();
            if find_table(tables, &key).is_none() && find_table(&found, &key).is_none() {
                found.push(parent.clone());
            }
            pending.push(parent);
        }
    }
    found
}

fn find_table<'a>(tables: &'a [AllowedTable], key: &str) -> Option<&'a AllowedTable> {
    tables.iter().find(|t| t.name.to_lowercase() == key)
}

fn rejected(reason: impl Into<String>) -> GuardError {
    GuardError::Rejected(reason.into())
}

/// written có thể là cả một biểu thức dài; lời báo chỉ chép phần đầu.
fn method_call(written: &str) -> GuardError {
    let mut shown: String = written.chars().take(80).collect();
    if shown.len() < written.len() {
        shown.push_str("...");
    }
    GuardError::Oversized(format!(
        "Không gọi hàm dạng phương thức ({shown}): viết dạng hàm thường, ví dụ concat(a, b) thay cho a.concat(b)"
    ))
}

fn unknown_name(reason: String) -> GuardError {
    GuardError::UnknownName(reason)
}

mod depth;
mod join_conditions;
mod join_keys;
mod join_nested;
mod join_sources;
#[cfg(test)]
mod join_tests;
mod joins;
#[cfg(test)]
mod parent_filter_tests;
mod pieces;
mod prompt_rules;
#[cfg(test)]
mod row_filter_tests;
#[cfg(test)]
mod sample_tests;
mod sizes;
#[cfg(test)]
mod tests;
mod width;
