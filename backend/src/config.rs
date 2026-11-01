// SPDX-License-Identifier: Apache-2.0
/* config.rs - đọc cấu hình từ biến môi trường, có giá trị mặc định chạy được trên máy.
Mọi biến khai ở .env.example; thêm biến mới thì thêm ở cả hai nơi. */

use std::env;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::engine::duckdb_limits::DuckdbLimits;
use crate::engine::excel_worker::ExcelWorker;
use crate::sample_database::{build_sample_database, split_sample_urls, SampleDatabase};

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
    /// Các khóa Gemini dự phòng (GEMINI_BACKUP_API_KEY, cách nhau bằng dấu phẩy), dùng lần lượt khi khóa trước hết
    /// hạn mức; rỗng thì không có.
    pub gemini_backup_api_keys: Vec<Secret>,
    pub gemini_model: String,
    /// Mô hình dùng khi mọi khóa hết hạn mức với gemini_model (GEMINI_FALLBACK_MODEL, mặc định gemini-3.1-flash-lite);
    /// đặt rỗng thì không có.
    pub gemini_fallback_model: String,
    /// Thời gian tối đa cho một lần gọi Gemini, kể cả lúc chờ mô hình trả lời.
    pub gemini_timeout: Duration,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Như gemini_timeout, nhưng mặc định dài hơn nhiều: mô hình chạy bằng CPU của máy thường chậm hơn Gemini nhiều
    /// lần, và lần gọi đầu còn phải nạp mô hình vào RAM.
    pub ollama_timeout: Duration,
    /// Thời gian tối đa của bước phân loại câu hỏi (CLASSIFY_TIMEOUT_SECS, service/ask/classify.rs), ngắn hơn nhiều so
    /// với timeout của nhà cung cấp: bước này chạy trước bước viết SQL, mô hình treo thì câu hỏi chuyển sang luật từ
    /// khóa sau mức này thay vì chờ hết timeout rồi mới gọi tiếp.
    pub classify_timeout: Duration,
    /// Cửa sổ ngữ cảnh, tính bằng token, gửi trong options.num_ctx của mỗi request tới Ollama (OLLAMA_NUM_CTX).
    /// Không gửi thì Ollama dùng mặc định của nó (4096 token trên máy dưới 24 GiB VRAM), và prompt dài hơn bị bỏ phần
    /// đầu, tức phần luật, mà không báo lỗi.
    pub ollama_num_ctx: u32,
    /// Số lượt lập hồ sơ ngữ nghĩa (service/semantic/mod.rs) gọi mô hình cùng lúc (SEMANTIC_JOBS). Mỗi lượt gửi ba
    /// prompt song song, nên mặc định 2 là tối đa sáu request cùng lúc: gói miễn phí của Gemini giới hạn số request mỗi
    /// phút, đặt cao hơn thì dễ gặp lỗi 429 khi nạp nhiều bảng liền.
    pub semantic_jobs: u32,
    /// Thời gian tối đa một câu SQL của câu hỏi chạy trên DuckDB (QUERY_TIMEOUT_SECS); quá thì câu bị dừng, trừ phần
    /// DuckDB tính lúc chuẩn bị câu (engine/query.rs).
    pub query_timeout: Duration,
    /// Trần số dòng guard đặt cho SQL của câu hỏi (ASK_MAX_ROWS). Cao hơn nhiều số dòng gửi về giao diện (1000) để
    /// total_rows nói đúng kết quả có bao nhiêu dòng; chạm trần thì giao diện ghi "ít nhất N dòng" (limit_reached).
    pub ask_max_rows: u64,
    /// Thư mục giao diện đã build (frontend/dist). Không đặt thì backend chỉ phục vụ API.
    pub static_dir: Option<PathBuf>,
    /// Nạp dataset mẫu vào workspace đầu tiên lúc thiết lập lần đầu (SEED_SAMPLE, service/samples.rs).
    pub seed_sample: bool,
    /// Đoạn trả lời ngắn của câu thường dùng lời mô hình viết cho phần giải thích, khi lời đó qua bước kiểm (ANSWER_TEXT,
    /// service/ask/summary.rs). false: đoạn trả lời luôn là câu khuôn do code viết. Không có lần gọi mô hình riêng nào
    /// cho đoạn trả lời, và không dòng kết quả nào được gửi đi.
    pub answer_text: bool,
    /// Phần giải thích của câu thường có lời do mô hình viết lại từ các câu code đã viết (INSIGHT_TEXT,
    /// service/insight/narrate.rs, #227). false: không gọi mô hình, phần giải thích chỉ có câu do code viết.
    pub insight_text: bool,
    /// Thư mục chứa file mẫu (SAMPLES_DIR): data/samples của repo, compose gắn vào /samples.
    pub samples_dir: PathBuf,
    /// Các database mẫu mà thiết lập lần đầu thêm làm nguồn và chụp bảng, mỗi database một nguồn (SAMPLE_DATABASE_URL,
    /// nhiều chuỗi cách nhau bằng dấu phẩy, service/samples.rs). Rỗng: nạp dataco.csv. Compose trỏ vào sales_demo và
    /// edu_demo của dịch vụ demo-db (data/postgres-demo, profile demo).
    pub sample_databases: Vec<SampleDatabase>,
    /// Cách chạy bộ đọc Excel: chính binary này với tham số excel-worker (không phải biến môi trường).
    pub excel_worker: ExcelWorker,
    /// Khóa mã hóa thông tin kết nối của nguồn database (SOURCE_SECRET_KEY, 32 byte dạng base64). Biến rỗng thì main.rs
    /// đặt khóa sinh sẵn trong DATA_DIR (service/sources/key_file.rs); service/sources/secret.rs kiểm độ dài khóa.
    pub source_secret_key: Secret,
    /// Máy chủ database được phép thêm làm nguồn (SOURCE_ALLOWED_HOSTS, cách nhau bằng dấu phẩy). Rỗng thì không thêm
    /// được nguồn nào: backend không bị dùng để dò máy trong mạng nội bộ.
    pub source_allowed_hosts: Vec<String>,
    /// Trần số dòng một bảng khi chụp (SOURCE_MAX_ROWS); None (mặc định) là không có trần dòng. Bảng quá trần chỉ lưu
    /// metadata thay vì chụp thiếu (service/sources/plan.rs).
    pub source_max_rows: Option<u64>,
    /// Trần dung lượng (theo Postgres) một bảng khi chụp (SOURCE_TABLE_MAX_MB); bảng lớn hơn chỉ lưu metadata, xét trước
    /// khi chép nên không ghi file nào cho bảng đó.
    pub source_table_max_bytes: u64,
    /// Trần tổng dung lượng (theo Postgres) các bảng chép dữ liệu trong một lần chụp (SOURCE_SNAPSHOT_MAX_MB, #247); bảng
    /// không còn chỗ chỉ lưu metadata (service/sources/plan.rs).
    pub source_snapshot_max_bytes: u64,
    /// Khoảng giữa hai lần chụp lại mọi nguồn theo lịch (SOURCE_RESNAPSHOT_HOURS, #247); None là không có lịch.
    pub source_resnapshot_every: Option<Duration>,
    /// Thời gian tối đa một lần chụp bảng, kể cả kết nối (SOURCE_TIMEOUT_SECS).
    pub source_timeout: Duration,
    /// Cách chép dữ liệu bảng khi chụp (SOURCE_SNAPSHOT_MODE, service/sources/direct.rs, #246).
    pub source_snapshot_mode: SnapshotMode,
    /// File extension postgres của DuckDB, đúng bản DuckDB của binary (SOURCE_POSTGRES_EXTENSION). Chỉ dùng với
    /// SOURCE_SNAPSHOT_MODE=direct; thiếu file thì chụp bằng COPY.
    pub source_postgres_extension: PathBuf,
    /// Trần bộ nhớ của mỗi kết nối DuckDB khi nạp, lập hồ sơ cột và chụp bảng, MiB (DUCKDB_MEMORY_LIMIT_MB,
    /// engine/duckdb_limits.rs). Không áp cho câu hỏi: engine/query.rs có trần riêng.
    pub duckdb_memory_limit_mb: u32,
}

