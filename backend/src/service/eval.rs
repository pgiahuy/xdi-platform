// SPDX-License-Identifier: Apache-2.0
/* service/eval.rs - bộ đánh giá câu hỏi mẫu (#37, #202): nạp dataset của một file bộ câu (định dạng ở
service/eval_cases.rs) vào workspace tạm, hỏi từng câu, so kết quả với SQL đúng viết tay (câu đúng cũng đi qua guard
và chạy trên DuckDB như câu mô hình sinh). Lệnh `xdi-backend eval` ở cli.rs gọi hàm run_eval.

So bằng tập dòng kết quả, không so chữ SQL: nhiều câu SQL khác nhau cùng trả lời đúng một câu hỏi. Luật so ở
service/eval_grade.rs. Một ca có thể có nhiều expected_sql (câu mơ hồ kiểu "doanh thu trước hay sau giảm giá"), khớp
một là đúng. Câu phải từ chối (must_refuse) đạt khi status là rejected. Ba trạng thái: đạt, trượt, cần xem lại
(Verdict); câu cần xem lại không tính vào tỷ lệ đúng.

Chạy với mock thì chỉ hỏi câu có cờ mock: mock chỉ trả lời câu mẫu viết sẵn, câu khác là lỗi 400 chứ không phải mô
hình trả lời sai. Câu tiếp nối chưa chạy được (cần hỏi qua cuộc trò chuyện), nên được ghi là bỏ qua kèm lý do. */

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::auth::{self, Member};
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::error::AppError;
use crate::service::ask::contract::{AskResponse, AskStatus, CellValue};
use crate::service::ask::result::build_result_table;
use crate::service::ask::{ask, AskHistory};
use crate::service::eval_cases::{CaseFile, CaseKind, EvalCase};
use crate::service::eval_grade::build_eval_verdict;
use crate::service::samples::{add_metrics_file, upload_local_file};
use crate::service::workspaces;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CaseOutcome {
    pub id: String,
    pub kind: CaseKind,
    pub question: String,
    pub verdict: Verdict,
    /// Ghi chú khi đạt (thừa cột, cột khác thứ tự), lý do khi trượt hay cần xem lại: trạng thái không như mong đợi,
    /// SQL sinh ra, chỗ kết quả lệch, lỗi của mô hình.
    pub detail: String,
}

