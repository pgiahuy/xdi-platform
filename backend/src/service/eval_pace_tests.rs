// SPDX-License-Identifier: Apache-2.0
/* service/eval_pace_tests.rs - ca test của lớp giãn cách lần gọi mô hình khi đánh giá (service/eval_pace.rs), với một
provider giả trả lỗi hết hạn mức theo kịch bản: hai lần gọi cách nhau đủ khoảng nghỉ, kể cả khi gọi song song; hết hạn
mức thì chờ rồi gọi lại; gọi lại có trần; lỗi khác trả về ngay; thời gian chờ được cộng vào thời hạn. Dùng đồng hồ thật
với khoảng vài chục mili giây vì tokio của crate không bật test-util. */

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::eval_pace::{build_wait_budget, is_quota, Pace, PaceStats, Paced};
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask, GEMINI_QUOTA_MESSAGE};
use crate::error::AppError;

/// Provider giả: `quota_errors` lần đầu trả lỗi hết hạn mức, sau đó trả "ok"; `other_error` thì luôn trả lỗi khác.
struct Scripted {
    quota_errors: u32,
    other_error: bool,
    calls: AtomicU32,
}

impl Scripted {
    fn new(quota_errors: u32) -> Self {
        Self {
            quota_errors,
            other_error: false,
            calls: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl LlmProvider for Scripted {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.other_error {
            return Err(AppError::Upstream("model overloaded".to_string()));
        }
        if n < self.quota_errors {
            return Err(AppError::Upstream(GEMINI_QUOTA_MESSAGE.to_string()));
        }
        Ok("ok".to_string())
    }
}

fn request() -> LlmRequest<'static> {
    LlmRequest {
        task: LlmTask::Sql,
        question: "q",
        prompt: "p",
        json_schema: None,
    }
}

fn pace(pause_ms: u64, wait_ms: u64, retries: u32) -> Pace {
    Pace {
        pause: Duration::from_millis(pause_ms),
        quota_wait: Duration::from_millis(wait_ms),
        quota_retries: retries,
    }
}

fn paced(inner: Arc<Scripted>, pace: Pace) -> (Paced, Arc<PaceStats>) {
    let stats = Arc::new(PaceStats::default());
    (Paced::new(inner, pace, Arc::clone(&stats)), stats)
}

#[tokio::test]
async fn calls_are_spaced_by_the_pause() {
    let (paced, stats) = paced(Arc::new(Scripted::new(0)), pace(60, 0, 0));
    let started = Instant::now();
    for _ in 0..3 {
        assert_eq!(paced.complete(&request()).await.unwrap(), "ok");
    }
    // Lần đầu gửi ngay, hai lần sau mỗi lần chờ một khoảng nghỉ.
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(stats.calls(), 3);
}

#[tokio::test]
async fn parallel_calls_queue_behind_each_other() {
    // Bước lập hồ sơ gửi ba prompt cùng lúc: chúng vẫn phải bắt đầu cách nhau một khoảng nghỉ.
    let (paced, stats) = paced(Arc::new(Scripted::new(0)), pace(60, 0, 0));
    let started = Instant::now();
    let r = request();
    let (a, b, c) = tokio::join!(paced.complete(&r), paced.complete(&r), paced.complete(&r));
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(stats.calls(), 3);
}

#[tokio::test]
async fn out_of_quota_waits_then_calls_again() {
    let inner = Arc::new(Scripted::new(2));
    let (paced, stats) = paced(Arc::clone(&inner), pace(0, 40, 3));
    let started = Instant::now();
    assert_eq!(paced.complete(&request()).await.unwrap(), "ok");
    assert!(
        started.elapsed() >= Duration::from_millis(80),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(inner.calls.load(Ordering::SeqCst), 3);
    assert_eq!((stats.calls(), stats.quota_waits()), (3, 2));
}

#[tokio::test]
async fn calling_again_stops_after_the_retries() {
    // Hết hạn mức theo ngày không hồi: sau quota_retries lần gọi lại, lỗi trả về người gọi.
    let inner = Arc::new(Scripted::new(u32::MAX));
    let (paced, stats) = paced(Arc::clone(&inner), pace(0, 5, 2));
    let error = paced.complete(&request()).await.unwrap_err();
    assert!(is_quota(&error));
    assert_eq!(inner.calls.load(Ordering::SeqCst), 3);
    assert_eq!(stats.quota_waits(), 2);
}

#[tokio::test]
async fn other_errors_are_returned_at_once() {
    let inner = Arc::new(Scripted {
        other_error: true,
        ..Scripted::new(0)
    });
    let (paced, stats) = paced(Arc::clone(&inner), pace(0, 1000, 3));
    let error = paced.complete(&request()).await.unwrap_err();
    assert!(!is_quota(&error));
    assert_eq!(inner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(stats.quota_waits(), 0);
}

#[test]
fn wait_budget_covers_the_queue_and_every_quota_wait() {
    // Hai lượt lập hồ sơ song song, mỗi lượt ba prompt, cộng chính lần gọi này: 7 khoảng nghỉ; cộng 3 lần chờ.
    let budget = build_wait_budget(pace(1000, 60_000, 3), 2);
    assert_eq!(budget, Duration::from_secs(7 + 180));
    // Mock không giãn cách: thời hạn giữ nguyên.
    assert_eq!(build_wait_budget(pace(0, 60_000, 0), 2), Duration::ZERO);
}
