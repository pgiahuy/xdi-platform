// SPDX-License-Identifier: Apache-2.0
/* engine/llm/gemini.rs - gọi Gemini API qua HTTPS. Khóa lấy từ GEMINI_API_KEY, không bao giờ ghi vào log. */

use async_trait::async_trait;

use super::{LlmProvider, LlmRequest};
use crate::error::AppError;

pub struct Gemini {
    api_key: String,
    model: String,
}

impl Gemini {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[async_trait]
impl LlmProvider for Gemini {
    fn name(&self) -> String {
        self.model.clone()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        if self.api_key.is_empty() {
            return Err(AppError::BadRequest(
                "Thiếu GEMINI_API_KEY trong .env".to_string(),
            ));
        }
        Err(AppError::NotImplemented("gọi Gemini"))
    }
}
