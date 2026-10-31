// SPDX-License-Identifier: Apache-2.0
/* service/semantic/parts.rs - lập hồ sơ ngữ nghĩa bằng ba lời gọi mô hình chạy song song (#184): phần cột (mô tả, bí
danh, đơn vị, cách gộp, đổi vai trò, cột nhạy cảm), phần bí danh giá trị, phần câu hỏi gợi ý. Trước #184 là một prompt
đòi cả năm khóa một lúc; mô hình viết hết câu trả lời rồi mới trả, nên thời gian chờ tăng theo độ dài cả câu trả lời.
Tách ra thì mỗi lời gọi viết một phần, chạy cùng lúc bằng tokio::join!, và thời gian chờ là của phần lâu nhất.

Ba phần độc lập: không phần nào cần câu trả lời của phần khác (prompt câu gợi ý tả cột cộng được và cột lấy trung bình
bằng lời, không dựa vào cách gộp phần cột ghi). Mỗi phần có prompt (mod.rs, build_profile_prompt) và schema riêng
(schema.rs, build_part_schema). Câu trả lời của mỗi phần chỉ được lấy đúng các khóa của phần đó, rồi ghép thành một
object và đi qua build_profile_answer như câu trả lời đầy đủ trước đây, nên bước kiểm từng mục không đổi.

Một phần lỗi thì giữ các phần đã có: hồ sơ được ghi với các phần còn lại, phần lỗi bỏ trống (log cảnh báo). Cả ba lỗi
thì là một lỗi như trước: có lỗi tạm thì cả lượt là lỗi tạm (lần khởi động sau thử lại), còn lại là lỗi vĩnh viễn. */

use uuid::Uuid;

use crate::engine::llm::{LlmRequest, LlmTask};
use crate::engine::store::{ColumnValue, Dataset, ProfileColumn};
use crate::error::AppError;
use crate::service::semantic::schema::build_part_schema;
use crate::service::semantic::{build_profile_prompt, parse_answer_json, SemanticFailure};
use crate::state::AppState;

/// Ba phần của hồ sơ ngữ nghĩa, mỗi phần một prompt và một lời gọi mô hình.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePart {
    /// Mô tả, bí danh, đơn vị, cách gộp của cột, đổi vai trò, cột nhạy cảm.
    Columns,
    /// Bí danh của giá trị cột dimension.
    Values,
    /// Câu hỏi gợi ý.
    Questions,
}

impl ProfilePart {
    /// Các khóa của câu trả lời mà phần này viết. Chỉ lấy các khóa này từ câu trả lời của phần: mock trả cùng một câu
    /// trả lời đầy đủ cho cả ba phần, lấy hết thì mỗi mục bị ghép ba lần.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            ProfilePart::Columns => &["columns", "role_changes", "sensitive_columns"],
            ProfilePart::Values => &["values"],
            ProfilePart::Questions => &["suggested_questions"],
        }
    }

    fn name(self) -> &'static str {
        match self {
            ProfilePart::Columns => "columns",
            ProfilePart::Values => "values",
            ProfilePart::Questions => "questions",
        }
    }
}

/// Những gì cả ba prompt cần.
pub struct PartInput<'a> {
    pub dataset: &'a Dataset,
    pub version_id: Uuid,
    pub row_count: Option<i64>,
    pub columns: &'a [ProfileColumn],
    pub values: &'a [ColumnValue],
}

/// Gọi ba phần cùng lúc rồi ghép thành câu trả lời dạng chữ cho build_profile_answer.
pub async fn ask_profile_parts(
    state: &AppState,
    input: &PartInput<'_>,
) -> Result<String, SemanticFailure> {
    // tokio::join! chạy ba future trong cùng task, xen kẽ nhau lúc chờ mạng; không cần tokio::spawn (spawn đòi dữ liệu
    // sở hữu 'static, ở đây các future chỉ mượn input).
    let (columns, values, questions) = tokio::join!(
        ask_part(state, ProfilePart::Columns, input),
        ask_part(state, ProfilePart::Values, input),
        ask_part(state, ProfilePart::Questions, input),
    );
    let answers = [
        (ProfilePart::Columns, columns),
        (ProfilePart::Values, values),
        (ProfilePart::Questions, questions),
    ];
    let merged = build_merged_answer(answers, input.version_id)?;
    Ok(merged.to_string())
}

