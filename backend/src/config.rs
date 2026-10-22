// SPDX-License-Identifier: Apache-2.0
/* config.rs - đọc cấu hình từ biến môi trường, có giá trị mặc định chạy được trên máy.
Mọi biến khai ở .env.example; thêm biến mới thì thêm ở cả hai nơi. */

use std::env;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::engine::excel_worker::ExcelWorker;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    /// Số kết nối tối đa tới Postgres.
    pub db_max_connections: u32,
    /// Thời gian tối đa một request chờ lấy kết nối Postgres (DB_ACQUIRE_TIMEOUT_SECS).
    pub db_acquire_timeout: Duration,
    /// Địa chỉ người dùng mở giao diện, ví dụ https://du-lieu.example.com. Để trống khi chạy trên máy.
    pub public_url: String,
    /// Thư mục giữ file Parquet của mọi workspace và file đang nạp dở.
    pub data_dir: PathBuf,
    /// Tổng dung lượng tối đa của một lần nạp file, tính bằng byte (đặt bằng MAX_UPLOAD_MB).
    pub max_upload_bytes: u64,
    /// "mock", "gemini" hoặc "ollama". Mặc định mock để chạy được không cần khóa.
    pub llm_provider: String,
    pub gemini_api_key: Secret,
    pub gemini_model: String,
    /// Thời gian tối đa cho một lần gọi Gemini, kể cả lúc chờ mô hình trả lời.
    pub gemini_timeout: Duration,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Như gemini_timeout, nhưng mặc định dài hơn nhiều: mô hình chạy bằng CPU của máy thường chậm hơn Gemini nhiều
    /// lần, và lần gọi đầu còn phải nạp mô hình vào RAM.
    pub ollama_timeout: Duration,
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
            db_max_connections: positive_number(
                "DB_MAX_CONNECTIONS",
                env::var("DB_MAX_CONNECTIONS").ok().as_deref(),
                10,
            ),
            db_acquire_timeout: Duration::from_secs(u64::from(positive_number(
                "DB_ACQUIRE_TIMEOUT_SECS",
                env::var("DB_ACQUIRE_TIMEOUT_SECS").ok().as_deref(),
                5,
            ))),
            public_url: var_or("PUBLIC_URL", ""),
            data_dir: PathBuf::from(var_or("DATA_DIR", "./data")),
            max_upload_bytes: upload_limit_bytes(env::var("MAX_UPLOAD_MB").ok().as_deref()),
            llm_provider: var_or("LLM_PROVIDER", "mock"),
            gemini_api_key: Secret::new(var_or("GEMINI_API_KEY", "")),
            gemini_model: var_or("GEMINI_MODEL", "gemini-2.5-flash"),
            gemini_timeout: timeout_secs("GEMINI_TIMEOUT_SECS", 60),
            ollama_url: var_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: var_or("OLLAMA_MODEL", "qwen2.5-coder:7b"),
            ollama_timeout: timeout_secs("OLLAMA_TIMEOUT_SECS", 300),
            static_dir: env::var("STATIC_DIR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            excel_worker: ExcelWorker::Process(worker_binary()),
        }
    }

    /// Cookie phiên có cờ Secure khi người dùng vào qua HTTPS: trình duyệt không bao giờ gửi cookie đó qua HTTP thường,
    /// nên người nghe lén mạng không lấy được phiên. Không bật khi chạy http://localhost, vì trình duyệt sẽ bỏ cookie
    /// Secure trên trang HTTP và không đăng nhập được.
    pub fn secure_cookies(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    /// In cảnh báo về cấu hình chưa an toàn khi đưa lên mạng. Server gọi một lần lúc khởi động; không chặn khởi động,
    /// vì chạy trên máy để thử thì các giá trị mẫu là bình thường.
    pub fn log_warnings(&self) {
        if database_password(&self.database_url) == Some(SAMPLE_DB_PASSWORD) {
            tracing::warn!(
                "DATABASE_URL uses the sample Postgres password; fine on a local machine, but set POSTGRES_PASSWORD \
                 before putting this server on a network (docs/INSTALL.md, section 3)"
            );
        }
        if !self.public_url.is_empty()
            && !self.public_url.starts_with("https://")
            && !self.public_url.starts_with("http://")
        {
            tracing::warn!(
                value = %self.public_url,
                "PUBLIC_URL should start with https:// or http://; session cookies are sent without the Secure flag"
            );
        }
    }
}

/// Mật khẩu mẫu trong docker-compose.yml và .env.example.
const SAMPLE_DB_PASSWORD: &str = "xdi";

