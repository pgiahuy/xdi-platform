// SPDX-License-Identifier: Apache-2.0
/* engine/llm/gemini.rs - gọi Gemini API (generateContent, REST, không stream) qua HTTPS.
Khóa lấy từ GEMINI_API_KEY và chỉ đi trong header x-goog-api-key, không đặt vào URL: lỗi của reqwest in URL đầy đủ,
khóa nằm trong query string thì sẽ theo lỗi vào log. Header đánh dấu nhạy cảm để reqwest không in nó khi debug.
Mọi chữ ghi vào log đi qua redact(), phòng khi Gemini trả lại khóa trong thông báo lỗi.
Có GEMINI_BACKUP_API_KEY thì khóa đầu hết hạn mức (429) chuyển sang khóa dự phòng ngay trong lần gọi đó: hạn mức theo
phút của khóa miễn phí hết giữa một câu hỏi (mỗi câu "tại sao" gọi mô hình nhiều lần) làm cả câu hỏi rơi về câu khuôn.
Lỗi khác (khóa sai, mô hình không có, máy chủ lỗi) không đổi khóa: khóa khác cũng gặp đúng lỗi đó. */

use std::time::Duration;

use async_trait::async_trait;
use reqwest::header::HeaderValue;
use serde::Deserialize;

use super::{http, LlmProvider, LlmReply, LlmRequest, TokenUsage};
use crate::{config::Secret, error::AppError};

/// Địa chỉ gốc của Gemini API. Test thay bằng địa chỉ server giả.
pub const API_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct Gemini {
    /// Địa chỉ đầy đủ của generateContent cho mô hình đã chọn.
    endpoint: String,
    /// Khóa chính rồi khóa dự phòng, chỉ các khóa không rỗng.
    api_keys: Vec<Secret>,
    model: String,
    timeout: Duration,
    /// Lỗi dựng client (máy không có chứng chỉ CA) giữ lại để báo khi có câu hỏi, không làm backend dừng.
    client: Result<reqwest::Client, String>,
}

impl Gemini {
    pub fn new(base_url: &str, api_keys: Vec<Secret>, model: String, timeout: Duration) -> Self {
        let client = http::build_client(timeout, http::CONNECT_TIMEOUT);
        if let Err(err) = &client {
            tracing::error!(error = %err, "cannot build HTTP client for Gemini");
        }
        Self {
            endpoint: format!(
                "{}/models/{model}:generateContent",
                base_url.trim_end_matches('/')
            ),
            api_keys: api_keys.into_iter().filter(|k| !k.is_empty()).collect(),
            model,
            timeout,
            client,
        }
    }

    /// Thay mọi khóa bằng [redacted] trong chữ sắp ghi vào log.
    fn redact(&self, text: &str) -> String {
        self.api_keys.iter().fold(text.to_string(), |text, key| {
            text.replace(key.expose(), "[redacted]")
        })
    }

    /// Lỗi khi gửi hoặc nhận (không có response, hoặc đứt giữa chừng).
    fn transport_error(&self, err: reqwest::Error) -> AppError {
        // Hết thời gian mở kết nối thì reqwest bật cả is_connect lẫn is_timeout. Xét is_connect trước, nếu không
        // mạng chặn Google sẽ bị báo thành Gemini trả lời chậm, và lời khuyên tăng timeout không giúp được gì.
        let (connect, timed_out) = (err.is_connect(), err.is_timeout());
        tracing::warn!(error = %self.redact(&http::describe(err)), "gemini request failed");
        let message = if connect {
            "Không kết nối được tới Gemini: kiểm tra kết nối Internet, tường lửa và proxy (HTTPS_PROXY) của máy chủ"
                .to_string()
        } else if timed_out {
            format!(
                "Gemini không trả lời trong {} giây. Thử lại, hoặc tăng GEMINI_TIMEOUT_SECS",
                self.timeout.as_secs()
            )
        } else {
            "Kết nối tới Gemini bị ngắt giữa chừng. Thử lại; lặp lại thì xem log của backend"
                .to_string()
        };
        AppError::Upstream(message)
    }

