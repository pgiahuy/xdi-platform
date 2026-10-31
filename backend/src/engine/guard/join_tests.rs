// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_tests.rs - test luật nối bảng (guard/joins.rs, #256): câu nối đúng khóa ngoại qua, câu nối sai
cột, thiếu cột của khóa nhiều cột, nối hai bảng không có quan hệ, tích chéo, nối qua WHERE, trong câu con, WITH và IN,
nối bảng với chính nó bị từ chối với lý do nêu đúng khóa. Lược đồ giống schema sales và edu của database mẫu
(data/postgres-demo), thêm employees có khóa tự trỏ và returns có hai khóa ngoại. */

use std::path::PathBuf;

use super::{
    build_guarded_sql, build_parent_tables, AllowedTable, GuardError, JoinLink, RowFilter,
};

/// Quan hệ nhiều-một many(many_columns) -> one(one_columns), cho test của guard.
pub(super) fn link(many: &str, many_columns: &[&str], one: &str, one_columns: &[&str]) -> JoinLink {
    let owned = |columns: &[&str]| columns.iter().map(|c| c.to_string()).collect();
    JoinLink {
        many: many.to_string(),
        many_columns: owned(many_columns),
        one: one.to_string(),
        one_columns: owned(one_columns),
        fans_out: true,
    }
}

fn table(name: &str, columns: &[&str]) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir: PathBuf::from(format!("/data/{name}")),
        row_filters: vec![],
    }
}

const STUDENT_KEY: [&str; 3] = ["code_module", "code_presentation", "id_student"];

fn tables() -> Vec<AllowedTable> {
    vec![
        table(
            "order_items",
            &[
                "order_item_id",
                "order_id",
                "product_id",
                "quantity",
                "item_total",
            ],
        ),
        table(
            "orders",
            &["order_id", "customer_id", "order_date", "market"],
        ),
        table("customers", &["customer_id", "segment", "country"]),
        table("products", &["product_id", "price"]),
        table(
            "returns",
            &["return_id", "order_id", "customer_id", "amount"],
        ),
        table("employees", &["employee_id", "manager_id", "name"]),
        table("targets", &["market", "target"]),
        table(
            "students",
            &["code_module", "code_presentation", "id_student", "region"],
        ),
        table(
            "student_assessments",
            &[
                "id_assessment",
                "code_module",
                "code_presentation",
                "id_student",
                "score",
            ],
        ),
    ]
}

fn links() -> Vec<JoinLink> {
    vec![
        link("order_items", &["order_id"], "orders", &["order_id"]),
        link("order_items", &["product_id"], "products", &["product_id"]),
        link("orders", &["customer_id"], "customers", &["customer_id"]),
        link("returns", &["order_id"], "orders", &["order_id"]),
        link("returns", &["customer_id"], "customers", &["customer_id"]),
        link("employees", &["manager_id"], "employees", &["employee_id"]),
        link(
            "student_assessments",
            &STUDENT_KEY,
            "students",
            &STUDENT_KEY,
        ),
    ]
}

fn check(sql: &str) -> Result<(), String> {
    match build_guarded_sql(sql, &tables(), &links(), 1000) {
        Ok(_) => Ok(()),
        Err(GuardError::Rejected(reason)) => Err(reason),
        Err(other) => panic!("unexpected guard error for {sql}: {other:?}"),
    }
}

fn assert_passes(sql: &str) {
    if let Err(reason) = check(sql) {
        panic!("rejected {sql}: {reason}");
    }
}

/// Trả lý do để test kiểm lý do nêu đúng bảng và khóa.
fn assert_refused(sql: &str) -> String {
    match check(sql) {
        Ok(()) => panic!("accepted {sql}"),
        Err(reason) => reason,
    }
}

