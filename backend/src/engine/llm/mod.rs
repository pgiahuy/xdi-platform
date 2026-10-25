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
lời trả về bị thay bằng câu chung, người dùng không biết cách sửa. */

mod gemini;
mod http;
mod mock;
mod ollama;

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::{config::Config, error::AppError};

/// Loại việc nhờ mô hình làm. Mock tra câu trả lời viết sẵn theo loại việc và câu hỏi; Gemini và Ollama không cần biết.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTask {
    /// Sinh SQL cho một câu hỏi.
    Sql,
    /// Sửa câu SQL vừa sinh, theo lời báo lỗi của guard hay DuckDB (service/ask_run.rs). Câu hỏi là câu hỏi gốc.
    Repair,
    /// Viết mô tả, bí danh, câu hỏi gợi ý cho một dataset lúc nạp.
    Profile,
    /// Viết lời diễn giải từ bảng phân rã đã tính (service/ask_narrate.rs). Câu hỏi là câu hỏi gốc.
    Narrate,
}

/// Một lần nhờ mô hình. Tách câu hỏi gốc khỏi prompt: prompt còn chứa các câu mẫu làm ví dụ, nên mock không tìm câu
/// hỏi trong prompt được (sẽ khớp nhầm vào ví dụ).
pub struct LlmRequest<'a> {
    pub task: LlmTask,
    /// Câu hỏi gốc của người dùng; với Profile là tên bảng logic của dataset.
    pub question: &'a str,
    /// Prompt đầy đủ gửi cho mô hình thật.
    pub prompt: &'a str,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Tên hiện trên giao diện và trong log, ví dụ "mock" hay "gemini-2.5-flash".
    fn name(&self) -> String;

    /// Gửi yêu cầu, nhận văn bản trả về.
    // Khung: service/ask gọi hàm này từ issue sinh SQL. Gỡ allow khi đó.
    #[allow(dead_code)]
    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError>;
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
