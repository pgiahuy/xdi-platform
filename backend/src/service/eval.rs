// SPDX-License-Identifier: Apache-2.0
/* service/eval.rs - bộ đánh giá câu hỏi mẫu (#37, #202): nạp dataset của một file bộ câu (định dạng ở
service/eval_cases.rs) vào workspace tạm, hỏi từng câu, so kết quả với SQL đúng viết tay (câu đúng cũng đi qua guard
và chạy trên DuckDB như câu mô hình sinh). Lệnh `xdi-backend eval` ở cli.rs gọi hàm run_eval.

So bằng tập dòng kết quả, không so chữ SQL: nhiều câu SQL khác nhau cùng trả lời đúng một câu hỏi. Một ca có thể có
nhiều expected_sql (câu mơ hồ kiểu "doanh thu trước hay sau giảm giá"), khớp một là đúng. Câu phải từ chối
(must_refuse) đạt khi status là rejected.

Chạy với mock thì chỉ hỏi câu có cờ mock: mock chỉ trả lời câu mẫu viết sẵn, câu khác là lỗi 400 chứ không phải mô
hình trả lời sai. Câu tiếp nối chưa chạy được (cần hỏi qua cuộc trò chuyện), nên được ghi là bỏ qua kèm lý do. */

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::auth::{self, Member};
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::error::AppError;
use crate::service::ask::contract::{AskStatus, CellValue};
use crate::service::ask::result::build_result_table;
use crate::service::ask::{ask, AskHistory};
use crate::service::eval_cases::{CaseFile, CaseKind, EvalCase};
use crate::service::samples::{add_metrics_file, upload_local_file};
use crate::service::workspaces;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CaseOutcome {
    pub id: String,
    pub kind: CaseKind,
    pub question: String,
    pub ok: bool,
    /// Lý do khi sai: trạng thái không như mong đợi, SQL sinh ra, hoặc chỗ kết quả lệch.
    pub detail: String,
}

/// Câu có trong file mà lần chạy này không hỏi.
#[derive(Debug, Serialize)]
pub struct SkippedCase {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    /// Số câu đã hỏi, không tính câu bỏ qua.
    pub total: usize,
    pub passed: usize,
    pub accuracy: f64,
    pub cases: Vec<CaseOutcome>,
    pub skipped: Vec<SkippedCase>,
}

/// Chạy cả bộ: tạo workspace tạm, nạp file dữ liệu của bộ câu, chờ hồ sơ ngữ nghĩa, hỏi từng câu rồi xóa workspace
/// tạm. data đè lên dataset.file của bộ câu khi có (cờ --data).
pub async fn run_eval(
    state: &AppState,
    file: &CaseFile,
    data: Option<&Path>,
) -> Result<EvalReport, AppError> {
    let mock = state.config.llm_provider == "mock";
    let (cases, skipped) = select_cases(&file.cases, mock);
    let data = data.unwrap_or_else(|| Path::new(&file.dataset.file));
    let metrics = file.dataset.metrics.as_deref().map(Path::new);
    let (member, dataset_id) = prepare(state, data, &file.dataset.name, metrics).await?;
    let outcome = run_cases(state, &member, &dataset_id, &cases).await;
    let _ = workspaces::delete(state, &member).await;
    let outcomes = outcome?;
    let passed = outcomes.iter().filter(|c| c.ok).count();
    Ok(EvalReport {
        total: outcomes.len(),
        passed,
        accuracy: passed as f64 / outcomes.len().max(1) as f64,
        cases: outcomes,
        skipped,
    })
}

/// Các câu lần chạy này hỏi, và các câu bỏ qua kèm lý do.
pub fn select_cases(cases: &[EvalCase], mock: bool) -> (Vec<&EvalCase>, Vec<SkippedCase>) {
    let mut run = Vec::new();
    let mut skipped = Vec::new();
    for case in cases {
        let reason = if case.follow_up_of.is_some() {
            Some("câu tiếp nối chưa chạy được: runner chưa hỏi qua cuộc trò chuyện")
        } else if mock && !case.mock {
            Some("mock không có câu trả lời cho câu này")
        } else {
            None
        };
        match reason {
            Some(reason) => skipped.push(SkippedCase {
                id: case.id.clone(),
                reason: reason.to_string(),
            }),
            None => run.push(case),
        }
    }
    (run, skipped)
}