/// Câu trả lời của một phần: object JSON chỉ gồm các khóa của phần đó, None khi phần không gọi mô hình.
type PartAnswer = Result<Option<serde_json::Map<String, serde_json::Value>>, SemanticFailure>;

/// Một phần. Phần bí danh giá trị không gọi mô hình khi phiên bản không có giá trị cột dimension nào: không có gì để
/// đặt bí danh, và đỡ một lời gọi trên hạn mức.
async fn ask_part(state: &AppState, part: ProfilePart, input: &PartInput<'_>) -> PartAnswer {
    if part == ProfilePart::Values && input.values.is_empty() {
        return Ok(None);
    }
    let prompt = build_profile_prompt(
        part,
        &input.dataset.name,
        &input.dataset.logical_table,
        input.row_count,
        input.columns,
        input.values,
    );
    let request = LlmRequest {
        task: LlmTask::Profile,
        question: &input.dataset.logical_table,
        prompt: &prompt,
        // Schema ép khuôn ở Gemini và Ollama; build_profile_answer vẫn kiểm từng mục vì schema không biết cột nào có
        // thật, và mock không theo schema.
        json_schema: Some(build_part_schema(part)),
    };
    let started = std::time::Instant::now();
    let text = state.llm.complete(&request).await.map_err(|e| match e {
        // Chỉ BadRequest là vĩnh viễn: Gemini chặn prompt vì nội dung, mock không có bảng. MAX_TOKENS, SAFETY và mã
        // 400 khác của Gemini đến dưới dạng Upstream nên là lỗi tạm, thử lại mỗi lần khởi động.
        AppError::BadRequest(_) => SemanticFailure::Permanent(e),
        other => SemanticFailure::Temporary(other),
    })?;
    let Some(serde_json::Value::Object(mut answer)) = parse_answer_json(&text) else {
        return Err(SemanticFailure::Permanent(AppError::Internal(format!(
            "semantic profile part {} is not a JSON object",
            part.name()
        ))));
    };
    answer.retain(|key, _| part.keys().contains(&key.as_str()));
    // Phần nào chậm nhất quyết định thời gian chờ của cả lượt; ghi riêng từng phần để biết nên chia lại phần nào.
    tracing::info!(
        version_id = %input.version_id,
        part = part.name(),
        duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "semantic profile part done"
    );
    Ok(Some(answer))
}

/// Ghép câu trả lời của các phần. Phần lỗi ghi log rồi bỏ; Err chỉ khi không phần nào có câu trả lời.
pub(crate) fn build_merged_answer(
    answers: [(ProfilePart, PartAnswer); 3],
    version_id: Uuid,
) -> Result<serde_json::Value, SemanticFailure> {
    let mut merged = serde_json::Map::new();
    let mut failures = Vec::new();
    let mut answered = 0;
    for (part, answer) in answers {
        match answer {
            Ok(Some(object)) => {
                answered += 1;
                merged.extend(object);
            }
            Ok(None) => {}
            Err(failure) => {
                tracing::warn!(version_id = %version_id, part = part.name(), error = ?failure,
                    "semantic profile part failed, keeping the other parts");
                failures.push(failure);
            }
        }
    }
    if answered > 0 {
        return Ok(serde_json::Value::Object(merged));
    }
    // Không phần nào xong: có lỗi tạm thì cả lượt là lỗi tạm, để lần khởi động sau thử lại thay vì đánh dấu đã thử.
    let temporary = failures
        .iter()
        .position(|f| matches!(f, SemanticFailure::Temporary(_)));
    let index = temporary.unwrap_or(0);
    if index < failures.len() {
        return Err(failures.swap_remove(index));
    }
    // Chỉ tới đây khi không phần nào gọi mô hình, mà phần cột và phần câu gợi ý luôn gọi.
    Err(SemanticFailure::Permanent(AppError::Internal(
        "semantic profile has no part to ask".to_string(),
    )))
}
