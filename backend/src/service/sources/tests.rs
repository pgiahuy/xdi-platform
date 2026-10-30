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

/// Khóa giữa hai bảng trong nhóm giữ một lần dù đọc từ cả hai đầu; khóa ra ngoài nhóm và khóa tới cột không UNIQUE bỏ.
#[test]
fn group_keys_stay_inside_the_group_once() {
    let items = table("shop", "order_items");
    let orders = table("shop", "orders");
    let customers = table("shop", "customers");
    let inside = key("items_order", items.clone(), orders.clone(), true);
    let keys = vec![
        inside.clone(),
        inside,
        key("orders_customer", orders.clone(), customers, true),
        key("items_channel", items.clone(), orders.clone(), false),
    ];
    let kept = relations::build_group_keys(keys, &[items.clone(), orders.clone()]);
    let names: Vec<&str> = kept.iter().map(|k| k.constraint.as_str()).collect();
    assert_eq!(names, ["items_order"]);

    let ids = [(items, Uuid::new_v4()), (orders, Uuid::new_v4())];
    let rel = snapshot::build_new_relations(&kept, &ids);
    assert_eq!(rel.len(), 1);
    assert_eq!(
        (rel[0].from_dataset_id, rel[0].to_dataset_id),
        (ids[0].1, ids[1].1)
    );
    assert_eq!(rel[0].cardinality, "many_to_one");
}

#[test]
fn row_limit_is_per_table() {
    let orders = table("shop", "orders");
    assert!(matches!(
        snapshot::check_row_count(&orders, 500, 100),
        Err(AppError::TooLarge(m)) if m.contains("shop.orders")
    ));
    assert!(snapshot::check_row_count(&orders, 100, 100).is_ok());
}
