// SPDX-License-Identifier: Apache-2.0
/* service/ask/row_filters_tests.rs - test phần thuần của ask/row_filters.rs: điều kiện lan theo khóa ngoại dựng từ
quan hệ và chính sách đã đọc, không cần Postgres. Câu SQL guard dựng từ các điều kiện này được test ở
engine/guard/parent_filter_tests.rs; đường đọc Postgres qua route ở api/row_filter_fk_tests.rs. */

use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::store::{DatasetRelation, StoredPolicy};
use crate::service::ask::row_filters::{
    build_member_filters, build_parent_ids, build_row_filters, PolicyTable,
};

const ORDER_ITEMS: u128 = 1;
const ORDERS: u128 = 2;
const CUSTOMERS: u128 = 3;
const PRODUCTS: u128 = 4;
const REGIONS: u128 = 5;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn relation(from: u128, from_columns: &[&str], to: u128, to_columns: &[&str]) -> DatasetRelation {
    DatasetRelation {
        from_dataset_id: id(from),
        from_dataset_name: format!("d{from}"),
        from_columns: from_columns.iter().map(|c| c.to_string()).collect(),
        to_dataset_id: id(to),
        to_dataset_name: format!("d{to}"),
        to_columns: to_columns.iter().map(|c| c.to_string()).collect(),
        constraint_name: format!("d{from}_{}_fkey", from_columns.join("_")),
        cardinality: "many_to_one".to_string(),
    }
}

/// order_items -> orders -> customers -> regions, order_items.gift_customer_id -> customers, order_items (maker, sku)
/// -> products.
fn shop() -> Vec<DatasetRelation> {
    vec![
        relation(ORDER_ITEMS, &["order_id"], ORDERS, &["id"]),
        relation(ORDER_ITEMS, &["gift_customer_id"], CUSTOMERS, &["id"]),
        relation(ORDER_ITEMS, &["maker", "sku"], PRODUCTS, &["maker", "sku"]),
        relation(ORDERS, &["customer_id"], CUSTOMERS, &["id"]),
        relation(CUSTOMERS, &["region_code"], REGIONS, &["code"]),
    ]
}

fn region_filter() -> RowFilter {
    RowFilter::Column {
        column: "region_code".to_string(),
        allowed_values: vec!["N".to_string()],
    }
}

fn readable(name: &str) -> Option<AllowedTable> {
    Some(AllowedTable {
        name: name.to_string(),
        columns: vec![],
        parquet_dir: PathBuf::from(format!("/data/{name}")),
        row_filters: vec![],
    })
}

/// Mọi bảng đọc được; chỉ customers có chính sách, trừ khi own thay.
fn policy_tables(own: &[(u128, RowFilter)]) -> HashMap<Uuid, PolicyTable> {
    let names = [
        (ORDER_ITEMS, "order_items"),
        (ORDERS, "orders"),
        (CUSTOMERS, "customers"),
        (PRODUCTS, "products"),
        (REGIONS, "regions"),
    ];
    names
        .iter()
        .map(|&(n, name)| {
            let filters = own
                .iter()
                .filter(|(owner, _)| *owner == n)
                .map(|(_, f)| f.clone())
                .collect();
            (
                id(n),
                PolicyTable {
                    own: filters,
                    table: readable(name),
                },
            )
        })
        .collect()
}

/// Tóm tắt điều kiện thành chữ ngắn để so: `col` cho chính sách, `fk->parent[...]` cho điều kiện lan.
fn describe(filters: &[RowFilter]) -> Vec<String> {
    filters
        .iter()
        .map(|f| match f {
            RowFilter::Column {
                column,
                allowed_values,
            } => format!("{column}={}", allowed_values.join("|")),
            RowFilter::Parent {
                columns, parent, ..
            } => format!(
                "{}->{}[{}]",
                columns.join("+"),
                parent.name,
                describe(&parent.row_filters).join(", ")
            ),
            RowFilter::AnyOf(groups) => groups
                .iter()
                .map(|g| format!("({})", describe(g).join(" AND ")))
                .collect::<Vec<_>>()
                .join(" OR "),
        })
        .collect()
}

