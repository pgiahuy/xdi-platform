// SPDX-License-Identifier: Apache-2.0
/* engine/llm/mock.rs - nhà cung cấp giả, chạy không cần khóa và dùng trong test.
Sau này trả SQL cố định cho bộ câu hỏi mẫu của dataset mẫu; câu khác thì báo cần cấu hình LLM thật. */

use async_trait::async_trait;

use super::LlmProvider;
use crate::error::AppError;

pub struct Mock;

#[async_trait]
impl LlmProvider for Mock {
    fn name(&self) -> String {
        "mock".to_string()
    }

    async fn complete(&self, _prompt: &str) -> Result<String, AppError> {
        Err(AppError::BadRequest(
            "Chế độ mock chỉ trả lời các câu hỏi mẫu. Đặt LLM_PROVIDER=gemini hoặc ollama để hỏi tự do".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_explains_how_to_enable_real_llm() {
        let err = Mock.complete("bất kỳ").await.unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("LLM_PROVIDER")),
            other => panic!("lỗi không mong đợi: {other:?}"),
        }
    }
}
