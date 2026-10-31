// SPDX-License-Identifier: Apache-2.0
/* service/ask/failures.rs - chỗ mô hình sai trong một câu hỏi, để ghi vào nhật ký lỗi (#228, service/lessons/). Các
bước dùng mô hình (ask/run.rs, explain/run.rs, ask/narrate.rs, ask/summary.rs) chỉ gom StepFailure vào kết quả của
chúng, không ghi Postgres: các bước đó chạy được trong test không cần database, và câu hỏi không bao giờ dừng vì ghi
nhật ký lỗi. ask/mod.rs và ask/why.rs ghi sau bước record, khi đã có query_id. */

/// Bước sai; chữ của as_db là giá trị cột error_log.step (migration 0013). Giá trị 'feedback' của cột đó không có ở đây:
/// dòng "Chưa đạt" ghi thẳng từ dòng lịch sử (Store::record_feedback_error), không qua luồng hỏi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedStep {
    /// Guard từ chối câu SQL của mô hình.
    Guard,
    /// Câu SQL qua guard nhưng DuckDB báo lỗi.
    Run,
    /// Câu cộng lặp khi nối bảng một-nhiều bị chặn (ask/fanout.rs).
    Fanout,
    /// Form câu "tại sao" sai (explain/check.rs).
    WhyForm,
    /// Bộ kiểm số hay kiểm chiều chặn lời mô hình viết.
    Narrate,
}

impl FailedStep {
    pub fn as_db(self) -> &'static str {
        match self {
            FailedStep::Guard => "guard",
            FailedStep::Run => "run",
            FailedStep::Fanout => "fanout",
            FailedStep::WhyForm => "why_form",
            FailedStep::Narrate => "narrate",
        }
    }
}

/// Một lần mô hình sai: bước, lời báo lỗi, và câu mô hình đã trả ở bước đó (SQL, form, lời viết).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFailure {
    pub step: FailedStep,
    pub error: String,
    pub model_output: Option<String>,
}

impl StepFailure {
    pub fn new(step: FailedStep, error: &str, model_output: Option<&str>) -> Self {
        StepFailure {
            step,
            error: error.to_string(),
            model_output: model_output.map(str::to_string),
        }
    }
}