#[test]
fn parent_ids_follow_foreign_keys_upward_only() {
    let ids = build_parent_ids(id(ORDERS), &shop());
    assert_eq!(ids, vec![id(ORDERS), id(CUSTOMERS), id(REGIONS)]);
    // customers không trỏ tới orders hay order_items: bảng phía nhiều không lọc bảng phía một.
    assert_eq!(
        build_parent_ids(id(CUSTOMERS), &shop()),
        vec![id(CUSTOMERS), id(REGIONS)]
    );
}

#[test]
fn filter_spreads_one_and_two_steps_and_along_both_paths() {
    let tables = policy_tables(&[(CUSTOMERS, region_filter())]);
    assert_eq!(
        describe(&build_row_filters(id(ORDERS), &shop(), &tables)),
        ["customer_id->customers[region_code=N]"]
    );
    // order_items: qua orders (hai bước) và qua gift_customer_id; products không bị lọc nên không thêm gì.
    assert_eq!(
        describe(&build_row_filters(id(ORDER_ITEMS), &shop(), &tables)),
        [
            "order_id->orders[customer_id->customers[region_code=N]]",
            "gift_customer_id->customers[region_code=N]"
        ]
    );
    // Bảng có chính sách giữ nguyên chính sách của nó; regions không bị lọc nên customers không nhận gì thêm.
    assert_eq!(
        describe(&build_row_filters(id(CUSTOMERS), &shop(), &tables)),
        ["region_code=N"]
    );
    assert!(build_row_filters(id(PRODUCTS), &shop(), &tables).is_empty());
}

/// Chính sách riêng của orders cộng điều kiện lan từ customers; chính sách ở regions lan qua customers xuống orders.
#[test]
fn own_policy_and_spread_filters_add_up() {
    let status = RowFilter::Column {
        column: "status".to_string(),
        allowed_values: vec!["done".to_string()],
    };
    let code = RowFilter::Column {
        column: "code".to_string(),
        allowed_values: vec!["N".to_string()],
    };
    let tables = policy_tables(&[(ORDERS, status), (REGIONS, code)]);
    assert_eq!(
        describe(&build_row_filters(id(ORDERS), &shop(), &tables)),
        [
            "status=done",
            "customer_id->customers[region_code->regions[code=N]]"
        ]
    );
}

/// Bảng trỏ vào chính nó và vòng A -> B -> A không lồng mãi; bảng cha bị lọc mà không đọc được thì ẩn hết bảng con.
#[test]
fn cycles_stop_and_unreadable_parents_hide_every_row() {
    let relations = vec![
        relation(ORDERS, &["parent_order_id"], ORDERS, &["id"]),
        relation(ORDERS, &["customer_id"], CUSTOMERS, &["id"]),
        relation(CUSTOMERS, &["first_order_id"], ORDERS, &["id"]),
    ];
    let tables = policy_tables(&[(CUSTOMERS, region_filter())]);
    assert_eq!(
        describe(&build_row_filters(id(ORDERS), &relations, &tables)),
        ["customer_id->customers[region_code=N]"]
    );

    let mut tables = policy_tables(&[(CUSTOMERS, region_filter())]);
    tables.get_mut(&id(CUSTOMERS)).unwrap().table = None;
    assert_eq!(
        describe(&build_row_filters(id(ORDERS), &shop(), &tables)),
        ["customer_id="]
    );
}

const SOUTH: u128 = 100;
const NORTH: u128 = 101;
const ELECTRONICS: u128 = 102;

fn policy(role: u128, table: u128, column: &str, values: &[&str]) -> StoredPolicy {
    StoredPolicy {
        data_role_id: id(role),
        table_id: id(table),
        column: column.to_string(),
        allowed_values: values.iter().map(|v| v.to_string()).collect(),
    }
}

