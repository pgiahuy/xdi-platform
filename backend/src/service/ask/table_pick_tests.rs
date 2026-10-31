// SPDX-License-Identifier: Apache-2.0
/* service/ask/table_pick_tests.rs - test phần thuần của câu hỏi trên cả database (#249): thứ tự bảng của nhóm
(ask/related.rs build_group_order), chọn bảng gửi chi tiết (ask/table_pick.rs), bảng câu trả lời cần mà chỉ có trong
danh sách gọn và hai bảng không có quan hệ (ask/missing_tables.rs). Đường qua route ở api/ask_database_tests.rs. */

use uuid::Uuid;

use crate::engine::store::{ColumnAggregation, DatasetRelation};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::group_prompt::CompactTable;
use crate::service::ask::missing_tables::{
    build_unrelated_message, find_needed_tables, find_unrelated_pair,
};
use crate::service::ask::related::build_group_order;
use crate::service::ask::table_pick::{
    build_choice_detail, build_table_choice, DetailReason, TableFacts, WHOLE_DATASET_MAX_CHARS,
};

// Lược đồ bán hàng sáu bảng của database mẫu, thêm students không nối với bảng nào.
const ORDER_ITEMS: u128 = 1;
const ORDERS: u128 = 2;
const CUSTOMERS: u128 = 3;
const PRODUCTS: u128 = 4;
const CATEGORIES: u128 = 5;
const DEPARTMENTS: u128 = 6;
const STUDENTS: u128 = 7;

const NAMES: [(u128, &str); 7] = [
    (ORDER_ITEMS, "order_items"),
    (ORDERS, "orders"),
    (CUSTOMERS, "customers"),
    (PRODUCTS, "products"),
    (CATEGORIES, "categories"),
    (DEPARTMENTS, "departments"),
    (STUDENTS, "students"),
];

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn name(n: u128) -> &'static str {
    NAMES.iter().find(|(i, _)| *i == n).unwrap().1
}

fn relation(from: u128, to: u128) -> DatasetRelation {
    DatasetRelation {
        from_dataset_id: id(from),
        from_dataset_name: name(from).to_string(),
        from_columns: vec![format!("{}_id", name(to))],
        to_dataset_id: id(to),
        to_dataset_name: name(to).to_string(),
        to_columns: vec![format!("{}_id", name(to))],
        constraint_name: format!("{}_{}_fkey", name(from), name(to)),
        cardinality: "many_to_one".to_string(),
    }
}

fn sales() -> Vec<DatasetRelation> {
    vec![
        relation(ORDER_ITEMS, ORDERS),
        relation(ORDER_ITEMS, PRODUCTS),
        relation(ORDERS, CUSTOMERS),
        relation(PRODUCTS, CATEGORIES),
        relation(CATEGORIES, DEPARTMENTS),
    ]
}

fn column(name: &str, role: &str, description_chars: usize) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        time_format: None,
        description: Some("x".repeat(description_chars)),
        aliases: Vec::new(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: false,
        unit: None,
        aggregation: ColumnAggregation::Sum,
        join_key: false,
        metric_column: false,
        picked: false,
        brief: false,
    }
}

/// Cột của từng bảng; description_chars đặt độ dài mô tả mỗi cột, để đẩy tổng metadata qua trần hay không.
struct Schema {
    columns: Vec<(u128, Vec<ColumnProfile>)>,
    descriptions: Vec<(u128, &'static str)>,
}

fn schema(description_chars: usize) -> Schema {
    let columns = NAMES
        .iter()
        .map(|(n, table)| {
            let mut list = vec![column(&format!("{table}_id"), "key", description_chars)];
            list.push(column("label", "dimension", description_chars));
            (*n, list)
        })
        .collect();
    Schema {
        columns,
        descriptions: vec![
            (DEPARTMENTS, "Phòng ban (nhóm hàng lớn) của cửa hàng"),
            (CUSTOMERS, "Khách hàng"),
        ],
    }
}

impl Schema {
    fn facts(&self) -> Vec<TableFacts<'_>> {
        self.columns
            .iter()
            .map(|(n, columns)| TableFacts {
                id: id(*n),
                table: name(*n),
                description: self
                    .descriptions
                    .iter()
                    .find(|(d, _)| d == n)
                    .map(|(_, text)| *text),
                columns,
                metrics: &[],
            })
            .collect()
    }

