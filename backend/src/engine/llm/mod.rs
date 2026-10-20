// SPDX-License-Identifier: Apache-2.0
/* engine/llm/mod.rs - trait duy nhất của crate: LlmProvider. Chọn cài đặt lúc khởi động theo LLM_PROVIDER.
Dùng async-trait vì trait có hàm async mà cần đứng sau Arc<dyn ...>; Rust chưa hỗ trợ trực tiếp trường hợp này. */

mod gemini;
mod mock;
mod ollama;

use std::sync::Arc;

use async_trait::async_trait;

use crate::{config::Config, error::AppError};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Tên hiện trên giao diện và trong log, ví dụ "mock" hay "gemini-2.5-flash".
    fn name(&self) -> String;

    /// Gửi prompt, nhận văn bản trả về.
    // Khung: service/ask gọi hàm này từ issue sinh SQL. Gỡ allow khi đó.
    #[allow(dead_code)]
    async fn complete(&self, prompt: &str) -> Result<String, AppError>;
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
            Arc::new(mock::Mock)
        }
    }
}
