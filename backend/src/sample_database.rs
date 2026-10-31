// SPDX-License-Identifier: Apache-2.0
/* sample_database.rs - đọc SAMPLE_DATABASE_URL: database mẫu mà thiết lập lần đầu kết nối làm nguồn và chụp bảng
(service/samples.rs). config.rs gọi build_sample_database lúc đọc cấu hình; đúng máy và cổng của database mẫu được phép thêm
làm nguồn (Config::source_host_allowed), vì người cài đã chọn nó.

Chỉ nhận dạng postgres://tài-khoản:mật-khẩu@máy:cổng/database?sslmode=..., đủ cho compose (demo-db) và cho máy dev.
Ký tự đặc biệt trong tài khoản, mật khẩu, tên database viết dạng %XX như mọi chuỗi kết nối Postgres. */

use crate::config::Secret;

/// Thông tin kết nối của database mẫu. Mật khẩu bọc Secret: Config derive Debug, lỡ ghi Config ra log thì mật khẩu
/// không lộ.
#[derive(Debug, Clone)]
pub struct SampleDatabase {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: Secret,
    pub sslmode: Option<String>,
}

/// Rỗng thì Ok(None): không dùng database mẫu, thiết lập lần đầu nạp dataco.csv. Sai dạng thì Err kèm lý do (không
/// chép chuỗi vào lý do, vì chuỗi có mật khẩu).
pub fn build_sample_database(raw: &str) -> Result<Option<SampleDatabase>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let rest = raw
        .strip_prefix("postgres://")
        .or_else(|| raw.strip_prefix("postgresql://"))
        .ok_or("expected postgres://user:password@host:port/database")?;
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, database) = rest.split_once('/').ok_or("missing /database")?;
    // Tách ở @ cuối cùng: mật khẩu lỡ chứa @ chưa viết thành %40 thì vẫn tách đúng.
    let (user_info, host_port) = authority.rsplit_once('@').ok_or("missing user@host")?;
    let (user, password) = user_info.split_once(':').unwrap_or((user_info, ""));
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().map_err(|_| "port is not a number")?),
        None => (host_port, 5432),
    };
    let sslmode = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("sslmode="))
        .map(str::to_string);
    let database = decode_percent(database)?;
    let user = decode_percent(user)?;
    if host.is_empty() || database.is_empty() || user.is_empty() {
        return Err("host, user and database must not be empty".to_string());
    }
    Ok(Some(SampleDatabase {
        host: host.to_lowercase(),
        port,
        database,
        user,
        password: Secret::new(decode_percent(password)?),
        sslmode,
    }))
}

/// Đổi %XX thành byte tương ứng, rồi đọc kết quả là UTF-8.
fn decode_percent(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3).ok_or("incomplete %XX escape")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| "invalid %XX escape")?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "escaped text is not UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_means_no_sample_database() {
        assert!(build_sample_database("  ").unwrap().is_none());
    }

    #[test]
    fn full_url_is_parsed() {
        let db = build_sample_database(
            "postgres://analyst:p%40ss@Demo-DB:5434/company_demo?sslmode=require",
        )
        .unwrap()
        .unwrap();
        assert_eq!(db.host, "demo-db");
        assert_eq!(db.port, 5434);
        assert_eq!(db.database, "company_demo");
        assert_eq!(db.user, "analyst");
        assert_eq!(db.password.expose(), "p@ss");
        assert_eq!(db.sslmode.as_deref(), Some("require"));
    }

    #[test]
    fn port_defaults_to_5432() {
        let db = build_sample_database("postgresql://analyst@demo-db/company_demo")
            .unwrap()
            .unwrap();
        assert_eq!(db.port, 5432);
        assert_eq!(db.password.expose(), "");
        assert_eq!(db.sslmode, None);
    }

    #[test]
    fn wrong_shapes_are_rejected_without_echoing_the_url() {
        for raw in [
            "mysql://a:b@h/db",
            "postgres://a:secret@h",
            "postgres://h/db",
            "postgres://a:secret@h:port/db",
            "postgres://a:secret@h/%zz",
        ] {
            let why = build_sample_database(raw).unwrap_err();
            assert!(!why.contains("secret"), "{why}");
        }
    }
}
