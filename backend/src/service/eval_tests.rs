// SPDX-License-Identifier: Apache-2.0
/* service/eval_tests.rs - ca test của bộ so kết quả đánh giá (build_eval_verdict), thuần không cần Postgres:
cùng dòng khác thứ tự, lệch một dòng, số lệch quanh ngưỡng, null với null, cột đổi tên, ordered khác thứ tự; và SQL
đúng của bộ câu JOIN eval/sales_joins.json qua guard. */

use super::eval::{build_eval_verdict, load_cases};
use crate::service::ask::contract::CellValue;

type Row = Vec<Option<CellValue>>;

fn row(cells: Vec<CellValue>) -> Row {
    cells.into_iter().map(Some).collect()
}

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

fn text(s: &str) -> CellValue {
    CellValue::String(s.to_string())
}

#[test]
fn same_rows_other_order_pass_when_unordered() {
    let actual = vec![
        row(vec![text("b"), num(2.0)]),
        row(vec![text("a"), num(1.0)]),
    ];
    let expected = vec![
        row(vec![text("a"), num(1.0)]),
        row(vec![text("b"), num(2.0)]),
    ];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
}

#[test]
fn same_rows_other_order_fail_when_ordered() {
    let actual = vec![row(vec![text("b")]), row(vec![text("a")])];
    let expected = vec![row(vec![text("a")]), row(vec![text("b")])];
    assert!(build_eval_verdict(&actual, &expected, true).is_err());
}

#[test]
fn one_row_off_is_a_mismatch() {
    let actual = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(3.0)])];
    let why = build_eval_verdict(&actual, &expected, false).unwrap_err();
    assert!(why.contains("dòng 1"), "{why}");
}

#[test]
fn numbers_allow_tiny_relative_drift() {
    let actual = vec![row(vec![num(1.0000000005)])];
    let expected = vec![row(vec![num(1.0)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let actual = vec![row(vec![num(1.5)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_err());
}

#[test]
fn null_matches_null_only() {
    let actual: Vec<Row> = vec![vec![None]];
    let expected: Vec<Row> = vec![vec![None]];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let expected = vec![row(vec![num(0.0)])];
    assert!(build_eval_verdict(&actual, &expected, false).is_err());
}

#[test]
fn column_names_do_not_matter_but_width_does() {
    // Tên cột không vào verdict (bí danh là chi tiết cách viết), chỉ số cột tính.
    let actual = vec![row(vec![text("x")])];
    let expected = vec![row(vec![text("x")])];
    assert!(build_eval_verdict(&actual, &expected, false).is_ok());
    let wide = vec![row(vec![text("x"), num(1.0)])];
    assert!(build_eval_verdict(&wide, &expected, false).is_err());
}

#[test]
fn row_count_mismatch_is_reported() {
    let actual = vec![row(vec![num(1.0)])];
    let expected = vec![row(vec![num(1.0)]), row(vec![num(2.0)])];
    let why = build_eval_verdict(&actual, &expected, false).unwrap_err();
    assert!(why.contains("số dòng"), "{why}");
}

/// Bộ câu JOIN trên schema sales của database mẫu (eval/sales_joins.json, #213): mỗi SQL đúng qua guard với sáu bảng như
/// ảnh chụp database mẫu, và đọc bảng của trường "dataset" (bảng người hỏi chọn). Chưa có lệnh chạy bộ này trên một nhóm
/// bảng chụp từ database; số kết quả đã đối chiếu bằng psql trên database mẫu.
#[test]
fn sales_join_cases_pass_the_guard() {
    use crate::engine::guard::{build_guarded_sql, AllowedTable};

    let schema: [(&str, &[&str]); 6] = [
        (
            "order_items",
            &[
                "order_item_id",
                "order_id",
                "product_id",
                "quantity",
                "unit_price",
                "sales",
                "discount",
                "discount_rate",
                "item_total",
                "profit",
                "profit_ratio",
            ],
        ),
        (
            "orders",
            &[
                "order_id",
                "customer_id",
                "order_date",
                "shipping_date",
                "payment_type",
                "order_status",
                "delivery_status",
                "late_delivery_risk",
                "days_for_shipping_real",
                "days_for_shipment_scheduled",
                "shipping_mode",
                "market",
                "order_region",
                "order_country",
                "order_state",
                "order_city",
                "order_zipcode",
            ],
        ),
        (
            "customers",
            &[
                "customer_id",
                "first_name",
                "last_name",
                "segment",
                "street",
                "city",
                "state",
                "country",
                "zipcode",
                "latitude",
                "longitude",
            ],
        ),
        (
            "products",
            &["product_id", "product_name", "category_id", "price"],
        ),
        (
            "categories",
            &["category_id", "category_name", "department_id"],
        ),
        ("departments", &["department_id", "department_name"]),
    ];
    let tables: Vec<AllowedTable> = schema
        .iter()
        .map(|(name, columns)| AllowedTable {
            name: name.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            parquet_dir: std::path::PathBuf::from(format!("/data/{name}")),
            row_filters: Vec::new(),
        })
        .collect();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval/sales_joins.json");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() >= 8);
    for case in &cases {
        let picked = case["dataset"].as_str().unwrap();
        for sql in case["expected_sql"].as_array().unwrap() {
            let guarded = build_guarded_sql(sql.as_str().unwrap(), &tables, 1000)
                .unwrap_or_else(|e| panic!("{}: {e:?}", case["id"]));
            assert!(
                guarded.tables_read.len() > 1 && guarded.tables_read.iter().any(|t| t == picked),
                "{}: {:?}",
                case["id"],
                guarded.tables_read
            );
        }
    }
    // Cùng file đọc được bằng load_cases của bộ đánh giá.
    assert_eq!(
        load_cases(std::path::Path::new(path)).unwrap().len(),
        cases.len()
    );
}