/// Mật khẩu trong DATABASE_URL dạng postgres://user:mat-khau@host/db, nếu có. Dạng khác (không có mật khẩu, hoặc
/// chuỗi key=value) trả None.
fn database_password(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    // Phần trước dấu / đầu tiên là user:mật-khẩu@máy:cổng. Tách ở @ cuối cùng: mật khẩu lỡ chứa @ chưa mã hóa thành
    // %40 thì vẫn tách đúng.
    let authority = rest.split(['/', '?']).next()?;
    let (user_info, _) = authority.rsplit_once('@')?;
    user_info.split_once(':').map(|(_, password)| password)
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

/// Biến phải là số nguyên dương (số kết nối, số giây). Không đặt thì dùng mặc định. Đặt sai (chữ, số âm, 0) thì cảnh
/// báo và dùng mặc định như MAX_UPLOAD_MB: 0 kết nối hay 0 giây chờ làm mọi request lỗi ngay mà không rõ vì sao.
fn positive_number(key: &str, raw: Option<&str>, default: u32) -> u32 {
    let Some(raw) = raw else {
        return default;
    };
    match raw.trim().parse::<u32>() {
        Ok(value) if value > 0 => value,
        _ => {
            tracing::warn!(
                variable = key,
                value = raw,
                default,
                "invalid value, expected a positive whole number; using default"
            );
            default
        }
    }
}

/// Timeout tính bằng giây từ biến môi trường, theo cùng quy tắc với positive_number: timeout 0 sẽ làm mọi lần gọi mô
/// hình thất bại ngay.
fn timeout_secs(key: &str, default: u32) -> Duration {
    Duration::from_secs(u64::from(positive_number(
        key,
        env::var(key).ok().as_deref(),
        default,
    )))
}

/// Giá trị bí mật như khóa API. Debug in "[redacted]" thay cho giá trị: Config derive Debug, nên lỡ ghi cả Config
/// vào log thì khóa cũng không lộ. Đọc giá trị thật bằng expose(), chỉ ở chỗ gửi khóa đi.
#[derive(Clone, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Biến không đặt hoặc đặt rỗng đều lấy mặc định. Compose đã tự làm vậy (dạng ${X:-mặc định}), nhưng chạy ngoài
/// compose với biến lấy từ .env chép từ .env.example (có dòng để trống như OLLAMA_URL=) thì backend nhận chuỗi rỗng;
/// coi rỗng là một giá trị thì OLLAMA_URL rỗng thay cho địa chỉ mặc định.
fn var_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
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
    fn database_password_parsing() {
        assert_eq!(
            database_password("postgres://xdi:xdi@localhost:5433/xdi"),
            Some("xdi")
        );
        assert_eq!(
            database_password("postgresql://xdi:s3cr%40t@postgres:5432/xdi?sslmode=disable"),
            Some("s3cr%40t")
        );
        assert_eq!(database_password("postgres://xdi@localhost/xdi"), None);
        assert_eq!(database_password("postgres://localhost/xdi"), None);
        assert_eq!(database_password("host=localhost user=xdi"), None);
    }

    #[test]
    fn secure_cookies_only_behind_https() {
        let mut config = Config::from_env();
        for (url, secure) in [
            ("https://du-lieu.example.com", true),
            ("http://localhost:8000", false),
            ("", false),
        ] {
            config.public_url = url.to_string();
            assert_eq!(config.secure_cookies(), secure, "{url}");
        }
    }

    #[test]
    fn positive_number_parsing() {
        assert_eq!(positive_number("X", None, 5), 5);
        assert_eq!(positive_number("X", Some(" 30 "), 5), 30);
        for bad in ["", "abc", "0", "-1", "1.5", "99999999999"] {
            assert_eq!(positive_number("X", Some(bad), 5), 5, "{bad}");
        }
    }

    #[test]
    fn upload_limit_parsing() {
        assert_eq!(upload_limit_bytes(None), 200 * 1024 * 1024);
        assert_eq!(upload_limit_bytes(Some("5")), 5 * 1024 * 1024);
        for bad in ["abc", "0", "-1", "99999999999999"] {
            assert_eq!(upload_limit_bytes(Some(bad)), 200 * 1024 * 1024, "{bad}");
        }
    }

    #[test]
    fn secret_is_hidden_in_debug() {
        let secret = Secret::new("AIza-test-secret");
        assert_eq!(secret.expose(), "AIza-test-secret");
        assert!(!format!("{secret:?}").contains("AIza"));
    }
}
