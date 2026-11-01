// SPDX-License-Identifier: Apache-2.0
/* engine/duckdb_limits.rs - trần bộ nhớ của kết nối DuckDB khi nạp dữ liệu (analytics::materialize), lập hồ sơ cột
(profile::build_column_stats) và chụp bảng Postgres (service/sources/direct.rs). Không đặt trần thì DuckDB dùng tới 80%
RAM của máy: bảng 5 triệu dòng 60 cột từng đẩy backend lên 3,6 GiB ở bước nạp và hồ sơ cột (docs/PERFORMANCE.md, mục
Chụp bảng lớn). Trần đọc từ DUCKDB_MEMORY_LIMIT_MB (config.rs). Chạm trần thì DuckDB ghi phần dư ra một thư mục tạm
riêng của kết nối dưới DATA_DIR/duckdb-spill/, nên bảng lớn vẫn nạp được, chỉ chậm hơn.

Câu hỏi (engine/query.rs) có trần riêng, cố định, và không ghi ra đĩa. */

use std::path::{Path, PathBuf};

use duckdb::Connection;
use uuid::Uuid;

use super::analytics;
use crate::error::AppError;

/// Trần của mỗi kết nối DuckDB nạp dữ liệu hay lập hồ sơ cột. Mỗi kết nối một trần: hai lần nạp chạy cùng lúc thì
/// dùng tới gấp đôi.
#[derive(Debug, Clone)]
pub struct DuckdbLimits {
    /// Trần bộ nhớ DuckDB quản lý, tính bằng MiB.
    pub memory_mib: u32,
    /// Thư mục cha của các thư mục tạm (DATA_DIR/duckdb-spill/), mỗi kết nối một thư mục con.
    pub spill_root: PathBuf,
}

impl DuckdbLimits {
    pub fn new(memory_mib: u32, data_dir: &Path) -> Self {
        Self {
            memory_mib,
            spill_root: data_dir.join("duckdb-spill"),
        }
    }

    /// Xóa thư mục tạm còn sót khi backend chết giữa một lần nạp. Gọi lúc khởi động, khi chưa có kết nối nào; giả định
    /// mỗi DATA_DIR chỉ một backend dùng, như service/ingest.rs, clear_staging.
    pub fn clear_leftovers(&self) {
        match std::fs::remove_dir_all(&self.spill_root) {
            Ok(()) => {
                tracing::info!(dir = %self.spill_root.display(), "cleared leftover duckdb spill")
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(dir = %self.spill_root.display(), error = %e, "cannot clear duckdb spill")
            }
        }
    }
}

/// Kết nối DuckDB có trần bộ nhớ, kèm thư mục tạm của nó. Dùng qua trường conn.
pub struct LimitedConnection {
    // Rust bỏ các trường theo thứ tự khai báo: conn đóng trước (DuckDB xóa file tạm của nó), rồi mới tới spill xóa thư
    // mục. Đổi thứ tự thì thư mục bị xóa khi DuckDB còn đang dùng.
    pub conn: Connection,
    _spill: SpillDir,
}

/// Thư mục tạm của một kết nối; tự xóa khi ra khỏi phạm vi (Drop).
struct SpillDir {
    path: PathBuf,
}

impl Drop for SpillDir {
    fn drop(&mut self) {
        // Lỗi xóa chỉ ghi log: thư mục còn sót sẽ bị xóa ở lần khởi động sau.
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(dir = %self.path.display(), error = %e, "cannot remove duckdb spill dir");
            }
        }
    }
}

/// Mở kết nối như analytics::open, thêm trần bộ nhớ và thư mục tạm riêng.
pub fn open_limited(limits: &DuckdbLimits) -> Result<LimitedConnection, AppError> {
    let path = limits.spill_root.join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&path).map_err(|e| {
        AppError::Internal(format!("create duckdb spill dir {}: {e}", path.display()))
    })?;
    // Tạo spill ngay sau khi có thư mục: lỗi ở các bước dưới thì thư mục vẫn được xóa.
    let spill = SpillDir { path };
    let path_text = spill.path.to_str().ok_or_else(|| {
        AppError::Internal(format!("path is not valid UTF-8: {}", spill.path.display()))
    })?;
    let conn = analytics::open()?;
    conn.execute_batch(&format!(
        "SET memory_limit = '{}MiB'; SET temp_directory = {};",
        limits.memory_mib,
        analytics::sql_string(path_text)
    ))
    .map_err(|e| AppError::Internal(format!("limit duckdb memory: {e}")))?;
    Ok(LimitedConnection {
        conn,
        _spill: spill,
    })
}

/// Trần cho test: thư mục tạm của hệ điều hành, trần đủ cho các bảng nhỏ của test.
#[cfg(test)]
pub fn test_limits() -> DuckdbLimits {
    DuckdbLimits {
        memory_mib: 1024,
        spill_root: std::env::temp_dir().join("xdi-test-duckdb-spill"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(conn: &Connection, name: &str) -> String {
        conn.query_row(&format!("SELECT current_setting('{name}')"), [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn connection_has_the_limit_and_its_own_spill_dir() {
        let root = std::env::temp_dir().join(format!("xdi-spill-{}", Uuid::new_v4()));
        let limits = DuckdbLimits {
            memory_mib: 256,
            spill_root: root.clone(),
        };
        let first = open_limited(&limits).unwrap();
        let second = open_limited(&limits).unwrap();
        // DuckDB in trần theo đơn vị của nó (256 MiB thành "256.0 MiB").
        assert!(setting(&first.conn, "memory_limit").starts_with("256"));
        let first_dir = setting(&first.conn, "temp_directory");
        assert!(first_dir.starts_with(root.to_str().unwrap()));
        assert_ne!(first_dir, setting(&second.conn, "temp_directory"));
        drop(first);
        drop(second);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Câu cần nhiều hơn trần (gộp nhóm 3 triệu chuỗi khác nhau trong 128 MiB) chạy xong nhờ ghi phần dư ra thư mục tạm;
    /// cùng câu, cùng trần mà tắt thư mục tạm thì DuckDB báo hết bộ nhớ, tức là trần có tác dụng.
    #[test]
    fn work_over_the_limit_spills_to_disk() {
        let root = std::env::temp_dir().join(format!("xdi-spill-{}", Uuid::new_v4()));
        let limits = DuckdbLimits {
            memory_mib: 128,
            spill_root: root.clone(),
        };
        let sql = "SELECT count(*) FROM (SELECT s, count(*) FROM \
                   (SELECT repeat('x', 40) || range::VARCHAR AS s FROM range(3000000)) GROUP BY s)";
        // Hai luồng để lượng bộ nhớ cần không phụ thuộc số nhân của máy chạy test.
        let spilling = open_limited(&limits).unwrap();
        spilling.conn.execute_batch("SET threads = 2").unwrap();
        let count: i64 = spilling.conn.query_row(sql, [], |r| r.get(0)).unwrap();
        assert_eq!(count, 3_000_000);
        let no_disk = open_limited(&limits).unwrap();
        no_disk
            .conn
            .execute_batch("SET threads = 2; SET temp_directory = '';")
            .unwrap();
        let err = no_disk
            .conn
            .query_row(sql, [], |r| r.get::<_, i64>(0))
            .unwrap_err();
        assert!(err.to_string().contains("Out of Memory"), "{err}");
        drop(spilling);
        drop(no_disk);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
