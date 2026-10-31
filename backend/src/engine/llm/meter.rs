// SPDX-License-Identifier: Apache-2.0
/* engine/llm/meter.rs - đo từng lần gọi mô hình (#184): loại việc, thời gian, đạt hay lỗi, số token khi nhà cung cấp
có báo. Metered bọc provider thật và ghi mỗi lần gọi vào một CallLog; mỗi câu hỏi (service/ask/mod.rs) và mỗi lượt
lập hồ sơ ngữ nghĩa (service/semantic/mod.rs) có CallLog riêng, nên số đếm không lẫn giữa các request chạy cùng lúc.
Mỗi lần gọi cũng ghi một dòng log "llm call" để đọc được từ log máy chủ mà không cần mở lịch sử câu hỏi.

Metered cũng là một LlmProvider, nên người gọi (summary.rs, classify.rs...) không phải đổi gì: họ vẫn gọi complete. */

use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;
use utoipa::ToSchema;

use super::{LlmProvider, LlmReply, LlmRequest, LlmTask};
use crate::error::AppError;

/// One call to the language model, in the order the calls finished.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LlmCall {
    pub task: LlmTask,
    /// Wall time from sending the request to reading the whole answer, in milliseconds.
    pub duration_ms: u64,
    /// False when the provider returned an error, or when the caller stopped waiting (its own timeout).
    pub ok: bool,
    /// Prompt tokens as counted by the provider; null when it did not say (mock, an error, a cancelled call).
    #[schema(required)]
    pub input_tokens: Option<u64>,
    /// Answer tokens as counted by the provider, thinking tokens included; null when it did not say.
    #[schema(required)]
    pub output_tokens: Option<u64>,
}

/// Danh sách lần gọi của một câu hỏi hay một lượt lập hồ sơ. Clone chỉ chép Arc: Metered và người đọc kết quả cùng
/// trỏ tới một danh sách. Mutex chuẩn là đủ vì không giữ khóa qua await.
#[derive(Clone)]
pub struct CallLog {
    calls: Arc<Mutex<Vec<LlmCall>>>,
    /// Lúc bắt đầu câu hỏi hay lượt lập hồ sơ, để tính tổng thời gian ở cùng chỗ với các lần gọi.
    started: Instant,
}

impl CallLog {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            started: Instant::now(),
        }
    }

    /// Bản chép các lần gọi đã xong tới lúc này.
    pub fn calls(&self) -> Vec<LlmCall> {
        // Mutex chỉ hỏng (poisoned) khi một luồng panic lúc đang giữ khóa; danh sách vẫn đọc được.
        match self.calls.lock() {
            Ok(calls) => calls.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Mili giây từ lúc tạo CallLog.
    pub fn elapsed_ms(&self) -> u64 {
        millis(self.started)
    }

    fn push(&self, call: LlmCall) {
        tracing::info!(
            task = ?call.task,
            duration_ms = call.duration_ms,
            ok = call.ok,
            input_tokens = call.input_tokens,
            output_tokens = call.output_tokens,
            "llm call"
        );
        match self.calls.lock() {
            Ok(mut calls) => calls.push(call),
            Err(poisoned) => poisoned.into_inner().push(call),
        }
    }
}

impl Default for CallLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Tổng của một danh sách lần gọi, cho dòng log cuối câu hỏi hay cuối lượt lập hồ sơ.
pub struct CallTotals {
    pub calls: usize,
    /// Tổng thời gian các lần gọi. Các lần gọi chạy song song thì tổng này lớn hơn thời gian chờ thật.
    pub model_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub fn build_call_totals(calls: &[LlmCall]) -> CallTotals {
    CallTotals {
        calls: calls.len(),
        model_ms: calls.iter().map(|c| c.duration_ms).sum(),
        input_tokens: calls.iter().filter_map(|c| c.input_tokens).sum(),
        output_tokens: calls.iter().filter_map(|c| c.output_tokens).sum(),
    }
}

/// Provider bọc ngoài provider thật, ghi mỗi lần gọi vào log.
pub struct Metered {
    inner: Arc<dyn LlmProvider>,
    log: CallLog,
}

impl Metered {
    pub fn new(inner: Arc<dyn LlmProvider>, log: CallLog) -> Self {
        Self { inner, log }
    }
}

/// Một lần gọi đang chờ. Người gọi bọc lời gọi trong tokio::time::timeout (summary.rs, classify.rs): hết giờ thì
/// future bị hủy giữa chừng và dòng sau .await không bao giờ chạy. Drop vẫn chạy khi future bị hủy, nên lần gọi đó
/// vẫn được ghi (ok false, không có token) thay vì biến mất khỏi số đếm.
struct PendingCall<'a> {
    log: &'a CallLog,
    task: LlmTask,
    started: Instant,
    finished: bool,
}

impl PendingCall<'_> {
    fn finish(mut self, result: &Result<LlmReply, AppError>) {
        self.finished = true;
        let usage = result.as_ref().ok().and_then(|reply| reply.usage);
        self.log.push(LlmCall {
            task: self.task,
            duration_ms: millis(self.started),
            ok: result.is_ok(),
            input_tokens: usage.map(|u| u.input_tokens),
            output_tokens: usage.map(|u| u.output_tokens),
        });
    }
}

impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.log.push(LlmCall {
                task: self.task,
                duration_ms: millis(self.started),
                ok: false,
                input_tokens: None,
                output_tokens: None,
            });
        }
    }
}

#[async_trait]
impl LlmProvider for Metered {
    fn name(&self) -> String {
        self.inner.name()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.complete_with_usage(request)
            .await
            .map(|reply| reply.text)
    }

    async fn complete_with_usage(&self, request: &LlmRequest<'_>) -> Result<LlmReply, AppError> {
        let pending = PendingCall {
            log: &self.log,
            task: request.task,
            started: Instant::now(),
            finished: false,
        };
        let result = self.inner.complete_with_usage(request).await;
        pending.finish(&result);
        result
    }
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::engine::llm::TokenUsage;

    struct Counted;

    #[async_trait]
    impl LlmProvider for Counted {
        fn name(&self) -> String {
            "counted".to_string()
        }

        async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
            Ok("ok".to_string())
        }

        async fn complete_with_usage(
            &self,
            request: &LlmRequest<'_>,
        ) -> Result<LlmReply, AppError> {
            if request.question == "slow" {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            if request.question == "fail" {
                return Err(AppError::Upstream("down".to_string()));
            }
            Ok(LlmReply {
                text: "ok".to_string(),
                usage: Some(TokenUsage {
                    input_tokens: 120,
                    output_tokens: 8,
                }),
            })
        }
    }

    fn request(question: &str) -> LlmRequest<'_> {
        LlmRequest {
            task: LlmTask::Summary,
            question,
            prompt: "prompt",
            json_schema: None,
        }
    }

    #[tokio::test]
    async fn records_every_call_with_tokens() {
        let log = CallLog::new();
        let metered = Metered::new(Arc::new(Counted), log.clone());
        assert_eq!(metered.complete(&request("a")).await.unwrap(), "ok");
        assert!(metered.complete(&request("fail")).await.is_err());

        let calls = log.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].ok);
        assert_eq!(calls[0].input_tokens, Some(120));
        assert_eq!(calls[0].output_tokens, Some(8));
        assert!(!calls[1].ok);
        assert_eq!(calls[1].input_tokens, None);
        let totals = build_call_totals(&calls);
        assert_eq!((totals.calls, totals.input_tokens), (2, 120));
    }

    #[tokio::test]
    async fn a_call_cut_by_the_caller_timeout_is_still_counted() {
        let log = CallLog::new();
        let metered = Metered::new(Arc::new(Counted), log.clone());
        let slow = request("slow");
        let waited = tokio::time::timeout(Duration::from_millis(50), metered.complete(&slow)).await;
        assert!(waited.is_err());

        let calls = log.calls();
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].ok);
        assert_eq!(calls[0].output_tokens, None);
    }
}
