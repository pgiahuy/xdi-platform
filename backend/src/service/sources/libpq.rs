// SPDX-License-Identifier: Apache-2.0
/* service/sources/libpq.rs - chuỗi kết nối và file tạm cho libpq, thư viện Postgres nằm trong extension postgres của
DuckDB mà direct.rs dùng. sqlx (postgres.rs) nhận mật khẩu và CA trong bộ nhớ; libpq thì đọc mật khẩu từ passfile và
CA từ file sslrootcert, nên hai thứ này ghi ra file quyền 0600 trong một thư mục tạm, xóa khi lần chụp xong hay lỗi.
Mật khẩu không vào chuỗi kết nối, nên không nằm trong câu SQL của DuckDB hay lời báo lỗi chép chuỗi kết nối. */

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use super::direct::DirectError;
use super::postgres::ConnectInfo;
use crate::error::AppError;

/// Chuỗi kết nối libpq dạng key='value'. Không có mật khẩu: libpq đọc nó từ passfile.
pub(super) fn build_conninfo(info: &ConnectInfo, timeout: Duration, files: &ConnFiles) -> String {
    let mut pairs = vec![
        ("host", info.host.clone()),
        ("port", info.port.to_string()),
        ("dbname", info.database.clone()),
        ("user", info.user.clone()),
        ("passfile", files.passfile.to_string_lossy().into_owned()),
        ("sslmode", info.sslmode.clone()),
        ("application_name", "xdi-platform".to_string()),
        ("connect_timeout", timeout.as_secs().max(2).to_string()),
        (
            "options",
            format!(
                "-c default_transaction_read_only=on -c statement_timeout={}",
                timeout.as_millis()
            ),
        ),
    ];
    if info.sslmode == "verify-full" {
        let root = match &files.root_cert {
            Some(path) => path.to_string_lossy().into_owned(),
            None => "system".to_string(),
        };
        pairs.push(("sslrootcert", root));
    }
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", conninfo_value(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Giá trị trong chuỗi kết nối libpq: bọc nháy đơn, thoát nháy đơn và dấu gạch ngược bằng dấu gạch ngược.
fn conninfo_value(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Thư mục tạm của một lần chụp, quyền 0700, chứa passfile và CA dán vào (nếu có), mỗi file quyền 0600. Nằm dưới
/// DATA_DIR/staging, nên backend chết giữa chừng thì lần khởi động sau xóa (ingest::clear_staging). Drop xóa cả thư mục.
pub(super) struct ConnFiles {
    dir: PathBuf,
    passfile: PathBuf,
    root_cert: Option<PathBuf>,
}

impl ConnFiles {
    pub(super) fn write(data_dir: &Path, info: &ConnectInfo) -> Result<Self, DirectError> {
        // passfile của libpq đọc từng dòng: mật khẩu có xuống dòng thì không ghi vào đó được, chép bằng COPY.
        if info.password.contains(['\n', '\r']) {
            return Err(DirectError::Unavailable(
                "password contains a line break, which a libpq passfile cannot hold".to_string(),
            ));
        }
        let dir = data_dir
            .join("staging")
            .join(format!("libpq-{}", Uuid::new_v4()));
        let internal = |e: std::io::Error| {
            DirectError::Failed(AppError::Internal(format!(
                "write libpq files in {}: {e}",
                dir.display()
            )))
        };
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(internal)?;
        // Tạo struct ngay khi có thư mục, để lỗi ở bước ghi file sau vẫn xóa thư mục qua Drop.
        let mut files = ConnFiles {
            passfile: dir.join("passfile"),
            root_cert: None,
            dir: dir.clone(),
        };
        // Dòng passfile host:port:database:user:password, * khớp mọi giá trị; ':' và '\' trong mật khẩu thoát bằng '\'.
        let password = info.password.replace('\\', "\\\\").replace(':', "\\:");
        write_private(&files.passfile, format!("*:*:*:*:{password}\n").as_bytes())
            .map_err(internal)?;
        if let Some(pem) = &info.ssl_root_cert {
            let path = dir.join("root.crt");
            write_private(&path, pem.as_bytes()).map_err(internal)?;
            files.root_cert = Some(path);
        }
        Ok(files)
    }
}

impl Drop for ConnFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Ghi file chỉ chủ sở hữu đọc được (0600) ngay từ lúc tạo; libpq bỏ qua passfile mà người khác đọc được.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(password: &str, sslmode: &str, ca: Option<&str>) -> ConnectInfo {
        ConnectInfo {
            host: "db.example.com".to_string(),
            port: 5432,
            database: "shop".to_string(),
            user: "reader".to_string(),
            password: password.to_string(),
            sslmode: sslmode.to_string(),
            ssl_root_cert: ca.map(str::to_string),
        }
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xdi-direct-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Mật khẩu chỉ nằm trong passfile quyền 0600, không trong chuỗi kết nối; thư mục tạm bị xóa khi xong.
    #[test]
    fn password_goes_to_a_private_passfile_only() {
        use std::os::unix::fs::PermissionsExt;
        let data = temp_dir();
        let source = info("pa:ss\\'word", "verify-full", Some("PEM"));
        let files = ConnFiles::write(&data, &source).ok().unwrap();
        let conninfo = build_conninfo(&source, Duration::from_secs(30), &files);
        assert!(!conninfo.contains("pa:ss"), "{conninfo}");
        assert!(conninfo.contains("sslmode='verify-full'"), "{conninfo}");
        assert!(conninfo.contains("statement_timeout=30000"), "{conninfo}");
        let passfile = fs::read_to_string(&files.passfile).unwrap();
        assert_eq!(passfile, "*:*:*:*:pa\\:ss\\\\'word\n");
        let mode = fs::metadata(&files.passfile).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let ca = files.root_cert.clone().unwrap();
        assert!(conninfo.contains(&format!("sslrootcert='{}'", ca.display())));
        let dir = files.dir.clone();
        drop(files);
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(&data);
    }

    /// verify-full không có CA dán vào dùng CA của hệ thống; mode khác không đặt sslrootcert.
    #[test]
    fn root_certificate_follows_the_mode() {
        let data = temp_dir();
        let full = info("pw", "verify-full", None);
        let files = ConnFiles::write(&data, &full).ok().unwrap();
        assert!(
            build_conninfo(&full, Duration::from_secs(5), &files).contains("sslrootcert='system'")
        );
        let require = info("pw", "require", None);
        assert!(!build_conninfo(&require, Duration::from_secs(5), &files).contains("sslrootcert"));
        drop(files);
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn line_break_in_password_falls_back() {
        let data = temp_dir();
        let result = ConnFiles::write(&data, &info("a\nb", "disable", None));
        assert!(matches!(result, Err(DirectError::Unavailable(_))));
        let _ = fs::remove_dir_all(&data);
    }
}