#[test]
fn joins_on_a_single_column_key_in_either_order() {
    assert_passes("SELECT o.market, SUM(i.item_total) FROM order_items i JOIN orders o ON i.order_id = o.order_id GROUP BY 1");
    assert_passes(
        "SELECT o.market FROM order_items i LEFT JOIN orders o ON o.order_id = i.order_id",
    );
    assert_passes(
        "SELECT c.segment, SUM(i.quantity) FROM order_items i JOIN orders o ON i.order_id = o.order_id \
         JOIN customers c ON o.customer_id = c.customer_id JOIN products p ON p.product_id = i.product_id GROUP BY 1",
    );
    // Không bí danh, tên bảng viết hoa trong nháy kép.
    assert_passes("SELECT \"ORDERS\".market FROM order_items JOIN \"ORDERS\" ON order_items.order_id = \"ORDERS\".order_id");
}

#[test]
fn joins_on_a_multi_column_key_in_any_pair_order() {
    assert_passes(
        "SELECT s.region, AVG(sa.score) FROM student_assessments sa JOIN students s ON sa.id_student = s.id_student \
         AND s.code_module = sa.code_module AND sa.code_presentation = s.code_presentation GROUP BY 1",
    );
}

#[test]
fn allows_extra_conditions_that_filter_one_table() {
    assert_passes(
        "SELECT o.market FROM order_items i JOIN orders o ON i.order_id = o.order_id AND o.market = 'Europe' \
         AND (o.order_date > '2017-01-01' OR o.market IS NULL) AND i.quantity > 1",
    );
}

#[test]
fn refuses_a_join_on_the_wrong_columns_and_names_the_key() {
    let reason = assert_refused(
        "SELECT o.market FROM order_items i JOIN orders o ON i.order_item_id = o.customer_id",
    );
    assert!(
        reason.contains("order_items") && reason.contains("orders"),
        "{reason}"
    );
    assert!(
        reason.contains("order_items.order_id = orders.order_id"),
        "{reason}"
    );
    // Có khóa nhưng thêm một phép bằng giữa hai bảng: không còn đúng bằng khóa.
    assert_refused(
        "SELECT o.market FROM order_items i JOIN orders o ON i.order_id = o.order_id AND i.product_id = o.customer_id",
    );
}

#[test]
fn refuses_a_multi_column_key_with_a_missing_column() {
    let reason = assert_refused(
        "SELECT s.region, AVG(sa.score) FROM student_assessments sa JOIN students s ON sa.id_student = s.id_student \
         AND sa.code_module = s.code_module GROUP BY 1",
    );
    assert!(
        reason.contains("student_assessments.code_presentation = students.code_presentation"),
        "{reason}"
    );
}

#[test]
fn refuses_tables_without_a_relation() {
    let reason =
        assert_refused("SELECT t.target FROM orders o JOIN targets t ON o.market = t.market");
    assert!(
        reason.contains("không có quan hệ trong database"),
        "{reason}"
    );
    assert!(
        reason.contains("orders") && reason.contains("targets"),
        "{reason}"
    );
    assert_refused(
        "SELECT c.segment FROM order_items i JOIN customers c ON i.order_id = c.customer_id",
    );
}

#[test]
fn refuses_cross_joins_in_every_spelling() {
    for sql in [
        "SELECT COUNT(*) FROM orders o CROSS JOIN customers c",
        "SELECT COUNT(*) FROM orders o JOIN customers c ON true",
        "SELECT COUNT(*) FROM orders o JOIN customers c ON o.market = 'Europe'",
        "SELECT COUNT(*) FROM orders o, customers c",
        "SELECT COUNT(*) FROM orders o, targets t WHERE t.target > 0",
    ] {
        let reason = assert_refused(sql);
        assert!(reason.contains("không có phép bằng nào"), "{sql}: {reason}");
    }
}

#[test]
fn refuses_or_and_comparisons_between_two_tables_in_on() {
    assert_refused(
        "SELECT o.market FROM order_items i JOIN orders o ON i.order_id = o.order_id OR i.product_id = o.order_id",
    );
    assert_refused("SELECT o.market FROM order_items i JOIN orders o ON i.order_id > o.order_id");
    assert_refused(
        "SELECT o.market FROM order_items i JOIN orders o ON i.order_id + 0 = o.order_id",
    );
}

