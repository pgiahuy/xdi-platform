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
        ssl_root_cert: None,
    }
}

/// Chứng chỉ CA tự ký chỉ dùng cho test (openssl req -x509, khóa EC P-256); không có khóa bí mật đi kèm trong repo.
const TEST_CA: &str = "-----BEGIN CERTIFICATE-----
MIIBgTCCASegAwIBAgIUUyvD0jrLX45yw/wGXD48lGR8wUYwCgYIKoZIzj0EAwIw
FjEUMBIGA1UEAwwLeGRpIHRlc3QgQ0EwHhcNMjYxMDMxMDg1MzQzWhcNMzYxMDI4
MDg1MzQzWjAWMRQwEgYDVQQDDAt4ZGkgdGVzdCBDQTBZMBMGByqGSM49AgEGCCqG
SM49AwEHA0IABEvArgkocMus/5qxLP+6gDCdLnHMQKGJkdjh6FaIQDbIZNGH4v/h
ExiYr9o24prejgA4J/mfKn9fqcU2kn8oxjijUzBRMB0GA1UdDgQWBBSz7MZ7xMQW
DmYNa24TsnG+FIbVYzAfBgNVHSMEGDAWgBSz7MZ7xMQWDmYNa24TsnG+FIbVYzAP
BgNVHRMBAf8EBTADAQH/MAoGCCqGSM49BAMCA0gAMEUCIQCHWVX64TFNHnsao7EO
JYlE9ymNYw5QZJP/ohI6MZDBkgIgI/VYsz9pbA95L0NPl7zVOtkpPjbZ7omapCiI
tZcs6v8=
-----END CERTIFICATE-----
";

#[test]
fn tls_modes_and_ca_certificate() {
    for mode in ["disable", "prefer", "require", "verify-full"] {
        let mut req = request("db.example.com");
        req.sslmode = Some(mode.to_string());
        assert_eq!(build_connect_info(req).unwrap().sslmode, mode);
    }
    let mut unknown = request("db.example.com");
    unknown.sslmode = Some("verify-ca".to_string());
    assert!(build_connect_info(unknown).is_err());

    // CA dán vào: nhận với verify-full, ghi vào config là có CA riêng; từ chối với mode không kiểm chứng chỉ hay PEM hỏng.
    let mut with_ca = request("db.example.com");
    with_ca.sslmode = Some("verify-full".to_string());
    with_ca.ssl_root_cert = Some(TEST_CA.to_string());
    let info = build_connect_info(with_ca).unwrap();
    assert_eq!(info.ssl_root_cert.as_deref(), Some(TEST_CA));
    assert_eq!(public_config(&info)["custom_ca"], true);
    let mut require_ca = request("db.example.com");
    require_ca.sslmode = Some("require".to_string());
    require_ca.ssl_root_cert = Some(TEST_CA.to_string());
    assert!(build_connect_info(require_ca).is_err());
    let mut broken = request("db.example.com");
    broken.sslmode = Some("verify-full".to_string());
    broken.ssl_root_cert = Some("-----BEGIN CERTIFICATE-----\nnot base64\n".to_string());
    assert!(build_connect_info(broken).is_err());
    // Ô để trống (chỉ khoảng trắng) là không có CA.
    let mut blank = request("db.example.com");
    blank.ssl_root_cert = Some("  \n".to_string());
    assert_eq!(build_connect_info(blank).unwrap().ssl_root_cert, None);
}

/// Thông tin kết nối mã hóa trước khi có TLS không có ssl_root_cert: vẫn đọc được.
#[test]
fn old_connect_info_without_ca_still_decodes() {
    let old =
        r#"{"host":"h","port":5432,"database":"d","user":"u","password":"p","sslmode":"prefer"}"#;
    let info: postgres::ConnectInfo = serde_json::from_str(old).unwrap();
    assert!(info.ssl_root_cert.is_none());
}

#[test]
fn host_must_be_listed() {
    let mut config = crate::config::Config::from_env();
    config.sample_databases = Vec::new();
    config.source_allowed_hosts = vec!["db.example.com".to_string()];
    assert!(check_host_allowed(&config, "db.example.com", 5432).is_ok());
    assert!(check_host_allowed(&config, "169.254.169.254", 5432).is_err());
    config.source_allowed_hosts = Vec::new();
    assert!(check_host_allowed(&config, "db.example.com", 5432).is_err());
}

