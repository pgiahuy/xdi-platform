// SPDX-License-Identifier: Apache-2.0
/* service/eval_check.rs - kiểm đáp án của bộ câu (#202) mà không hỏi mô hình: chạy expected_sql và check_sql (câu
người duyệt tự viết để tính lại đáp án) của từng câu, so hai kết quả bằng đúng luật chấm ở service/eval_grade.rs.
Lệnh `xdi-backend eval --check-answers` ở cli.rs gọi hàm check_answers.

Người duyệt dùng lệnh này để kiểm đáp án mà không tốn hạn mức mô hình; CI chạy nó trên DataCo trong `make eval`.
Dataset vẫn nạp qua service/eval.rs prepare như khi chấm, để tên bảng và kiểu cột đúng như lúc chấm. */

use std::path::Path;

use serde::Serialize;

use crate::engine::guard::AllowedTable;
use crate::error::AppError;
use crate::service::eval::{build_allowed_table, prepare, run_sql_rows};
use crate::service::eval_cases::{CaseFile, EvalCase};
use crate::service::eval_grade::build_eval_verdict;
use crate::service::workspaces;
use crate::state::AppState;

/// Kết quả kiểm một câu có check_sql.
#[derive(Debug, Serialize)]
pub struct CheckOutcome {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub cases: Vec<CheckOutcome>,
    /// Id các câu có đáp án mà chưa có check_sql: chưa được tính lại độc lập.
    pub unchecked: Vec<String>,
}

/// Nạp dataset của bộ câu vào workspace tạm, kiểm từng câu có check_sql rồi xóa workspace.
pub async fn check_answers(
    state: &AppState,
    file: &CaseFile,
    data: Option<&Path>,
) -> Result<CheckReport, AppError> {
    let data = data.unwrap_or_else(|| Path::new(&file.dataset.file));
    let metrics = file.dataset.metrics.as_deref().map(Path::new);
    let (member, dataset_id) = prepare(state, data, &file.dataset.name, metrics).await?;
    let table = build_allowed_table(state, &member, &dataset_id).await;
    let mut report = CheckReport {
        cases: Vec::new(),
        unchecked: Vec::new(),
    };
    if let Ok(table) = &table {
        for case in file.cases.iter().filter(|c| !c.must_refuse) {
            match &case.check_sql {
                Some(check) => {
                    let (ok, detail) = check_case(state, table, case, check).await;
                    report.cases.push(CheckOutcome {
                        id: case.id.clone(),
                        ok,
                        detail,
                    });
                }
                None => report.unchecked.push(case.id.clone()),
            }
        }
    }
    let _ = workspaces::delete(state, &member).await;
    table.map_err(AppError::Internal)?;
    Ok(report)
}

/// check_sql khớp một expected_sql là đạt, như kết quả của mô hình khớp một đáp án. Mọi expected_sql cũng phải chạy
/// được: câu đáp án hỏng thì chấm không bao giờ khớp nó, mà lỗi chỉ lộ ra khi mô hình viết đúng cách hiểu đó.
async fn check_case(
    state: &AppState,
    table: &AllowedTable,
    case: &EvalCase,
    check: &str,
) -> (bool, String) {
    let check_rows = match run_sql_rows(state, table, check).await {
        Ok(rows) => rows,
        Err(e) => return (false, format!("check_sql: {e}")),
    };
    let mut matched = None;
    let mut first_error = None;
    for (i, sql) in case.expected_sql.iter().enumerate() {
        let rows = match run_sql_rows(state, table, sql).await {
            Ok(rows) => rows,
            Err(e) => return (false, format!("expected_sql[{i}]: {e}")),
        };
        match build_eval_verdict(&check_rows, &rows, case.ordered, &case.required_columns) {
            Ok(_) => {
                matched.get_or_insert(i);
            }
            Err(why) => {
                first_error.get_or_insert(format!("expected_sql[{i}]: {why}"));
            }
        }
    }
    match matched {
        Some(i) => (true, format!("khớp expected_sql[{i}]")),
        None => (false, first_error.unwrap_or_default()),
    }
}