#[test]
fn checks_comma_joins_through_where() {
    assert_passes(
        "SELECT o.market FROM order_items i, orders o WHERE i.order_id = o.order_id AND o.market = 'Europe'",
    );
    let reason = assert_refused(
        "SELECT o.market FROM order_items i, orders o WHERE i.product_id = o.order_id",
    );
    assert!(
        reason.contains("order_items.order_id = orders.order_id"),
        "{reason}"
    );
    // OR trong WHERE không phải điều kiện nối: hai bảng thành tích chéo.
    assert_refused(
        "SELECT o.market FROM order_items i, orders o WHERE i.order_id = o.order_id OR o.market = 'Europe'",
    );
}

#[test]
fn checks_using_and_refuses_natural_join() {
    assert_passes("SELECT market FROM order_items JOIN orders USING (order_id)");
    assert_refused("SELECT market FROM returns JOIN orders USING (customer_id)");
    let reason = assert_refused("SELECT market FROM order_items NATURAL JOIN orders");
    assert!(reason.contains("NATURAL"), "{reason}");
}

#[test]
fn checks_joins_inside_subqueries_and_with() {
    assert_passes(
        "SELECT t.market, SUM(t.item_total) FROM (SELECT o.market, i.item_total FROM order_items i \
         JOIN orders o ON i.order_id = o.order_id) t GROUP BY 1",
    );
    assert_refused(
        "SELECT t.market FROM (SELECT o.market FROM order_items i JOIN orders o ON i.product_id = o.order_id) t",
    );
    assert_refused(
        "WITH x AS (SELECT o.market FROM order_items i JOIN orders o ON i.product_id = o.order_id) \
         SELECT market FROM x",
    );
    assert_refused(
        "SELECT market FROM orders WHERE order_id IN (SELECT i.order_id FROM order_items i \
         JOIN products p ON i.order_id = p.product_id)",
    );
}

#[test]
fn checks_joins_with_aggregated_with_tables() {
    // Gộp bên nhiều trước theo khóa rồi nối, như lời gợi ý của luật cộng lặp.
    assert_passes(
        "WITH t AS (SELECT order_id, SUM(item_total) AS total FROM order_items GROUP BY order_id) \
         SELECT o.market, AVG(t.total) FROM t JOIN orders o ON t.order_id = o.order_id GROUP BY 1",
    );
    // Hai bảng tạm gộp theo khách từ hai bảng khác nhau: hai khóa ngoại cùng trỏ customers.customer_id.
    assert_passes(
        "WITH a AS (SELECT customer_id, COUNT(*) AS n FROM orders GROUP BY 1), \
         b AS (SELECT customer_id, SUM(amount) AS r FROM returns GROUP BY 1) \
         SELECT a.n, b.r FROM a LEFT JOIN b ON a.customer_id = b.customer_id",
    );
    // Bảng tạm chỉ lọc một bảng vẫn là bảng đó: nối phải đúng khóa.
    assert_passes(
        "WITH c AS (SELECT * FROM customers WHERE segment = 'Retail') \
         SELECT c.country FROM orders o JOIN c ON o.customer_id = c.customer_id",
    );
    assert_refused(
        "WITH c AS (SELECT * FROM customers WHERE segment = 'Retail') \
         SELECT c.country FROM orders o JOIN c ON o.order_id = c.customer_id",
    );
    // Bảng tạm gộp theo đơn nối vào customers theo order_id: hai cột không cùng miền.
    assert_refused(
        "WITH t AS (SELECT order_id, SUM(item_total) AS total FROM order_items GROUP BY order_id) \
         SELECT c.segment FROM t JOIN customers c ON t.order_id = c.customer_id",
    );
    // Tỷ trọng trên tổng: nối với câu con một dòng không phải nối hai bảng.
    assert_passes(
        "SELECT o.market, COUNT(*) * 1.0 / t.n FROM orders o CROSS JOIN (SELECT COUNT(*) AS n FROM orders) t \
         GROUP BY o.market, t.n",
    );
    // Bảng tạm theo tháng nối với chính nó lệch một tháng: cột tính ra, không phải cột của bảng nào.
    assert_passes(
        "WITH m AS (SELECT date_trunc('month', order_date) AS month, COUNT(*) AS n FROM orders GROUP BY 1) \
         SELECT a.month, a.n, b.n FROM m a LEFT JOIN m b ON b.month = a.month - INTERVAL 1 MONTH",
    );
}