/// Cách chép dữ liệu một bảng nguồn ra file khi chụp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotMode {
    /// Postgres COPY ra CSV, DuckDB đọc CSV với kiểu cột lấy từ catalog rồi ghi Parquet (service/sources/postgres.rs).
    Copy,
    /// DuckDB đọc thẳng Postgres bằng extension postgres nạp từ file cục bộ, ghi Parquet một bước
    /// (service/sources/direct.rs). Extension không nạp được thì quay về Copy.
    Direct,
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
            gemini_backup_api_keys: split_keys(&var_or("GEMINI_BACKUP_API_KEY", "")),
            gemini_model: var_or("GEMINI_MODEL", "gemini-3.5-flash-lite"),
            // Khác var_or: đặt rỗng (GEMINI_FALLBACK_MODEL=) là tắt mô hình dự phòng, không đặt mới dùng mặc định.
            gemini_fallback_model: env::var("GEMINI_FALLBACK_MODEL")
                .unwrap_or_else(|_| "gemini-3.1-flash-lite".to_string()),
            gemini_timeout: timeout_secs("GEMINI_TIMEOUT_SECS", 60),
            ollama_url: var_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: var_or("OLLAMA_MODEL", "qwen2.5-coder:7b"),
            ollama_timeout: timeout_secs("OLLAMA_TIMEOUT_SECS", 300),
            classify_timeout: timeout_secs("CLASSIFY_TIMEOUT_SECS", 15),
            ollama_num_ctx: positive_number(
                "OLLAMA_NUM_CTX",
                env::var("OLLAMA_NUM_CTX").ok().as_deref(),
                OLLAMA_NUM_CTX_DEFAULT,
            ),
            semantic_jobs: positive_number(
                "SEMANTIC_JOBS",
                env::var("SEMANTIC_JOBS").ok().as_deref(),
                2,
            ),
            query_timeout: timeout_secs("QUERY_TIMEOUT_SECS", 30),
            ask_max_rows: u64::from(positive_number(
                "ASK_MAX_ROWS",
                env::var("ASK_MAX_ROWS").ok().as_deref(),
                100_000,
            )),
            static_dir: env::var("STATIC_DIR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            seed_sample: flag("SEED_SAMPLE", env::var("SEED_SAMPLE").ok().as_deref(), true),
            answer_text: flag("ANSWER_TEXT", env::var("ANSWER_TEXT").ok().as_deref(), true),
            insight_text: flag(
                "INSIGHT_TEXT",
                env::var("INSIGHT_TEXT").ok().as_deref(),
                true,
            ),
            // Mặc định tính từ thư mục backend/, nơi make dev chạy cargo run, như DATA_DIR=./data là backend/data.
            samples_dir: PathBuf::from(var_or("SAMPLES_DIR", "../data/samples")),
            excel_worker: ExcelWorker::Process(worker_binary()),
            source_secret_key: Secret::new(var_or("SOURCE_SECRET_KEY", "")),
            source_allowed_hosts: host_list(&var_or("SOURCE_ALLOWED_HOSTS", "")),
            source_max_rows: optional_limit(
                "SOURCE_MAX_ROWS",
                env::var("SOURCE_MAX_ROWS").ok().as_deref(),
            ),
            source_table_max_bytes: u64::from(positive_number(
                "SOURCE_TABLE_MAX_MB",
                env::var("SOURCE_TABLE_MAX_MB").ok().as_deref(),
                DEFAULT_SOURCE_TABLE_MAX_MB,
            )) * 1024
                * 1024,
            source_snapshot_max_bytes: u64::from(positive_number(
                "SOURCE_SNAPSHOT_MAX_MB",
                env::var("SOURCE_SNAPSHOT_MAX_MB").ok().as_deref(),
                DEFAULT_SOURCE_SNAPSHOT_MAX_MB,
            )) * 1024
                * 1024,
            source_resnapshot_every: resnapshot_every(
                env::var("SOURCE_RESNAPSHOT_HOURS").ok().as_deref(),
            ),
            source_timeout: timeout_secs("SOURCE_TIMEOUT_SECS", DEFAULT_SOURCE_TIMEOUT_SECS),
            source_snapshot_mode: snapshot_mode(env::var("SOURCE_SNAPSHOT_MODE").ok().as_deref()),
            source_postgres_extension: PathBuf::from(var_or(
                "SOURCE_POSTGRES_EXTENSION",
                DEFAULT_POSTGRES_EXTENSION,
            )),
            sample_databases: sample_databases(&var_or("SAMPLE_DATABASE_URL", "")),
            duckdb_memory_limit_mb: positive_number(
                "DUCKDB_MEMORY_LIMIT_MB",
                env::var("DUCKDB_MEMORY_LIMIT_MB").ok().as_deref(),
                DEFAULT_DUCKDB_MEMORY_LIMIT_MB,
            ),
        }
    }

    /// Trần của kết nối DuckDB khi nạp và lập hồ sơ cột; thư mục tạm nằm dưới DATA_DIR.
    pub fn duckdb_limits(&self) -> DuckdbLimits {
        DuckdbLimits::new(self.duckdb_memory_limit_mb, &self.data_dir)
    }

    /// Máy chủ được thêm làm nguồn: có trong SOURCE_ALLOWED_HOSTS (mọi cổng), hoặc đúng máy và cổng của
    /// SAMPLE_DATABASE_URL. Người cài đã chọn database mẫu, bắt thêm nó vào SOURCE_ALLOWED_HOSTS thì quên một biến là
    /// thiết lập lần đầu không chụp được. Chỉ cho đúng cổng đó: make dev đặt database mẫu ở 127.0.0.1:5440, cho cả máy
    /// 127.0.0.1 thì admin chụp được chính Postgres của ứng dụng (cổng 5433).
    pub fn source_host_allowed(&self, host: &str, port: u16) -> bool {
        let sample = self
            .sample_databases
            .iter()
            .any(|s| s.host == host && s.port == port);
        sample || self.source_allowed_hosts.iter().any(|h| h == host)
    }

    /// Lệnh đánh giá (`eval`, `eval-classify`, main.rs) gọi Gemini bằng khóa riêng `EVAL_GEMINI_API_KEY` khi có, để
    /// chạy cả bộ câu không ăn vào hạn mức lượt gọi của khóa mà ứng dụng đang dùng. Khóa riêng rỗng hay không đặt thì
    /// giữ `GEMINI_API_KEY`. Trả về true khi đã đổi khóa.
    pub fn apply_eval_gemini_key(&mut self, eval_key: Option<String>) -> bool {
        match eval_key.filter(|key| !key.trim().is_empty()) {
            Some(key) => {
                self.gemini_api_key = Secret::new(key.trim());
                true
            }
            None => false,
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
        // Đặt nhỏ hơn mặc định để tiết kiệm RAM là hợp lệ với dataset ít cột, nên chỉ cảnh báo: người quản trị cần biết
        // vì sao câu trả lời tệ đi khi prompt dài, vì Ollama cắt prompt mà không báo lỗi.
        if self.llm_provider == "ollama" && self.ollama_num_ctx < OLLAMA_NUM_CTX_DEFAULT {
            tracing::warn!(
                value = self.ollama_num_ctx,
                recommended = OLLAMA_NUM_CTX_DEFAULT,
                "OLLAMA_NUM_CTX is below the default: Ollama silently drops the start of longer prompts (the rules); \
                 raise OLLAMA_NUM_CTX if answers ignore the rules (docs/INSTALL.md, section 1)"
            );
        }
    }
}