    /// Lỗi Gemini trả về bằng mã HTTP. Chi tiết chỉ ghi vào log; người dùng nhận lời hướng dẫn.
    fn status_error(&self, status: reqwest::StatusCode, body: &str) -> AppError {
        let detail = serde_json::from_str::<ErrorResponse>(body)
            .map(|response| response.error)
            .unwrap_or_default();
        tracing::warn!(
            status = status.as_u16(),
            gemini_status = %detail.status,
            message = %self.redact(&detail.message),
            "gemini returned an error"
        );
        let message = match status.as_u16() {
            // Khóa sai thì Gemini trả 400 kèm lý do API_KEY_INVALID, không phải 401.
            400 if detail.is_invalid_key() => key_rejected(status),
            401 | 403 => key_rejected(status),
            300..=399 => format!(
                "Gemini trả về chuyển hướng (mã {}); backend không đi theo để khóa không bị gửi sang máy khác. Kiểm tra proxy của máy chủ",
                status.as_u16()
            ),
            404 => format!(
                "Gemini không có mô hình {}: kiểm tra GEMINI_MODEL",
                self.model
            ),
            429 => "Đã hết hạn mức gọi Gemini của khóa này. Chờ một lúc rồi thử lại, hoặc xem hạn mức trong Google AI Studio".to_string(),
            500..=599 => format!(
                "Gemini đang lỗi hoặc quá tải (mã {}). Thử lại sau",
                status.as_u16()
            ),
            code => format!("Gemini từ chối yêu cầu (mã {code}), xem log của backend"),
        };
        AppError::Upstream(message)
    }
}

fn key_rejected(status: reqwest::StatusCode) -> String {
    format!(
        "Gemini từ chối khóa API (mã {}): kiểm tra GEMINI_API_KEY",
        status.as_u16()
    )
}

#[async_trait]
impl LlmProvider for Gemini {
    fn name(&self) -> String {
        self.model.clone()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.complete_with_usage(request)
            .await
            .map(|reply| reply.text)
    }

    async fn complete_with_usage(&self, request: &LlmRequest<'_>) -> Result<LlmReply, AppError> {
        if self.api_keys.is_empty() {
            return Err(AppError::Unavailable(
                "Thiếu khóa Gemini: đặt GEMINI_API_KEY trong .env rồi khởi động lại backend"
                    .to_string(),
            ));
        }
        let client = self.client.as_ref().map_err(|_| {
            AppError::Unavailable(
                "Máy chủ không tạo được kết nối HTTPS tới Gemini, xem log của backend".to_string(),
            )
        })?;
        let mut body = serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": request.prompt }] }]
        });
        // responseJsonSchema nhận JSON Schema chuẩn (Gemini 2.5 trở lên), nên dùng được đúng schema gửi cho Ollama.
        // responseSchema cũ chỉ nhận một phần của OpenAPI 3.0 và phải viết schema riêng cho Gemini.
        if let Some(schema) = &request.json_schema {
            body["generationConfig"] = serde_json::json!({
                "responseMimeType": "application/json",
                "responseJsonSchema": schema,
            });
        }
        for (index, api_key) in self.api_keys.iter().enumerate() {
            // Lỗi của from_str không chứa giá trị header, nên không lộ khóa.
            let mut key = HeaderValue::from_str(api_key.expose()).map_err(|_| {
                AppError::Unavailable("Khóa Gemini chứa ký tự không hợp lệ".to_string())
            })?;
            key.set_sensitive(true);
            let response = client
                .post(&self.endpoint)
                .header("x-goog-api-key", key)
                .json(&body)
                .send()
                .await
                .map_err(|err| self.transport_error(err))?;
            let status = response.status();
            // Đọc body dạng chữ trước rồi mới parse: body lỗi cũng cần đọc, và timeout lúc đang nhận body vẫn là lỗi
            // đường truyền.
            let text = response
                .text()
                .await
                .map_err(|err| self.transport_error(err))?;
            let has_next = index + 1 < self.api_keys.len();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && has_next {
                tracing::warn!(
                    key_index = index,
                    "gemini key is out of quota, trying the backup key"
                );
                continue;
            }
            if !status.is_success() {
                return Err(self.status_error(status, &text));
            }
            return build_answer(&text);
        }
        unreachable!("the loop returns on the last key")
    }
}

