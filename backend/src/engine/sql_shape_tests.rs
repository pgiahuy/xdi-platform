// SPDX-License-Identifier: Apache-2.0
/* engine/sql_shape_tests.rs - test của engine/sql_shape.rs: đọc cột kết quả, vế lọc, khóa nhóm, thứ tự, LIMIT và bảng
từ câu thường, câu đã qua guard có phân quyền, CTE, phép tập hợp, `*`. Thẻ dựng từ hình dạng này test ở
service/understanding/tests.rs. */

use std::path::PathBuf;

use super::guard::{build_guarded_sql, AllowedTable, RowFilter};
use super::sql_shape::build_query_shape;

fn guarded(sql: &str, row_filters: Vec<RowFilter>) -> String {
    let tables = vec![AllowedTable {
        name: "sales".to_string(),
        columns: vec![
            "region".to_string(),
            "segment".to_string(),
            "amount".to_string(),
        ],
        parquet_dir: PathBuf::from("/data/sales"),
        row_filters,
    }];
    build_guarded_sql(sql, &tables, &[], 1000).unwrap().shown
}

fn south_only() -> Vec<RowFilter> {
    vec![RowFilter::Column {
        column: "region".to_string(),
        allowed_values: vec!["South".to_string()],
    }]
}

#[test]
fn reads_outputs_keys_order_and_limit() {
    let shape = build_query_shape(
        "SELECT region AS r, SUM(amount) AS total, amount FROM sales WHERE amount > 1 AND segment = 'A' \
         GROUP BY region, amount ORDER BY total DESC, r LIMIT 3",
    )
    .unwrap();
    let outputs: Vec<(&str, &str, bool)> = shape
        .outputs
        .iter()
        .map(|o| (o.name.as_str(), o.expr.as_str(), o.is_aggregate))
        .collect();
    assert_eq!(
        outputs,
        [
            ("r", "region", false),
            ("total", "SUM(amount)", true),
            ("amount", "amount", false)
        ]
    );
    let filters: Vec<&str> = shape.filters.iter().map(|f| f.sql.as_str()).collect();
    assert_eq!(filters, ["amount > 1", "segment = 'A'"]);
    let keys: Vec<Option<usize>> = shape.group_by.iter().map(|k| k.output).collect();
    assert_eq!(keys, [Some(0), Some(2)]);
    let order: Vec<(&str, Option<usize>, bool)> = shape
        .order_by
        .iter()
        .map(|o| (o.sql.as_str(), o.output, o.descending))
        .collect();
    assert_eq!(
        order,
        [("total DESC", Some(1), true), ("r", Some(0), false)]
    );
    assert_eq!(shape.limit.as_deref(), Some("3"));
    assert_eq!(shape.table.as_deref(), Some("sales"));
    assert!(!shape.set_operation);
}

#[test]
fn positional_keys_point_to_outputs() {
    let shape =
        build_query_shape("SELECT region, SUM(amount) FROM sales GROUP BY 1 ORDER BY 2 DESC")
            .unwrap();
    assert_eq!(shape.group_by[0].output, Some(0));
    assert_eq!(shape.order_by[0].output, Some(1));
    assert_eq!(shape.order_by[0].sql, "2 DESC");
}

#[test]
fn group_by_all_takes_every_plain_output() {
    let shape =
        build_query_shape("SELECT region, segment, SUM(amount) FROM sales GROUP BY ALL").unwrap();
    let keys: Vec<Option<usize>> = shape.group_by.iter().map(|k| k.output).collect();
    assert_eq!(keys, [Some(0), Some(1)]);
}

#[test]
fn window_functions_and_subqueries_are_not_aggregates() {
    let shape = build_query_shape(
        "SELECT SUM(amount) OVER (PARTITION BY region) AS w, (SELECT COUNT(*) FROM sales) AS n, \
         ROUND(AVG(amount), 2) AS a FROM sales GROUP BY region",
    )
    .unwrap();
    let flags: Vec<bool> = shape.outputs.iter().map(|o| o.is_aggregate).collect();
    assert_eq!(flags, [false, false, true]);
}

#[test]
fn reads_through_the_row_filter_box_of_the_guard() {
    let sql = guarded(
        "SELECT region, SUM(amount) AS total FROM sales WHERE segment = 'A' GROUP BY region",
        south_only(),
    );
    let shape = build_query_shape(&sql).unwrap();
    assert_eq!(shape.table.as_deref(), Some("sales"));
    let filters: Vec<(&str, bool)> = shape
        .filters
        .iter()
        .map(|f| (f.sql.as_str(), f.in_table_box))
        .collect();
    assert_eq!(
        filters,
        [("\"region\" IN ('South')", true), ("segment = 'A'", false)]
    );
    for filter in &shape.filters {
        assert!(sql.contains(&filter.sql), "{} not in {sql}", filter.sql);
    }
}

#[test]
fn nested_boxes_keep_inner_conditions_first() {
    let sql = guarded(
        "SELECT region FROM (SELECT * FROM sales WHERE amount > 1) AS s",
        south_only(),
    );
    let shape = build_query_shape(&sql).unwrap();
    assert_eq!(shape.table.as_deref(), Some("sales"));
    let filters: Vec<&str> = shape.filters.iter().map(|f| f.sql.as_str()).collect();
    assert_eq!(filters, ["\"region\" IN ('South')", "amount > 1"]);
}

#[test]
fn cte_and_other_subqueries_have_no_table() {
    let cte = build_query_shape(
        "WITH t AS (SELECT * FROM sales) SELECT region, SUM(amount) FROM t GROUP BY region",
    )
    .unwrap();
    assert_eq!(cte.table, None);
    assert_eq!(cte.group_by[0].output, Some(0));
    let derived = build_query_shape(
        "SELECT r FROM (SELECT region AS r FROM sales GROUP BY region) AS x WHERE r <> 'North'",
    )
    .unwrap();
    assert_eq!(derived.table, None);
    assert_eq!(derived.filters.len(), 1);
    let join =
        build_query_shape("SELECT a.region FROM sales a JOIN sales b ON a.region = b.region")
            .unwrap();
    assert_eq!(join.table, None);
}

#[test]
fn set_operations_keep_names_order_and_limit_only() {
    let shape = build_query_shape(
        "SELECT region AS name FROM sales WHERE amount > 1 UNION ALL SELECT segment FROM sales ORDER BY 1 LIMIT 5",
    )
    .unwrap();
    assert!(shape.set_operation);
    assert_eq!(shape.outputs[0].name, "name");
    assert!(shape.filters.is_empty());
    assert_eq!(shape.table, None);
    assert_eq!(shape.order_by[0].output, Some(0));
    assert_eq!(shape.limit.as_deref(), Some("5"));
}

#[test]
fn wildcard_outputs_have_no_tree_and_no_position() {
    let shape = build_query_shape("SELECT *, amount FROM sales ORDER BY 2").unwrap();
    assert_eq!(shape.outputs[0].expr, "*");
    assert!(shape.outputs[0].tree.is_none());
    // `*` ra bao nhiêu cột thì không biết, nên số thứ tự 2 không trỏ được về cột nào.
    assert_eq!(shape.order_by[0].output, None);
}

#[test]
fn rejects_text_that_is_not_one_query() {
    assert!(build_query_shape("not sql at all").is_err());
    assert!(build_query_shape("SELECT 1; SELECT 2").is_err());
    assert!(build_query_shape("DELETE FROM sales").is_err());
}
