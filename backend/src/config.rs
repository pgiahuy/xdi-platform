// SPDX-License-Identifier: Apache-2.0
/* config.rs - đọc cấu hình từ biến môi trường, có giá trị mặc định chạy được trên máy.
Mọi biến khai ở .env.example; thêm biến mới thì thêm ở cả hai nơi. */

use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    /// Số kết nối tối đa tới Postgres.
    pub db_max_connections: u32,
    // Khung: data_dir dùng từ issue nạp dữ liệu, jwt_secret từ issue auth. Gỡ allow khi có code đọc tới.
    #[allow(dead_code)]
    pub data_dir: String,
    #[allow(dead_code)]
    pub jwt_secret: String,
    /// "mock", "gemini" hoặc "ollama". Mặc định mock để chạy được không cần khóa.
    pub llm_provider: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Thư mục giao diện đã build (frontend/dist). Không đặt thì backend chỉ phục vụ API.
    pub static_dir: Option<PathBuf>,
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
            data_dir: var_or("DATA_DIR", "./data"),
            jwt_secret: var_or("JWT_SECRET", "change-me-before-going-public"),
            llm_provider: var_or("LLM_PROVIDER", "mock"),
            gemini_api_key: var_or("GEMINI_API_KEY", ""),
            gemini_model: var_or("GEMINI_MODEL", "gemini-2.5-flash"),
            ollama_url: var_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: var_or("OLLAMA_MODEL", "qwen2.5-coder:7b"),
            static_dir: env::var("STATIC_DIR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
