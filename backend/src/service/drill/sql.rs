// SPDX-License-Identifier: Apache-2.0
/* service/drill/sql.rs - dựng câu SQL truy dòng nguồn của một ô (#34) từ câu đã chạy (queries.sql_guarded): giữ nguyên
FROM (kể cả JOIN theo khóa ngoại và hộp lọc) và WHERE của câu, thêm một điều kiện `(<biểu thức nhóm>) = <giá trị ô>`
cho mỗi cột khóa của dòng được bấm, bỏ GROUP BY, ORDER BY, LIMIT. Ra hai câu: một trang dòng (LIMIT, OFFSET) và một câu
đếm tổng kèm các độ đo tính lại trên mọi dòng (drill/measures.rs). Cột khóa tìm theo tên cột DuckDB trả cho câu gốc
(build_names_sql), vì đó là tên giao diện gửi lại. service/drill/mod.rs đưa các câu qua guard với quyền của người
đang bấm rồi chạy. Test ở drill/tests.rs và drill/join_tests.rs.

Cột của trang: câu đọc một chỗ thì `*` như trước; câu nối bảng thì `<bảng sự kiện>.*` (drill/sources.rs chọn bảng),
rồi cột khóa và cột lọc thuộc bảng nối khác, đặt tên theo cột kết quả hay "bảng.cột". Sau cùng là cột từng dòng của
mỗi độ đo SUM trên một biểu thức (tên theo cột kết quả); SUM trên một cột trang đã có thì trỏ tới cột đó.

Dòng phân rã của câu "tại sao" dùng cùng cách trên câu của chiều được bấm (DimensionBreakdown.sql, nhóm theo cột chiều),
cộng điều kiện của lát cắt được bấm (ExplanationSlice.filter): câu chiều đọc cả hai lát cắt, mỗi độ đo lọc một lát
bằng CASE WHEN, nên thiếu điều kiện lát cắt thì trang ra dòng của cả hai kỳ. Phân rã theo cột của bảng liên quan
(#227) đọc một câu con `LEFT JOIN` ở FROM; câu con đó giữ dòng của bảng chính nên truy được như một bảng.

Đọc câu bằng QueryShape (engine/sql_shape.rs) như thẻ "Cách tính", nên hai chỗ hiểu cột khóa và vế lọc giống nhau.
Câu dạng chưa truy được thì báo rõ lý do (400) thay vì trả dòng có thể sai: phép tập hợp, HAVING, hàm cửa sổ, ROLLUP,
`*`, khóa nhóm không có trong SELECT, câu con hay bảng tạm gộp nhóm (drill/sources.rs).

Vì sao dựng từ sql_guarded chứ không từ sql_generated: đó là câu đã ra con số người dùng thấy, kể cả hộp lọc phân quyền
của người đã hỏi. Giữ hộp đó là đúng nghĩa (dòng nguồn của ô chỉ gồm dòng người hỏi được thấy); guard thêm hộp của
người đang bấm ra ngoài, nên người bấm không bao giờ thấy quá phần của mình.

Trang sắp theo mọi cột (`ORDER BY ALL`): với câu nối bảng (và nửa nối của hộp lọc lan theo khóa ngoại) DuckDB không giữ
thứ tự dòng giữa hai lần chạy, nên LIMIT, OFFSET không thứ tự cho hai trang trùng dòng và sót dòng (đo trên database
mẫu: đọc hết 32 226 dòng của Home Office theo trang 1000 dòng chỉ ra 24 000 tới 27 000 mã dòng khác nhau). Sắp theo
mọi cột thì thứ tự cố định; hai dòng bằng nhau mọi cột đổi chỗ cho nhau cũng không đổi trang. */

use std::collections::HashSet;