    fn column_mut(&mut self, table: u128, column: &str) -> &mut ColumnProfile {
        let (_, columns) = self.columns.iter_mut().find(|(n, _)| *n == table).unwrap();
        columns.iter_mut().find(|c| c.name == column).unwrap()
    }
}

fn phrases(list: &[&str]) -> Vec<String> {
    list.iter().map(|p| p.to_string()).collect()
}

/// Thứ tự bảng: bảng bắt đầu, rồi theo số bước quan hệ, không giới hạn bước (departments ba bước từ order_items), bảng
/// không nối được ở cuối.
#[test]
fn group_order_reaches_every_linked_table_then_the_rest() {
    let others: Vec<(Uuid, String)> = NAMES
        .iter()
        .filter(|(n, _)| *n != ORDER_ITEMS)
        .map(|(n, t)| (id(*n), t.to_string()))
        .collect();
    let order = build_group_order(id(ORDER_ITEMS), &others, &sales());
    let expected: Vec<Uuid> = [
        ORDER_ITEMS,
        ORDERS,
        PRODUCTS,
        CUSTOMERS,
        CATEGORIES,
        DEPARTMENTS,
        STUDENTS,
    ]
    .iter()
    .map(|&n| id(n))
    .collect();
    assert_eq!(order, expected);
}

/// Database nhỏ: mọi bảng gửi chi tiết, kể cả departments ba bước và students không nối được, không cần câu hỏi khớp.
#[test]
fn whole_database_is_sent_when_it_fits() {
    let schema = schema(10);
    let choice = build_table_choice(&schema.facts(), &sales(), &phrases(&["phong ban"]), &[]);
    assert!(choice.all_fit);
    assert_eq!(choice.detailed.len(), NAMES.len());
    assert_eq!(choice.detailed[0].reason, DetailReason::Start);
    assert!(choice.contains(id(DEPARTMENTS)));
    let detail = build_choice_detail(&choice, &[], &["student_daily_clicks".to_string()]);
    assert!(
        detail.contains("mọi bảng của database (7 bảng)"),
        "{detail}"
    );
    assert!(detail.contains("Chưa chụp"), "{detail}");
}

/// Vượt trần: bảng gửi chi tiết là hợp của bảng bắt đầu, bảng khớp chữ (mô tả bảng departments, cột khớp của
/// customers), bảng nằm trên đường khóa ngoại tới chúng và bảng mô hình chọn; bảng không nguồn nào chọn chỉ có tên.
#[test]
fn over_the_limit_sends_the_union_of_sources() {
    // Mỗi bảng hai cột, mỗi cột 1 000 ký tự mô tả: 7 bảng cỡ 14 000 ký tự, vượt trần; năm bảng thì chưa.
    let mut schema = schema(1_000);
    schema.column_mut(CUSTOMERS, "label").matched = true;
    let facts = schema.facts();
    let total: usize = facts
        .iter()
        .flat_map(|t| t.columns.iter())
        .map(crate::service::ask::context::metadata_chars)
        .sum();
    assert!(total > WHOLE_DATASET_MAX_CHARS, "{total}");

    let choice = build_table_choice(&facts, &sales(), &phrases(&["phong ban"]), &[]);
    assert!(!choice.all_fit);
    let reason = |n: u128| {
        choice
            .detailed
            .iter()
            .find(|t| t.id == id(n))
            .map(|t| t.reason.clone())
    };
    assert_eq!(reason(ORDER_ITEMS), Some(DetailReason::Start));
    assert_eq!(
        reason(DEPARTMENTS),
        Some(DetailReason::Matched {
            words: vec!["phong ban".to_string()],
            columns: Vec::new(),
        })
    );
    assert_eq!(
        reason(CUSTOMERS),
        Some(DetailReason::Matched {
            words: Vec::new(),
            columns: vec!["label".to_string()],
        })
    );
    // Đường order_items -> products -> categories -> departments và order_items -> orders -> customers.
    for n in [PRODUCTS, CATEGORIES] {
        assert_eq!(
            reason(n),
            Some(DetailReason::Path {
                to: "departments".to_string()
            })
        );
    }
    assert_eq!(
        reason(ORDERS),
        Some(DetailReason::Path {
            to: "customers".to_string()
        })
    );
    assert_eq!(reason(STUDENTS), None);

    // Lượt phân loại chọn students: thêm vào, lý do mô hình chọn.
    let choice = build_table_choice(
        &facts,
        &sales(),
        &phrases(&["phong ban"]),
        &["students".to_string()],
    );
    assert!(choice.contains(id(STUDENTS)));
    let detail = build_choice_detail(&choice, &[], &[]);
    assert!(detail.contains("students (mô hình chọn)"), "{detail}");
    assert!(
        detail.contains("departments (khớp chữ \"phong ban\")"),
        "{detail}"
    );
}