/// Mặc định của OLLAMA_NUM_CTX. Prompt ở trần PROMPT_MAX_CHARS (24 000 ký tự) khoảng 7 200 đến 8 000 token theo tokenizer
/// của qwen2.5-coder, cộng prompt sửa và câu trả lời thì vượt 8192; 12 288 còn dư khoảng 3 000 token. Từ #230 prompt có
/// thể dài hơn trần đó: mọi cột luôn có metadata, tới 60 000 ký tự metadata (COLUMN_METADATA_MAX_CHARS) trước khi rút
/// gọn, cộng danh sách giá trị và phần luật thì cỡ 85 000 ký tự, ước chừng 25 000 token; schema vài trăm cột cần đặt
/// OLLAMA_NUM_CTX=32768. Ollama backend (engine/llm/ollama.rs) cảnh báo khi một prompt ước lượng vượt giá trị đã đặt.
/// Cách đo ở docs/INSTALL.md, mục 1.
const OLLAMA_NUM_CTX_DEFAULT: u32 = 12_288;

/// Mật khẩu mẫu trong docker-compose.yml và .env.example.
const SAMPLE_DB_PASSWORD: &str = "xdi";

/// Mỗi chuỗi của SAMPLE_DATABASE_URL (cách nhau bằng dấu phẩy) một database mẫu. Chuỗi sai dạng thì cảnh báo và bỏ
/// riêng chuỗi đó, không làm backend chết lúc khởi động; không còn chuỗi nào thì thiết lập lần đầu nạp dataco.csv.
fn sample_databases(raw: &str) -> Vec<SampleDatabase> {
    let mut databases = Vec::new();
    for (index, part) in split_sample_urls(raw).into_iter().enumerate() {
        match build_sample_database(part) {
            Ok(Some(db)) => databases.push(db),
            Ok(None) => {}
            // Ghi vị trí, không ghi chuỗi: chuỗi có mật khẩu.
            Err(why) => {
                tracing::warn!(%why, index, "invalid entry in SAMPLE_DATABASE_URL, skipped")
            }
        }
    }
    databases
}

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

