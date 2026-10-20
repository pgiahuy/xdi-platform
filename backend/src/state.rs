// SPDX-License-Identifier: Apache-2.0
/* state.rs - trạng thái dùng chung của cả backend, đưa vào Axum một lần.
Không biến toàn cục: mọi thứ handler cần đều lấy từ đây. DuckDB (Analytics) thêm khi có issue nạp dữ liệu. */

use std::sync::Arc;

use crate::config::Config;
use crate::engine::llm::{self, LlmProvider};
use crate::engine::store::Store;

#[derive(Clone)]
pub struct AppState {
    // Arc để mỗi request giữ một bản trỏ tới cùng dữ liệu, không chép cả struct.
    // Khung: service đọc cấu hình từ issue nạp dữ liệu. Gỡ allow khi có code đọc tới.
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub llm: Arc<dyn LlmProvider>,
    // Khung: service dùng store từ issue tầng lưu trữ. Gỡ allow khi có code đọc tới.
    #[allow(dead_code)]
    pub store: Store,
}

impl AppState {
    pub fn new(config: Config, store: Store) -> Self {
        let llm = llm::from_config(&config);
        Self {
            config: Arc::new(config),
            llm,
            store,
        }
    }
}
