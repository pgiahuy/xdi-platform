// SPDX-License-Identifier: Apache-2.0
/* service/eval.rs - bộ đánh giá câu hỏi mẫu (#37): nạp dữ liệu mẫu vào workspace tạm (service/eval_prepare.rs), hỏi
từng câu của file ca, so kết quả với SQL đúng viết tay (câu đúng cũng đi qua guard và chạy trên DuckDB như câu mô hình
sinh). Lệnh `xdi-backend eval` ở cli.rs gọi hàm run_eval.

So bằng tập dòng kết quả, không so chữ SQL: nhiều câu SQL khác nhau cùng trả lời đúng một câu hỏi. Một ca có thể có
nhiều expected_sql (câu mơ hồ kiểu "doanh thu trước hay sau giảm giá"), khớp một là đúng.

File ca theo định dạng của #202 (eval/README.md) chạy được một phần: mỗi cách hỏi trong variants là một lượt chấm
riêng, ca ghi tables thì hỏi trên dataset có bảng logic đó, ca must_refuse đúng khi hệ thống từ chối. Ca nhóm multi
(cần JOIN nhiều dataset, chờ #213) và câu tiếp nối (earlier, runner chưa gửi câu hỏi trước) bị bỏ qua và ghi vào
skipped. Các trường khác (kind, required_columns, check_sql...) runner chưa đọc. */

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::auth::Member;
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::error::AppError;
use crate::service::ask::contract::{AskResponse, AskStatus, CellValue};
use crate::service::ask::result::build_result_table;
use crate::service::ask::{ask, AskHistory};
use crate::service::eval_prepare::{prepare_datasets, Datasets};
use crate::service::workspaces;
use crate::state::AppState;

/// Một câu đánh giá: câu hỏi, các câu SQL đúng viết tay, và có so thứ tự dòng không (top N, chuỗi thời gian thì có).
#[derive(Debug, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub question: String,
    pub expected_sql: Vec<String>,
    #[serde(default)]
    pub ordered: bool,
    /// Các cách hỏi khác của cùng câu, chấm với cùng đáp án.
    #[serde(default)]
    pub variants: Vec<String>,
    /// Bảng logic câu hỏi đọc; bảng đầu là dataset để hỏi. Rỗng khi chỉ nạp một dataset (DataCo).
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(default)]
    pub group: CaseGroup,
    /// Dữ liệu không có câu trả lời: hệ thống từ chối (rejected) là đúng.
    #[serde(default)]
    pub must_refuse: bool,
    /// Các câu hỏi trước của cuộc trò chuyện, cũ trước.
    #[serde(default)]
    pub earlier: Vec<String>,
}

/// single: hỏi trên một bảng. multi: cần JOIN nhiều bảng chụp từ database (#213).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseGroup {
    #[default]
    Single,
    Multi,
}

/// Một lượt hỏi: câu gốc hoặc một cách hỏi trong variants. id của cách hỏi thứ n là "<id ca>/v<n>".
pub struct Phrasing<'a> {
    pub id: String,
    pub question: &'a str,
    pub case: &'a EvalCase,
}

#[derive(Debug, Serialize)]
pub struct CaseOutcome {
    pub id: String,
    pub question: String,
    pub ok: bool,
    /// Lý do khi sai: trạng thái không phải answered, SQL sinh ra, hoặc chỗ kết quả lệch.
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub total: usize,
    pub passed: usize,
    pub accuracy: f64,
    pub cases: Vec<CaseOutcome>,
    /// Ca không chạy, kèm lý do. Không tính vào total; vắng trong JSON khi rỗng để kết quả DataCo giữ nguyên dạng.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// Đọc file ca đánh giá. Lỗi đọc hay JSON hỏng là lỗi hạ tầng (mã thoát 2 ở cli), không phải câu trả lời sai.
pub fn load_cases(path: &Path) -> Result<Vec<EvalCase>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Các lượt hỏi chạy được (câu gốc và mọi cách hỏi) và danh sách ca bỏ qua kèm lý do.
pub fn build_phrasings(cases: &[EvalCase]) -> (Vec<Phrasing<'_>>, Vec<String>) {
    let mut runnable = Vec::new();
    let mut skipped = Vec::new();
    for case in cases {
        if let Some(reason) = skip_reason(case) {
            skipped.push(format!("{}: {reason}", case.id));
            continue;
        }
        runnable.push(Phrasing {
            id: case.id.clone(),
            question: &case.question,
            case,
        });
        for (index, variant) in case.variants.iter().enumerate() {
            runnable.push(Phrasing {
                id: format!("{}/v{}", case.id, index + 1),
                question: variant,
                case,
            });
        }
    }
    (runnable, skipped)
}

fn skip_reason(case: &EvalCase) -> Option<&'static str> {
    if case.group == CaseGroup::Multi {
        return Some("cần JOIN nhiều dataset, chờ #213");
    }
    if !case.earlier.is_empty() {
        return Some("câu tiếp nối, runner chưa gửi câu hỏi trước");
    }
    None
}

