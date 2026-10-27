// SPDX-License-Identifier: Apache-2.0
/* service/drill/sql.rs - dựng câu SQL truy dòng nguồn của một ô (#34) từ câu đã chạy (queries.sql_guarded): giữ nguyên
chỗ đọc bảng và WHERE của câu, thêm một điều kiện `(<biểu thức nhóm>) = <giá trị ô>` cho mỗi cột khóa của dòng được
bấm, bỏ GROUP BY, ORDER BY, LIMIT, lấy mọi cột. Ra hai câu: một trang dòng (LIMIT, OFFSET) và một câu COUNT(*) đếm
tổng. Cột khóa tìm theo tên cột DuckDB trả cho câu gốc (build_names_sql), vì đó là tên giao diện gửi lại.
service/drill.rs đưa các câu qua guard với quyền của người đang bấm rồi chạy. Test ở drill/tests.rs.

Dòng phân rã của câu "tại sao" dùng cùng cách trên câu của chiều được bấm (DimensionBreakdown.sql, nhóm theo cột chiều),
cộng điều kiện của lát cắt được bấm (ExplanationSlice.filter): câu chiều đọc cả hai lát cắt, mỗi độ đo lọc một lát
bằng CASE WHEN, nên thiếu điều kiện lát cắt thì trang ra dòng của cả hai kỳ.

Đọc câu bằng QueryShape (engine/sql_shape.rs) như thẻ "Đã hiểu là", nên hai chỗ hiểu cột khóa và vế lọc giống nhau.
Câu dạng chưa truy được thì báo rõ lý do (400) thay vì trả dòng có thể sai: phép tập hợp, nhiều bảng hay bảng tạm,
HAVING, hàm cửa sổ, ROLLUP, `*`, khóa nhóm không có trong SELECT.

Vì sao dựng từ sql_guarded chứ không từ sql_generated: đó là câu đã ra con số người dùng thấy, kể cả hộp lọc phân quyền
của người đã hỏi. Giữ hộp đó là đúng nghĩa (dòng nguồn của ô chỉ gồm dòng người hỏi được thấy); guard thêm hộp của
người đang bấm ra ngoài, nên người bấm không bao giờ thấy quá phần của mình.

Độ đo có điều kiện riêng (`SUM(CASE WHEN ...)`, `COUNT(*) FILTER (WHERE ...)`) thì trả mọi dòng của nhóm: tổng các dòng
trả về chỉ bằng giá trị ô với độ đo cộng không điều kiện. Không ORDER BY: DuckDB giữ thứ tự đọc
(preserve_insertion_order mặc định bật), nên hai trang liền nhau không trùng dòng mà không phải sắp xếp cả nhóm. */

use std::collections::HashSet;
use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, GroupByExpr, LimitClause, Offset, OffsetRows, Query, Select, SelectItem,
    SetExpr, Statement, Value, Visit, Visitor,
};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use crate::engine::sql_shape::{build_query_shape, QueryShape};
use crate::error::AppError;
use crate::service::ask::contract::{build_unique_names, CellValue};
use crate::service::drill::contract::DrillKey;

/// Câu đã chạy, đã kiểm là truy được dòng nguồn.
pub struct DrillPlan {
    shape: QueryShape,
    query: Query,
}

/// Hai câu chưa qua guard, in từ cây.
#[derive(Debug)]
pub struct DrillSql {
    /// Một trang dòng nguồn, mọi cột.
    pub page: String,
    /// Một dòng một cột: số dòng nguồn của ô.
    pub count: String,
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
    Ok(DrillPlan { shape, query })
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