#[test]
fn checks_in_and_exists_subqueries() {
    assert_passes(
        "SELECT AVG(o.order_id) FROM orders o WHERE o.order_id IN (SELECT i.order_id FROM order_items i \
         JOIN products p ON i.product_id = p.product_id WHERE p.price > 10)",
    );
    assert_passes(
        "SELECT segment FROM customers c WHERE EXISTS (SELECT 1 FROM orders o WHERE o.customer_id = c.customer_id)",
    );
    assert_passes(
        "SELECT c.segment, (SELECT COUNT(*) FROM orders o WHERE o.customer_id = c.customer_id) AS n FROM customers c",
    );
    // Lọc theo giá trị của chính cột đó: top thị trường.
    assert_passes(
        "SELECT market, COUNT(*) FROM orders WHERE market IN (SELECT market FROM orders GROUP BY market \
         ORDER BY COUNT(*) DESC LIMIT 3) GROUP BY market",
    );
    // Hai khóa ngoại trỏ cùng cột của bảng cha: đơn của khách đã trả hàng.
    assert_passes(
        "SELECT COUNT(*) FROM orders WHERE customer_id IN (SELECT customer_id FROM returns)",
    );
    let reason = assert_refused(
        "SELECT COUNT(*) FROM orders WHERE order_id IN (SELECT customer_id FROM customers)",
    );
    assert!(
        reason.contains("orders.order_id") && reason.contains("customers.customer_id"),
        "{reason}"
    );
    assert_refused(
        "SELECT segment FROM customers c WHERE EXISTS (SELECT 1 FROM orders o WHERE o.order_id = c.customer_id)",
    );
    assert_refused("SELECT COUNT(*) FROM orders WHERE market IN (SELECT market FROM targets)");
    // Nửa nối vẫn phải đủ cột của khóa nhiều cột.
    assert_refused(
        "SELECT AVG(score) FROM student_assessments sa WHERE EXISTS (SELECT 1 FROM students s \
         WHERE s.id_student = sa.id_student AND s.region = 'East')",
    );
}

#[test]
fn self_joins_need_a_self_referencing_key() {
    assert_passes("SELECT e.name, m.name FROM employees e LEFT JOIN employees m ON e.manager_id = m.employee_id");
    assert_passes(
        "SELECT e.name, m.name FROM employees m JOIN employees e ON m.employee_id = e.manager_id",
    );
    assert_refused(
        "SELECT e.name FROM employees e JOIN employees m ON e.employee_id = m.employee_id",
    );
    let reason = assert_refused(
        "SELECT a.market FROM orders a JOIN orders b ON a.customer_id = b.customer_id",
    );
    assert!(
        reason.contains("không có khóa ngoại trỏ vào chính nó"),
        "{reason}"
    );
}

#[test]
fn refuses_joining_two_children_of_the_same_parent() {
    // orders và returns cùng trỏ customers: nối thẳng theo customer_id nhân dòng hai phía.
    let reason = assert_refused(
        "SELECT o.market, r.amount FROM orders o JOIN returns r ON o.customer_id = r.customer_id",
    );
    assert!(
        reason.contains("returns.order_id = orders.order_id"),
        "{reason}"
    );
}

#[test]
fn asks_to_name_the_table_of_an_ambiguous_column() {
    let reason =
        assert_refused("SELECT o.market FROM order_items i JOIN orders o ON order_id = o.order_id");
    assert!(reason.contains("ghi rõ bảng"), "{reason}");
}

