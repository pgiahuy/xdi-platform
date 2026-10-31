// SPDX-License-Identifier: Apache-2.0
/* service/ask/related_tests.rs - test phần thuần của ask/related.rs và ask/group_prompt.rs: chọn bảng liên quan theo số
bước và số bảng, luật lọc dòng tạm thời (bỏ bảng trỏ tới bảng bị lọc), và prompt SQL nhiều bảng của ask/prompt.rs (khối
cột mỗi bảng, quan hệ, cột khóa, trần độ dài). Đường đọc Postgres của load_group được test qua route ở
api/ask_related_tests.rs. */

use std::collections::HashSet;

use uuid::Uuid;

use crate::engine::store::{ColumnAggregation, DatasetRelation, MetricDefinition, ValueRow};
use crate::service::ask::context::{
    apply_group_column_cap, apply_group_values, AskContext, ColumnProfile, CONTEXT_VALUES_MAX,
    FULL_SCHEMA_MAX_COLUMNS,
};
use crate::service::ask::group_prompt::{GroupPrompt, JoinLine, TableContext};
use crate::service::ask::prompt::{build_sql_prompt, PromptInput, PROMPT_MAX_CHARS};
use crate::service::ask::related::{
    build_blocked_ids, build_group_ids, build_left_out_note, MAX_GROUP_TABLES,
};
use crate::service::ask::values::{ValueEntry, ValueIndex};
use crate::service::prompt_fixtures::check_fixture;

/// Id cố định theo số, để lời báo của assert đọc được.
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Quan hệ nhiều-một từ dataset from tới dataset to.
fn relation(from: u128, to: u128) -> DatasetRelation {
    DatasetRelation {
        from_dataset_id: id(from),
        from_dataset_name: format!("d{from}"),
        from_columns: vec![format!("d{to}_id")],
        to_dataset_id: id(to),
        to_dataset_name: format!("d{to}"),
        to_columns: vec!["id".to_string()],
        constraint_name: format!("d{from}_d{to}_fkey"),
        cardinality: "many_to_one".to_string(),
    }
}

// Lược đồ bán hàng: 1 order_items, 2 orders, 3 customers, 4 products, 5 categories, 6 departments, 7 regions
// (customers -> regions), 9 returns (returns -> orders).
const ORDER_ITEMS: u128 = 1;
const ORDERS: u128 = 2;
const CUSTOMERS: u128 = 3;
const PRODUCTS: u128 = 4;
const CATEGORIES: u128 = 5;
const DEPARTMENTS: u128 = 6;
const REGIONS: u128 = 7;
const RETURNS: u128 = 9;

fn sales() -> Vec<DatasetRelation> {
    vec![
        relation(ORDER_ITEMS, ORDERS),
        relation(ORDER_ITEMS, PRODUCTS),
        relation(ORDERS, CUSTOMERS),
        relation(PRODUCTS, CATEGORIES),
        relation(CATEGORIES, DEPARTMENTS),
        relation(CUSTOMERS, REGIONS),
        relation(RETURNS, ORDERS),
    ]
}

fn ids(list: &[u128]) -> Vec<Uuid> {
    list.iter().map(|&n| id(n)).collect()
}

/// Hai bước, cả hai chiều, gần trước xa sau: từ order_items tới customers và categories, chưa tới regions hay
/// departments (ba bước). returns trỏ vào orders: hai bước theo chiều ngược.
#[test]
fn group_follows_both_directions_up_to_two_hops() {
    let group = build_group_ids(id(ORDER_ITEMS), &sales(), |_| true);
    assert_eq!(
        group,
        ids(&[
            ORDER_ITEMS,
            ORDERS,
            PRODUCTS,
            CUSTOMERS,
            RETURNS,
            CATEGORIES
        ])
    );
    // Từ customers: orders (ngược) và regions một bước; order_items, returns qua orders.
    let group = build_group_ids(id(CUSTOMERS), &sales(), |_| true);
    assert_eq!(
        group,
        ids(&[CUSTOMERS, ORDERS, REGIONS, ORDER_ITEMS, RETURNS])
    );
}

