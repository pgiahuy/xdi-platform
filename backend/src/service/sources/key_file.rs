// SPDX-License-Identifier: Apache-2.0
/* service/sources/key_file.rs - khóa mã hóa thông tin kết nối khi SOURCE_SECRET_KEY để trống: lần đầu backend tự sinh
một khóa ngẫu nhiên 32 byte, ghi dạng base64 vào <DATA_DIR>/source_secret.key (quyền 0600), các lần sau đọc lại file
đó. main.rs gọi apply_key_file một lần, trước khi dựng AppState; secret.rs đọc khóa từ Config như khi biến được đặt.

Vì sao: bản cài mới chụp được database mẫu (và database thật) mà người cài không phải tạo khóa, và repo không phải chứa
một khóa ai cũng biết. Đổi lại, khóa nằm cùng volume dữ liệu: mất file thì mất mật khẩu của mọi nguồn đã lưu (thêm lại
nguồn), và bản sao lưu của make backup có cả file này. Bản chạy thật nên đặt SOURCE_SECRET_KEY (docs/INSTALL.md). */

use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;

use super::secret::SecretKey;
use crate::config::{Config, Secret};

/// Tên file khóa trong DATA_DIR.
pub const KEY_FILE: &str = "source_secret.key";

/// SOURCE_SECRET_KEY rỗng thì đặt khóa của file (sinh mới nếu chưa có). Lỗi (không ghi được file, file hỏng) thì để
/// khóa rỗng, nghĩa là tính năng nguồn database tắt kèm lời báo cách sửa, và trả lời báo để main.rs ghi log.
pub fn apply_key_file(config: &mut Config) -> Result<(), String> {
    if !config.source_secret_key.is_empty() {
        return Ok(());
    }
    let key = load_or_create(&config.data_dir.join(KEY_FILE))?;
    config.source_secret_key = Secret::new(key);
    Ok(())
}

/// Khóa base64 đọc từ file, hay khóa mới ghi vào file khi file chưa có.
fn load_or_create(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let key = text.trim().to_string();
            // Kiểm ngay lúc khởi động: file sửa tay hỏng thì báo ở đây, không đợi tới lần đầu thêm nguồn.
            SecretKey::from_base64(&key)
                .map_err(|_| format!("{} is not a 32 byte base64 key", path.display()))?;
            Ok(key)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => create(path),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Sinh khóa và ghi file. create_new: hai tiến trình cùng khởi động (server và một lệnh quản trị) thì tiến trình sau
/// không ghi đè khóa của tiến trình trước mà đọc lại nó.
fn create(path: &Path) -> Result<String, String> {
    let dir: PathBuf = path.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("cannot generate a key: {e}"))?;
    let key = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    // Chỉ user chạy backend đọc được file (0600): khóa giải mã được mật khẩu của mọi nguồn.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return load_or_create(path),
        Err(e) => return Err(format!("cannot create {}: {e}", path.display())),
    };
    file.write_all(format!("{key}\n").as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    // Không in khóa, chỉ nơi lưu.
    tracing::info!(path = %path.display(), "generated SOURCE_SECRET_KEY and saved it to the data directory");
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xdi-key-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn config_in(dir: &Path, key: &str) -> Config {
        let mut config = Config::from_env();
        config.data_dir = dir.to_path_buf();
        config.source_secret_key = Secret::new(key);
        config
    }

    #[test]
    fn empty_key_is_generated_once_then_read_back() {
        let dir = temp_dir();
        let mut first = config_in(&dir, "");
        apply_key_file(&mut first).unwrap();
        let key = first.source_secret_key.expose().to_string();
        assert!(SecretKey::from_base64(&key).is_ok());
        let mut second = config_in(&dir, "");
        apply_key_file(&mut second).unwrap();
        assert_eq!(second.source_secret_key.expose(), key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_from_the_environment_wins_and_writes_no_file() {
        let dir = temp_dir();
        let set = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";
        let mut config = config_in(&dir, set);
        apply_key_file(&mut config).unwrap();
        assert_eq!(config.source_secret_key.expose(), set);
        assert!(!dir.join(KEY_FILE).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn damaged_key_file_is_reported_and_the_feature_stays_off() {
        let dir = temp_dir();
        std::fs::write(dir.join(KEY_FILE), "not a key\n").unwrap();
        let mut config = config_in(&dir, "");
        let why = apply_key_file(&mut config).unwrap_err();
        assert!(why.contains(KEY_FILE), "{why}");
        assert!(config.source_secret_key.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
