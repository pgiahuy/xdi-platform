// SPDX-License-Identifier: Apache-2.0
/* engine/llm/ollama.rs - gọi Ollama chạy trên máy hoặc mạng nội bộ, cho chế độ offline.
Dùng POST /api/generate với stream false: một request, một response JSON có trường response. Gửi đúng một chuỗi
prompt như Gemini; /api/chat chỉ cần khi có hội thoại nhiều lượt. */

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{http, LlmProvider, LlmRequest};
use crate::error::AppError;

pub struct Ollama {
    /// Địa chỉ gốc, đã bỏ dấu / ở cuối.
    url: String,
    /// Địa chỉ để ghi log, chỉ scheme, host, cổng: OLLAMA_URL có thể chứa user:pass của proxy đứng trước Ollama.
    log_url: String,
    model: String,
    timeout: Duration,
    /// Như ở gemini.rs: lỗi dựng client giữ lại để báo khi có câu hỏi.
    client: Result<reqwest::Client, String>,
}

impl Ollama {
    pub fn new(url: &str, model: String, timeout: Duration) -> Self {
        let client = http::build_client(timeout, http::CONNECT_TIMEOUT);
        if let Err(err) = &client {
            tracing::error!(error = %err, "cannot build HTTP client for Ollama");
        }
        Self {
            url: url.trim_end_matches('/').to_string(),
            log_url: http::build_log_url(url),
            model,
            timeout,
            client,
        }
    }

    /// Lỗi khi gửi hoặc nhận. Ghi địa chỉ Ollama vào log để người quản trị thấy backend đang gọi đi đâu.
    fn transport_error(&self, err: reqwest::Error) -> AppError {
        let (bad_url, connect, timed_out) = (err.is_builder(), err.is_connect(), err.is_timeout());
        tracing::warn!(url = %self.log_url, error = %http::describe(err), "ollama request failed");
        if bad_url {
            return AppError::Unavailable(
                "OLLAMA_URL không phải địa chỉ hợp lệ, ví dụ đúng: http://localhost:11434"
                    .to_string(),
            );
        }
        // Hết thời gian mở kết nối (tường lửa bỏ gói) thì reqwest bật cả is_connect lẫn is_timeout. Xét is_connect
        // trước, nếu không người dùng được khuyên tăng OLLAMA_TIMEOUT_SECS, việc không giúp được gì.
        let message = if connect {
            "Không kết nối được tới Ollama: kiểm tra Ollama đang chạy, OLLAMA_URL trỏ đúng máy và tường lửa không chặn cổng"
                .to_string()
        } else if timed_out {
            format!(
                "Ollama không trả lời trong {} giây. Mô hình chạy trên máy có thể cần lâu hơn: tăng OLLAMA_TIMEOUT_SECS",
                self.timeout.as_secs()
            )
        } else {
            "Kết nối tới Ollama bị ngắt giữa chừng. Thử lại; lặp lại thì xem log của backend"
                .to_string()
        };
        AppError::Upstream(message)
    }

    /// Lỗi Ollama trả về bằng mã HTTP. Body lỗi có dạng {"error": "..."}; chỉ ghi vào log.
    fn status_error(&self, status: reqwest::StatusCode, body: &str) -> AppError {
        let detail = serde_json::from_str::<ErrorResponse>(body)
            .map(|response| response.error)
            .unwrap_or_default();
        tracing::warn!(status = status.as_u16(), error = %detail, "ollama returned an error");
        let message = match status.as_u16() {
            // Ollama báo mô hình chưa tải bằng 404 kèm {"error": "model \"x\" not found, try pulling it first"}.
            // 404 không có body như vậy là đường dẫn không tồn tại: OLLAMA_URL trỏ sang dịch vụ khác hoặc proxy sai.
            404 if detail.contains("model") && detail.contains("not found") => format!(
                "Ollama chưa có mô hình {0}: chạy ollama pull {0} trên máy chạy Ollama, hoặc đổi OLLAMA_MODEL",
                self.model
            ),
            404 => "Không tìm thấy Ollama ở OLLAMA_URL (đường dẫn /api/generate trả 404): kiểm tra OLLAMA_URL trỏ tới Ollama, không phải dịch vụ khác".to_string(),
            300..=399 => format!(
                "OLLAMA_URL trả về chuyển hướng (mã {}): đặt OLLAMA_URL đúng địa chỉ cuối cùng, ví dụ đổi http thành https",
                status.as_u16()
            ),
            code => format!("Ollama báo lỗi (mã {code}), xem log của backend"),
        };
        AppError::Upstream(message)
    }
}