/// Bảng không dùng được (chưa có phiên bản) không vào nhóm, và bảng chỉ tới được qua nó cũng không.
#[test]
fn unusable_tables_cut_the_path() {
    let group = build_group_ids(id(ORDER_ITEMS), &sales(), |d| d != id(ORDERS));
    assert_eq!(group, ids(&[ORDER_ITEMS, PRODUCTS, CATEGORIES]));
}

/// Hình sao: một bảng sự kiện với mười bảng chiều chỉ lấy MAX_GROUP_TABLES bảng, bảng đang chọn ở đầu.
#[test]
fn group_stops_at_the_table_limit() {
    let relations: Vec<DatasetRelation> = (10..20).map(|d| relation(1, d)).collect();
    let group = build_group_ids(id(1), &relations, |_| true);
    assert_eq!(group.len(), MAX_GROUP_TABLES);
    assert_eq!(group[0], id(1));
}

/// Người hỏi bị lọc trên customers: orders trỏ thẳng tới customers, order_items và returns trỏ qua orders, nên cả ba
/// bị bỏ, trừ bảng đang chọn. products, categories không dính tới customers nên giữ.
#[test]
fn tables_pointing_to_a_filtered_table_are_blocked() {
    let relations = sales();
    let filtered: HashSet<Uuid> = [id(CUSTOMERS)].into();
    let group = build_group_ids(id(CUSTOMERS), &relations, |_| true);
    let blocked = build_blocked_ids(id(CUSTOMERS), &group, &relations, &filtered);
    assert_eq!(blocked, [id(ORDERS), id(ORDER_ITEMS), id(RETURNS)].into());

    // Hỏi trên order_items: order_items là bảng đang chọn nên giữ; orders bị bỏ; products vẫn nối được.
    let group = build_group_ids(id(ORDER_ITEMS), &relations, |_| true);
    let blocked = build_blocked_ids(id(ORDER_ITEMS), &group, &relations, &filtered);
    assert_eq!(blocked, [id(ORDERS), id(RETURNS)].into());
    let kept = build_group_ids(id(ORDER_ITEMS), &relations, |d| !blocked.contains(&d));
    assert_eq!(kept, ids(&[ORDER_ITEMS, PRODUCTS, CATEGORIES]));
}

/// Không ai bị lọc thì không bỏ bảng nào; bảng tự trỏ vào chính nó (quản lý của nhân viên) không tự chặn mình.
#[test]
fn nothing_is_blocked_without_filters() {
    let relations = sales();
    let group = build_group_ids(id(ORDER_ITEMS), &relations, |_| true);
    assert!(build_blocked_ids(id(ORDER_ITEMS), &group, &relations, &HashSet::new()).is_empty());

    let relations = vec![relation(20, 20), relation(21, 20)];
    let filtered: HashSet<Uuid> = [id(20)].into();
    let group = build_group_ids(id(20), &relations, |_| true);
    let blocked = build_blocked_ids(id(20), &group, &relations, &filtered);
    assert_eq!(blocked, [id(21)].into());
}

#[test]
fn left_out_note_names_the_tables() {
    assert_eq!(build_left_out_note(&[]), None);
    let note = build_left_out_note(&["orders".to_string(), "returns".to_string()]).unwrap();
    assert!(note.contains("giới hạn dữ liệu theo vai"), "{note}");
    assert!(note.ends_with("orders, returns."), "{note}");
}

// Prompt SQL nhiều bảng

fn column(name: &str, data_type: &str, role: &str) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: data_type.to_string(),
        role: role.to_string(),
        time_format: None,
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: false,
        unit: None,
        aggregation: ColumnAggregation::Sum,
        join_key: false,
    }
}

