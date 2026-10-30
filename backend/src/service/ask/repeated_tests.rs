// SPDX-License-Identifier: Apache-2.0
/* service/ask/repeated_tests.rs - test luật chặn SUM cột lặp lại từ bảng nối (ask/repeated.rs): câu nào bị gửi đi
sửa, câu nào chạy như thường. Chỉ đọc câu SQL, không cần DuckDB hay Postgres. */

use crate::service::ask::repeated::{find_repeated_sum, RepeatedColumn};

fn repeated(name: &str, table: &str, key: &[&str]) -> RepeatedColumn {
    RepeatedColumn {
        name: name.to_string(),
        table: table.to_string(),
        key: key.iter().map(|k| k.to_string()).collect(),
    }
}

fn credits() -> Vec<RepeatedColumn> {
    let key = ["code_module", "id_student"];
    vec![
        repeated("students.studied_credits", "edu.students", &key),
        repeated("students.extra_credits", "edu.students", &key),
    ]
}

/// Bảng dòng hàng nối products, orders và currencies: giá, trạng thái đơn, tỷ giá là cột lặp lại.
fn shop() -> Vec<RepeatedColumn> {
    vec![
        repeated("products.price", "sales.products", &["sku"]),
        repeated("orders.status", "sales.orders", &["order_id"]),
        repeated("currencies.rate", "sales.currencies", &["currency"]),
    ]
}

#[test]
fn sum_of_a_repeated_column_on_the_table_is_caught() {
    let sql = r#"SELECT "students.region", sum("students.studied_credits") FROM marks GROUP BY 1"#;
    let reason = find_repeated_sum(sql, "marks", &credits()).unwrap();
    assert!(
        reason.contains("SELECT DISTINCT \"code_module\", \"id_student\""),
        "{reason}"
    );
    // Trong câu con đọc thẳng bảng cũng bị bắt.
    let nested = r#"SELECT * FROM (SELECT SUM("Students.Studied_Credits") AS c FROM marks) AS x"#;
    assert!(find_repeated_sum(nested, "marks", &credits()).is_some());
}

/// Cộng riêng cột lặp dù viết vòng: ngoặc, CAST, dấu âm, COALESCE, DISTINCT, nhân hằng số, cộng cột cùng bảng nối,
/// hay CASE mà nhánh THEN là cột lặp.
#[test]
fn repeated_column_summed_on_its_own_is_caught() {
    let caught = [
        r#"SELECT SUM("products.price") FROM order_items"#,
        r#"SELECT SUM(DISTINCT "products.price") FROM order_items"#,
        r#"SELECT SUM(("products.price" * 1.1)) FROM order_items"#,
        r#"SELECT SUM(-CAST("products.price" AS DOUBLE)) FROM order_items"#,
        r#"SELECT SUM("products.price"::DOUBLE) FROM order_items"#,
        r#"SELECT SUM(COALESCE("products.price", 0)) FROM order_items"#,
        r#"SELECT SUM(IFNULL("products.price", 0)) FROM order_items"#,
        r#"SELECT SUM(order_items."products.price") FROM order_items"#,
        r#"SELECT SUM(CASE WHEN "orders.status" = 'Delivered' THEN "products.price" ELSE 0 END) FROM order_items"#,
        r#"SELECT "sku" FROM order_items GROUP BY 1 HAVING SUM("products.price") > 10"#,
    ];
    for sql in caught {
        let reason = find_repeated_sum(sql, "order_items", &shop());
        assert!(
            reason.is_some_and(|r| r.contains("\"products.price\"")),
            "{sql}"
        );
    }
    let same_table =
        r#"SELECT SUM("students.studied_credits" + "students.extra_credits") FROM marks"#;
    assert!(find_repeated_sum(same_table, "marks", &credits()).is_some());
}

/// Câu đúng trên bảng phẳng nhiều-một: cột lặp nhân với cột của bảng chính, hay chỉ nằm trong điều kiện CASE.
#[test]
fn repeated_column_with_a_fact_column_passes() {
    let ok = [
        r#"SELECT "products.category", SUM("quantity" * "products.price") FROM order_items GROUP BY 1"#,
        r#"SELECT SUM(CASE WHEN "orders.status" = 'Delivered' THEN "amount" ELSE 0 END) FROM order_items"#,
        r#"SELECT SUM("amount" * "currencies.rate") FROM order_items"#,
        r#"SELECT SUM(("quantity" * COALESCE("products.price", 0)) - "discount") FROM order_items"#,
        // Hai bảng nối khác nhau: không chặn (xem chú thích ở repeated.rs).
        r#"SELECT SUM("products.price" * "currencies.rate") FROM order_items"#,
        // Hàm không nằm trong danh sách đi qua: không chắc nên không chặn.
        r#"SELECT SUM(greatest("products.price", 0)) FROM order_items"#,
    ];
    for sql in ok {
        assert_eq!(
            find_repeated_sum(sql, "order_items", &shop()),
            None,
            "{sql}"
        );
    }
}

#[test]
fn other_aggregates_and_distinct_rows_pass() {
    let ok = [
        r#"SELECT avg("students.studied_credits"), sum("score") FROM marks"#,
        r#"WITH one_row AS (SELECT DISTINCT "code_module", "id_student", "students.studied_credits" FROM marks)
           SELECT SUM("students.studied_credits") FROM one_row"#,
    ];
    for sql in ok {
        assert_eq!(find_repeated_sum(sql, "marks", &credits()), None, "{sql}");
    }
    assert_eq!(
        find_repeated_sum("SELECT sum(x) FROM marks", "marks", &[]),
        None
    );
}