use sqlparser::ast::{
    BinaryOperator, Expr, GroupByExpr, LimitClause, Offset, OffsetRows, Query, Select, SelectItem,
    SetExpr, Statement, Value,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::guard::{AllowedTable, JoinLink};
use crate::engine::sql_shape::{build_query_shape, QueryShape};
use crate::error::AppError;
use crate::service::ask::contract::{build_unique_names, CellValue};
use crate::service::drill::columns::{build_page_columns, PageColumns};
use crate::service::drill::contract::DrillKey;
use crate::service::drill::measures::{build_measures, PlanMeasure};
use crate::service::drill::sources::{build_sources, has_window, Source};

/// Câu đã chạy, đã kiểm là truy được dòng nguồn.
pub struct DrillPlan {
    shape: QueryShape,
    query: Query,
    sources: Vec<Source>,
    measures: Vec<PlanMeasure>,
}

/// Những gì build_sql cần ngoài câu đã chạy.
pub struct PageRequest<'a> {
    /// Tên cột DuckDB trả cho câu gốc (chạy build_names_sql), theo thứ tự.
    pub names: &'a [String],
    pub keys: &'a [DrillKey],
    /// Điều kiện lát cắt của dòng phân rã, do code dựng và lưu cùng câu hỏi (không bao giờ từ request).
    pub slice_filter: Option<&'a str>,
    /// Chỉ báo độ đo có tên cột kết quả này (độ đo của lát cắt được bấm); None là mọi độ đo.
    pub only_measure: Option<&'a str>,
    pub offset: u64,
    pub limit: u32,
    /// Hồ sơ bảng và quan hệ, để biết cột thuộc bảng nào khi câu nối bảng.
    pub tables: &'a [AllowedTable],
    pub links: &'a [JoinLink],
}

/// Hai câu chưa qua guard, in từ cây, cùng cách đọc cột của trang.
#[derive(Debug)]
pub struct DrillSql {
    /// Một trang dòng nguồn.
    pub page: String,
    /// Một dòng: count(*) rồi mỗi độ đo trong `columns.measures` một cột.
    pub count: String,
    pub columns: PageColumns,
}

/// Err là BadRequest với lời cho người dùng khi câu chưa truy được; Internal khi sql_guarded không đọc được (câu đó do
/// guard in ra nên luôn đọc được).
pub fn build_drill_plan(sql_guarded: &str) -> Result<DrillPlan, AppError> {
    let shape = build_query_shape(sql_guarded)?;
    check_shape(&shape)?;
    let query = parse_query(sql_guarded)?;
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Err(unsupported("câu có dạng lồng ngoặc chưa đọc được"));
    };
    check_select(select)?;
    let sources = build_sources(&query, select)?;
    let measures = build_measures(&shape);
    Ok(DrillPlan {
        shape,
        query,
        sources,
        measures,
    })
}

impl DrillPlan {
    /// Câu gốc với LIMIT 0: chạy để biết tên cột DuckDB đặt cho kết quả. Không đoán được tên từ cây: `SELECT market`
    /// trên cột "Market" ra cột tên Market, biểu thức không bí danh thì DuckDB tự in chữ của nó.
    pub fn build_names_sql(&self) -> String {
        let mut query = self.query.clone();
        query.limit_clause = Some(LimitClause::LimitOffset {
            limit: Some(Expr::value(Value::Number("0".to_string(), false))),
            offset: None,
            limit_by: Vec::new(),
        });
        query.to_string()
    }

    /// Err là BadRequest khi khóa gửi lên không khớp các cột nhóm của câu.
    pub fn build_sql(&self, request: &PageRequest<'_>) -> Result<DrillSql, AppError> {
        if request.names.len() != self.shape.outputs.len() {
            return Err(AppError::Internal(format!(
                "drill: {} result columns, {} in the select list",
                request.names.len(),
                self.shape.outputs.len()
            )));
        }
        // Tên như ResultTable.columns: cùng hàm làm tên duy nhất trên tên DuckDB trả.
        let names = build_unique_names(request.names);
        let mut conditions = build_key_conditions(&self.shape, &names, request.keys)?;
        if let Some(filter) = request.slice_filter {
            // Bọc ngoặc như điều kiện khóa: lát cắt là `a >= x AND a < y`, hay `"cột" = 'giá trị'`.
            conditions.push(Expr::Nested(Box::new(parse_expr(filter)?)));
        }
        let SetExpr::Select(select) = self.query.body.as_ref() else {
            return Err(unsupported("câu có dạng lồng ngoặc chưa đọc được"));
        };
        let measures: Vec<&PlanMeasure> = self
            .measures
            .iter()
            .filter(|m| {
                request
                    .only_measure
                    .is_none_or(|only| names[m.output] == only)
            })
            .collect();
        let columns = build_page_columns(
            &self.shape,
            select,
            &self.sources,
            &names,
            &measures,
            (request.tables, request.links),
        )?;
        let mut page = build_source_query(
            &self.query,
            select,
            conditions.clone(),
            columns.items.clone(),
        );
        page.order_by = parse_query("SELECT 1 ORDER BY ALL")?.order_by;
        page.limit_clause = Some(LimitClause::LimitOffset {
            limit: Some(Expr::value(Value::Number(request.limit.to_string(), false))),
            offset: Some(Offset {
                value: Expr::value(Value::Number(request.offset.to_string(), false)),
                rows: OffsetRows::None,
            }),
            limit_by: Vec::new(),
        });
        let mut totals = build_projection("SELECT count(*)")?;
        totals.extend(
            measures
                .iter()
                .map(|m| SelectItem::UnnamedExpr(m.aggregate.clone())),
        );
        let count = build_source_query(&self.query, select, conditions, totals);
        Ok(DrillSql {
            page: page.to_string(),
            count: count.to_string(),
            columns: columns.info,
        })
    }
}

