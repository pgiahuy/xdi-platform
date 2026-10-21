// SPDX-License-Identifier: Apache-2.0
/* state.rs - trạng thái dùng chung của cả backend, đưa vào Axum một lần.
Không biến toàn cục: mọi thứ handler cần đều lấy từ đây. DuckDB (Analytics) thêm khi có issue nạp dữ liệu. */

use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::engine::llm::{self, LlmProvider};
use crate::engine::store::Store;

#[derive(Clone)]
pub struct AppState {
    // Arc để mỗi request giữ một bản trỏ tới cùng dữ liệu, không chép cả struct.
    pub config: Arc<Config>,
    pub llm: Arc<dyn LlmProvider>,
    pub store: Store,
    /// Mã thiết lập lần đầu: có khi lúc khởi động chưa có tài khoản nào, về None khi đã tạo tài khoản quản trị.
    /// Mutex chuẩn là đủ vì không giữ khóa qua await.
    pub setup_code: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new(config: Config, store: Store) -> Self {
        let llm = llm::from_config(&config);
        Self {
            config: Arc::new(config),
            llm,
            store,
            setup_code: Arc::new(Mutex::new(None)),
        }
    }
}
