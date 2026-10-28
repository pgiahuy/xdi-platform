// SPDX-License-Identifier: Apache-2.0
/* service/metrics/check.rs - kiểm một chỉ số trước khi lưu: công thức là một hàm tổng hợp trên cột có thật, điều kiện
không phải hàm tổng hợp, cột thời gian có thật, rồi SELECT <công thức> FROM <bảng> [WHERE <điều kiện>] qua được guard
và chạy được trên phiên bản hiện hành, như câu SQL mô hình sẽ viết từ chỉ số này. Hai người gọi: màn Chỉ số
(service/metrics/mod.rs, lỗi trả về người dùng theo từng trường) và bước nạp dataset mẫu (service/samples.rs, chỉ số
mẫu không chạy được thì bỏ và ghi log).

Lời báo viết tiếng Việt cho admin; lý do gốc của guard hay DuckDB (tiếng Anh, có thể chứa đường dẫn file Parquet) chỉ
ghi log. Kiểm cột và hàm tổng hợp bằng cây cú pháp trước khi gọi guard, để lỗi hay gặp nhất (gõ sai tên cột, quên
SUM) có lời báo nói đúng chỗ sai thay cho câu chung "bộ kiểm SQL không cho chạy". */

use std::fmt;
use std::ops::ControlFlow;

use sqlparser::ast::{visit_expressions, Expr};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use crate::auth::WorkspaceId;
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::sql_shape::is_aggregate;
use crate::engine::store::{Dataset, DatasetVersion, NewMetric};
use crate::error::AppError;
use crate::service::ask::run::run_query_in_turn;
use crate::state::AppState;

/// Một chỗ sai của chỉ số: tên trường JSON của request (name, expression, filter, time_column...) và lời báo tiếng
/// Việt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricProblem {
    pub field: &'static str,
    pub message: String,
}

impl MetricProblem {
    pub fn new(field: &'static str, message: impl Into<String>) -> MetricProblem {
        MetricProblem {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for MetricProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl From<MetricProblem> for AppError {
    fn from(problem: MetricProblem) -> AppError {
        AppError::InvalidField {
            field: problem.field.to_string(),
            message: problem.message,
        }
    }
}

/// Bảng để chạy thử chỉ số: mọi cột của phiên bản, file Parquet của phiên bản, không lọc dòng (người kiểm là admin
/// hay bước nạp mẫu, như guard không lọc dòng với admin).
pub async fn build_check_table(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    version: &DatasetVersion,
) -> Result<AllowedTable, AppError> {
    let relative = version
        .parquet_path
        .as_ref()
        .ok_or_else(|| AppError::Internal("dataset version has no parquet path".to_string()))?;
    let columns = state
        .store
        .list_columns(ws, version.id)
        .await?
        .into_iter()
        .map(|c| c.name)
        .collect();
    Ok(AllowedTable {
        name: dataset.logical_table.clone(),
        columns,
        parquet_dir: state.config.data_dir.join(relative),
        row_filters: Vec::new(),
    })
}

/// Kiểm cột thời gian, công thức, điều kiện, rồi chạy thử. Trả lỗi đầu tiên gặp; Ok khi chỉ số dùng được.
pub async fn check_metric(
    state: &AppState,
    table: &AllowedTable,
    metric: &NewMetric,
) -> Result<(), MetricProblem> {
    if let Some(column) = &metric.time_column {
        if find_column(table, column).is_none() {
            return Err(MetricProblem::new(
                "time_column",
                format!("Cột thời gian \"{column}\" không có trong dataset"),
            ));
        }
    }
    let expression = parse_part(&metric.expression, "expression", "Công thức")?;
    check_columns(&expression, table, "expression")?;
    if !is_aggregate(&expression) {
        return Err(MetricProblem::new(
            "expression",
            "Công thức phải gộp nhiều dòng thành một số, ví dụ SUM(\"cột\"), AVG(\"cột\"), COUNT(*) hay \
             COUNT(DISTINCT \"cột\")",
        ));
    }
    if let Some(filter) = &metric.filter {
        let parsed = parse_part(filter, "filter", "Điều kiện")?;
        check_columns(&parsed, table, "filter")?;
        if is_aggregate(&parsed) {
            return Err(MetricProblem::new(
                "filter",
                "Điều kiện lọc từng dòng, không dùng hàm gộp như SUM hay COUNT",
            ));
        }
    }

    // Chạy cả công thức và điều kiện. Lỗi mà có điều kiện thì chạy lại riêng công thức để biết trường nào sai.
    let full = build_check_sql(table, &metric.expression, metric.filter.as_deref());
    let Err(failure) = run_check(state, table, &full).await else {
        return Ok(());
    };
    let filter_is_wrong = match &metric.filter {
        Some(_) => {
            let alone = build_check_sql(table, &metric.expression, None);
            run_check(state, table, &alone).await.is_ok()
        }
        None => false,
    };
    let (field, part) = if filter_is_wrong {
        ("filter", "Điều kiện")
    } else {
        ("expression", "Công thức")
    };
    tracing::info!(metric = %metric.name, field, reason = %failure, "metric check failed");
    Err(MetricProblem::new(
        field,
        format!("{part} {}", failure.text()),
    ))
}

/// Vì sao câu chạy thử không qua. Lý do gốc chỉ để ghi log.
enum CheckFailure {
    Guard(String),
    Run(String),
}

impl CheckFailure {
    /// Phần sau chữ "Công thức" hay "Điều kiện" trong lời báo.
    fn text(&self) -> &'static str {
        match self {
            CheckFailure::Guard(_) => {
                "dùng hàm hay cú pháp mà bộ kiểm SQL không cho chạy; chỉ dùng hàm tính toán thường trên cột của dataset"
            }
            CheckFailure::Run(_) => {
                "không chạy được trên dữ liệu hiện hành, ví dụ cộng một cột chữ hay so một cột số với chữ"
            }
        }
    }
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckFailure::Guard(reason) => write!(f, "guard: {reason}"),
            CheckFailure::Run(reason) => write!(f, "query: {reason}"),
        }
    }
}

