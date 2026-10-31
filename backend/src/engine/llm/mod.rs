// SPDX-License-Identifier: Apache-2.0
/* engine/llm/mod.rs - trait duy nhất của crate: LlmProvider. Chọn cài đặt lúc khởi động theo LLM_PROVIDER.
Dùng async-trait vì trait có hàm async mà cần đứng sau Arc<dyn ...>; Rust chưa hỗ trợ trực tiếp trường hợp này.
Lỗi khi gọi mô hình trả lời hướng dẫn tiếng Việt, chi tiết kỹ thuật chỉ ghi vào log. Chọn mã theo bên gây lỗi, để log
truy cập và giao diện phân biệt được "câu hỏi có vấn đề" với "mô hình đang lỗi":
- Upstream (502): dịch vụ mô hình lỗi, quá tải, hết hạn mức, không trả lời, không kết nối được, từ chối khóa, không có
  mô hình đã chọn, trả dữ liệu sai hay câu trả lời bị cắt. Request của người dùng không sai.
- Unavailable (503): máy chủ thiếu cấu hình (chưa đặt khóa, OLLAMA_URL không hợp lệ), chưa gửi gì đi.
- BadRequest (400): chỉ khi mô hình từ chối vì nội dung câu hỏi; gửi lại y nguyên vẫn bị từ chối.
Không dùng Unauthorized hay Forbidden cho khóa sai: giao diện hiểu 401 là phiên đăng nhập hết hạn. Không dùng Internal:
lời trả về bị thay bằng câu chung, người dùng không biết cách sửa.
Số token và thời gian của từng lần gọi đo ở engine/llm/meter.rs (#184). */

mod gemini;
mod http;
pub mod meter;
// pub(crate) để test ở tầng service dùng được mô hình giả (ask/narrate_tests.rs kiểm câu narrate mẫu qua bước kiểm số).
pub(crate) mod mock;
mod ollama;

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{config::Config, error::AppError};

// Loại việc nhờ mô hình làm. Mock tra câu trả lời viết sẵn theo loại việc và câu hỏi; Gemini và Ollama không cần biết.
// Serialize và ToSchema vì tên loại việc đi ra API trong danh sách lượt gọi mô hình của câu hỏi (meter.rs); chú thích
// /// của enum vào openapi.json nên viết tiếng Anh.
/// What the language model was asked to do in one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LlmTask {
    /// Sinh SQL cho một câu hỏi. Câu trả lời là object JSON có SQL bên trong (service/ask/reply.rs).
    Sql,
    /// Sửa câu trả lời vừa sinh, theo lời báo lỗi của guard hay DuckDB, hoặc vì câu trả lời không đọc được
    /// (service/ask/run.rs). Câu hỏi là câu hỏi gốc; câu trả lời cùng dạng với Sql.
    Repair,
    /// Viết mô tả, bí danh, câu hỏi gợi ý cho một dataset lúc nạp.
    Profile,
    /// Viết lời diễn giải từ bảng phân rã đã tính (service/ask/narrate.rs). Câu hỏi là câu hỏi gốc.
    Narrate,
    /// Điền form của câu "tại sao" (service/ask/why_prompt.rs), cả lần đầu lẫn lần sửa theo lỗi từng trường
    /// (service/explain/run.rs). Câu hỏi là câu hỏi gốc; câu trả lời là object JSON của form.
    Why,
    /// Viết đoạn trả lời bằng lời từ các dòng đầu của kết quả câu thường (service/ask/summary.rs). Câu hỏi là câu hỏi
    /// gốc; câu trả lời là văn bản tự do.
    Summary,
    /// Viết lời của phần giải thích câu thường từ các câu code đã viết (service/insight/narrate.rs, #227). Câu hỏi là
    /// câu hỏi gốc; câu trả lời là văn bản tự do.
    Insight,
    /// Phân loại câu hỏi trước khi viết SQL hay điền form (service/ask/classify.rs): câu thường hay câu hỏi tìm nguyên
    /// nhân. Câu hỏi là câu hỏi gốc; câu trả lời là object JSON {kind, reason}.
    Classify,
}

/// Một lần nhờ mô hình. Tách câu hỏi gốc khỏi prompt: prompt còn chứa các câu mẫu làm ví dụ, nên mock không tìm câu
/// hỏi trong prompt được (sẽ khớp nhầm vào ví dụ).
pub struct LlmRequest<'a> {
    pub task: LlmTask,
    /// Câu hỏi gốc của người dùng; với Profile là tên bảng logic của dataset.
    pub question: &'a str,
    /// Prompt đầy đủ gửi cho mô hình thật.
    pub prompt: &'a str,
    /// JSON Schema câu trả lời phải theo; None là văn bản tự do. Gemini nhận qua generationConfig.responseJsonSchema,
    /// Ollama qua format: mô hình bị ép trả đúng một object JSON, nên người gọi không phải đoán SQL nằm ở đâu trong
    /// lời giải thích. Mock bỏ qua trường này, vì câu viết sẵn đã đúng dạng.
    pub json_schema: Option<serde_json::Value>,
}

/// Số token nhà cung cấp báo cho một lần gọi. output_tokens gồm cả token suy nghĩ nháp của mô hình (Gemini
/// thoughtsTokenCount), vì bảng giá của Google tính chúng như token trả lời.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Văn bản trả về cùng số token, khi nhà cung cấp có báo.
pub struct LlmReply {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Tên hiện trên giao diện và trong log, ví dụ "mock" hay "gemini-2.5-flash".
    fn name(&self) -> String;

    /// Gửi yêu cầu, nhận văn bản trả về.
    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError>;

    /// Như complete, kèm số token. Mặc định không có số token, để mock và các mô hình giả trong test chỉ cần viết
    /// complete; Gemini và Ollama viết lại hàm này vì response của chúng có số token.
    async fn complete_with_usage(&self, request: &LlmRequest<'_>) -> Result<LlmReply, AppError> {
        let text = self.complete(request).await?;
        Ok(LlmReply { text, usage: None })
    }
}

pub fn from_config(config: &Config) -> Arc<dyn LlmProvider> {
    match config.llm_provider.as_str() {
        "gemini" => {
            // Thiếu khóa thì chỉ cảnh báo, không dừng backend: đăng nhập, nạp dữ liệu, quản lý workspace vẫn dùng
            // được, và mỗi câu hỏi nhận lỗi nói rõ cần đặt GEMINI_API_KEY. Dừng hẳn thì compose khởi động lại liên tục
            // mà người dùng chỉ thấy trang không mở được.
            if config.gemini_api_key.is_empty() {
                tracing::warn!(
                    "LLM_PROVIDER=gemini but GEMINI_API_KEY is empty: questions will fail until it is set"
                );
            }
            Arc::new(gemini::Gemini::new(
                gemini::API_URL,
                config.gemini_api_key.clone(),
                config.gemini_model.clone(),
                config.gemini_timeout,
            ))
        }
        "ollama" => Arc::new(ollama::Ollama::new(
            &config.ollama_url,
            config.ollama_model.clone(),
            config.ollama_timeout,
            config.ollama_num_ctx,
        )),
        other => {
            if other != "mock" {
                tracing::warn!(
                    provider = other,
                    "unknown LLM_PROVIDER, falling back to mock"
                );
            }
            Arc::new(mock::Mock::new())
        }
    }
}