    /// names: tên cột DuckDB trả cho câu gốc (chạy build_names_sql), theo thứ tự. slice_filter: điều kiện lát cắt
    /// của dòng phân rã, do code dựng và lưu cùng câu hỏi (không bao giờ từ request). Err là BadRequest khi khóa gửi
    /// lên không khớp các cột nhóm của câu.
    pub fn build_sql(
        &self,
        names: &[String],
        keys: &[DrillKey],
        slice_filter: Option<&str>,
        offset: u64,
        limit: u32,
    ) -> Result<DrillSql, AppError> {
        if names.len() != self.shape.outputs.len() {
            return Err(AppError::Internal(format!(
                "drill: {} result columns, {} in the select list",
                names.len(),
                self.shape.outputs.len()
            )));
        }
        let mut conditions = build_key_conditions(&self.shape, names, keys)?;
        if let Some(filter) = slice_filter {
            // Bọc ngoặc như điều kiện khóa: lát cắt là `a >= x AND a < y`, hay `"cột" = 'giá trị'`.
            conditions.push(Expr::Nested(Box::new(parse_expr(filter)?)));
        }
        let SetExpr::Select(select) = self.query.body.as_ref() else {
            return Err(unsupported("câu có dạng lồng ngoặc chưa đọc được"));
        };
        let mut page = build_source_query(&self.query, select, conditions.clone(), "SELECT *")?;
        page.limit_clause = Some(LimitClause::LimitOffset {
            limit: Some(Expr::value(Value::Number(limit.to_string(), false))),
            offset: Some(Offset {
                value: Expr::value(Value::Number(offset.to_string(), false)),
                rows: OffsetRows::None,
            }),
            limit_by: Vec::new(),
        });
        let count = build_source_query(&self.query, select, conditions, "SELECT count(*)")?;
        Ok(DrillSql {
            page: page.to_string(),
            count: count.to_string(),
        })
    }
}

/// Những gì QueryShape đã đọc được: một câu SELECT trên một bảng, có độ đo, không HAVING.
fn check_shape(shape: &QueryShape) -> Result<(), AppError> {
    if shape.set_operation {
        return Err(unsupported(
            "câu ghép nhiều câu SELECT (UNION, EXCEPT, INTERSECT)",
        ));
    }
    if shape.table.is_none() {
        return Err(unsupported(
            "câu đọc nhiều bảng, bảng tạm (WITH) hay câu con ở FROM",
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
    let windowed = select.qualify.is_some()
        || select
            .projection
            .iter()
            .any(|item| item.visit(&mut WindowFinder).is_break());
    if windowed {
        return Err(unsupported("câu dùng hàm cửa sổ (OVER)"));
    }
    Ok(())
}

struct WindowFinder;

impl Visitor for WindowFinder {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        match expr {
            Expr::Function(function) if function.over.is_some() => ControlFlow::Break(()),
            _ => ControlFlow::Continue(()),
        }
    }
}

/// Một điều kiện cho mỗi cột khóa. Khóa phải đủ đúng các cột khóa của câu: thiếu một cột thì trang ra dòng của nhiều
/// ô, tổng không còn khớp giá trị ô.
fn build_key_conditions(
    shape: &QueryShape,
    names: &[String],
    keys: &[DrillKey],
) -> Result<Vec<Expr>, AppError> {
    // Tên như ResultTable.columns: cùng hàm làm tên duy nhất trên tên DuckDB trả. Câu không có `*` (check_shape) nên
    // cột thứ i của kết quả là mục thứ i của SELECT.
    let names = build_unique_names(names);
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

/// Câu đọc dòng nguồn: giữ FROM (cả bí danh và hộp lọc) và WHERE của câu gốc, thêm điều kiện khóa, cột lấy từ
/// projection_sql. Mọi phần khác của SELECT về rỗng.
fn build_source_query(
    original: &Query,
    select: &Select,
    conditions: Vec<Expr>,
    projection_sql: &str,
) -> Result<Query, AppError> {
    let mut query = original.clone();
    let mut source = select.clone();
    source.projection = build_projection(projection_sql)?;
    source.distinct = None;
    source.group_by = GroupByExpr::Expressions(Vec::new(), Vec::new());
    source.having = None;
    source.named_window = Vec::new();
    source.sort_by = Vec::new();
    source.selection = build_where(select.selection.clone(), conditions);
    query.body = Box::new(SetExpr::Select(Box::new(source)));
    query.order_by = None;
    query.limit_clause = None;
    Ok(query)
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
fn build_projection(template: &str) -> Result<Vec<SelectItem>, AppError> {
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