fn build_check_sql(table: &AllowedTable, expression: &str, filter: Option<&str>) -> String {
    let mut sql = format!("SELECT {expression} AS value FROM {}", table.name);
    if let Some(filter) = filter {
        sql.push_str(&format!(" WHERE {filter}"));
    }
    sql
}

async fn run_check(state: &AppState, table: &AllowedTable, sql: &str) -> Result<(), CheckFailure> {
    let guarded = build_guarded_sql(sql, std::slice::from_ref(table), 1)
        .map_err(|e| CheckFailure::Guard(format!("{e:?}")))?;
    run_query_in_turn(
        &state.query_jobs,
        guarded.runnable,
        state.config.query_timeout,
    )
    .await
    .map(|_| ())
    .map_err(|e| CheckFailure::Run(format!("{e:?}")))
}

/// Đọc một biểu thức SQL. Chữ thừa sau biểu thức (ví dụ "SUM(a) FROM b") là lỗi: ghép vào câu chạy thử thì guard đọc
/// ra một câu khác hẳn định nghĩa.
fn parse_part(text: &str, field: &'static str, label: &str) -> Result<Expr, MetricProblem> {
    if text.trim().is_empty() {
        return Err(MetricProblem::new(
            field,
            format!("{label} không được để trống"),
        ));
    }
    let unreadable = || {
        MetricProblem::new(
            field,
            format!(
                "{label} không đọc được như một biểu thức SQL; tên cột đặt trong ngoặc kép, ví dụ SUM(\"Sales\")"
            ),
        )
    };
    let mut parser = Parser::new(&DuckDbDialect {})
        .try_with_sql(text)
        .map_err(|_| unreadable())?;
    let expr = parser.parse_expr().map_err(|_| unreadable())?;
    if parser.peek_token().token != Token::EOF {
        return Err(unreadable());
    }
    Ok(expr)
}

/// Mọi tên cột trong biểu thức phải là cột của bảng. So không phân biệt hoa thường, như guard.
fn check_columns(
    expr: &Expr,
    table: &AllowedTable,
    field: &'static str,
) -> Result<(), MetricProblem> {
    let mut unknown: Option<String> = None;
    // visit_expressions đi qua mọi nút biểu thức, kể cả đối số của hàm; Break dừng ở tên lạ đầu tiên. Kết quả của nó
    // chỉ nói có dừng sớm hay không, điều unknown đã giữ, nên bỏ qua.
    let _ = visit_expressions(expr, |e| {
        let name = match e {
            Expr::Identifier(ident) => Some(&ident.value),
            Expr::CompoundIdentifier(parts) => parts.last().map(|p| &p.value),
            _ => None,
        };
        match name {
            Some(name) if find_column(table, name).is_none() => {
                unknown = Some(name.clone());
                ControlFlow::Break(())
            }
            _ => ControlFlow::Continue(()),
        }
    });
    match unknown {
        Some(name) => Err(MetricProblem::new(
            field,
            format!(
                "Cột \"{name}\" không có trong dataset; tên cột viết đúng như trong bảng, đặt trong ngoặc kép"
            ),
        )),
        None => Ok(()),
    }
}

fn find_column<'a>(table: &'a AllowedTable, name: &str) -> Option<&'a String> {
    table.columns.iter().find(|c| c.eq_ignore_ascii_case(name))
}