/// Lấy văn bản trả lời và số token từ body của generateContent.
fn build_answer(body: &str) -> Result<LlmReply, AppError> {
    let response: GenerateResponse = serde_json::from_str(body).map_err(|err| {
        // Chỉ ghi lỗi parse, không ghi body: body có thể chứa dữ liệu của người dùng.
        tracing::warn!(error = %err, "gemini response is not valid JSON of the expected shape");
        AppError::Upstream(
            "Gemini trả về dữ liệu không đọc được. Thử lại; lặp lại thì xem log của backend"
                .to_string(),
        )
    })?;
    // Bị chặn vì nội dung câu hỏi: gửi lại y nguyên vẫn bị chặn, nên là lỗi của request (400).
    if let Some(reason) = response.prompt_feedback.and_then(|f| f.block_reason) {
        return Err(AppError::BadRequest(format!(
            "Gemini từ chối trả lời câu này (lý do {reason}). Thử diễn đạt lại câu hỏi"
        )));
    }
    let usage = response.usage_metadata.map(|u| TokenUsage {
        input_tokens: u.prompt_token_count,
        output_tokens: u.candidates_token_count + u.thoughts_token_count,
    });
    let Some(candidate) = response.candidates.into_iter().next() else {
        tracing::warn!("gemini returned no candidate");
        return Err(AppError::Upstream(
            "Gemini không trả về câu trả lời nào. Thử lại".to_string(),
        ));
    };
    // Chỉ STOP (hoặc không ghi lý do) là trả lời xong. MAX_TOKENS là câu trả lời bị cắt: SQL cụt mà đi tiếp thì người
    // dùng nhận lỗi cú pháp khó hiểu ở bước sau. Lý do khác (SAFETY, RECITATION...) là mô hình dừng giữa chừng.
    match candidate.finish_reason.as_deref().unwrap_or("STOP") {
        "STOP" => {}
        "MAX_TOKENS" => {
            tracing::warn!("gemini answer was cut at MAX_TOKENS");
            return Err(AppError::Upstream(
                "Câu trả lời của Gemini bị cắt vì vượt giới hạn độ dài (MAX_TOKENS). Thử lại hoặc hỏi ngắn gọn hơn"
                    .to_string(),
            ));
        }
        other => {
            tracing::warn!(finish_reason = other, "gemini stopped before finishing");
            return Err(AppError::Upstream(format!(
                "Gemini dừng trả lời giữa chừng (lý do {other}). Thử diễn đạt lại câu hỏi"
            )));
        }
    }
    // Phần "thought" là suy nghĩ nháp của mô hình (chỉ có khi bật includeThoughts), không phải câu trả lời.
    let text: String = candidate
        .content
        .map(|content| content.parts)
        .unwrap_or_default()
        .into_iter()
        .filter(|part| !part.thought)
        .filter_map(|part| part.text)
        .collect();
    if text.trim().is_empty() {
        tracing::warn!("gemini returned no text");
        return Err(AppError::Upstream(
            "Gemini không trả về nội dung. Thử lại hoặc diễn đạt lại câu hỏi".to_string(),
        ));
    }
    Ok(LlmReply { text, usage })
}

// Các struct dưới đây chỉ khai những trường cần đọc; serde bỏ qua trường khác như modelVersion.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    prompt_feedback: Option<PromptFeedback>,
    usage_metadata: Option<UsageMetadata>,
}

/// Số token Gemini đếm cho lần gọi. thoughtsTokenCount chỉ có khi mô hình suy nghĩ nháp trước khi trả lời; Google
/// tính tiền phần đó như token trả lời.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    #[serde(default)]
    prompt_token_count: u64,
    #[serde(default)]
    candidates_token_count: u64,
    #[serde(default)]
    thoughts_token_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<Content>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    text: Option<String>,
    #[serde(default)]
    thought: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptFeedback {
    block_reason: Option<String>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize, Default)]
struct ErrorDetail {
    #[serde(default)]
    message: String,
    /// Mã dạng chữ của Google, ví dụ INVALID_ARGUMENT, RESOURCE_EXHAUSTED.
    #[serde(default)]
    status: String,
    #[serde(default)]
    details: Vec<serde_json::Value>,
}