/// Những gì QueryShape đã đọc được: một câu SELECT, có độ đo, không HAVING. Phần FROM do drill/sources.rs kiểm.
fn check_shape(shape: &QueryShape) -> Result<(), AppError> {
    if shape.set_operation {
        return Err(unsupported(
            "câu ghép nhiều câu SELECT (UNION, EXCEPT, INTERSECT)",
        ));
    }
    // Lọc nhóm sau khi tổng hợp: điều kiện của HAVING không áp được lên từng dòng nguồn.
    if !shape.having.is_empty() {
        return Err(unsupported("câu lọc nhóm sau khi tổng hợp (HAVING)"));
    }
    // `*` không cho biết cột kết quả nào ứng với biểu thức nào, nên không tìm được cột khóa theo tên.
    if shape.outputs.iter().any(|o| o.tree.is_none()) {
        return Err(unsupported("câu lấy mọi cột bằng *"));
    }
    if !shape.outputs.iter().any(|o| o.is_aggregate) {
        return Err(unsupported(
            "câu không có độ đo tổng hợp, mỗi dòng kết quả đã là dòng dữ liệu",
        ));
    }
    if let Some(key) = shape.group_by.iter().find(|k| k.output.is_none()) {
        return Err(unsupported(&format!(
            "khóa nhóm {} không có trong cột kết quả",
            key.sql
        )));
    }
    Ok(())
}

/// Những gì QueryShape không giữ: kiểu GROUP BY và hàm cửa sổ.
fn check_select(select: &Select) -> Result<(), AppError> {
    let rollup = match &select.group_by {
        GroupByExpr::Expressions(keys, modifiers) => {
            !modifiers.is_empty()
                || keys
                    .iter()
                    .any(|k| matches!(k, Expr::Rollup(_) | Expr::Cube(_) | Expr::GroupingSets(_)))
        }
        GroupByExpr::All(modifiers) => !modifiers.is_empty(),
    };
    if rollup {
        return Err(unsupported("câu nhóm theo ROLLUP, CUBE hay GROUPING SETS"));
    }
    // Hàm cửa sổ tính trên cả kết quả, không trên từng nhóm, nên ô của nó không ứng với một nhóm dòng nguồn.
    if has_window(select) {
        return Err(unsupported("câu dùng hàm cửa sổ (OVER)"));
    }
    Ok(())
}

/// Một điều kiện cho mỗi cột khóa. Khóa phải đủ đúng các cột khóa của câu: thiếu một cột thì trang ra dòng của nhiều
/// ô, tổng không còn khớp giá trị ô.
fn build_key_conditions(
    shape: &QueryShape,
    names: &[String],
    keys: &[DrillKey],
) -> Result<Vec<Expr>, AppError> {
    // names đã là tên duy nhất như ResultTable.columns. Câu không có `*` (check_shape) nên cột thứ i của kết quả là
    // mục thứ i của SELECT.
    let key_outputs: Vec<usize> = shape.group_by.iter().filter_map(|k| k.output).collect();
    let mut seen = HashSet::new();
    let mut conditions = Vec::new();
    for key in keys {
        let Some(index) = names.iter().position(|name| *name == key.column) else {
            return Err(bad_key(format!("Kết quả không có cột {}", key.column)));
        };
        if !key_outputs.contains(&index) {
            return Err(bad_key(format!(
                "Cột {} không phải cột nhóm của kết quả",
                key.column
            )));
        }
        if !seen.insert(index) {
            return Err(bad_key(format!("Cột {} gửi hai lần", key.column)));
        }
        // check_shape đã bảo đảm mọi cột kết quả có cây.
        let Some(tree) = &shape.outputs[index].tree else {
            return Err(unsupported("câu lấy mọi cột bằng *"));
        };
        conditions.push(build_key_condition(tree, key.value.as_ref()));
    }
    if let Some(missing) = key_outputs.iter().find(|i| !seen.contains(*i)) {
        return Err(bad_key(format!(
            "Thiếu giá trị của cột {} trong dòng được bấm",
            names[*missing]
        )));
    }
    Ok(conditions)
}