#[test]
fn connect_info_normalizes_and_validates() {
    let info = build_connect_info(request(" DB.Example.com ")).unwrap();
    assert_eq!(info.host, "db.example.com");
    assert_eq!(info.port, 5432);
    assert_eq!(info.sslmode, "prefer");
    assert!(build_connect_info(request("http://db/x")).is_err());
    assert!(build_connect_info(request("")).is_err());
}

#[test]
fn stored_config_and_label_have_no_password() {
    let info = build_connect_info(request("db.example.com")).unwrap();
    let config = public_config(&info).to_string();
    assert!(!config.contains("s3cret-pw"), "{config}");
    let label = source_file_label(&info, "public.orders");
    assert_eq!(label, "postgres://db.example.com/shop/public.orders");
}

fn table(schema: &str, name: &str) -> SourceTable {
    SourceTable {
        schema: schema.to_string(),
        name: name.to_string(),
        kind: "table".to_string(),
    }
}

fn key(
    constraint: &str,
    from: SourceTable,
    to: SourceTable,
    unique_target: bool,
) -> relations::ForeignKey {
    relations::ForeignKey {
        constraint: constraint.to_string(),
        from,
        from_columns: vec!["order_id".to_string()],
        to,
        to_columns: vec!["id".to_string()],
        unique_target,
        unique_source: false,
        readable_source: true,
        readable_target: true,
    }
}

/// Khóa giữa hai bảng đang giữ giữ một lần dù đọc hai lần; khóa tới bảng chưa chụp, tới cột không UNIQUE, và khóa giữa
/// hai bảng không có trong lần chụp này bỏ.
#[test]
fn group_keys_need_both_ends_held_and_one_in_the_run() {
    let items = table("shop", "order_items");
    let orders = table("shop", "orders");
    let customers = table("shop", "customers");
    let regions = table("shop", "regions");
    let inside = key("items_order", items.clone(), orders.clone(), true);
    let keys = vec![
        inside.clone(),
        inside,
        key("orders_customer", orders.clone(), customers.clone(), true),
        key("customers_region", customers.clone(), regions.clone(), true),
        key("items_channel", items.clone(), orders.clone(), false),
    ];
    let held: Vec<String> = [&items, &orders, &customers, &regions]
        .iter()
        .map(|t| t.full_name())
        .collect();
    // Lần này chụp order_items và orders; customers, regions đã chụp từ trước.
    let run = vec![items.full_name(), orders.full_name()];
    let kept = relations::build_group_keys(keys, &held, &run);
    let names: Vec<&str> = kept.iter().map(|k| k.constraint.as_str()).collect();
    assert_eq!(names, ["items_order", "orders_customer"]);

    let ids: Vec<(Uuid, String)> = held.iter().map(|t| (Uuid::new_v4(), t.clone())).collect();
    let rel = snapshot::build_new_relations(&kept, &ids);
    assert_eq!(rel.len(), 2);
    assert_eq!(
        (rel[0].from_dataset_id, rel[0].to_dataset_id),
        (ids[0].0, ids[1].0)
    );
    assert_eq!(rel[0].cardinality, "many_to_one");
}

/// Lỗi TLS ở prefer (bắt tay lỗi) không khuyên chọn lại prefer; ở require thì gợi ý prefer hay disable.
#[test]
fn tls_error_advice_depends_on_the_mode() {
    let mut info = build_connect_info(request("db.example.com")).unwrap();
    let message = |info: &postgres::ConnectInfo| match postgres::tls_error(info) {
        AppError::BadRequest(text) => text,
        other => panic!("{other:?}"),
    };
    let prefer = message(&info);
    assert!(
        prefer.contains("bắt tay TLS lỗi") && !prefer.contains("chọn prefer"),
        "{prefer}"
    );
    info.sslmode = "require".to_string();
    assert!(message(&info).contains("chọn prefer hoặc disable"));
}

/// Comment vào hash của bản chụp chỉ khi bảng có comment: bảng không có comment giữ hash như trước #245, đổi comment
/// thì hash đổi.
#[test]
fn comments_enter_the_snapshot_hash_only_when_present() {
    use crate::service::ingest::SourceDescriptions;

    assert_eq!(
        snapshot::build_comment_part(&SourceDescriptions::default()),
        ""
    );
    let with = |text: &str| SourceDescriptions {
        table: None,
        columns: vec![("region".to_string(), text.to_string())],
    };
    let first = snapshot::build_comment_part(&with("Vùng giao hàng"));
    assert!(!first.is_empty());
    assert_ne!(first, snapshot::build_comment_part(&with("Vùng nhận hàng")));
}
