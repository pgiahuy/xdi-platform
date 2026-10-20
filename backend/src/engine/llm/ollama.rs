// SPDX-License-Identifier: Apache-2.0
/* engine/llm/ollama.rs - gọi Ollama chạy trên máy hoặc mạng nội bộ, cho chế độ offline. */

use async_trait::async_trait;

use super::LlmProvider;
use crate::error::AppError;

pub struct Ollama {
    url: String,
    model: String,
}

impl Ollama {
    pub fn new(url: String, model: String) -> Self {
        Self { url, model }
    }
}

#[async_trait]
impl LlmProvider for Ollama {
    fn name(&self) -> String {
        format!("ollama:{}", self.model)
    }

    async fn complete(&self, _prompt: &str) -> Result<String, AppError> {
        tracing::debug!(url = %self.url, "ollama provider not implemented yet");
        Err(AppError::NotImplemented("gọi Ollama"))
    }
}