/// Trần không bắt buộc (SOURCE_MAX_ROWS): không đặt hay để trống là không có trần; số nguyên dương là trần. Giá trị
/// khác (chữ, số âm, 0) thì cảnh báo và không đặt trần, như khi không đặt biến.
fn optional_limit(key: &str, raw: Option<&str>) -> Option<u64> {
    let raw = raw.map(str::trim).filter(|v| !v.is_empty())?;
    match raw.parse::<u64>() {
        Ok(value) if value > 0 => Some(value),
        _ => {
            tracing::warn!(
                variable = key,
                value = raw,
                "invalid value, expected a positive whole number; no limit is set"
            );
            None
        }
    }
}

/// Biến bật tắt: true, false, 1, 0, yes, no (không phân biệt hoa thường). Không đặt hoặc để trống thì dùng mặc định;
/// giá trị khác thì cảnh báo và dùng mặc định như positive_number, để lỗi gõ không lặng lẽ đổi hành vi.
fn flag(key: &str, raw: Option<&str>, default: bool) -> bool {
    let Some(raw) = raw.filter(|v| !v.trim().is_empty()) else {
        return default;
    };
    match raw.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => {
            tracing::warn!(
                variable = key,
                value = raw,
                default,
                "invalid value, expected true or false; using default"
            );
            default
        }
    }
}