// Lấy biểu thức của cột kết quả, không lấy chữ của khóa GROUP BY: khóa có thể là số thứ tự (`GROUP BY 1`) hay bí danh.
// Bọc ngoặc vì biểu thức có thể là phép toán ưu tiên thấp hơn `=` (`a OR b`).
fn build_key_condition(expr: &Expr, value: Option<&CellValue>) -> Expr {
    let expr = Box::new(Expr::Nested(Box::new(expr.clone())));
    let Some(value) = value else {
        return Expr::IsNull(expr);
    };
    // Literal dựng trong cây, sqlparser tự thoát dấu nháy khi in. Ô dạng chuỗi (chữ, ngày, giờ, số thập phân lớn) so
    // dưới dạng chuỗi hằng: DuckDB tự đổi chuỗi hằng sang kiểu của vế kia, nên '2024-01-01' so được với DATE và
    // '12345678901' với BIGINT mà không cần biết kiểu cột.
    let literal = match value {
        CellValue::Number(number) => Value::Number(number.to_string(), false),
        CellValue::String(text) => Value::SingleQuotedString(text.clone()),
        CellValue::Boolean(flag) => Value::Boolean(*flag),
    };
    Expr::BinaryOp {
        left: expr,
        op: BinaryOperator::Eq,
        right: Box::new(Expr::value(literal)),
    }
}

/// Câu đọc dòng nguồn: giữ WITH, FROM (cả JOIN, bí danh và hộp lọc) và WHERE của câu gốc, thêm điều kiện khóa, cột
/// là projection. Mọi phần khác của SELECT về rỗng.
fn build_source_query(
    original: &Query,
    select: &Select,
    conditions: Vec<Expr>,
    projection: Vec<SelectItem>,
) -> Query {
    let mut query = original.clone();
    let mut source = select.clone();
    source.projection = projection;
    source.distinct = None;
    source.group_by = GroupByExpr::Expressions(Vec::new(), Vec::new());
    source.having = None;
    source.named_window = Vec::new();
    source.sort_by = Vec::new();
    source.selection = build_where(select.selection.clone(), conditions);
    query.body = Box::new(SetExpr::Select(Box::new(source)));
    query.order_by = None;
    query.limit_clause = None;
    query
}

fn build_where(original: Option<Expr>, conditions: Vec<Expr>) -> Option<Expr> {
    // WHERE gốc dạng `a OR b` phải bọc ngoặc, không thì `a OR b AND khóa` thành `a OR (b AND khóa)`.
    let original = original.map(|expr| match expr {
        Expr::BinaryOp {
            op: BinaryOperator::Or,
            ..
        } => Expr::Nested(Box::new(expr)),
        other => other,
    });
    original
        .into_iter()
        .chain(conditions)
        .reduce(|left, right| Expr::BinaryOp {
            left: Box::new(left),
            op: BinaryOperator::And,
            right: Box::new(right),
        })
}

/// Danh sách cột lấy từ một câu mẫu viết sẵn, như guard dựng hộp lọc: dựng tay nút gọi hàm count(*) của sqlparser
/// dài và dễ sai hơn.
pub(super) fn build_projection(template: &str) -> Result<Vec<SelectItem>, AppError> {
    let query = parse_query(template)?;
    match *query.body {
        SetExpr::Select(select) => Ok(select.projection),
        _ => Err(AppError::Internal(format!(
            "drill: template {template} is not a select"
        ))),
    }
}

fn parse_query(sql: &str) -> Result<Query, AppError> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql)
        .map_err(|e| AppError::Internal(format!("drill: cannot parse sql: {e}")))?;
    match (statements.pop(), statements.is_empty()) {
        (Some(Statement::Query(query)), true) => Ok(*query),
        _ => Err(AppError::Internal(
            "drill: sql is not a single query".to_string(),
        )),
    }
}

fn parse_expr(sql: &str) -> Result<Expr, AppError> {
    Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_expr())
        .map_err(|e| AppError::Internal(format!("drill: cannot parse slice filter: {e}")))
}

fn unsupported(reason: &str) -> AppError {
    AppError::BadRequest(format!("Câu hỏi này chưa truy được dòng nguồn: {reason}"))
}

fn bad_key(message: String) -> AppError {
    AppError::BadRequest(message)
}