/// Bảng nối một bước với bảng đã chọn chỉ được thêm khi tổng metadata còn dưới trần; bảng có cột thời gian trước.
#[test]
fn neighbors_are_added_within_the_limit_time_tables_first() {
    let mut schema = schema(1_000);
    // orders có cột thời gian; products không.
    schema
        .columns
        .iter_mut()
        .find(|(n, _)| *n == ORDERS)
        .unwrap()
        .1
        .push(column("order_date", "time", 10));
    let choice = build_table_choice(&schema.facts(), &sales(), &[], &[]);
    // Chỉ bảng bắt đầu là hạt giống; trần đủ cho vài bảng nữa, orders đứng trước products.
    let ids = choice.ids();
    assert_eq!(ids[0], id(ORDER_ITEMS));
    assert_eq!(ids[1], id(ORDERS));
    assert_eq!(
        choice.detailed[1].reason,
        DetailReason::Neighbor {
            of: "order_items".to_string()
        }
    );
    assert!(!choice.contains(id(DEPARTMENTS)));
}

fn compact(table: &str, unsnapshotted: bool) -> CompactTable {
    CompactTable {
        table: table.to_string(),
        description: String::new(),
        unsnapshotted,
    }
}

/// Bảng chỉ có trong danh sách gọn mà câu trả lời cần: mô hình ghi trong needed_tables hay SQL đọc sau FROM, JOIN.
/// Bảng chưa chụp không tính (không có chi tiết để gửi).
#[test]
fn needed_tables_come_from_the_field_and_the_sql() {
    let others = vec![
        compact("departments", false),
        compact("categories", false),
        compact("student_daily_clicks", true),
    ];
    let asked =
        r#"{"terms": [], "assumptions": [], "needed_tables": ["sales.Departments"], "sql": ""}"#;
    assert_eq!(find_needed_tables(asked, &others), ["departments"]);
    let sql = r#"Here: {"terms": [], "assumptions": [], "sql": "SELECT d.department_name, SUM(oi.item_total) FROM order_items oi JOIN \"categories\" c ON 1 = 1 JOIN student_daily_clicks s ON 1 = 1 GROUP BY 1"}"#;
    assert_eq!(find_needed_tables(sql, &others), ["categories"]);
    let none = r#"{"terms": [], "assumptions": [], "sql": "SELECT EXTRACT(YEAR FROM order_date) FROM orders"}"#;
    assert!(find_needed_tables(none, &others).is_empty());
    assert!(find_needed_tables("not json", &others).is_empty());
}

/// Hai bảng không nối được qua chuỗi khóa ngoại nào: lời nêu đúng hai bảng. Bảng nối được qua bảng khác thì không báo.
#[test]
fn unrelated_tables_are_named() {
    let tables: Vec<(Uuid, String)> = NAMES.iter().map(|(n, t)| (id(*n), t.to_string())).collect();
    let sql = "SELECT s.region, SUM(oi.item_total) FROM order_items oi JOIN students s ON oi.order_id = s.id_student GROUP BY 1";
    let pair = find_unrelated_pair(sql, &tables, &sales()).unwrap();
    assert_eq!(pair, ("order_items".to_string(), "students".to_string()));
    let message = build_unrelated_message(&pair.0, &pair.1);
    assert!(
        message.contains("order_items và bảng students không có quan hệ"),
        "{message}"
    );
    let linked = "SELECT d.department_name FROM order_items oi JOIN departments d ON 1 = 1";
    assert!(find_unrelated_pair(linked, &tables, &sales()).is_none());
    assert!(find_unrelated_pair("SELECT 1 FROM students", &tables, &sales()).is_none());
}