#[async_trait]
impl LlmProvider for Ollama {
    fn name(&self) -> String {
        format!("ollama:{}", self.model)
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        let client = self.client.as_ref().map_err(|_| {
            AppError::Unavailable(
                "Máy chủ không tạo được client HTTP để gọi Ollama, xem log của backend".to_string(),
            )
        })?;
        let mut body = serde_json::json!({
            "model": self.model,
            "prompt": request.prompt,
            "stream": false,
        });
        // format nhận cả "json" (JSON bất kỳ) lẫn một JSON Schema; gửi schema thì Ollama ép mô hình theo đúng các trường.
        if let Some(schema) = &request.json_schema {
            body["format"] = schema.clone();
        }
        let response = client
            .post(format!("{}/api/generate", self.url))
            .json(&body)
            .send()
            .await
            .map_err(|err| self.transport_error(err))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| self.transport_error(err))?;
        if !status.is_success() {
            return Err(self.status_error(status, &text));
        }
        build_answer(&text)
    }
}

/// Lấy văn bản trả lời từ body của /api/generate.
fn build_answer(body: &str) -> Result<String, AppError> {
    let response: GenerateResponse = serde_json::from_str(body).map_err(|err| {
        // Chỉ ghi lỗi parse, không ghi body: body có thể chứa dữ liệu của người dùng.
        tracing::warn!(error = %err, "ollama response is not valid JSON of the expected shape");
        AppError::Upstream(
            "Ollama trả về dữ liệu không đọc được. Kiểm tra OLLAMA_URL trỏ tới Ollama, không phải dịch vụ khác"
                .to_string(),
        )
    })?;
    // done_reason "length": câu trả lời bị cắt vì chạm giới hạn số token. SQL cụt mà đi tiếp thì người dùng nhận lỗi
    // cú pháp khó hiểu ở bước sau. Các giá trị khác ("stop", hoặc không có) là trả lời xong.
    if response.done_reason.as_deref() == Some("length") {
        tracing::warn!("ollama answer was cut at the length limit");
        return Err(AppError::Upstream(
            "Câu trả lời của Ollama bị cắt vì vượt giới hạn độ dài. Thử lại hoặc hỏi ngắn gọn hơn"
                .to_string(),
        ));
    }
    if response.response.trim().is_empty() {
        tracing::warn!("ollama returned an empty response");
        return Err(AppError::Upstream(
            "Ollama không trả về nội dung. Thử lại hoặc diễn đạt lại câu hỏi".to_string(),
        ));
    }
    Ok(response.response)
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    done_reason: Option<String>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::llm::http::test_support::{closed_url, spawn, spawn_redirect, unresponsive};
    use crate::engine::llm::LlmTask;

    const PROMPT: &str = "Viết SQL: số đơn theo khu vực";

    // Body theo định dạng response thật của Ollama /api/generate khi stream false.
    const OK_BODY: &str = r#"{"model":"qwen2.5-coder:7b","created_at":"2026-10-22T08:00:00.000000Z","response":"SELECT 1","done":true,"done_reason":"stop","context":[1,2,3],"total_duration":5000000000,"load_duration":3000000000,"prompt_eval_count":12,"prompt_eval_duration":1000000000,"eval_count":3,"eval_duration":1000000000}"#;

    fn ollama(url: &str, timeout: Duration) -> Ollama {
        Ollama::new(url, "qwen2.5-coder:7b".to_string(), timeout)
    }

    async fn ask(provider: &Ollama) -> Result<String, AppError> {
        let request = LlmRequest {
            task: LlmTask::Sql,
            question: "số đơn theo khu vực",
            prompt: PROMPT,
            json_schema: None,
        };
        provider.complete(&request).await
    }

    /// Mã HTTP mà lỗi sẽ trả, kèm lời báo.
    fn failure(result: Result<String, AppError>) -> (u16, String) {
        match result {
            Err(AppError::BadRequest(message)) => (400, message),
            Err(AppError::Upstream(message)) => (502, message),
            Err(AppError::Unavailable(message)) => (503, message),
            other => panic!("expected a model error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn sends_prompt_without_streaming() {
        let (url, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        // Dấu / ở cuối OLLAMA_URL không làm hỏng đường dẫn.
        let answer = ask(&ollama(&format!("{url}/"), Duration::from_secs(5))).await;
        assert_eq!(answer.expect("answer"), "SELECT 1");

        let received = received.lock().unwrap().clone().expect("request");
        assert_eq!(received.path, "/api/generate");
        assert_eq!(received.body["model"], "qwen2.5-coder:7b");
        assert_eq!(received.body["prompt"], PROMPT);
        assert_eq!(received.body["stream"], false);
        assert!(received.body.get("format").is_none());
    }

    #[tokio::test]
    async fn sends_json_schema_as_format() {
        let (url, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let schema =
            serde_json::json!({"type": "object", "properties": {"sql": {"type": "string"}}});
        let request = LlmRequest {
            task: LlmTask::Sql,
            question: "số đơn theo khu vực",
            prompt: PROMPT,
            json_schema: Some(schema.clone()),
        };
        ollama(&url, Duration::from_secs(5))
            .complete(&request)
            .await
            .expect("answer");

        let received = received.lock().unwrap().clone().expect("request");
        assert_eq!(received.body["format"], schema);
    }

    #[tokio::test]
    async fn errors_become_clear_messages() {
        // Mã response của server giả, body, đoạn chữ phải có trong lời báo. Mọi trường hợp đều trả 502.
        let cases: &[(u16, &str, &str)] = &[
            (
                404,
                r#"{"error":"model \"qwen2.5-coder:7b\" not found, try pulling it first"}"#,
                "ollama pull qwen2.5-coder:7b",
            ),
            // 404 không phải của Ollama: OLLAMA_URL trỏ sang dịch vụ khác, không được khuyên pull mô hình.
            (
                404,
                "<html>Not Found</html>",
                "Không tìm thấy Ollama ở OLLAMA_URL",
            ),
            (
                404,
                "404 page not found",
                "Không tìm thấy Ollama ở OLLAMA_URL",
            ),
            (
                500,
                r#"{"error":"llama runner process has terminated"}"#,
                "mã 500",
            ),
            (200, "<html>not ollama</html>", "không đọc được"),
            (200, r#"{"model":"x","done":true}"#, "không đọc được"),
            (
                200,
                r#"{"response":"  ","done":true}"#,
                "không trả về nội dung",
            ),
            (
                200,
                r#"{"response":"SELECT a, b FROM t WHERE","done":true,"done_reason":"length"}"#,
                "bị cắt",
            ),
        ];
        for (status, body, expected) in cases {
            let (url, _) = spawn(*status, *body, Duration::ZERO).await;
            let (code, text) = failure(ask(&ollama(&url, Duration::from_secs(5))).await);
            assert!(text.contains(expected), "{status} {body}: {text}");
            assert_eq!(code, 502, "{status} {body}: {text}");
        }
    }

    #[tokio::test]
    async fn unreachable_ollama_is_reported() {
        let url = closed_url().await;
        let (code, text) = failure(ask(&ollama(&url, Duration::from_secs(5))).await);
        assert!(text.contains("Không kết nối được"), "{text}");
        assert_eq!(code, 502);
    }

    #[tokio::test]
    async fn unanswered_connect_is_not_reported_as_slow_model() {
        // Như tường lửa bỏ gói: hết thời gian mở kết nối phải báo "không kết nối được", không khuyên tăng timeout.
        let target = unresponsive().await;
        let mut provider = ollama(&target.url, Duration::from_secs(5));
        provider.client = http::build_client(Duration::from_secs(5), Duration::from_millis(300));
        let started = std::time::Instant::now();
        let (code, text) = failure(ask(&provider).await);
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "waited for the total timeout"
        );
        assert!(text.contains("Không kết nối được"), "{text}");
        assert!(!text.contains("TIMEOUT_SECS"), "{text}");
        assert_eq!(code, 502);
    }

    #[tokio::test]
    async fn redirect_is_reported_not_followed() {
        let (elsewhere, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let url = spawn_redirect(format!("{elsewhere}/api/generate")).await;
        let (code, text) = failure(ask(&ollama(&url, Duration::from_secs(5))).await);
        assert!(text.contains("chuyển hướng"), "{text}");
        assert_eq!(code, 502);
        assert!(received.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn invalid_url_is_a_server_configuration_error() {
        let (code, text) = failure(ask(&ollama("localhost:11434", Duration::from_secs(5))).await);
        assert!(text.contains("không phải địa chỉ hợp lệ"), "{text}");
        assert_eq!(code, 503);
    }

    #[tokio::test]
    async fn slow_model_times_out() {
        let (url, _) = spawn(200, OK_BODY, Duration::from_secs(5)).await;
        let (code, text) = failure(ask(&ollama(&url, Duration::from_millis(200))).await);
        assert!(text.contains("OLLAMA_TIMEOUT_SECS"), "{text}");
        assert_eq!(code, 502);
    }

    #[test]
    fn log_url_has_no_credentials() {
        let provider = ollama(
            "http://admin:hunter2@ollama.lan:11434/",
            Duration::from_secs(5),
        );
        assert_eq!(provider.log_url, "http://ollama.lan:11434");
    }
}
