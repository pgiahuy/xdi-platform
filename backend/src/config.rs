// SPDX-License-Identifier: Apache-2.0
/* config.rs - đọc cấu hình từ biến môi trường, có giá trị mặc định chạy được trên máy.
Mọi biến khai ở .env.example; thêm biến mới thì thêm ở cả hai nơi. */

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    // Khung: dùng từ issue nạp dữ liệu và auth. Gỡ dòng allow này khi có code đọc tới.
    #[allow(dead_code)]
    pub database_url: String,
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
            data_dir: var_or("DATA_DIR", "./data"),
            jwt_secret: var_or("JWT_SECRET", "change-me-before-going-public"),
            llm_provider: var_or("LLM_PROVIDER", "mock"),
            gemini_api_key: var_or("GEMINI_API_KEY", ""),
            gemini_model: var_or("GEMINI_MODEL", "gemini-2.5-flash"),
            ollama_url: var_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: var_or("OLLAMA_MODEL", "qwen2.5-coder:7b"),
        }
    }
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
