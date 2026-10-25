// SPDX-License-Identifier: Apache-2.0
/* state.rs - trạng thái dùng chung của cả backend, đưa vào Axum một lần.
Không biến toàn cục: mọi thứ handler cần đều lấy từ đây. DuckDB (Analytics) thêm khi có issue nạp dữ liệu. */

use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;

use crate::config::Config;
use crate::engine::llm::{self, LlmProvider};
use crate::engine::store::Store;
use crate::service::ask_values::ValueCache;

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
    /// Tối đa hai câu SQL của câu hỏi chạy trên DuckDB cùng lúc (engine/query.rs): mỗi câu có trần 1 GB cho phần bộ
    /// nhớ DuckDB quản lý (không chặn được mọi câu, xem đầu engine/query.rs). Câu sau chờ, không bị từ chối.
    // Endpoint hỏi (#33) truyền semaphore này vào service/ask_run.rs; gỡ allow khi đó.
    #[allow(dead_code)]
    pub query_jobs: Arc<Semaphore>,
    /// Giá trị đã chuẩn hóa của các phiên bản dataset vừa được hỏi (service/ask_values.rs), để mỗi câu hỏi không phải
    /// tải và bỏ dấu lại mọi giá trị.
    pub value_cache: Arc<ValueCache>,
}

impl AppState {
    pub fn new(config: Config, store: Store) -> Self {
        let llm = llm::from_config(&config);
        Self {
            config: Arc::new(config),
            llm,
            store,
            setup_code: Arc::new(Mutex::new(None)),
            excel_jobs: Arc::new(Semaphore::new(1)),
            query_jobs: Arc::new(Semaphore::new(2)),
            value_cache: Arc::new(ValueCache::new()),
        }
    }
}