/// Timeout tính bằng giây từ biến môi trường, theo cùng quy tắc với positive_number: timeout 0 sẽ làm mọi lần gọi mô
/// hình thất bại ngay.
/// Chỗ Dockerfile đặt extension postgres của DuckDB; make dev và gói phát hành trỏ chỗ khác qua biến môi trường.
pub const DEFAULT_POSTGRES_EXTENSION: &str = "/usr/local/lib/xdi/postgres_scanner.duckdb_extension";

/// SOURCE_SNAPSHOT_MODE: copy hoặc direct; trống thì DEFAULT_SNAPSHOT_MODE, giá trị lạ thì cảnh báo và dùng mặc định.
fn snapshot_mode(raw: Option<&str>) -> SnapshotMode {
    let Some(raw) = raw.filter(|v| !v.trim().is_empty()) else {
        return DEFAULT_SNAPSHOT_MODE;
    };
    match raw.trim().to_lowercase().as_str() {
        "copy" => SnapshotMode::Copy,
        "direct" => SnapshotMode::Direct,
        _ => {
            tracing::warn!(
                variable = "SOURCE_SNAPSHOT_MODE",
                value = raw,
                default = ?DEFAULT_SNAPSHOT_MODE,
                "invalid value, expected copy or direct; using default"
            );
            DEFAULT_SNAPSHOT_MODE
        }
    }
}