/// Dataset nạp từ file: không có quan hệ nào. Câu một bảng như trước; chỉ nối bảng với chính nó bị từ chối.
#[test]
fn file_datasets_keep_single_table_questions() {
    let sales = [table("sales", &["region", "amount", "order_date"])];
    for sql in [
        "SELECT region, SUM(amount) FROM sales GROUP BY region",
        "SELECT region FROM sales WHERE amount > (SELECT AVG(amount) FROM sales)",
        "SELECT region FROM sales WHERE region IN (SELECT region FROM sales GROUP BY 1 ORDER BY SUM(amount) DESC LIMIT 3)",
        "WITH y AS (SELECT region, year(order_date) AS yr, SUM(amount) AS v FROM sales GROUP BY 1, 2) \
         SELECT a.region, a.v / b.v FROM y a JOIN y b ON a.region = b.region AND a.yr = b.yr + 1",
        "SELECT region, amount / SUM(amount) OVER () FROM sales",
    ] {
        if let Err(e) = build_guarded_sql(sql, &sales, &[], 1000) {
            panic!("{sql}: {e:?}");
        }
    }
    let self_join = "SELECT a.region FROM sales a JOIN sales b ON a.region = b.region";
    assert!(matches!(
        build_guarded_sql(self_join, &sales, &[], 1000),
        Err(GuardError::Rejected(_))
    ));
}

/// Phần FROM do code dựng cho phân rã theo bảng liên quan (explain/source.rs, build_from_sql), kể cả khóa nhiều cột.
#[test]
fn code_built_related_breakdowns_pass() {
    assert_passes(
        "SELECT \"customers.segment\", SUM(amount) FROM (SELECT \"b\".*, \"j2\".\"segment\" AS \
         \"customers.segment\" FROM \"returns\" AS \"b\" LEFT JOIN \"orders\" AS \"j1\" ON \"b\".\"order_id\" = \
         \"j1\".\"order_id\" LEFT JOIN \"customers\" AS \"j2\" ON \"j1\".\"customer_id\" = \"j2\".\"customer_id\") \
         AS \"joined\" GROUP BY 1",
    );
    assert_passes(
        "SELECT \"students.region\", AVG(score) FROM (SELECT \"b\".*, \"j1\".\"region\" AS \"students.region\" FROM \
         \"student_assessments\" AS \"b\" LEFT JOIN \"students\" AS \"j1\" ON \"b\".\"code_module\" = \
         \"j1\".\"code_module\" AND \"b\".\"code_presentation\" = \"j1\".\"code_presentation\" AND \
         \"b\".\"id_student\" = \"j1\".\"id_student\") AS \"joined\" GROUP BY 1",
    );
}

/// Truy dòng nguồn đưa lại sql_guarded qua guard: hộp lọc lan theo khóa ngoại của người đã hỏi có nửa nối `IN`, và
/// guard nhận ra quan hệ đó từ RowFilter::Parent dù service không truyền quan hệ nào.
#[test]
fn guarded_sql_with_parent_filters_passes_again() {
    let customers = AllowedTable {
        row_filters: vec![RowFilter::Column {
            column: "segment".to_string(),
            allowed_values: vec!["Retail".to_string()],
        }],
        ..table("customers", &["customer_id", "segment", "country"])
    };
    let orders = AllowedTable {
        row_filters: vec![RowFilter::Parent {
            columns: vec!["customer_id".to_string()],
            parent_columns: vec!["customer_id".to_string()],
            parent: customers,
        }],
        ..table(
            "orders",
            &["order_id", "customer_id", "order_date", "market"],
        )
    };
    let mut tables = vec![orders];
    let first = build_guarded_sql(
        "SELECT market, COUNT(*) FROM orders GROUP BY 1",
        &tables,
        &[],
        1000,
    )
    .unwrap();
    assert!(first.shown.contains("IN (SELECT"), "{}", first.shown);
    // Như service/drill/mod.rs: bảng cha của hộp lọc có trong danh sách.
    let parents = build_parent_tables(&tables);
    tables.extend(parents);
    let again = build_guarded_sql(&first.shown, &tables, &[], 1000);
    assert!(again.is_ok(), "{again:?}");
}
