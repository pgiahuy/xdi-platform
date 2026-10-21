// SPDX-License-Identifier: Apache-2.0
/* engine/llm/mod.rs - trait duy nhất của crate: LlmProvider. Chọn cài đặt lúc khởi động theo LLM_PROVIDER.
Dùng async-trait vì trait có hàm async mà cần đứng sau Arc<dyn ...>; Rust chưa hỗ trợ trực tiếp trường hợp này. */

mod gemini;
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
    /// Viết mô tả, bí danh, câu hỏi gợi ý cho một dataset lúc nạp.
    Profile,
}

/// Một lần nhờ mô hình. Tách câu hỏi gốc khỏi prompt: prompt còn chứa các câu mẫu làm ví dụ, nên mock không tìm câu
/// hỏi trong prompt được (sẽ khớp nhầm vào ví dụ).
pub struct LlmRequest<'a> {
    pub task: LlmTask,
    /// Câu hỏi gốc của người dùng; với Profile là tên bảng logic của dataset.
    pub question: &'a str,
    /// Prompt đầy đủ gửi cho mô hình thật.
    // Khung: Gemini và Ollama đọc trường này từ issue gọi thật (#27). Gỡ allow khi đó.
    #[allow(dead_code)]
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
        "gemini" => Arc::new(gemini::Gemini::new(
            config.gemini_api_key.clone(),
            config.gemini_model.clone(),
        )),
        "ollama" => Arc::new(ollama::Ollama::new(
            config.ollama_url.clone(),
            config.ollama_model.clone(),
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
