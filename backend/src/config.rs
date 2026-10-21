// SPDX-License-Identifier: Apache-2.0
/* config.rs - đọc cấu hình từ biến môi trường, có giá trị mặc định chạy được trên máy.
Mọi biến khai ở .env.example; thêm biến mới thì thêm ở cả hai nơi. */

use std::env;
use std::path::PathBuf;

use crate::engine::excel_worker::ExcelWorker;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    /// Số kết nối tối đa tới Postgres.
    pub db_max_connections: u32,
    /// Thư mục giữ file Parquet của mọi workspace và file đang nạp dở.
    pub data_dir: PathBuf,
    /// Tổng dung lượng tối đa của một lần nạp file, tính bằng byte (đặt bằng MAX_UPLOAD_MB).
    pub max_upload_bytes: u64,
    /// "mock", "gemini" hoặc "ollama". Mặc định mock để chạy được không cần khóa.
    pub llm_provider: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Thư mục giao diện đã build (frontend/dist). Không đặt thì backend chỉ phục vụ API.
    pub static_dir: Option<PathBuf>,
    /// Cách chạy bộ đọc Excel: chính binary này với tham số excel-worker (không phải biến môi trường).
    pub excel_worker: ExcelWorker,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            // Mặc định 8000 để compose, Makefile và tài liệu dùng chung một số.
            port: env::var("BACKEND_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8000),
            database_url: var_or("DATABASE_URL", "postgres://xdi:xdi@localhost:5433/xdi"),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            data_dir: PathBuf::from(var_or("DATA_DIR", "./data")),
            max_upload_bytes: upload_limit_bytes(env::var("MAX_UPLOAD_MB").ok().as_deref()),
            llm_provider: var_or("LLM_PROVIDER", "mock"),
            gemini_api_key: var_or("GEMINI_API_KEY", ""),
            gemini_model: var_or("GEMINI_MODEL", "gemini-2.5-flash"),
            ollama_url: var_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: var_or("OLLAMA_MODEL", "qwen2.5-coder:7b"),
            static_dir: env::var("STATIC_DIR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            excel_worker: ExcelWorker::Process(worker_binary()),
        }
    }
}

/// MAX_UPLOAD_MB ra byte. Giá trị không phải số dương hoặc lớn tới mức tràn số thì cảnh báo và dùng mặc định 200,
/// không để backend chết lúc khởi động hay lặng lẽ chặn mọi lần nạp.
fn upload_limit_bytes(raw: Option<&str>) -> u64 {
    const DEFAULT_MB: u64 = 200;
    let Some(raw) = raw else {
        return DEFAULT_MB * 1024 * 1024;
    };
    let parsed = raw
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|mb| *mb > 0)
        .and_then(|mb| mb.checked_mul(1024 * 1024));
    match parsed {
        Some(bytes) => bytes,
        None => {
            tracing::warn!(
                value = raw,
                default_mb = DEFAULT_MB,
                "invalid MAX_UPLOAD_MB, using default"
            );
            DEFAULT_MB * 1024 * 1024
        }
    }
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Binary chạy tiến trình con đọc Excel: chính binary đang chạy. Trên Linux dùng /proc/self/exe, trỏ đúng bản đang chạy
/// kể cả khi file trên đĩa đã bị thay (cài bản mới mà chưa khởi động lại), để hai bên luôn cùng một phiên bản.
fn worker_binary() -> PathBuf {
    if cfg!(target_os = "linux") {
        return PathBuf::from("/proc/self/exe");
    }
    env::current_exe().unwrap_or_else(|_| PathBuf::from("xdi-backend"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_limit_parsing() {
        assert_eq!(upload_limit_bytes(None), 200 * 1024 * 1024);
        assert_eq!(upload_limit_bytes(Some("5")), 5 * 1024 * 1024);
        for bad in ["abc", "0", "-1", "99999999999999"] {
            assert_eq!(upload_limit_bytes(Some(bad)), 200 * 1024 * 1024, "{bad}");
        }
    }
}