fn key(name: &str) -> ColumnProfile {
    let mut column = column(name, "INTEGER", "key");
    column.join_key = true;
    column
}

fn join(from: &str, from_column: &str, to: &str, to_column: &str) -> JoinLine {
    JoinLine {
        from_table: from.to_string(),
        from_columns: vec![from_column.to_string()],
        to_table: to.to_string(),
        to_columns: vec![to_column.to_string()],
        one_to_one: false,
    }
}

fn metric(name: &str, expression: &str) -> MetricDefinition {
    MetricDefinition {
        name: name.to_string(),
        description: String::new(),
        expression: expression.to_string(),
        filter: None,
        time_column: None,
        unit: None,
        confirmed: true,
    }
}

/// Bảng đang chọn order_items (câu hỏi khớp cột item_total), cùng orders và customers như ảnh chụp database mẫu.
fn sales_context() -> AskContext {
    let mut total = column("item_total", "DECIMAL(18,3)", "measure");
    total.matched = true;
    total.description = Some("Doanh thu sau giảm giá của dòng hàng".to_string());
    AskContext {
        columns: vec![
            key("order_id"),
            column("quantity", "INTEGER", "measure"),
            total,
        ],
        hints: Vec::new(),
        ambiguous: Vec::new(),
        picks: Vec::new(),
    }
}

fn sales_group() -> GroupPrompt {
    let mut segment = column("segment", "VARCHAR", "dimension");
    segment.matched = true;
    segment.values = vec![
        ValueEntry {
            value: "Consumer".to_string(),
            aliases: vec!["khách lẻ".to_string()],
        },
        ValueEntry {
            value: "Corporate".to_string(),
            aliases: Vec::new(),
        },
    ];
    segment.values_complete = true;
    let mut ordered = column("order_date", "TIMESTAMP", "time");
    ordered.description = Some("Ngày đặt hàng".to_string());
    GroupPrompt {
        tables: vec![
            TableContext {
                table: "orders".to_string(),
                columns: vec![
                    key("order_id"),
                    key("customer_id"),
                    ordered,
                    column("order_status", "VARCHAR", "dimension"),
                ],
                metrics: vec![metric("số đơn", "COUNT(DISTINCT \"order_id\")")],
            },
            TableContext {
                table: "customers".to_string(),
                columns: vec![
                    key("customer_id"),
                    segment,
                    column("city", "VARCHAR", "dimension"),
                ],
                metrics: Vec::new(),
            },
        ],
        joins: vec![
            join("order_items", "order_id", "orders", "order_id"),
            join("orders", "customer_id", "customers", "customer_id"),
        ],
    }
}

fn group_input<'a>(
    question: &'a str,
    context: &'a AskContext,
    metrics: &'a [MetricDefinition],
    group: &'a GroupPrompt,
) -> PromptInput<'a> {
    PromptInput {
        table: "order_items",
        question,
        context,
        metrics,
        examples: &[],
        domain: None,
        choices: &[],
        history: "",
        group: Some(group),
    }
}

const SEGMENT_QUESTION: &str = "Doanh thu theo phân khúc khách hàng";

