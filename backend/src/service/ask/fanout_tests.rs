// SPDX-License-Identifier: Apache-2.0
/* service/ask/fanout_tests.rs - test luật chặn cộng lặp khi nối bảng một-nhiều (ask/fanout.rs), thuần không cần
Postgres: các dạng bị bắt và không bị bắt trên lược đồ giống edu (OULAD) và shop, và mọi SQL đúng của bộ câu JOIN
eval/sales_joins.json không bị bắt. */

use std::path::PathBuf;

use crate::engine::guard::AllowedTable;
use crate::service::ask::fanout::{build_hint, find_double_count, DoubleCount, JoinLink};

fn table(name: &str, columns: &[&str]) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir: PathBuf::from(format!("/data/{name}")),
        row_filters: Vec::new(),
    }
}

fn link(many: &str, many_columns: &[&str], one: &str, one_columns: &[&str]) -> JoinLink {
    JoinLink {
        many: many.to_string(),
        many_columns: many_columns.iter().map(|c| c.to_string()).collect(),
        one: one.to_string(),
        one_columns: one_columns.iter().map(|c| c.to_string()).collect(),
        fans_out: true,
    }
}

fn one_to_one(many: &str, many_columns: &[&str], one: &str, one_columns: &[&str]) -> JoinLink {
    JoinLink {
        fans_out: false,
        ..link(many, many_columns, one, one_columns)
    }
}

const STUDENT_KEY: [&str; 3] = ["code_module", "code_presentation", "id_student"];

/// Giống schema edu của database mẫu (data/postgres-demo/sql/edu_schema.sql), bớt cột.
fn edu() -> (Vec<AllowedTable>, Vec<JoinLink>) {
    let tables = vec![
        table(
            "students",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "region",
                "studied_credits",
                "final_result",
            ],
        ),
        table(
            "student_assessments",
            &[
                "id_assessment",
                "id_student",
                "code_module",
                "code_presentation",
                "score",
            ],
        ),
        table("assessments", &["id_assessment", "weight"]),
        table(
            "registrations",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "date_registration",
            ],
        ),
    ];
    let links = vec![
        link(
            "student_assessments",
            &STUDENT_KEY,
            "students",
            &STUDENT_KEY,
        ),
        link(
            "student_assessments",
            &["id_assessment"],
            "assessments",
            &["id_assessment"],
        ),
        // Khóa chính của registrations cũng là khóa ngoại tới students: một-một.
        one_to_one("registrations", &STUDENT_KEY, "students", &STUDENT_KEY),
    ];
    (tables, links)
}

/// Giống schema shop của api/source_relations_tests.rs.
fn shop() -> (Vec<AllowedTable>, Vec<JoinLink>) {
    let tables = vec![
        table("order_items", &["order_id", "maker", "sku", "qty"]),
        table("orders", &["id", "customer_id", "status"]),
        table("products", &["maker", "sku", "title", "price"]),
        table("customers", &["id", "name", "region_code"]),
        table("returns", &["id", "order_id"]),
        table("employees", &["id", "manager_id", "salary"]),
    ];
    let links = vec![
        link("order_items", &["order_id"], "orders", &["id"]),
        link(
            "order_items",
            &["maker", "sku"],
            "products",
            &["maker", "sku"],
        ),
        link("orders", &["customer_id"], "customers", &["id"]),
        link("returns", &["order_id"], "orders", &["id"]),
        link("employees", &["manager_id"], "employees", &["id"]),
    ];
    (tables, links)
}

fn edu_check(sql: &str) -> Option<DoubleCount> {
    let (tables, links) = edu();
    find_double_count(sql, &tables, &links)
}

fn shop_check(sql: &str) -> Option<DoubleCount> {
    let (tables, links) = shop();
    find_double_count(sql, &tables, &links)
}

const JOIN_STUDENTS: &str =
    "FROM student_assessments sa JOIN students s ON sa.code_module = s.code_module \
     AND sa.code_presentation = s.code_presentation AND sa.id_student = s.id_student";

/// Câu Gemini từng viết cho "Tổng tín chỉ đăng ký theo vùng" khi có bảng bài nộp: tín chỉ của mỗi sinh viên cộng một
/// lần cho mỗi bài nộp.
#[test]
fn credits_by_region_over_submissions_is_flagged() {
    let sql = format!(
        "SELECT s.region, SUM(s.studied_credits) AS credits {JOIN_STUDENTS} GROUP BY s.region"
    );
    let found = edu_check(&sql).expect("flagged");
    assert_eq!(found.function, "SUM");
    assert_eq!(found.argument, "s.studied_credits");
    assert_eq!(found.table, "students");
    assert_eq!(found.link.many, "student_assessments");
    let hint = build_hint(&found);
    assert!(
        hint.contains("subquery or CTE grouped by code_module, code_presentation, id_student")
            && hint.contains("SELECT DISTINCT"),
        "{hint}"
    );
}

