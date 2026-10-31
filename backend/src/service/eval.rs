// SPDX-License-Identifier: Apache-2.0
/* service/eval.rs - bộ đánh giá câu hỏi mẫu (#37): nạp một dataset mẫu vào workspace tạm, hỏi từng câu của file ca,
so kết quả với SQL đúng viết tay (câu đúng cũng đi qua guard và chạy trên DuckDB như câu mô hình sinh). Lệnh
`xdi-backend eval` ở cli.rs gọi hàm run_eval.

So bằng tập dòng kết quả, không so chữ SQL: nhiều câu SQL khác nhau cùng trả lời đúng một câu hỏi. Một ca có thể có
nhiều expected_sql (câu mơ hồ kiểu "doanh thu trước hay sau giảm giá"), khớp một là đúng.

Đo bài học (#228): có file bài học (`--lessons`) thì cả bộ chạy hai lượt trên cùng workspace tạm, lượt đầu không bài
nào, rồi tạo và bật các bài đó qua đúng đường admin dùng (service/lessons/), lượt sau có bài học. Báo cáo là của lượt
có bài học, kèm tỷ lệ đúng của lượt không bài và các câu đổi kết quả giữa hai lượt. Mock không đọc prompt nên hai lượt
luôn như nhau; số đo có nghĩa khi chạy với mô hình thật. */

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::auth::{self, Member};
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::error::AppError;
use crate::service;
use crate::service::ask::contract::{AskStatus, CellValue};
use crate::service::ask::result::build_result_table;
use crate::service::ask::{ask, AskHistory};
use crate::service::lessons::contract::LessonRequest;
use crate::service::samples::{upload_local_file, SAMPLE_NAME};
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
    /// Chỉ có khi chạy với file bài học; vắng thì eval/result.json giữ đúng hình dạng cũ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lessons: Option<LessonsComparison>,
}

/// Một bài học trong file `--lessons`: chữ của bài, và có áp cho cả workspace không (mặc định chỉ dataset đang đánh giá).
#[derive(Debug, Deserialize)]
pub struct EvalLesson {
    pub text: String,
    #[serde(default)]
    pub workspace: bool,
}

/// So hai lượt: không bài học và có bài học.
#[derive(Debug, Serialize)]
pub struct LessonsComparison {
    pub lessons: usize,
    pub passed_without: usize,
    pub accuracy_without: f64,
    /// Id các câu đúng ở một lượt mà sai ở lượt kia.
    pub changed: Vec<String>,
}

/// Đọc file ca đánh giá. Lỗi đọc hay JSON hỏng là lỗi hạ tầng (mã thoát 2 ở cli), không phải câu trả lời sai.
pub fn load_cases(path: &Path) -> Result<Vec<EvalCase>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Đọc file bài học: mảng JSON [{"text": "...", "workspace": false}]. Lỗi là lỗi hạ tầng như load_cases.
pub fn load_lessons(path: &Path) -> Result<Vec<EvalLesson>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Chạy cả bộ: tạo workspace tạm, nạp file mẫu, chờ hồ sơ ngữ nghĩa, hỏi từng câu rồi xóa workspace tạm. lessons rỗng
/// là một lượt như trước; có bài thì hai lượt (đầu file).
pub async fn run_eval(
    state: &AppState,
    data: &Path,
    cases: &[EvalCase],
    lessons: &[EvalLesson],
) -> Result<EvalReport, AppError> {
    let (member, dataset_id) = prepare(state, data).await?;
    let outcome = run_rounds(state, &member, &dataset_id, cases, lessons).await;
    let _ = workspaces::delete(state, &member).await;
    outcome
}

async fn run_rounds(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    cases: &[EvalCase],
    lessons: &[EvalLesson],
) -> Result<EvalReport, AppError> {
    let without = run_cases(state, member, dataset_id, cases).await?;
    if lessons.is_empty() {
        return Ok(build_report(without, None));
    }
    add_lessons(state, member, dataset_id, lessons).await?;
    let with = run_cases(state, member, dataset_id, cases).await?;
    let changed = with
        .iter()
        .zip(&without)
        .filter(|(a, b)| a.ok != b.ok)
        .map(|(a, _)| a.id.clone())
        .collect();
    let passed_without = without.iter().filter(|c| c.ok).count();
    let comparison = LessonsComparison {
        lessons: lessons.len(),
        passed_without,
        accuracy_without: build_accuracy(passed_without, without.len()),
        changed,
    };
    Ok(build_report(with, Some(comparison)))
}

fn build_report(cases: Vec<CaseOutcome>, lessons: Option<LessonsComparison>) -> EvalReport {
    let passed = cases.iter().filter(|c| c.ok).count();
    EvalReport {
        total: cases.len(),
        passed,
        accuracy: build_accuracy(passed, cases.len()),
        cases,
        lessons,
    }
}

fn build_accuracy(passed: usize, total: usize) -> f64 {
    passed as f64 / total.max(1) as f64
}

/// Tạo rồi bật từng bài, như admin làm ở màn Bài học; người chạy eval là admin của workspace tạm.
async fn add_lessons(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    lessons: &[EvalLesson],
) -> Result<(), AppError> {
    let dataset = Some(parse_uuid(dataset_id).map_err(AppError::Internal)?);
    for lesson in lessons {
        let request = LessonRequest {
            text: lesson.text.clone(),
            dataset_id: if lesson.workspace { None } else { dataset },
            error_id: None,
        };
        let created = service::lessons::create_lesson(state, member, request).await?;
        service::lessons::set_enabled(state, member, &created.id.to_string(), true).await?;
    }
    Ok(())
}

/// Tài khoản và workspace tạm cho lần đánh giá, với dataset mẫu đã nạp và hồ sơ ngữ nghĩa xong (mock thì có ngay).
/// Trả id của dataset để hỏi. Workspace bị xóa sau khi chạy xong (run_eval), kéo theo user rác trong bảng users:
/// chấp nhận được vì eval nhắm vào database thử, không phải bản cài thật.
pub(crate) async fn prepare(state: &AppState, data: &Path) -> Result<(Member, String), AppError> {
    let email = format!("eval-{}@example.com", uuid::Uuid::new_v4());
    let user = state.store.create_user(&email, "Eval", "unused").await?;
    let membership = workspaces::create(state, &user, "Eval").await?;
    let member = auth::member_of(&state.store, user, &membership.slug).await?;

    let result = upload_local_file(state, &member, data, SAMPLE_NAME).await?;

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
    cases: &[EvalCase],
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
            question: case.question.clone(),
            ok: true,
            detail,
        },
        Err(detail) => CaseOutcome {
            id: case.id.clone(),
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
    let expected = expected_rows(state, member, dataset_id, case).await?;
    // Mỗi câu của bộ đánh giá là một câu độc lập, không có hội thoại trước.
    let history = AskHistory::default();
    let response = ask(state, member, dataset_id, &case.question, &[], &history)
        .await
        .map_err(|e| format!("lỗi hệ thống khi hỏi: {e:?}"))?;
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