/// Mọi bảng cha của shop đọc được, như load_row_filters đọc khi có luật trên bảng cha.
fn readable_tables() -> HashMap<Uuid, Option<AllowedTable>> {
    [
        (ORDERS, "orders"),
        (CUSTOMERS, "customers"),
        (PRODUCTS, "products"),
        (REGIONS, "regions"),
    ]
    .iter()
    .map(|&(n, name)| (id(n), readable(name)))
    .collect()
}

/// Một vai: đúng điều kiện như trước khi có nhiều vai (không bọc AnyOf); AND trong vai giữ nguyên.
#[test]
fn one_role_keeps_its_filters_as_before() {
    let policies = [
        policy(SOUTH, CUSTOMERS, "region_code", &["S"]),
        policy(SOUTH, CUSTOMERS, "segment", &["Consumer"]),
    ];
    let filters = build_member_filters(id(CUSTOMERS), &shop(), &policies, &readable_tables());
    assert_eq!(describe(&filters), ["region_code=S", "segment=Consumer"]);
}

/// Hai vai cùng giới hạn một bảng: OR giữa hai vai, AND trong từng vai; đường lan sang bảng con cũng mang OR đó.
#[test]
fn two_roles_on_one_table_are_joined_with_or() {
    let policies = [
        policy(SOUTH, CUSTOMERS, "region_code", &["S"]),
        policy(SOUTH, CUSTOMERS, "segment", &["Consumer"]),
        policy(NORTH, CUSTOMERS, "region_code", &["N"]),
    ];
    let filters = build_member_filters(id(CUSTOMERS), &shop(), &policies, &readable_tables());
    assert_eq!(
        describe(&filters),
        ["(region_code=S AND segment=Consumer) OR (region_code=N)"]
    );
    let filters = build_member_filters(id(ORDERS), &shop(), &policies, &readable_tables());
    assert_eq!(
        describe(&filters),
        ["(customer_id->customers[region_code=S, segment=Consumer]) OR (customer_id->customers[region_code=N])"]
    );
}

/// Vai không có luật trên bảng đang đọc (hay bảng nó trỏ tới) không mở thêm dòng nào của bảng đó: người mang vai
/// "Miền Nam" (customers) và vai "Điện tử" (products) chỉ thấy khách miền Nam. Bảng nối tới cả hai thì OR.
#[test]
fn role_without_rules_on_the_table_does_not_widen_it() {
    let policies = [
        policy(SOUTH, CUSTOMERS, "region_code", &["S"]),
        policy(ELECTRONICS, PRODUCTS, "category", &["Electronics"]),
    ];
    let tables = readable_tables();
    let filters = build_member_filters(id(CUSTOMERS), &shop(), &policies, &tables);
    assert_eq!(describe(&filters), ["region_code=S"]);
    let filters = build_member_filters(id(PRODUCTS), &shop(), &policies, &tables);
    assert_eq!(describe(&filters), ["category=Electronics"]);
    let filters = build_member_filters(id(REGIONS), &shop(), &policies, &tables);
    assert!(filters.is_empty());
    let filters = build_member_filters(id(ORDER_ITEMS), &shop(), &policies, &tables);
    let text = describe(&filters).join("; ");
    assert!(text.starts_with("(") && text.contains(") OR ("), "{text}");
    assert!(
        text.contains("customers[region_code=S]")
            && text.contains("products[category=Electronics]"),
        "{text}"
    );
}

/// Hai vai cùng điều kiện trên bảng này thì giữ một nhóm; không vai nào có luật thì không lọc.
#[test]
fn same_filters_of_two_roles_are_kept_once() {
    let policies = [
        policy(SOUTH, CUSTOMERS, "region_code", &["S"]),
        policy(NORTH, CUSTOMERS, "region_code", &["S"]),
        policy(NORTH, PRODUCTS, "category", &["Toys"]),
    ];
    let filters = build_member_filters(id(CUSTOMERS), &shop(), &policies, &readable_tables());
    assert_eq!(describe(&filters), ["region_code=S"]);
    assert!(build_member_filters(id(CUSTOMERS), &shop(), &[], &readable_tables()).is_empty());
}