#[test]
fn one_side_column_shapes_are_flagged() {
    let shapes = [
        "SUM(s.studied_credits)",
        "AVG(s.studied_credits)",
        "SUM(DISTINCT s.studied_credits)",
        "SUM(-CAST(s.studied_credits AS DOUBLE))",
        "SUM(COALESCE(s.studied_credits, 0) * 1.5)",
        "SUM(ROUND(ABS(s.studied_credits), 1))",
        "SUM((s.studied_credits + 10))",
        // Điều kiện WHEN theo cột bảng nhiều, nhánh THEN vẫn là cột của students.
        "SUM(CASE WHEN sa.score >= 40 THEN s.studied_credits ELSE 0 END)",
        "SUM(CASE WHEN sa.score >= 40 THEN s.studied_credits END)",
        // Tên bảng làm bí danh, và cột không ghi bảng mà chỉ students có.
        "SUM(students.studied_credits)",
        "SUM(studied_credits)",
    ];
    for shape in shapes {
        let from = if shape.contains("students.") {
            "FROM student_assessments JOIN students USING (code_module, code_presentation, id_student)"
        } else {
            JOIN_STUDENTS
        };
        let sql = format!("SELECT s.region, {shape} AS v {from} GROUP BY 1");
        assert!(edu_check(&sql).is_some(), "{sql}");
    }
}

#[test]
fn sum_in_having_is_flagged() {
    let sql = format!(
        "SELECT s.region {JOIN_STUDENTS} GROUP BY s.region HAVING SUM(s.studied_credits) > 100"
    );
    assert!(edu_check(&sql).is_some(), "{sql}");
}

/// Cột không ghi bảng mà cả hai bảng đều có thì không chắc của bảng nào: không bắt.
#[test]
fn ambiguous_unqualified_column_is_not_flagged() {
    let sql = format!("SELECT SUM(id_student) AS v {JOIN_STUDENTS}");
    assert!(edu_check(&sql).is_none());
}

#[test]
fn row_values_of_the_many_side_are_not_flagged() {
    let allowed = [
        "SUM(sa.score)",
        "AVG(score)",
        // Cột của students chỉ nằm trong điều kiện.
        "SUM(CASE WHEN s.final_result = 'Pass' THEN sa.score ELSE 0 END)",
        "MAX(s.studied_credits)",
        "COUNT(DISTINCT s.id_student)",
        // Đếm lặp sau khi nối nằm ngoài phạm vi kiểm tra này.
        "COUNT(*)",
        // Bí danh lạ: không chắc.
        "SUM(x.studied_credits)",
        // Hàm không đi xuyên qua.
        "SUM(GREATEST(s.studied_credits, 0))",
    ];
    for shape in allowed {
        let sql = format!("SELECT {shape} AS v {JOIN_STUDENTS}");
        assert!(edu_check(&sql).is_none(), "{sql}");
    }
}

/// Số lượng nhân giá là doanh thu của từng dòng hàng: không cộng lặp.
#[test]
fn quantity_times_price_is_not_flagged() {
    let sql = "SELECT p.title, SUM(oi.qty * p.price) AS revenue FROM order_items oi \
               JOIN products p ON oi.maker = p.maker AND oi.sku = p.sku GROUP BY p.title";
    assert!(shop_check(sql).is_none());
    let summed = "SELECT p.title, SUM(p.price) AS total FROM order_items oi \
                  JOIN products p ON oi.maker = p.maker AND oi.sku = p.sku GROUP BY p.title";
    let found = shop_check(summed).expect("flagged");
    assert_eq!(found.link.many, "order_items");
}

#[test]
fn one_to_one_join_is_not_flagged() {
    let sql =
        "SELECT s.region, SUM(s.studied_credits) AS credits FROM registrations r JOIN students s \
               ON r.code_module = s.code_module AND r.code_presentation = s.code_presentation \
               AND r.id_student = s.id_student GROUP BY s.region";
    assert!(edu_check(sql).is_none());
}

/// Đi lên bên một không nhân dòng; hai bảng cùng trỏ vào orders thì dòng của bảng này lặp theo bảng kia.
#[test]
fn walking_up_does_not_fan_out_but_siblings_do() {
    let up = "SELECT c.region_code, SUM(oi.qty) AS qty FROM order_items oi JOIN orders o ON oi.order_id = o.id \
              JOIN customers c ON o.customer_id = c.id GROUP BY c.region_code";
    assert!(shop_check(up).is_none());
    let siblings = "SELECT o.status, SUM(oi.qty) AS qty FROM order_items oi JOIN orders o ON oi.order_id = o.id \
                    JOIN returns r ON r.order_id = o.id GROUP BY o.status";
    let found = shop_check(siblings).expect("flagged");
    assert_eq!(found.table, "order_items");
    assert_eq!(found.link.many, "returns");
    // Mỗi khách hàng lặp theo số đơn; hàm không đi xuyên qua (LENGTH) thì không xét.
    let per_order = "SELECT SUM(LENGTH(c.name)) AS v, SUM(c.id) AS w FROM orders o \
                    JOIN customers c ON o.customer_id = c.id";
    assert_eq!(shop_check(per_order).unwrap().argument, "c.id");
}