/// Chạy cả bộ: tạo workspace tạm, nạp dữ liệu, chờ hồ sơ ngữ nghĩa, hỏi từng lượt rồi xóa workspace tạm. pause là
/// khoảng nghỉ giữa hai lượt, để không vượt hạn mức số lần gọi mỗi phút của khóa mô hình.
pub async fn run_eval(
    state: &AppState,
    data: &Path,
    cases: &[EvalCase],
    pause: Duration,
) -> Result<EvalReport, AppError> {
    let (phrasings, skipped) = build_phrasings(cases);
    let tables: BTreeSet<String> = phrasings
        .iter()
        .filter_map(|p| p.case.tables.first().cloned())
        .collect();
    let (member, datasets) = prepare_datasets(state, data, &tables).await?;
    let mut outcomes = Vec::new();
    for (index, phrasing) in phrasings.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(pause).await;
        }
        outcomes.push(run_case(state, &member, &datasets, phrasing).await);
    }
    let _ = workspaces::delete(state, &member).await;
    let passed = outcomes.iter().filter(|c| c.ok).count();
    Ok(EvalReport {
        total: outcomes.len(),
        passed,
        accuracy: passed as f64 / outcomes.len().max(1) as f64,
        cases: outcomes,
        skipped,
    })
}

async fn run_case(
    state: &AppState,
    member: &Member,
    datasets: &Datasets,
    phrasing: &Phrasing<'_>,
) -> CaseOutcome {
    let result = run_case_inner(state, member, datasets, phrasing).await;
    CaseOutcome {
        id: phrasing.id.clone(),
        question: phrasing.question.to_string(),
        ok: result.is_ok(),
        detail: result.unwrap_or_else(|why| why),
    }
}

async fn run_case_inner(
    state: &AppState,
    member: &Member,
    datasets: &Datasets,
    phrasing: &Phrasing<'_>,
) -> Result<String, String> {
    let case = phrasing.case;
    let dataset_id = pick_dataset(datasets, case)?;
    // Câu phải từ chối không có SQL đúng; câu thường thì chạy SQL đúng trước, lỗi ở đó là lỗi của file ca.
    let expected = if case.must_refuse {
        Vec::new()
    } else {
        expected_rows(state, member, dataset_id, case).await?
    };
    // Mỗi câu của bộ đánh giá là một câu độc lập, không có hội thoại trước.
    let history = AskHistory::default();
    let response = ask(state, member, dataset_id, phrasing.question, &[], &history)
        .await
        .map_err(|e| format!("lỗi hệ thống khi hỏi: {e:?}"))?;
    if case.must_refuse {
        return build_refuse_verdict(&response);
    }
    if response.status != AskStatus::Answered {
        return Err(format!(
            "status {:?} thay vì answered; message: {}",
            response.status,
            response.message.unwrap_or_default()
        ));
    }
    let result = response.result.ok_or("answered nhưng không có result")?;
    build_any_verdict(&result.rows, &expected, case.ordered).map_err(|why| {
        format!(
            "{why}; sql sinh ra: {}",
            response.sql_generated.unwrap_or_default()
        )
    })?;
    Ok("khớp".to_string())
}

/// Khớp một trong các đáp án là đúng. Sai thì báo chỗ lệch so với đáp án đầu, là cách hiểu chính của câu.
pub fn build_any_verdict(
    actual: &[Vec<Option<CellValue>>],
    expected: &[Vec<Vec<Option<CellValue>>>],
    ordered: bool,
) -> Result<(), String> {
    let mut first_error = None;
    for rows in expected {
        match build_eval_verdict(actual, rows, ordered) {
            Ok(()) => return Ok(()),
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "không có đáp án để so".to_string()))
}

/// Dataset để hỏi: dataset có bảng logic là bảng đầu của tables; ca không ghi tables thì dùng dataset duy nhất.
pub fn pick_dataset<'a>(datasets: &'a Datasets, case: &EvalCase) -> Result<&'a str, String> {
    match case.tables.first() {
        Some(table) => datasets
            .get(table)
            .map(String::as_str)
            .ok_or_else(|| format!("không có dataset nào có bảng {table}")),
        None if datasets.len() == 1 => datasets
            .values()
            .next()
            .map(String::as_str)
            .ok_or_else(|| "không có dataset nào".to_string()),
        None => Err("ca không ghi tables mà đã nạp nhiều dataset".to_string()),
    }
}

/// Câu phải từ chối: đúng khi trạng thái là rejected. needs_choice (hỏi lại) hay failed (lỗi) không phải từ chối.
pub fn build_refuse_verdict(response: &AskResponse) -> Result<String, String> {
    if response.status == AskStatus::Rejected {
        return Ok(format!(
            "từ chối đúng: {}",
            response.message.clone().unwrap_or_default()
        ));
    }
    Err(format!(
        "status {:?} thay vì rejected; sql sinh ra: {}",
        response.status,
        response.sql_generated.clone().unwrap_or_default()
    ))
}

