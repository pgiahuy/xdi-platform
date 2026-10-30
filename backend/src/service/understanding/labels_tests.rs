// SPDX-License-Identifier: Apache-2.0
/* service/understanding/labels_tests.rs - test nhãn cột và nhãn độ đo của thẻ "Đã hiểu là" (understanding/labels.rs)
khi mô tả cột do mô hình viết là một câu trọn: bỏ dấu câu cuối, không lặp tiền tố "Tổng", "Trung bình", "Số". */

use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use uuid::Uuid;

use crate::service::ask::context::ColumnProfile;
use crate::service::understanding::labels::{build_column_label, build_measure_labels};

fn column(name: &str, description: &str) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::nil(),
        role: "measure".to_string(),
        name: name.to_string(),
        data_type: "DOUBLE".to_string(),
        time_format: None,
        description: Some(description.to_string()),
        aliases: vec![],
        is_sensitive: false,
        values: vec![],
        values_complete: false,
        matched: false,
        unit: None,
        origin_table: None,
    }
}

fn measure_label(sql: &str, profile: &[ColumnProfile]) -> String {
    let tree = Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .and_then(|mut parser| parser.parse_expr())
        .unwrap();
    build_measure_labels(&tree, "value", &[], profile).1
}

#[test]
fn column_label_drops_the_final_punctuation() {
    let profile = [column("Sales", "Doanh số bán hàng.")];
    assert_eq!(
        build_column_label("Sales", &profile, false),
        "Doanh số bán hàng (Sales)"
    );
    let profile = [column("Sales", "  Doanh số bán hàng!  ")];
    assert_eq!(
        build_column_label("Sales", &profile, true),
        "doanh số bán hàng (Sales)"
    );
}

#[test]
fn measure_label_does_not_repeat_the_prefix() {
    let profile = [
        column("Sales", "Tổng doanh số bán hàng."),
        column("Price", "trung bình giá bán"),
        column("Product", "Số sản phẩm"),
        column("Profit", "Lợi nhuận của đơn hàng."),
    ];
    assert_eq!(
        measure_label(r#"SUM("Sales")"#, &profile),
        "Tổng doanh số bán hàng (Sales)"
    );
    assert_eq!(
        measure_label(r#"AVG("Price")"#, &profile),
        "Trung bình giá bán (Price)"
    );
    assert_eq!(
        measure_label(r#"COUNT(DISTINCT "Product")"#, &profile),
        "Số sản phẩm (Product) khác nhau"
    );
    // Mô tả không bắt đầu bằng tiền tố thì vẫn thêm tiền tố như trước.
    assert_eq!(
        measure_label(r#"SUM("Profit")"#, &profile),
        "Tổng lợi nhuận của đơn hàng (Profit)"
    );
}
