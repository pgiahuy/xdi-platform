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

fn relation(
    id: &str,
    parent: Option<&str>,
    from: SourceTable,
    from_columns: &[&str],
    to: SourceTable,
    to_columns: &[&str],
    target_columns: &[&str],
) -> relations::Relation {
    let strings = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let key = relations::ForeignKey {
        constraint: id.rsplit('/').next().unwrap().to_string(),
        from,
        from_columns: strings(from_columns),
        to,
        to_columns: strings(to_columns),
        unique_target: true,
        readable_target: true,
    };
    relations::Relation {
        info: relations::SourceRelation {
            id: id.to_string(),
            depth: if parent.is_some() { 2 } else { 1 },
            parent: parent.map(str::to_string),
            constraint: key.constraint.clone(),
            from_table: key.from.full_name(),
            from_columns: key.from_columns.clone(),
            to_table: key.to.full_name(),
            to_columns: key.to_columns.clone(),
            joinable: true,
            reason: None,
            column_count: 0,
        },
        key,
        target_columns: strings(target_columns),
    }
}

#[test]
fn no_relation_keeps_the_plain_copy() {
    let plan = join_sql::build_join_plan(&table("public", "orders"), &[], &[]);
    assert_eq!(plan.select, "SELECT * FROM \"public\".\"orders\"");
    assert!(plan.joins.is_empty());
}

/// Khóa nhiều cột ghép theo cặp, bước 2 nối từ bí danh của bước 1, tên có ngoặc kép được nhân đôi, tên trùng thêm _2.
#[test]
fn join_plan_names_and_quotes_columns() {
    let fact = table("sales", "order_items");
    let orders = relation(
        "sales.order_items/fk_order",
        None,
        fact.clone(),
        &["order_id"],
        table("sales", "orders"),
        &["id"],
        &["id", "customer_id", "status"],
    );
    let customers = relation(
        "sales.order_items/fk_order>sales.orders/fk_customer",
        Some("sales.order_items/fk_order"),
        table("sales", "orders"),
        &["customer_id"],
        table("crm", "orders"),
        &["code"],
        &["code", "say \"hi\""],
    );
    let fact_columns = vec!["order_id".to_string(), "ORDERS.STATUS".to_string()];
    let plan = join_sql::build_join_plan(&fact, &fact_columns, &[&orders, &customers]);
    assert_eq!(
        plan.select,
        "SELECT t0.\"order_id\", t0.\"ORDERS.STATUS\", t1.\"customer_id\" AS \"orders.customer_id\", \
         t1.\"status\" AS \"orders.status_2\", t2.\"say \"\"hi\"\"\" AS \"orders_2.say \"\"hi\"\"\" \
         FROM \"sales\".\"order_items\" AS t0 \
         LEFT JOIN \"sales\".\"orders\" AS t1 ON t0.\"order_id\" = t1.\"id\" \
         LEFT JOIN \"crm\".\"orders\" AS t2 ON t1.\"customer_id\" = t2.\"code\""
    );
    assert_eq!(plan.joins[1].prefix, "orders_2");
    assert_eq!(plan.joins[1].columns, ["orders_2.say \"hi\""]);
    assert_eq!(plan.joins[0].to_table, "sales.orders");
    // Khóa của dòng bảng đích trong dataset: bước 1 là cột của bảng chính, bước 2 là tên đã đặt ở bước 1.
    assert_eq!(plan.joins[0].key_columns, ["order_id"]);
    assert_eq!(plan.joins[1].key_columns, ["orders.customer_id"]);
}

#[test]
fn row_count_change_is_refused() {
    let err = check_row_counts("sales.order_items", 5, 7, 100).unwrap_err();
    assert!(matches!(err, AppError::BadRequest(m) if m.contains("từ 5 thành 7")));
    assert!(matches!(
        check_row_counts("sales.order_items", 500, 500, 100),
        Err(AppError::TooLarge(_))
    ));
    assert!(check_row_counts("sales.order_items", 5, 5, 100).is_ok());
}
