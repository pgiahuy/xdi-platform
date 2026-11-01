// SPDX-License-Identifier: Apache-2.0
/* state.rs - trạng thái dùng chung của cả backend, đưa vào Axum một lần.
Không biến toàn cục: mọi thứ handler cần đều lấy từ đây. DuckDB (Analytics) thêm khi có issue nạp dữ liệu. */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use crate::config::Config;
use crate::engine::llm::meter::{CallLog, Metered};
use crate::engine::llm::{self, LlmProvider};
use crate::engine::store::Store;
use crate::service::ask::values::ValueCache;
use crate::service::overview::cache::OverviewCache;
use crate::service::sample_questions::load_sample_questions;
use crate::service::semantic::quota::QuotaGate;

#[derive(Clone)]
pub struct AppState {
    // Arc để mỗi request giữ một bản trỏ tới cùng dữ liệu, không chép cả struct.
    pub config: Arc<Config>,
    pub llm: Arc<dyn LlmProvider>,
    pub store: Store,
    /// Mã thiết lập lần đầu: có khi lúc khởi động chưa có tài khoản nào, về None khi đã tạo tài khoản quản trị.
    /// Mutex chuẩn là đủ vì không giữ khóa qua await.
    pub setup_code: Arc<Mutex<Option<String>>>,
    /// Chỉ một tiến trình con đọc file Excel chạy mỗi lúc (engine/excel_worker.rs): mỗi tiến trình được dùng tới 2 GB
    /// RAM, không để nhiều request cộng dồn. Request sau chờ request trước xong.
    pub excel_jobs: Arc<Semaphore>,
    /// Tối đa hai câu SQL (của câu hỏi hay của truy dòng nguồn) chạy trên DuckDB cùng lúc (engine/query.rs): mỗi câu
    /// có trần 1 GB cho phần bộ nhớ DuckDB quản lý (không chặn được mọi câu, xem đầu engine/query.rs). Câu sau chờ,
    /// không bị từ chối.
    pub query_jobs: Arc<Semaphore>,
    /// Giá trị đã chuẩn hóa của các phiên bản dataset vừa được hỏi (service/ask/values.rs), để mỗi câu hỏi không phải
    /// tải và bỏ dấu lại mọi giá trị.
    pub value_cache: Arc<ValueCache>,
    /// Tối đa SEMANTIC_JOBS lượt lập hồ sơ ngữ nghĩa (service/semantic/mod.rs) gọi mô hình cùng lúc: nạp nhiều file
    /// liền hay việc nền lúc khởi động cũng không dồn request lên nhà cung cấp (gói miễn phí dễ trả 429). Lượt sau chờ,
    /// không bị từ chối.
    pub semantic_jobs: Arc<Semaphore>,
    /// Nhịp gọi mô hình của lớp hồ sơ ngữ nghĩa sau khi provider báo hết hạn mức (service/semantic/quota.rs): chờ theo
    /// số giây Gemini dặn rồi gọi lại, và giãn các lần gọi của mọi lượt lập hồ sơ đang chạy.
    pub semantic_quota: Arc<QuotaGate>,
    /// Câu gợi ý chọn tay theo tên database mẫu (service/sample_questions.rs), đọc một lần lúc khởi động.
    pub sample_questions: Arc<HashMap<String, Vec<String>>>,
    /// Kết quả các câu SQL của trang tổng quan dataset theo phiên bản (service/overview/cache.rs), để mở lại khung tổng
    /// quan không chạy lại câu.
    pub overview_cache: Arc<OverviewCache>,
    /// Các lần gọi mô hình của một câu hỏi hay một lượt lập hồ sơ (engine/llm/meter.rs). None ở AppState chung của
    /// server; with_call_log dựng bản riêng cho từng câu hỏi, trong đó llm đã được bọc để ghi vào danh sách này.
    pub call_log: Option<CallLog>,
}

impl AppState {
    pub fn new(config: Config, store: Store) -> Self {
        let llm = llm::from_config(&config);
        let sample_questions = Arc::new(load_sample_questions(&config));
        let semantic_jobs = usize::try_from(config.semantic_jobs).unwrap_or(1);
        Self {
            config: Arc::new(config),
            llm,
            store,
            setup_code: Arc::new(Mutex::new(None)),
            excel_jobs: Arc::new(Semaphore::new(1)),
            query_jobs: Arc::new(Semaphore::new(2)),
            value_cache: Arc::new(ValueCache::new()),
            semantic_jobs: Arc::new(Semaphore::new(semantic_jobs)),
            semantic_quota: Arc::new(QuotaGate::new()),
            sample_questions,
            overview_cache: Arc::new(OverviewCache::new()),
            call_log: None,
        }
    }

    /// Bản chép của state cho một câu hỏi hay một lượt lập hồ sơ: mọi lần gọi mô hình đi qua state này được ghi vào
    /// log. Chép AppState rẻ vì mọi trường là Arc hay pool kết nối (cũng là Arc bên trong).
    pub fn with_call_log(&self, log: &CallLog) -> AppState {
        let metered = Metered::new(Arc::clone(&self.llm), log.clone());
        AppState {
            llm: Arc::new(metered),
            call_log: Some(log.clone()),
            ..self.clone()
        }
    }
}