impl ErrorDetail {
    fn is_invalid_key(&self) -> bool {
        self.details
            .iter()
            .any(|d| d.get("reason").and_then(|r| r.as_str()) == Some("API_KEY_INVALID"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::llm::http::test_support::{
        capture_logs, closed_url, spawn, spawn_redirect, unresponsive,
    };
    use crate::engine::llm::LlmTask;

    const KEY: &str = "AIzaTEST-not-a-real-key-123";
    const PROMPT: &str = "Viết SQL: tổng doanh thu theo năm";

    // Body lấy theo định dạng response thật của generateContent và của lỗi Google API.
    const OK_BODY: &str = r#"{"candidates":[{"content":{"parts":[{"text":"SELECT 1"}],"role":"model"},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":3,"totalTokenCount":15},"modelVersion":"gemini-2.5-flash","responseId":"abc123"}"#;
    const INVALID_KEY_BODY: &str = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID","domain":"googleapis.com","metadata":{"service":"generativelanguage.googleapis.com"}}]}}"#;

    // Các test ở đây chạy lần lượt. tracing nhớ sẵn cho mỗi chỗ ghi log là có ai nghe hay không; test khác ghi cùng
    // dòng log ở luồng khác đúng lúc key_never_reaches_logs_or_errors đặt bộ nhận log thì có thể ghi nhớ sai, và log
    // của test đó bị mất (đã gặp khi chạy song song).
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn gemini(base_url: &str, key: &str, timeout: Duration) -> Gemini {
        Gemini::new(
            base_url,
            vec![Secret::new(key)],
            "gemini-test".to_string(),
            timeout,
        )
    }

    async fn ask(provider: &Gemini) -> Result<String, AppError> {
        let request = LlmRequest {
            task: LlmTask::Sql,
            question: "doanh thu theo năm",
            prompt: PROMPT,
            json_schema: None,
        };
        provider.complete(&request).await
    }

    /// Khóa chính hết hạn mức (429) thì cùng lần gọi đó chuyển sang khóa dự phòng; máy chủ giả trả 429 cho khóa chính và
    /// trả lời bình thường cho khóa dự phòng.
    #[tokio::test]
    async fn backup_key_is_used_when_the_first_is_out_of_quota() {
        let _serial = SERIAL.lock().await;
        let app = axum::Router::new().fallback(|headers: axum::http::HeaderMap| async move {
            let key = headers
                .get("x-goog-api-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if key == KEY {
                (
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED"}}"#,
                )
            } else {
                (axum::http::StatusCode::OK, OK_BODY)
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("addr"));
        tokio::spawn(async move { axum::serve(listener, app).await });
        let two_keys = Gemini::new(
            &url,
            vec![Secret::new(KEY), Secret::new("AIzaTEST-backup-key")],
            "gemini-test".to_string(),
            Duration::from_secs(5),
        );
        assert_eq!(ask(&two_keys).await.unwrap(), "SELECT 1");
        // Chỉ có khóa chính thì vẫn báo hết hạn mức như trước.
        let (_, message) = failure(ask(&gemini(&url, KEY, Duration::from_secs(5))).await);
        assert!(message.contains("hạn mức"), "{message}");
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
    async fn sends_prompt_with_key_in_header() {
        let _serial = SERIAL.lock().await;
        let (url, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let answer = ask(&gemini(&url, KEY, Duration::from_secs(5))).await;
        assert_eq!(answer.expect("answer"), "SELECT 1");

        let received = received.lock().unwrap().clone().expect("request");
        assert_eq!(received.path, "/models/gemini-test:generateContent");
        assert_eq!(received.query, None, "key must not be in the URL");
        assert_eq!(received.headers["x-goog-api-key"], KEY);
        assert_eq!(received.body["contents"][0]["parts"][0]["text"], PROMPT);
        // Không yêu cầu JSON thì không gửi generationConfig, để mô hình trả văn bản tự do như trước.
        assert!(received.body.get("generationConfig").is_none());
    }

    #[tokio::test]
    async fn sends_json_schema_when_asked() {
        let _serial = SERIAL.lock().await;
        let (url, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let schema =
            serde_json::json!({"type": "object", "properties": {"sql": {"type": "string"}}});
        let request = LlmRequest {
            task: LlmTask::Sql,
            question: "doanh thu theo năm",
            prompt: PROMPT,
            json_schema: Some(schema.clone()),
        };
        gemini(&url, KEY, Duration::from_secs(5))
            .complete(&request)
            .await
            .expect("answer");

        let received = received.lock().unwrap().clone().expect("request");
        let config = &received.body["generationConfig"];
        assert_eq!(config["responseMimeType"], "application/json");
        assert_eq!(config["responseJsonSchema"], schema);
    }

    #[tokio::test]
    async fn reads_token_counts() {
        let _serial = SERIAL.lock().await;
        const BODY: &str = r#"{"candidates":[{"content":{"parts":[{"text":"SELECT 1"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":3,"thoughtsTokenCount":5,"totalTokenCount":20}}"#;
        let (url, _) = spawn(200, BODY, Duration::ZERO).await;
        let request = LlmRequest {
            task: LlmTask::Sql,
            question: "doanh thu theo năm",
            prompt: PROMPT,
            json_schema: None,
        };
        let reply = gemini(&url, KEY, Duration::from_secs(5))
            .complete_with_usage(&request)
            .await
            .expect("answer");
        let usage = reply.usage.expect("usage");
        assert_eq!((usage.input_tokens, usage.output_tokens), (12, 8));
    }

    #[tokio::test]
    async fn skips_thought_parts() {
        let _serial = SERIAL.lock().await;
        const BODY: &str = r#"{"candidates":[{"content":{"parts":[{"text":"cần nhóm theo năm","thought":true},{"text":"SELECT 2"}],"role":"model"},"finishReason":"STOP"}]}"#;
        let (url, _) = spawn(200, BODY, Duration::ZERO).await;
        let answer = ask(&gemini(&url, KEY, Duration::from_secs(5))).await;
        assert_eq!(answer.expect("answer"), "SELECT 2");
    }

    #[tokio::test]
    async fn errors_become_clear_messages() {
        let _serial = SERIAL.lock().await;
        // Mã response của server giả, body, đoạn chữ phải có trong lời báo, mã HTTP backend trả.
        let cases: &[(u16, &str, &str, u16)] = &[
            (400, INVALID_KEY_BODY, "GEMINI_API_KEY", 502),
            (
                403,
                r#"{"error":{"code":403,"message":"Method doesn't allow unregistered callers.","status":"PERMISSION_DENIED"}}"#,
                "GEMINI_API_KEY",
                502,
            ),
            (
                429,
                r#"{"error":{"code":429,"message":"You exceeded your current quota.","status":"RESOURCE_EXHAUSTED"}}"#,
                "hạn mức",
                502,
            ),
            (
                404,
                r#"{"error":{"code":404,"message":"models/gemini-test is not found for API version v1beta.","status":"NOT_FOUND"}}"#,
                "GEMINI_MODEL",
                502,
            ),
            (
                503,
                r#"{"error":{"code":503,"message":"The model is overloaded. Please try again later.","status":"UNAVAILABLE"}}"#,
                "Thử lại sau",
                502,
            ),
            (502, "<html>Bad Gateway</html>", "mã 502", 502),
            (
                400,
                r#"{"error":{"code":400,"message":"Invalid JSON payload received.","status":"INVALID_ARGUMENT"}}"#,
                "mã 400",
                502,
            ),
            (200, "<html>proxy error</html>", "không đọc được", 502),
            (200, r#"{"candidates":"oops"}"#, "không đọc được", 502),
            (
                200,
                r#"{"promptFeedback":{"blockReason":"PROHIBITED_CONTENT"}}"#,
                "PROHIBITED_CONTENT",
                400,
            ),
            (
                200,
                r#"{"candidates":[]}"#,
                "không trả về câu trả lời nào",
                502,
            ),
            (200, "{}", "không trả về câu trả lời nào", 502),
            // Câu trả lời bị cắt hay bị dừng giữa chừng không được trả như thành công, kể cả khi đã có chữ.
            (
                200,
                r#"{"candidates":[{"content":{"parts":[{"text":"SELECT a, b FROM t WHERE"}],"role":"model"},"finishReason":"MAX_TOKENS"}]}"#,
                "bị cắt",
                502,
            ),
            (
                200,
                r#"{"candidates":[{"content":{"parts":[{"text":"SELECT 1"}],"role":"model"},"finishReason":"SAFETY"}]}"#,
                "SAFETY",
                502,
            ),
            (
                200,
                r#"{"candidates":[{"content":{"parts":[{"text":" "}],"role":"model"},"finishReason":"STOP"}]}"#,
                "không trả về nội dung.",
                502,
            ),
        ];
        for (status, body, expected, http_status) in cases {
            let (url, _) = spawn(*status, *body, Duration::ZERO).await;
            let (code, text) = failure(ask(&gemini(&url, KEY, Duration::from_secs(5))).await);
            assert!(text.contains(expected), "{status} {body}: {text}");
            assert!(!text.contains("lý do )"), "{status} {body}: {text}");
            assert_eq!(code, *http_status, "{status} {body}: {text}");
        }
    }

    #[tokio::test]
    async fn missing_key_is_reported_without_calling() {
        let _serial = SERIAL.lock().await;
        let (url, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let (code, text) = failure(ask(&gemini(&url, "", Duration::from_secs(5))).await);
        assert!(text.contains("GEMINI_API_KEY"), "{text}");
        assert_eq!(code, 503);
        assert!(received.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn slow_response_times_out() {
        let _serial = SERIAL.lock().await;
        let (url, _) = spawn(200, OK_BODY, Duration::from_secs(5)).await;
        let (code, text) = failure(ask(&gemini(&url, KEY, Duration::from_millis(200))).await);
        assert!(text.contains("GEMINI_TIMEOUT_SECS"), "{text}");
        assert_eq!(code, 502);
    }

    #[tokio::test]
    async fn connection_error_is_reported() {
        let _serial = SERIAL.lock().await;
        let url = closed_url().await;
        let (code, text) = failure(ask(&gemini(&url, KEY, Duration::from_secs(5))).await);
        assert!(text.contains("Không kết nối được"), "{text}");
        assert_eq!(code, 502);
    }

    #[tokio::test]
    async fn unanswered_connect_is_not_reported_as_slow_model() {
        let _serial = SERIAL.lock().await;
        // Như tường lửa bỏ gói: hết thời gian mở kết nối phải báo "không kết nối được", không khuyên tăng timeout.
        let target = unresponsive().await;
        let mut provider = gemini(&target.url, KEY, Duration::from_secs(5));
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
    async fn redirect_is_not_followed() {
        let _serial = SERIAL.lock().await;
        // Máy khác mà response 307 trỏ tới không được nhận request nào, tức không nhận khóa.
        let (elsewhere, received) = spawn(200, OK_BODY, Duration::ZERO).await;
        let url = spawn_redirect(format!("{elsewhere}/steal")).await;
        let (code, text) = failure(ask(&gemini(&url, KEY, Duration::from_secs(5))).await);
        assert!(text.contains("chuyển hướng"), "{text}");
        assert_eq!(code, 502);
        assert!(received.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn key_never_reaches_logs_or_errors() {
        let _serial = SERIAL.lock().await;
        let (_guard, logs) = capture_logs();

        // Lỗi kết nối: lỗi reqwest là chỗ hay in URL.
        let url = closed_url().await;
        let connect = ask(&gemini(&url, KEY, Duration::from_secs(5))).await;
        // Gemini trả lại khóa trong thông báo lỗi: redact() phải che.
        let echo_body = format!(
            r#"{{"error":{{"code":400,"message":"bad key {KEY}","status":"INVALID_ARGUMENT"}}}}"#
        );
        let (url, _) = spawn(400, echo_body, Duration::ZERO).await;
        let echoed = ask(&gemini(&url, KEY, Duration::from_secs(5))).await;
        // Gọi thành công: log mức trace của reqwest và hyper cũng không được có khóa.
        let (url, _) = spawn(200, OK_BODY, Duration::ZERO).await;
        ask(&gemini(&url, KEY, Duration::from_secs(5)))
            .await
            .expect("answer");

        let logs = logs.text();
        assert!(logs.contains("gemini request failed"), "{logs}");
        // Log của reqwest cũng phải vào bộ đệm, nếu không phần kiểm log của thư viện HTTP không còn nghĩa.
        assert!(logs.contains("reqwest::connect"), "{logs}");
        assert!(logs.contains("bad key [redacted]"), "{logs}");
        assert!(!logs.contains(KEY), "{logs}");
        for result in [connect, echoed] {
            let error = format!("{:?}", result.expect_err("error"));
            assert!(!error.contains(KEY), "{error}");
        }
    }
}
