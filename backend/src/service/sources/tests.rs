// SPDX-License-Identifier: Apache-2.0
/* service/sources/tests.rs - test phần không cần database của nguồn Postgres: danh sách máy chủ được phép, kiểm form
thêm nguồn, cột _source_file và config lưu dạng chữ không có mật khẩu. Test qua API với Postgres thật ở
api/sources_tests.rs. */

use super::*;

fn request(host: &str) -> CreateSourceRequest {
    CreateSourceRequest {
        host: host.to_string(),
        port: None,
        database: "shop".to_string(),
        user: "reader".to_string(),
        password: "s3cret-pw".to_string(),
        sslmode: None,
    }
}

#[test]
fn host_must_be_listed() {
    let allowed = vec!["db.example.com".to_string()];
    assert!(check_host_allowed(&allowed, "db.example.com").is_ok());
    assert!(check_host_allowed(&allowed, "169.254.169.254").is_err());
    assert!(check_host_allowed(&[], "db.example.com").is_err());
}

#[test]
fn connect_info_normalizes_and_validates() {
    let info = build_connect_info(request(" DB.Example.com ")).unwrap();
    assert_eq!(info.host, "db.example.com");
    assert_eq!(info.port, 5432);
    assert_eq!(info.sslmode, "prefer");
    assert!(build_connect_info(request("http://db/x")).is_err());
    assert!(build_connect_info(request("")).is_err());
    let mut tls = request("db.example.com");
    tls.sslmode = Some("verify-full".to_string());
    assert!(build_connect_info(tls).is_err());
}

#[test]
fn stored_config_and_label_have_no_password() {
    let info = build_connect_info(request("db.example.com")).unwrap();
    let config = public_config(&info).to_string();
    assert!(!config.contains("s3cret-pw"), "{config}");
    let label = source_file_label(&info, "public.orders");
    assert_eq!(label, "postgres://db.example.com/shop/public.orders");
}