/// Kết quả chấm một câu (#202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    /// Chưa chấm được: mô hình không trả lời (hết hạn mức, quá tải), đáp án không chạy, hay hệ thống hỏi lại để chọn
    /// cách hiểu. Không tính vào tỷ lệ đúng.
    Review,
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
    pub failed: usize,
    pub review: usize,
    /// passed / (passed + failed): câu cần xem lại không vào mẫu số.
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
    let count = |v: Verdict| outcomes.iter().filter(|c| c.verdict == v).count();
    let (passed, failed, review) = (
        count(Verdict::Pass),
        count(Verdict::Fail),
        count(Verdict::Review),
    );
    Ok(EvalReport {
        total: outcomes.len(),
        passed,
        failed,
        review,
        accuracy: passed as f64 / (passed + failed).max(1) as f64,
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
    let (verdict, detail) = grade_case(state, member, dataset_id, case).await;
    CaseOutcome {
        id: case.id.clone(),
        kind: case.kind,
        question: case.question.clone(),
        verdict,
        detail,
    }
}

async fn grade_case(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> (Verdict, String) {
    // Mỗi câu của bộ đánh giá là một câu độc lập, không có hội thoại trước.
    let history = AskHistory::default();
    let response = match ask(state, member, dataset_id, &case.question, &[], &history).await {
        Ok(r) => r,
        Err(e) => return build_ask_error_verdict(&e),
    };
    if case.must_refuse {
        return build_refuse_verdict(response.status, response.sql_generated.as_deref());
    }
    let expected = match expected_tables(state, member, dataset_id, case).await {
        Ok(t) => t,
        // Lỗi của bộ câu, không phải của mô hình: không tính vào tỷ lệ đúng, người viết câu phải sửa.
        Err(why) => return (Verdict::Review, format!("đáp án không chạy được: {why}")),
    };
    build_answer_verdict(&response, case, &expected)
}

/// ask trả lỗi. Mô hình hết hạn mức, quá tải hay máy chủ thiếu khóa thì chưa biết câu trả lời đúng hay sai, nên là
/// cần xem lại; tính là trượt thì một lần hết hạn mức kéo tỷ lệ đúng xuống mà mô hình chưa trả lời câu nào.
pub fn build_ask_error_verdict(error: &AppError) -> (Verdict, String) {
    match error {
        AppError::Upstream(m) | AppError::Unavailable(m) => {
            (Verdict::Review, format!("mô hình không trả lời được: {m}"))
        }
        e => (Verdict::Fail, format!("lỗi hệ thống khi hỏi: {e:?}")),
    }
}

/// Chấm câu trả lời của một câu không phải từ chối với các bảng đáp án (mỗi expected_sql chạy được một bảng, khớp
/// một là đạt).
pub fn build_answer_verdict(
    response: &AskResponse,
    case: &EvalCase,
    expected: &[Vec<Vec<Option<CellValue>>>],
) -> (Verdict, String) {
    let sql = response.sql_generated.as_deref().unwrap_or_default();
    let message = response.message.as_deref().unwrap_or_default();
    match response.status {
        AskStatus::Answered => {}
        // Hỏi lại để chọn cách hiểu không sai với câu mơ hồ; người đọc kết quả quyết định câu đó có mơ hồ không.
        AskStatus::NeedsChoice => {
            return (
                Verdict::Review,
                format!("hệ thống hỏi lại để chọn cách hiểu: {message}"),
            )
        }
        status => {
            return (
                Verdict::Fail,
                format!("status {status:?} thay vì answered; message: {message}"),
            )
        }
    }
    // Bảng khớp mà không có phần giải thích thì hệ thống đã trả lời như câu thường: người hỏi "vì sao" không nhận
    // được nguyên nhân.
    if case.kind.is_why() && response.explanation.is_none() {
        return (
            Verdict::Fail,
            format!("câu tìm nguyên nhân rơi về đường câu thường; sql sinh ra: {sql}"),
        );
    }
    let Some(result) = &response.result else {
        return (Verdict::Fail, "answered nhưng không có result".to_string());
    };
    let mut first_error = None;
    for table in expected {
        match build_eval_verdict(&result.rows, table, case.ordered, &case.required_columns) {
            Ok(note) => return (Verdict::Pass, note),
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    (
        Verdict::Fail,
        format!("{}; sql sinh ra: {sql}", first_error.unwrap_or_default()),
    )
}

/// Câu phải từ chối: đạt khi status là rejected. Trả lời được hay chạy lỗi đều là trượt, vì người hỏi không được
/// báo rằng câu này không làm được.
pub fn build_refuse_verdict(status: AskStatus, sql: Option<&str>) -> (Verdict, String) {
    if status == AskStatus::Rejected {
        return (Verdict::Pass, "từ chối đúng".to_string());
    }
    (
        Verdict::Fail,
        format!(
            "status {status:?} thay vì rejected; sql sinh ra: {}",
            sql.unwrap_or_default()
        ),
    )
}

/// Bảng kết quả của mọi expected_sql chạy được. Câu nào không chạy được thì bỏ, chỉ báo lỗi khi không câu nào chạy.
async fn expected_tables(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
    case: &EvalCase,
) -> Result<Vec<Vec<Vec<Option<CellValue>>>>, String> {
    let table = build_allowed_table(state, member, dataset_id).await?;
    let mut tables = Vec::new();
    let mut last_error = String::new();
    for sql in &case.expected_sql {
        match run_sql_rows(state, &table, sql).await {
            Ok(rows) => tables.push(rows),
            Err(e) => last_error = e,
        }
    }
    if tables.is_empty() {
        return Err(format!("không expected_sql nào chạy được: {last_error}"));
    }
    Ok(tables)
}

/// Bảng logic của dataset như guard thấy khi người dùng hỏi, không lọc dòng theo vai.
pub(crate) async fn build_allowed_table(
    state: &AppState,
    member: &Member,
    dataset_id: &str,
) -> Result<AllowedTable, String> {
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
    Ok(AllowedTable {
        name: dataset.logical_table,
        columns,
        parquet_dir: state.config.data_dir.join(
            version
                .parquet_path
                .ok_or("phiên bản không có file parquet")?,
        ),
        row_filters: Vec::new(),
    })
}

/// Chạy một câu SQL viết tay qua guard và DuckDB. Câu đáp án và câu mô hình sinh qua cùng guard và cùng engine, nên
/// định dạng ô hai phía giống nhau và cách so không thiên vị cách viết SQL.
pub(crate) async fn run_sql_rows(
    state: &AppState,
    table: &AllowedTable,
    sql: &str,
) -> Result<Vec<Vec<Option<CellValue>>>, String> {
    let guarded = build_guarded_sql(sql, std::slice::from_ref(table), state.config.ask_max_rows)
        .map_err(|e| format!("SQL không qua được guard: {e:?}"))?;
    let runnable = guarded.runnable;
    let timeout = state.config.query_timeout;
    let output = tokio::task::spawn_blocking(move || run_query(&runnable, 100_000, timeout))
        .await
        .map_err(|e| format!("{e:?}"))?
        .map_err(|e| format!("SQL chạy lỗi: {e:?}"))?;
    Ok(build_result_table(output, state.config.ask_max_rows).rows)
}

fn parse_uuid(text: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(text).map_err(|e| e.to_string())
}