/// Trần mặc định của chụp bảng, đặt từ số đo ở docs/PERFORMANCE.md (mục Chụp bảng lớn, scripts/bench-snapshot.sh): bảng
/// 1,8 GB (60 cột) chép trong khoảng 20 giây, nên 4096 MB cả lần còn xa 300 giây kể cả khi đọc chậm hơn vài lần (đĩa
/// nguồn chưa có trong cache). Bộ nhớ lúc nạp bảng lớn do DUCKDB_MEMORY_LIMIT_MB giữ, không do trần này.
const DEFAULT_SOURCE_TABLE_MAX_MB: u32 = 2048;
const DEFAULT_SOURCE_SNAPSHOT_MAX_MB: u32 = 4096;
const DEFAULT_SOURCE_TIMEOUT_SECS: u32 = 300;

/// Trần mặc định của mỗi kết nối DuckDB nạp dữ liệu, đặt từ số đo ở docs/PERFORMANCE.md (mục Chụp bảng lớn).
const DEFAULT_DUCKDB_MEMORY_LIMIT_MB: u32 = 1024;

/// direct nhanh hơn và tốn ít đĩa tạm hơn copy trên database mẫu (docs/PERFORMANCE.md, #246); copy vẫn là đường dự phòng
/// khi extension không nạp được.
const DEFAULT_SNAPSHOT_MODE: SnapshotMode = SnapshotMode::Direct;