/// Mỗi bảng một khối cột, bảng đang chọn trước; quan hệ in đúng dạng; luật nối bảng; luật và guard kể mọi bảng; chỉ số
/// kèm tên bảng.
#[test]
fn group_prompt_has_a_block_per_table_and_the_relationships() {
    let context = sales_context();
    let group = sales_group();
    let metrics = [metric("doanh thu", "SUM(\"item_total\")")];
    let prompt = build_sql_prompt(&group_input(SEGMENT_QUESTION, &context, &metrics, &group));
    let items = prompt.find("Columns of table order_items:\n").unwrap();
    let orders = prompt.find("Columns of table orders:\n").unwrap();
    let customers = prompt.find("Columns of table customers:\n").unwrap();
    assert!(items < orders && orders < customers, "{prompt}");
    assert!(!prompt.contains("\nColumns:\n"), "{prompt}");
    assert!(prompt.contains(
        "- order_items.order_id = orders.order_id (many order_items rows per orders row)\n\
         - orders.customer_id = customers.customer_id (many orders rows per customers row)"
    ));
    assert!(prompt.contains("aggregate A first in a subquery or WITH table"));
    assert!(prompt.contains("about the tables order_items, orders, customers."));
    assert!(prompt.contains("from the tables order_items, orders, customers,"));
    assert!(prompt.contains("- \"doanh thu\" (table order_items): SUM(\"item_total\")"));
    assert!(prompt.contains("- \"số đơn\" (table orders): COUNT(DISTINCT \"order_id\")"));
    assert!(prompt.contains("All values: 'Consumer' (also: khách lẻ), 'Corporate'"));
    check_fixture("sql_join_prompt.txt", &prompt);
}

/// Quan hệ một-một và khóa nhiều cột; tên cột không trơn được đặt trong nháy kép.
#[test]
fn join_lines_cover_one_to_one_and_composite_keys() {
    let context = sales_context();
    let mut group = sales_group();
    group.joins = vec![
        JoinLine {
            from_table: "order_items".to_string(),
            from_columns: vec!["maker".to_string(), "Sku Code".to_string()],
            to_table: "products".to_string(),
            to_columns: vec!["maker".to_string(), "sku".to_string()],
            one_to_one: false,
        },
        JoinLine {
            one_to_one: true,
            ..join("invoices", "order_id", "orders", "order_id")
        },
    ];
    let prompt = build_sql_prompt(&group_input(SEGMENT_QUESTION, &context, &[], &group));
    assert!(prompt.contains(
        "- order_items.maker = products.maker AND order_items.\"Sku Code\" = products.sku (many order_items rows \
         per products row)"
    ));
    assert!(prompt.contains(
        "- invoices.order_id = orders.order_id (at most one invoices row per orders row)"
    ));
}

/// Prompt dài phải hạ xuống mức chỉ gửi cột khớp: cột khóa vẫn còn ở mọi bảng, cột không khớp không khóa thì mất.
#[test]
fn matched_columns_keep_the_join_keys() {
    let mut context = sales_context();
    let mut group = sales_group();
    let long = "y".repeat(4_000);
    for columns in
        std::iter::once(&mut context.columns).chain(group.tables.iter_mut().map(|t| &mut t.columns))
    {
        for column in columns.iter_mut() {
            column.description = Some(long.clone());
        }
    }
    let prompt = build_sql_prompt(&group_input(SEGMENT_QUESTION, &context, &[], &group));
    assert!(!prompt.contains("- \"quantity\""), "unmatched column stays");
    assert!(!prompt.contains("- \"city\""), "unmatched column stays");
    for kept in [
        "order_id",
        "customer_id",
        "segment",
        "item_total",
        "order_date",
    ] {
        assert!(prompt.contains(&format!("- \"{kept}\"")), "{kept} is gone");
    }
    assert!(prompt.contains("Columns of table customers:"));
}