/// Quan hệ của một bảng với chính nó: luật theo tên bảng không phân biệt nhân viên với quản lý, nên không bắt.
#[test]
fn self_relation_is_not_flagged() {
    let sql = "SELECT m.id, SUM(e.salary) AS total FROM employees e JOIN employees m ON e.manager_id = m.id \
               GROUP BY m.id";
    assert!(shop_check(sql).is_none());
}

/// Không lần theo WITH hay bảng con (giới hạn ghi ở đầu ask/fanout.rs), nhưng SELECT bên trong WITH vẫn được xét với
/// FROM của chính nó.
#[test]
fn ctes_and_derived_tables_are_not_traced() {
    let distinct_first = format!(
        "WITH one_row AS (SELECT DISTINCT s.code_module, s.code_presentation, s.id_student, s.region, \
         s.studied_credits {JOIN_STUDENTS}) SELECT region, SUM(studied_credits) FROM one_row GROUP BY region"
    );
    assert!(edu_check(&distinct_first).is_none());
    let not_traced = format!(
        "WITH joined AS (SELECT s.region, s.studied_credits {JOIN_STUDENTS}) \
         SELECT region, SUM(studied_credits) FROM joined GROUP BY region"
    );
    assert!(edu_check(&not_traced).is_none());
    let inside = format!(
        "WITH t AS (SELECT s.region, SUM(s.studied_credits) AS c {JOIN_STUDENTS} GROUP BY s.region) SELECT * FROM t"
    );
    assert!(edu_check(&inside).is_some());
    // Cột không ghi bảng khi FROM có bảng con: có thể đến từ bảng con.
    let derived = "SELECT SUM(studied_credits) FROM (SELECT id_student AS k FROM student_assessments) t \
                   JOIN students s ON t.k = s.id_student JOIN student_assessments sa ON sa.id_student = s.id_student";
    assert!(edu_check(derived).is_none());
}

/// SUM của câu con trong danh sách cột thuộc FROM của câu con, không thuộc FROM của câu ngoài.
#[test]
fn scalar_subquery_uses_its_own_from() {
    let sql = format!(
        "SELECT (SELECT SUM(studied_credits) FROM students) AS all_credits, COUNT(*) AS n {JOIN_STUDENTS}"
    );
    assert!(edu_check(&sql).is_none());
}

#[test]
fn single_table_and_no_relations_are_not_flagged() {
    assert!(
        edu_check("SELECT region, SUM(studied_credits) FROM students GROUP BY region").is_none()
    );
    let (tables, _) = edu();
    let sql = format!("SELECT SUM(s.studied_credits) {JOIN_STUDENTS}");
    assert!(find_double_count(&sql, &tables, &[]).is_none());
}

#[test]
fn union_branches_are_checked() {
    let sql = format!(
        "SELECT 'a' AS k, SUM(sa.score) AS v {JOIN_STUDENTS} \
         UNION ALL SELECT 'b', SUM(s.studied_credits) {JOIN_STUDENTS}"
    );
    assert!(edu_check(&sql).is_some());
}

/// Mọi SQL đúng của bộ câu JOIN trên schema sales (eval/sales_joins.json) không bị bắt.
#[test]
fn sales_join_cases_are_not_flagged() {
    let tables = vec![
        table(
            "order_items",
            &[
                "order_item_id",
                "order_id",
                "product_id",
                "quantity",
                "unit_price",
                "sales",
                "item_total",
                "profit",
            ],
        ),
        table(
            "orders",
            &[
                "order_id",
                "customer_id",
                "order_date",
                "order_status",
                "late_delivery_risk",
                "days_for_shipping_real",
                "shipping_mode",
                "market",
            ],
        ),
        table("customers", &["customer_id", "segment", "country"]),
        table(
            "products",
            &["product_id", "product_name", "category_id", "price"],
        ),
        table(
            "categories",
            &["category_id", "category_name", "department_id"],
        ),
        table("departments", &["department_id", "department_name"]),
    ];
    let links = vec![
        link("order_items", &["order_id"], "orders", &["order_id"]),
        link("order_items", &["product_id"], "products", &["product_id"]),
        link("orders", &["customer_id"], "customers", &["customer_id"]),
        link("products", &["category_id"], "categories", &["category_id"]),
        link(
            "categories",
            &["department_id"],
            "departments",
            &["department_id"],
        ),
    ];
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval/sales_joins.json");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for case in &cases {
        for sql in case["expected_sql"].as_array().unwrap() {
            let sql = sql.as_str().unwrap();
            let found = find_double_count(sql, &tables, &links);
            assert!(found.is_none(), "{}: {found:?}", case["id"]);
        }
    }
}