/// SOURCE_RESNAPSHOT_HOURS: trống hay 0 là không có lịch; số giờ dương là khoảng giữa hai lần; giá trị lạ thì cảnh báo
/// và không bật lịch, vì chụp ngoài ý muốn tốn tải của database nguồn hơn là không chụp.
fn resnapshot_every(raw: Option<&str>) -> Option<Duration> {
    let raw = raw.map(str::trim).filter(|v| !v.is_empty())?;
    match raw.parse::<u32>() {
        Ok(0) => None,
        Ok(hours) => Some(Duration::from_secs(u64::from(hours) * 3600)),
        Err(_) => {
            tracing::warn!(
                variable = "SOURCE_RESNAPSHOT_HOURS",
                value = raw,
                "invalid value, expected a whole number of hours; scheduled re-snapshot is off"
            );
            None
        }
    }
}

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
/// Tách danh sách khóa cách nhau bằng dấu phẩy, bỏ khoảng trắng và mục rỗng.
fn split_keys(value: &str) -> Vec<Secret> {
    value
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(Secret::new)
        .collect()
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// "db.example.com, 10.0.0.5" thành ["db.example.com", "10.0.0.5"]; chữ thường để so không phân biệt hoa thường.
fn host_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|h| h.trim().to_lowercase())
        .filter(|h| !h.is_empty())
        .collect()
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
    fn backup_keys_are_split_on_commas() {
        let keys = split_keys(" a1 , ,b2,");
        let keys: Vec<&str> = keys.iter().map(|k| k.expose()).collect();
        assert_eq!(keys, vec!["a1", "b2"]);
        assert!(split_keys("").is_empty());
    }

    #[test]
    fn only_the_sample_host_and_port_are_allowed_without_listing() {
        let mut config = Config::from_env();
        config.sample_databases = sample_databases(
            "postgres://analyst:x@127.0.0.1:5440/sales_demo,postgres://analyst:x@127.0.0.1:5440/edu_demo",
        );
        assert_eq!(config.sample_databases.len(), 2);
        // Chuỗi sai dạng bị bỏ riêng, chuỗi đúng vẫn giữ.
        assert_eq!(
            sample_databases("mysql://a:b@h/db, postgres://a:b@h/db").len(),
            1
        );
        config.source_allowed_hosts = vec!["db.example.com".to_string()];
        assert!(config.source_host_allowed("127.0.0.1", 5440));
        // Cùng máy, cổng khác (Postgres của ứng dụng ở 5433): không được.
        assert!(!config.source_host_allowed("127.0.0.1", 5433));
        assert!(config.source_host_allowed("db.example.com", 6543));
        assert!(!config.source_host_allowed("10.0.0.5", 5432));
    }

    #[test]
    fn eval_key_replaces_the_app_key_only_when_set() {
        let mut config = Config::from_env();
        config.gemini_api_key = Secret::new("app-key");
        assert!(!config.apply_eval_gemini_key(None));
        assert!(!config.apply_eval_gemini_key(Some("  ".to_string())));
        assert_eq!(config.gemini_api_key.expose(), "app-key");
        assert!(config.apply_eval_gemini_key(Some(" eval-key ".to_string())));
        assert_eq!(config.gemini_api_key.expose(), "eval-key");
    }

    #[test]
    fn host_list_parsing() {
        assert_eq!(
            host_list(" DB.example.com ,10.0.0.5,,"),
            vec!["db.example.com", "10.0.0.5"]
        );
        assert!(host_list("").is_empty());
    }

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
    fn resnapshot_hours_parsing() {
        assert_eq!(resnapshot_every(None), None);
        assert_eq!(resnapshot_every(Some(" ")), None);
        assert_eq!(resnapshot_every(Some("0")), None);
        assert_eq!(resnapshot_every(Some("abc")), None);
        assert_eq!(
            resnapshot_every(Some(" 24 ")),
            Some(Duration::from_secs(24 * 3600))
        );
    }

    #[test]
    fn optional_limit_parsing() {
        assert_eq!(optional_limit("X", None), None);
        assert_eq!(optional_limit("X", Some(" ")), None);
        assert_eq!(optional_limit("X", Some("5000000")), Some(5_000_000));
        assert_eq!(optional_limit("X", Some("0")), None);
        assert_eq!(optional_limit("X", Some("many")), None);
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
    fn snapshot_mode_parsing() {
        assert_eq!(snapshot_mode(None), DEFAULT_SNAPSHOT_MODE);
        assert_eq!(snapshot_mode(Some(" Direct ")), SnapshotMode::Direct);
        assert_eq!(snapshot_mode(Some("copy")), SnapshotMode::Copy);
        assert_eq!(snapshot_mode(Some("fast")), DEFAULT_SNAPSHOT_MODE);
    }

    #[test]
    fn flag_parsing() {
        assert!(flag("X", None, true));
        assert!(!flag("X", None, false));
        assert!(flag("X", Some(""), true));
        for on in ["true", "TRUE", " 1 ", "yes"] {
            assert!(flag("X", Some(on), false), "{on}");
        }
        for off in ["false", "False", "0", "no"] {
            assert!(!flag("X", Some(off), true), "{off}");
        }
        for bad in ["of", "2", "bật"] {
            assert!(flag("X", Some(bad), true), "{bad}");
            assert!(!flag("X", Some(bad), false), "{bad}");
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