/// Sáu bảng như database mẫu, mỗi bảng mười cột có mô tả, giá trị chia chung trần 400: prompt vẫn trong
/// PROMPT_MAX_CHARS, đủ sáu khối và câu hỏi ở cuối.
#[test]
fn six_tables_fit_in_the_prompt_limit() {
    let names = [
        "order_items",
        "orders",
        "customers",
        "products",
        "categories",
        "departments",
    ];
    // 400 giá trị chia cho 6 bảng, mỗi bảng 2 cột chiều.
    let per_column = 400 / (names.len() * 2);
    let build_columns = |table: &str| -> Vec<ColumnProfile> {
        (0..10)
            .map(|i| {
                let role = if i < 2 { "dimension" } else { "measure" };
                let mut c = column(&format!("{table}_col_{i}"), "VARCHAR", role);
                c.description = Some(format!(
                    "Mô tả cột {i} của bảng {table}, viết dài như mô tả mô hình sinh"
                ));
                if i < 2 {
                    c.values = (0..per_column)
                        .map(|v| ValueEntry {
                            value: format!("{table} value {v}"),
                            aliases: Vec::new(),
                        })
                        .collect();
                    c.values_complete = true;
                }
                c
            })
            .collect()
    };
    let context = AskContext {
        columns: build_columns(names[0]),
        hints: Vec::new(),
        ambiguous: Vec::new(),
        picks: Vec::new(),
    };
    let group = GroupPrompt {
        tables: names[1..]
            .iter()
            .map(|name| TableContext {
                table: name.to_string(),
                columns: build_columns(name),
                metrics: vec![metric(&format!("tổng {name}"), "COUNT(*)")],
            })
            .collect(),
        joins: names
            .windows(2)
            .map(|pair| join(pair[0], "id", pair[1], "id"))
            .collect(),
    };
    let prompt = build_sql_prompt(&group_input(SEGMENT_QUESTION, &context, &[], &group));
    assert!(
        prompt.chars().count() <= PROMPT_MAX_CHARS,
        "{} chars",
        prompt.chars().count()
    );
    for name in names {
        assert!(
            prompt.contains(&format!("Columns of table {name}:")),
            "{name}"
        );
    }
    assert!(prompt.contains("- categories.id = departments.id"));
    assert!(prompt.ends_with(&format!("Question: {SEGMENT_QUESTION}")));
}

// Trần chung của ngữ cảnh

/// Tổng cột vượt 60: mọi bảng chỉ giữ cột khớp, cột thời gian và cột khóa. Không vượt thì giữ hết.
#[test]
fn column_cap_counts_every_table() {
    let build = |table: &str, n: usize| -> Vec<ColumnProfile> {
        (0..n)
            .map(|i| column(&format!("{table}_{i}"), "INTEGER", "measure"))
            .collect()
    };
    let mut items = build("items", 30);
    items[0].matched = true;
    let mut orders = build("orders", 25);
    orders[0].join_key = true;
    orders[1].role = "time".to_string();
    let mut small = build("small", 3);
    apply_group_column_cap(&mut [&mut items, &mut small]);
    assert_eq!((items.len(), small.len()), (30, 3));

    let mut customers = build("customers", FULL_SCHEMA_MAX_COLUMNS - 50);
    apply_group_column_cap(&mut [&mut items, &mut orders, &mut customers]);
    assert_eq!((items.len(), orders.len(), customers.len()), (1, 2, 0));
}

/// Hai bảng, mỗi bảng mười cột chiều 40 giá trị (800 giá trị, mỗi bảng riêng đã 400): tổng gửi không vượt
/// CONTEXT_VALUES_MAX, và cột của cả hai bảng đều có giá trị.
#[test]
fn value_budget_is_shared_by_the_tables() {
    let table = |name: &str| {
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        for c in 0..10 {
            let column = column(&format!("{name}_{c}"), "VARCHAR", "dimension");
            rows.extend((0..40).map(|i| ValueRow {
                column_id: column.id,
                column_name: column.name.clone(),
                value: format!("{name} {c} {i}"),
                aliases: Vec::new(),
            }));
            columns.push(column);
        }
        (columns, ValueIndex::build(&rows))
    };
    let (mut first, first_index) = table("segment");
    let (mut second, second_index) = table("city");
    apply_group_values(&mut [(&mut first, &first_index), (&mut second, &second_index)]);
    let count = |columns: &[ColumnProfile]| columns.iter().map(|c| c.values.len()).sum::<usize>();
    assert!(count(&first) + count(&second) <= CONTEXT_VALUES_MAX);
    assert_eq!(count(&first), count(&second));
    assert!(first
        .iter()
        .all(|c| !c.values.is_empty() && !c.values_complete));
}
