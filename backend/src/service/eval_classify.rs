// SPDX-License-Identifier: Apache-2.0
/* service/eval_classify.rs - bộ câu thử phân loại câu hỏi (#197): với mỗi câu của file ca, so loại câu đúng (viết tay)
với loại của luật từ khóa (build_question_kind, explain/form.rs) và loại của bước classify (ask/classify.rs), và đo
thời gian của lần gọi classify. Lệnh `xdi-backend eval-classify` ở cli.rs gọi run_eval_classify.

Dùng cùng cách chuẩn bị với bộ đánh giá câu hỏi (service/eval.rs, prepare): workspace tạm, nạp file mẫu, chờ hồ sơ ngữ
nghĩa, để prompt có đúng tên cột, bí danh và mô tả như khi người dùng hỏi thật. Mỗi câu đi qua build_ask_context rồi
classify, không qua bước hỏi lại khi câu mơ hồ: ở đây chỉ đo phân loại. Luật từ khóa không đọc câu hỏi trước, nên với
câu tiếp nối nó chỉ xét chữ của câu đó, đúng như đường dự phòng làm khi chạy thật.

Với mock, câu không có câu viết sẵn rơi về luật từ khóa, nên tỷ lệ của "mô hình" chỉ có nghĩa khi chạy với mô hình
thật; báo cáo đếm số câu rơi về luật từ khóa để thấy điều đó. */

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::service::ask::classify::{build_classify_timeout, classify, ClassifyInput};
use crate::service::ask::context::build_ask_context;
use crate::service::eval::prepare;
use crate::service::explain::form::{build_question_kind, QuestionKind};
use crate::service::workspaces;
use crate::state::AppState;

/// Loại câu, viết như trong file ca và trong câu trả lời của mô hình.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    Plain,
    Cause,
}

impl CaseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CaseKind::Plain => "plain",
            CaseKind::Cause => "cause",
        }
    }
}

impl From<QuestionKind> for CaseKind {
    fn from(kind: QuestionKind) -> Self {
        match kind {
            QuestionKind::Plain => CaseKind::Plain,
            QuestionKind::Why => CaseKind::Cause,
        }
    }
}

/// Một câu thử: câu hỏi, các câu hỏi trước của cuộc trò chuyện (cũ trước, có thể vắng) và loại đúng.
#[derive(Debug, Deserialize)]
pub struct ClassifyCase {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub earlier: Vec<String>,
    pub kind: CaseKind,
}

#[derive(Debug, Serialize)]
pub struct ClassifyOutcome {
    pub id: String,
    pub question: String,
    pub expected: CaseKind,
    pub keyword: CaseKind,
    /// Loại bước classify chọn; là loại của luật từ khóa khi by_model là false.
    pub model: CaseKind,
    pub by_model: bool,
    pub duration_ms: u64,
    /// Chi tiết của bước classify: lý do của mô hình hoặc lý do dùng luật từ khóa.
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct ClassifyReport {
    pub total: usize,
    pub keyword_correct: usize,
    pub model_correct: usize,
    /// Số câu bước classify rơi về luật từ khóa (mô hình lỗi, quá giờ, câu trả lời không đọc được).
    pub fallbacks: usize,
    /// Trung vị thời gian của bước classify trên mọi câu, tính bằng mili giây.
    pub median_ms: u64,
    pub cases: Vec<ClassifyOutcome>,
}

/// Đọc file ca. Lỗi đọc hay JSON hỏng là lỗi hạ tầng (mã thoát 2 ở cli).
pub fn load_cases(path: &Path) -> Result<Vec<ClassifyCase>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Chạy cả bộ trên một workspace tạm rồi xóa workspace đó. pause là khoảng nghỉ giữa hai câu, để không vượt hạn mức
/// số lần gọi mỗi phút của khóa mô hình.
pub async fn run_eval_classify(
    state: &AppState,
    data: &Path,
    cases: &[ClassifyCase],
    pause: Duration,
) -> Result<ClassifyReport, AppError> {
    let (member, dataset_id) = prepare(state, data).await?;
    let outcome = run_cases(state, &member, &dataset_id, cases, pause).await;
    let _ = workspaces::delete(state, &member).await;
    let outcomes = outcome?;
    Ok(build_report(outcomes))
}

async fn run_cases(
    state: &AppState,
    member: &crate::auth::Member,
    dataset_id: &str,
    cases: &[ClassifyCase],
    pause: Duration,
) -> Result<Vec<ClassifyOutcome>, AppError> {
    let ws = &member.workspace;
    let id = uuid::Uuid::parse_str(dataset_id)
        .map_err(|e| AppError::Internal(format!("eval dataset id: {e}")))?;
    let not_found = || AppError::Internal("eval dataset vanished".to_string());
    let dataset = state
        .store
        .get_dataset(ws, id)
        .await?
        .ok_or_else(not_found)?;
    let version = state
        .store
        .get_current_version(ws, id)
        .await?
        .ok_or_else(not_found)?;
    let timeout = build_classify_timeout(&state.config);
    let mut outcomes = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(pause).await;
        }
        let context = build_ask_context(state, ws, &version, &case.question, &[]).await?;
        let input = ClassifyInput {
            table: &dataset.logical_table,
            question: &case.question,
            columns: &context.columns,
            earlier: &case.earlier,
        };
        let classified = classify(state.llm.as_ref(), &input, timeout).await;
        outcomes.push(ClassifyOutcome {
            id: case.id.clone(),
            question: case.question.clone(),
            expected: case.kind,
            keyword: build_question_kind(&case.question).into(),
            model: classified.kind.into(),
            by_model: classified.by_model,
            duration_ms: classified.step.duration_ms,
            detail: classified.step.detail.unwrap_or_default(),
        });
    }
    Ok(outcomes)
}

/// Đếm câu đúng của hai cách và tính trung vị thời gian.
pub fn build_report(cases: Vec<ClassifyOutcome>) -> ClassifyReport {
    let keyword_correct = cases.iter().filter(|c| c.keyword == c.expected).count();
    let model_correct = cases.iter().filter(|c| c.model == c.expected).count();
    let fallbacks = cases.iter().filter(|c| !c.by_model).count();
    let mut durations: Vec<u64> = cases.iter().map(|c| c.duration_ms).collect();
    durations.sort_unstable();
    // Số câu chẵn thì lấy trung bình hai số giữa.
    let median_ms = match durations.len() {
        0 => 0,
        n if n % 2 == 1 => durations[n / 2],
        n => (durations[n / 2 - 1] + durations[n / 2]) / 2,
    };
    ClassifyReport {
        total: cases.len(),
        keyword_correct,
        model_correct,
        fallbacks,
        median_ms,
        cases,
    }
}