/// Tài khoản và workspace tạm cho lần đánh giá, với dataset mẫu đã nạp và hồ sơ ngữ nghĩa xong (mock thì có ngay).
/// Trả id của dataset để hỏi. Workspace bị xóa sau khi chạy xong (run_eval), kéo theo user rác trong bảng users:
/// chấp nhận được vì eval nhắm vào database thử, không phải bản cài thật.
pub(crate) async fn prepare(
    state: &AppState,
    data: &Path,
    name: &str,
    metrics: Option<&Path>,
) -> Result<(Member, String), AppError> {
    let email = format!("eval-{}@example.com", uuid::Uuid::new_v4());
    let user = state.store.create_user(&email, "Eval", "unused").await?;
    let membership = workspaces::create(state, &user, "Eval").await?;
    let member = auth::member_of(&state.store, user, &membership.slug).await?;

    let result = upload_local_file(state, &member, data, name).await?;
    if let Some(path) = metrics {
        // load_metrics coi file vắng là không có chỉ số; với bộ câu thì file đã ghi tên mà không có là file sai.
        if !path.is_file() {
            return Err(AppError::Internal(format!(
                "metrics file {} not found",
                path.display()
            )));
        }
        add_metrics_file(state, &member, &result, path).await?;
    }

    // Hồ sơ ngữ nghĩa chạy nền; hỏi khi chưa xong thì ngữ cảnh thiếu bí danh và câu gợi ý.
    let dataset_id = result.dataset.id.to_string();
    for _ in 0..120 {
        let dataset = state
            .store
            .get_dataset(&member.workspace, result.dataset.id)
            .await?
            .ok_or_else(|| AppError::Internal("eval dataset vanished".to_string()))?;
        if dataset.current_profile_status.as_deref() == Some("semantic") {
            return Ok((member, dataset_id));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(AppError::Internal(
        "hồ sơ ngữ nghĩa của dataset mẫu chưa xong sau 120 giây".to_string(),
    ))
}

async fn run_cases(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    cases: &[&EvalCase],
) -> Result<Vec<CaseOutcome>, AppError> {
    let mut outcomes = Vec::new();
    for case in cases {
        outcomes.push(run_case(state, member, dataset_id, case).await);
    }
    Ok(outcomes)
}

async fn run_case(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> CaseOutcome {
    match run_case_inner(state, member, dataset_id, case).await {
        Ok(detail) => CaseOutcome {
            id: case.id.clone(),
            kind: case.kind,
            question: case.question.clone(),
            ok: true,
            detail,
        },
        Err(detail) => CaseOutcome {
            id: case.id.clone(),
            kind: case.kind,
            question: case.question.clone(),
            ok: false,
            detail,
        },
    }
}

async fn run_case_inner(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> Result<String, String> {
    // Mỗi câu của bộ đánh giá là một câu độc lập, không có hội thoại trước.
    let history = AskHistory::default();
    let response = ask(state, member, dataset_id, &case.question, &[], &history)
        .await
        .map_err(|e| format!("lỗi hệ thống khi hỏi: {e:?}"))?;
    if case.must_refuse {
        return build_refuse_verdict(response.status, response.sql_generated.as_deref());
    }
    let expected = expected_rows(state, member, dataset_id, case).await?;
    if response.status != AskStatus::Answered {
        return Err(format!(
            "status {:?} thay vì answered; message: {}",
            response.status,
            response.message.unwrap_or_default()
        ));
    }
    let result = response.result.ok_or("answered nhưng không có result")?;
    build_eval_verdict(&result.rows, &expected, case.ordered).map_err(|why| {
        format!(
            "{why}; sql sinh ra: {}",
            response.sql_generated.unwrap_or_default()
        )
    })?;
    Ok("khớp".to_string())
}

/// Câu phải từ chối: đạt khi status là rejected. Trả lời được hay chạy lỗi đều là trượt, vì người hỏi không được
/// báo rằng câu này không làm được.
pub fn build_refuse_verdict(status: AskStatus, sql: Option<&str>) -> Result<String, String> {
    if status == AskStatus::Rejected {
        return Ok("từ chối đúng".to_string());
    }
    Err(format!(
        "status {status:?} thay vì rejected; sql sinh ra: {}",
        sql.unwrap_or_default()
    ))
}

/// Chạy các câu SQL đúng viết tay qua guard và DuckDB, lấy câu đầu chạy được. Cả expected lẫn câu mô hình sinh đều
/// qua cùng một guard và cùng một engine nên cách so không thiên vị cách viết SQL.
async fn expected_rows(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> Result<Vec<Vec<Option<CellValue>>>, String> {
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
            Ok(o) => return Ok(build_result_table(o, state.config.ask_max_rows).rows),
            Err(e) => last_error = format!("expected_sql chạy lỗi: {e:?}"),
        }
    }
    Err(format!("không expected_sql nào chạy được: {last_error}"))
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
