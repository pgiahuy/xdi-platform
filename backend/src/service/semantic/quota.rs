// SPDX-License-Identifier: Apache-2.0
/* service/semantic/quota.rs - nhịp gọi mô hình của lớp hồ sơ ngữ nghĩa khi provider báo hết hạn mức (Gemini 429).
parts.rs gọi wait_turn trước mỗi lần gọi một phần hồ sơ và note_quota khi lần gọi đó gặp lỗi hết hạn mức; AppState giữ
một QuotaGate chung (semantic_quota) cho mọi lượt lập hồ sơ đang chạy.

Vì sao cần: khóa Gemini miễn phí có hạn mức theo phút. Chụp hai database mẫu (13 bảng) gửi ba phần của mỗi bảng gần như
cùng lúc; lần chạy ngày 01/11 có 7 bảng gặp 429 ("Please retry in 24s"), phần lỗi bị bỏ và bảng ở lại `structure` tới
lần khởi động sau. Hai việc:
- Một lần gọi gặp hết hạn mức thì đẩy mốc của cả hàng lùi lại theo số giây Gemini dặn (có trần), không riêng lần đó:
  mọi lần gọi khác cũng sẽ gặp đúng lỗi đó.
- Sau lần hết hạn mức gần nhất, trong một khoảng `calm`, các lần gọi đi lần lượt, cách nhau ít nhất `spacing`, thay vì
  bắn hết cùng lúc. Qua khoảng đó mà không gặp lại thì gọi như thường.
Chưa gặp hết hạn mức lần nào thì wait_turn trả ngay: dùng mock hay Ollama không chậm đi. */

use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// Mặc định cho máy chủ thật. Khóa miễn phí của gemini-3.5-flash-lite cho khoảng 15 lần gọi một phút, nên 5 giây một
/// lần gọi còn chỗ cho câu hỏi của người dùng chạy cùng lúc.
const SPACING: Duration = Duration::from_secs(5);
const CALM: Duration = Duration::from_secs(120);
/// Chờ bao lâu khi Gemini không nói số giây, và trần của số giây Gemini nói (hết hạn mức theo ngày có thể dặn chờ hàng
/// giờ; chờ lâu như vậy thì thà để lần khởi động sau thử lại).
const DEFAULT_WAIT: Duration = Duration::from_secs(30);
const MAX_WAIT: Duration = Duration::from_secs(60);

/// Nhịp gọi chung, xem đầu file.
pub struct QuotaGate {
    timing: GateTiming,
    /// Mutex của tokio vì khóa được giữ qua lúc chờ: các lần gọi xếp hàng ở đây, lần sau chỉ đi khi lần trước đã nhích
    /// mốc lên.
    state: Mutex<GateState>,
}

/// Các khoảng thời gian của QuotaGate; test dùng khoảng rất ngắn.
#[derive(Debug, Clone, Copy)]
pub struct GateTiming {
    pub spacing: Duration,
    pub calm: Duration,
    pub default_wait: Duration,
    pub max_wait: Duration,
}

struct GateState {
    /// Lúc sớm nhất lần gọi kế tiếp được bắt đầu.
    next_start: Instant,
    /// Lúc gặp hết hạn mức gần nhất; None khi chưa gặp hay đã qua khoảng calm.
    last_quota: Option<Instant>,
}

impl QuotaGate {
    pub fn new() -> Self {
        Self::with_timing(GateTiming {
            spacing: SPACING,
            calm: CALM,
            default_wait: DEFAULT_WAIT,
            max_wait: MAX_WAIT,
        })
    }

    pub fn with_timing(timing: GateTiming) -> Self {
        Self {
            timing,
            state: Mutex::new(GateState {
                next_start: Instant::now(),
                last_quota: None,
            }),
        }
    }

    /// Chờ tới lượt mình trước khi gọi mô hình (xem đầu file).
    pub async fn wait_turn(&self) {
        let mut state = self.state.lock().await;
        let Some(last) = state.last_quota else {
            return;
        };
        if last.elapsed() > self.timing.calm {
            state.last_quota = None;
            return;
        }
        tokio::time::sleep_until(state.next_start).await;
        state.next_start = Instant::now() + self.timing.spacing;
    }

    /// Một lần gọi vừa gặp hết hạn mức; hint là số giây provider dặn chờ, nếu có. Trả về thời gian chờ đã chọn (để ghi
    /// log).
    pub async fn note_quota(&self, hint: Option<Duration>) -> Duration {
        let wait = hint
            .unwrap_or(self.timing.default_wait)
            .min(self.timing.max_wait);
        let mut state = self.state.lock().await;
        let now = Instant::now();
        state.last_quota = Some(now);
        state.next_start = state.next_start.max(now + wait);
        wait
    }
}

impl Default for QuotaGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short() -> GateTiming {
        GateTiming {
            spacing: Duration::from_millis(40),
            calm: Duration::from_secs(5),
            default_wait: Duration::from_millis(100),
            max_wait: Duration::from_millis(200),
        }
    }

    /// Chưa gặp hết hạn mức thì không chờ; gặp rồi thì lần gọi sau chờ hết số giây được dặn (có trần), các lần sau nữa
    /// cách nhau ít nhất spacing.
    #[tokio::test]
    async fn waits_only_after_a_quota_error() {
        let gate = QuotaGate::with_timing(short());
        let started = Instant::now();
        gate.wait_turn().await;
        gate.wait_turn().await;
        assert!(started.elapsed() < Duration::from_millis(30));

        // Lời dặn 10 giây bị hạ xuống trần 200 ms.
        let wait = gate.note_quota(Some(Duration::from_secs(10))).await;
        assert_eq!(wait, Duration::from_millis(200));
        let noted = Instant::now();
        gate.wait_turn().await;
        assert!(noted.elapsed() >= Duration::from_millis(190));
        let first = Instant::now();
        gate.wait_turn().await;
        assert!(first.elapsed() >= Duration::from_millis(35));
    }

    /// Không có lời dặn thì chờ default_wait.
    #[tokio::test]
    async fn missing_hint_uses_the_default_wait() {
        let gate = QuotaGate::with_timing(short());
        assert_eq!(gate.note_quota(None).await, Duration::from_millis(100));
    }
}
