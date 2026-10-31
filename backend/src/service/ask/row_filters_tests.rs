// SPDX-License-Identifier: Apache-2.0
/* service/ask/row_filters_tests.rs - test phần thuần của ask/row_filters.rs: điều kiện lan theo khóa ngoại dựng từ
quan hệ và chính sách đã đọc, không cần Postgres. Câu SQL guard dựng từ các điều kiện này được test ở
engine/guard/parent_filter_tests.rs; đường đọc Postgres qua route ở api/row_filter_fk_tests.rs. */

use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::store::DatasetRelation;
use crate::service::ask::row_filters::{build_parent_ids, build_row_filters, PolicyTable};

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
