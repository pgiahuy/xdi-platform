// SPDX-License-Identifier: Apache-2.0
/* service/eval_pace.rs - giãn cách các lần gọi mô hình khi chạy bộ đánh giá (#202). Paced bọc provider thật như
engine/llm/meter.rs bọc để đếm: người gọi (bước phân loại, viết SQL, điền form, viết lời, lập hồ sơ ngữ nghĩa) vẫn gọi
complete, không biết mình đang được giãn cách. service/eval.rs và cli.rs dựng AppState của lượt đánh giá bằng
build_paced_state.

Vì sao cần: runner gọi liên tục, khóa Gemini miễn phí có hạn mức theo phút (15 lần với gemini-3.5-flash-lite). Lần
chạy thử ngày 31/10 hết hạn mức ngay ở bước lập hồ sơ 12 bảng (khoảng 36 lần gọi dồn trong chưa tới một phút), rồi cả
5 câu nhận lỗi hết hạn mức và bị tính trượt dù mô hình chưa trả lời câu nào. Hai việc:
- Hai lần gọi liên tiếp bắt đầu cách nhau ít nhất `pause`, kể cả các lần gọi chạy song song của bước lập hồ sơ.
- Mọi khóa đều hết hạn mức (Gemini đã tự thử khóa dự phòng, engine/llm/gemini.rs) thì chờ `quota_wait` rồi gọi lại,
  tối đa `quota_retries` lần. Hạn mức theo phút hồi sau dưới một phút; hết hạn mức theo ngày thì chờ mấy cũng không hồi,
  nên số lần gọi lại có trần và lỗi cuối cùng vẫn trả về người gọi.

Thời gian chờ nằm trong lời gọi, mà bước phân loại và các bước viết lời có thời hạn riêng (classify_timeout,
gemini_timeout qua build_narrate_timeout): hết thời hạn thì bước phân loại rơi về luật từ khóa, lời rơi về câu của code,
và số đo sai mà không ai biết. build_paced_state vì thế nới hai thời hạn đó ra thêm đúng phần có thể phải chờ. Thời
hạn của chính lời gọi HTTP (engine/llm/http.rs) không đổi, vì client đã dựng sẵn. */

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::engine::llm::{is_quota_error, LlmProvider, LlmReply, LlmRequest};
use crate::error::AppError;
use crate::state::AppState;

/// Cách giãn cách của một lượt đánh giá.
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    /// Khoảng tối thiểu giữa lúc bắt đầu hai lần gọi liên tiếp; 0 là không giãn cách (mock).
    pub pause: Duration,
    /// Chờ bao lâu khi mọi khóa đều hết hạn mức, trước khi gọi lại.
    pub quota_wait: Duration,
    /// Số lần gọi lại tối đa của một lần gọi gặp hết hạn mức.
    pub quota_retries: u32,
}

/// Số đếm của một lượt đánh giá, đọc được sau khi chạy xong. Arc để Paced và người đọc cùng trỏ tới một chỗ.
#[derive(Debug, Default)]
pub struct PaceStats {
    /// Số lần gửi yêu cầu tới provider, kể cả các lần gọi lại sau khi hết hạn mức.
    pub calls: AtomicU64,
    /// Số lần phải chờ vì hết hạn mức.
    pub quota_waits: AtomicU64,
}

impl PaceStats {
    pub fn calls(&self) -> u64 {
        self.calls.load(Ordering::Relaxed)
    }

    pub fn quota_waits(&self) -> u64 {
        self.quota_waits.load(Ordering::Relaxed)
    }
}

/// Provider bọc ngoài provider thật, giãn cách và gọi lại khi hết hạn mức.
pub struct Paced {
    inner: Arc<dyn LlmProvider>,
    pace: Pace,
    /// Lúc sớm nhất lần gọi kế tiếp được bắt đầu. Mutex của tokio vì khóa được giữ qua lúc chờ: các lần gọi song song
    /// xếp hàng ở đây, lần sau chỉ bắt đầu khi lần trước đã nhích mốc này lên.
    next_start: Mutex<Instant>,
    stats: Arc<PaceStats>,
}

impl Paced {
    pub fn new(inner: Arc<dyn LlmProvider>, pace: Pace, stats: Arc<PaceStats>) -> Self {
        Self {
            inner,
            pace,
            next_start: Mutex::new(Instant::now()),
            stats,
        }
    }

    /// Chờ tới lượt mình rồi đặt mốc cho lần gọi sau.
    async fn wait_turn(&self) {
        let mut next = self.next_start.lock().await;
        tokio::time::sleep_until(*next).await;
        *next = Instant::now() + self.pace.pause;
    }

    /// Hết hạn mức thì mọi lần gọi khác cũng sẽ gặp đúng lỗi đó: đẩy mốc của cả hàng lùi lại, không riêng lần này.
    async fn wait_quota(&self) {
        self.stats.quota_waits.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            wait_ms = self.pace.quota_wait.as_millis() as u64,
            "evaluation: every Gemini key is out of quota, waiting before calling again"
        );
        let mut next = self.next_start.lock().await;
        *next = (*next).max(Instant::now() + self.pace.quota_wait);
    }

    async fn call(&self, request: &LlmRequest<'_>) -> Result<LlmReply, AppError> {
        let mut retries = 0;
        loop {
            self.wait_turn().await;
            self.stats.calls.fetch_add(1, Ordering::Relaxed);
            match self.inner.complete_with_usage(request).await {
                Err(error) if is_quota(&error) && retries < self.pace.quota_retries => {
                    retries += 1;
                    self.wait_quota().await;
                }
                result => return result,
            }
        }
    }
}

/// Lỗi hết hạn mức của mọi khóa Gemini. Ollama và mock không có hạn mức.
pub fn is_quota(error: &AppError) -> bool {
    is_quota_error(error)
}

#[async_trait]
impl LlmProvider for Paced {
    fn name(&self) -> String {
        self.inner.name()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.call(request).await.map(|reply| reply.text)
    }

    async fn complete_with_usage(&self, request: &LlmRequest<'_>) -> Result<LlmReply, AppError> {
        self.call(request).await
    }
}

/// AppState của lượt đánh giá: llm đã bọc Paced, và thời hạn của bước phân loại và các bước viết lời nới ra thêm phần
/// có thể phải chờ (xem đầu file). Trả về cả số đếm để in sau khi chạy.
pub fn build_paced_state(state: &AppState, pace: Pace) -> (AppState, Arc<PaceStats>) {
    let stats = Arc::new(PaceStats::default());
    let paced = Paced::new(Arc::clone(&state.llm), pace, Arc::clone(&stats));
    let mut config = (*state.config).clone();
    let waiting = build_wait_budget(pace, state.config.semantic_jobs);
    config.classify_timeout += waiting;
    config.gemini_timeout += waiting;
    config.ollama_timeout += waiting;
    let paced_state = AppState {
        config: Arc::new(config),
        llm: Arc::new(paced),
        ..state.clone()
    };
    (paced_state, stats)
}

/// Thời gian dài nhất một lần gọi có thể phải chờ trước khi được gửi: xếp sau các lần gọi lập hồ sơ đang chạy cùng
/// lúc (mỗi lượt lập hồ sơ ba prompt song song, service/semantic/parts.rs), cộng mọi lần chờ hết hạn mức.
pub fn build_wait_budget(pace: Pace, semantic_jobs: u32) -> Duration {
    let queued = pace.pause * (3 * semantic_jobs.max(1) + 1);
    queued + pace.quota_wait * pace.quota_retries
}
