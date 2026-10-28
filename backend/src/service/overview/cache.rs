// SPDX-License-Identifier: Apache-2.0
/* service/overview/cache.rs - kết quả các câu SQL của trang tổng quan (service/overview/query.rs), giữ trong bộ nhớ
(AppState::overview_cache) để mở lại khung tổng quan không chạy lại câu trên DuckDB.

Khóa là phiên bản cộng chính câu chạy (GuardedSql::runnable). Câu chạy đã gồm đường dẫn Parquet của phiên bản và điều
kiện lọc dòng của vai dữ liệu, nên hai người khác vai không bao giờ dùng chung một kết quả, và kết quả không bao giờ cũ:
file của một phiên bản không đổi sau khi nạp. Đổi chỉ số hay hồ sơ thì câu khác đi, thành khóa mới.

Không dùng bảng Postgres: kết quả tính lại được bất cứ lúc nào từ Parquet, khởi động lại chỉ mất lần chạy đầu, và không
phải thêm migration. */

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::service::ask::contract::ResultTable;

/// Số kết quả giữ tối đa, cũ nhất bị bỏ trước. Mỗi dataset cần ba câu cho mỗi vai dữ liệu; một kết quả tối đa 1000 dòng
/// ba cột. 256 chọn ở đây, chưa đo.
pub const OVERVIEW_CACHE_ENTRIES: usize = 256;

#[derive(Default)]
pub struct OverviewCache {
    // Mutex chuẩn là đủ: không giữ khóa qua await, mỗi lần chỉ tìm hay thêm trong danh sách ngắn.
    entries: Mutex<VecDeque<Entry>>,
}

struct Entry {
    version_id: Uuid,
    runnable: String,
    // Arc để lấy ra không phải chép cả bảng; người gọi chép khi cần sửa.
    result: Arc<ResultTable>,
}

impl OverviewCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, version_id: Uuid, runnable: &str) -> Option<Arc<ResultTable>> {
        let entries = self.entries.lock().ok()?;
        entries
            .iter()
            .find(|e| e.version_id == version_id && e.runnable == runnable)
            .map(|e| Arc::clone(&e.result))
    }

    pub fn put(&self, version_id: Uuid, runnable: String, result: Arc<ResultTable>) {
        // Mutex bị đầu độc (một luồng panic khi đang giữ) thì bỏ qua cache: lần sau chạy lại câu, không sai số.
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|e| !(e.version_id == version_id && e.runnable == runnable));
        entries.push_back(Entry {
            version_id,
            runnable,
            result,
        });
        while entries.len() > OVERVIEW_CACHE_ENTRIES {
            entries.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(total_rows: u64) -> Arc<ResultTable> {
        Arc::new(ResultTable {
            columns: Vec::new(),
            rows: Vec::new(),
            total_rows,
            truncated: false,
            limit_reached: false,
        })
    }

    #[test]
    fn keys_on_version_and_runnable_sql() {
        let cache = OverviewCache::new();
        let version = Uuid::new_v4();
        cache.put(version, "SELECT 1".to_string(), table(1));
        assert_eq!(cache.get(version, "SELECT 1").unwrap().total_rows, 1);
        assert!(cache.get(version, "SELECT 2").is_none());
        assert!(cache.get(Uuid::new_v4(), "SELECT 1").is_none());
    }

    #[test]
    fn drops_the_oldest_entry_past_the_cap() {
        let cache = OverviewCache::new();
        let version = Uuid::new_v4();
        for i in 0..=OVERVIEW_CACHE_ENTRIES {
            cache.put(version, format!("SELECT {i}"), table(i as u64));
        }
        assert!(cache.get(version, "SELECT 0").is_none());
        assert!(cache.get(version, "SELECT 1").is_some());
    }
}