/// Chạy các câu SQL đúng viết tay qua guard và DuckDB, giữ kết quả của mọi câu chạy được (mỗi câu là một cách hiểu
/// chấp nhận được). Cả expected lẫn câu mô hình sinh đều qua cùng một guard và cùng một engine nên cách so không thiên
/// vị cách viết SQL.
async fn expected_rows(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> Result<Vec<Vec<Vec<Option<CellValue>>>>, String> {
    let id = parse_uuid(dataset_id)?;
    let dataset = state
        .store
        .get_dataset(&member.workspace, id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .ok_or("không đọc được dataset")?;
    let version = state
        .store
        .get_current_version(&member.workspace, id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .ok_or("dataset chưa có phiên bản")?;
    let columns = state
        .store
        .list_columns(&member.workspace, version.id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .iter()
        .map(|c| c.name.clone())
        .collect();
    let table = AllowedTable {
        name: dataset.logical_table,
        columns,
        parquet_dir: state.config.data_dir.join(
            version
                .parquet_path
                .ok_or("phiên bản không có file parquet")?,
        ),
        row_filters: Vec::new(),
    };
    let mut answers = Vec::new();
    let mut last_error = String::new();
    for sql in &case.expected_sql {
        let guarded =
            match build_guarded_sql(sql, std::slice::from_ref(&table), state.config.ask_max_rows) {
                Ok(g) => g,
                Err(e) => {
                    last_error = format!("expected_sql không qua được guard: {e:?}");
                    continue;
                }
            };
        let runnable = guarded.runnable;
        let timeout = state.config.query_timeout;
        let output = tokio::task::spawn_blocking(move || run_query(&runnable, 100_000, timeout))
            .await
            .map_err(|e| format!("{e:?}"))?;
        match output {
            Ok(o) => answers.push(build_result_table(o, state.config.ask_max_rows).rows),
            Err(e) => last_error = format!("expected_sql chạy lỗi: {e:?}"),
        }
    }
    if answers.is_empty() {
        return Err(format!("không expected_sql nào chạy được: {last_error}"));
    }
    Ok(answers)
}

fn parse_uuid(text: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(text).map_err(|e| e.to_string())
}

/// Một ô kết quả đưa về dạng so được; cả hai phía đã qua build_result_table nên định dạng ô giống nhau.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Null,
    Num(f64),
    Str(String),
    Bool(bool),
}

impl Cell {
    /// Khóa sắp xếp cho chế độ không thứ tự: xếp theo loại rồi theo chữ.
    fn sort_key(&self) -> String {
        match self {
            Cell::Null => "0".to_string(),
            Cell::Bool(b) => format!("1{b}"),
            Cell::Num(n) => format!("2{n:020}"),
            Cell::Str(s) => format!("3{s}"),
        }
    }
}

fn cell_of_answer(cell: &Option<CellValue>) -> Cell {
    match cell {
        None => Cell::Null,
        Some(CellValue::Number(n)) => Cell::Num(*n),
        Some(CellValue::String(s)) => Cell::Str(s.clone()),
        Some(CellValue::Boolean(b)) => Cell::Bool(*b),
    }
}

/// So hai tập dòng kết quả: cùng số cột, cùng số dòng, từng ô khớp. Số so với sai số tương đối 1e-6; không `ordered`
/// thì sắp xếp hai phía trước khi so. Bỏ qua tên cột vì bí danh SQL là chi tiết cách viết, không phải đúng sai.
pub fn build_eval_verdict(
    actual: &[Vec<Option<CellValue>>],
    expected: &[Vec<Option<CellValue>>],
    ordered: bool,
) -> Result<(), String> {
    let mut left: Vec<Vec<Cell>> = actual
        .iter()
        .map(|row| row.iter().map(cell_of_answer).collect())
        .collect();
    let mut right: Vec<Vec<Cell>> = expected
        .iter()
        .map(|row| row.iter().map(cell_of_answer).collect())
        .collect();
    let width = right.first().map(|r| r.len()).unwrap_or(0);
    if left.first().map(|r| r.len()).unwrap_or(0) != width {
        return Err(format!(
            "số cột khác: {} thay vì {width}",
            left.first().map(|r| r.len()).unwrap_or(0)
        ));
    }
    if left.len() != right.len() {
        return Err(format!(
            "số dòng khác: {} thay vì {}",
            left.len(),
            right.len()
        ));
    }
    if !ordered {
        let key = |row: &Vec<Cell>| {
            row.iter()
                .map(|c| c.sort_key())
                .collect::<Vec<_>>()
                .join("\u{1}")
        };
        left.sort_by_key(key);
        right.sort_by_key(key);
    }
    for (r, (a, e)) in left.iter().zip(&right).enumerate() {
        for (c, (a, e)) in a.iter().zip(e).enumerate() {
            if !cells_match(a, e) {
                return Err(format!("dòng {r} cột {c} lệch: {a:?} thay vì {e:?}"));
            }
        }
    }
    Ok(())
}

fn cells_match(a: &Cell, e: &Cell) -> bool {
    match (a, e) {
        (Cell::Num(x), Cell::Num(y)) => (x - y).abs() <= 1e-6 * y.abs().max(1.0),
        _ => a == e,
    }
}
