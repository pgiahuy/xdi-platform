// SPDX-License-Identifier: Apache-2.0
/* service/understanding/labels_tests.rs - test nhãn cột và nhãn độ đo của thẻ "Cách tính" (understanding/labels.rs)
khi mô tả cột do mô hình viết là một câu trọn: bỏ dấu câu cuối, không lặp tiền tố "Tổng", "Trung bình", "Số"; chỉ lấy
phần đầu của mô tả; tên tiếng Việt cho lời giải thích (build_display_name). */

use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use uuid::Uuid;

use crate::service::ask::context::ColumnProfile;
use crate::service::understanding::labels::{
    build_column_label, build_display_name, build_measure_labels, build_row_filter_label,
    build_subject_label,
};

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
        aggregation: crate::engine::store::ColumnAggregation::Sum,
        join_key: false,
        metric_column: false,
        picked: false,
        brief: false,
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

/// Cột "mỗi đơn" cộng lại là tổng (#260): thẻ gọi "Tổng lợi nhuận", không phải "Tổng lợi nhuận trên mỗi đơn hàng".
/// AVG và COUNT giữ nghĩa "mỗi đơn" của cột.
#[test]
fn sum_of_a_per_order_column_is_a_total() {
    let mut bare = column("Order Profit Per Order", "");
    bare.description = None;
    let profile = [
        column("Benefit per order", "Lợi nhuận trên mỗi đơn hàng."),
        bare,
    ];
    assert_eq!(
        measure_label(r#"SUM("Benefit per order")"#, &profile),
        "Tổng lợi nhuận (Benefit per order)"
    );
    assert_eq!(
        measure_label(r#"SUM("Order Profit Per Order")"#, &profile),
        "Tổng Order Profit (Order Profit Per Order)"
    );
    assert_eq!(
        measure_label(r#"AVG("Benefit per order")"#, &profile),
        "Trung bình lợi nhuận trên mỗi đơn hàng (Benefit per order)"
    );
    assert_eq!(
        measure_label(r#"COUNT("Benefit per order")"#, &profile),
        "Số dòng có lợi nhuận trên mỗi đơn hàng (Benefit per order)"
    );
}

/// Mô tả của database mẫu có tên gốc và ghi chú hồ sơ trong ngoặc, ví dụ sau dấu phẩy: nhãn chỉ lấy phần đầu, và không
/// thành hai ngoặc liền nhau "(...) (item_total)" (lỗi gặp 01/11).
#[test]
fn column_label_keeps_only_the_first_part_of_the_description() {
    let profile = [
        column(
            "item_total",
            "Thành tiền sau giảm giá (Order Item Total; cột Sales per customer trong nguồn có cùng giá trị ở mọi dòng)",
        ),
        column("category_name", "Tên danh mục, ví dụ Cleats, Fishing (Category Name)"),
        column("shipping_mode", "Cách giao: Standard Class, Second Class, First Class, Same Day"),
        column("score", "Điểm từ 0 đến 100; dưới 40 là trượt bài. Rỗng khi nguồn ghi \"?\""),
        column("note", "(ghi chú của nguồn)"),
    ];
    assert_eq!(
        measure_label(r#"SUM("item_total")"#, &profile),
        "Tổng thành tiền sau giảm giá (item_total)"
    );
    assert_eq!(
        build_column_label("category_name", &profile, false),
        "Tên danh mục (category_name)"
    );
    assert_eq!(
        build_column_label("shipping_mode", &profile, true),
        "cách giao (shipping_mode)"
    );
    assert_eq!(
        measure_label(r#"AVG("score")"#, &profile),
        "Trung bình điểm từ 0 đến 100 (score)"
    );
    assert_eq!(build_column_label("note", &profile, false), "note");
}

/// Nhãn của lời giải thích: bí danh có dấu trước, bí danh không dấu ("ten danh muc") bị bỏ để dùng phần đầu của mô tả,
/// không có gì thì None (bên gọi dùng tên cột).
#[test]
fn display_name_skips_aliases_without_diacritics() {
    let aliases = |list: &[&str]| list.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    assert_eq!(
        build_display_name("shipping_mode", &aliases(&["cach giao", "cách giao"]), None),
        Some("cách giao".to_string())
    );
    assert_eq!(
        build_display_name(
            "category_name",
            &aliases(&["ten danh muc", "ten loai"]),
            Some("Tên danh mục, ví dụ Cleats, Fishing (Category Name)")
        ),
        Some("Tên danh mục".to_string())
    );
    assert_eq!(
        build_display_name("market", &aliases(&["thi truong"]), None),
        None
    );
    assert_eq!(
        build_display_name("Market", &aliases(&["market"]), Some("Market")),
        None
    );
}

/// strftime(ngày, '%Y-%m') làm khóa nhóm: thẻ "Cách tính" ghi "Tháng của ...", không chép nguyên đoạn SQL.
#[test]
fn strftime_key_reads_as_a_time_part() {
    let profile = [column(
        "order_date",
        "Thời điểm đặt hàng (order date (DateOrders))",
    )];
    let label = |sql: &str| {
        let tree = Parser::new(&DuckDbDialect {})
            .try_with_sql(sql)
            .and_then(|mut parser| parser.parse_expr())
            .unwrap();
        build_subject_label(&tree, &profile, false)
    };
    assert_eq!(
        label(r#"strftime(orders."order_date", '%Y-%m')"#),
        "Tháng của thời điểm đặt hàng (order_date)"
    );
    assert_eq!(
        label(r#"strftime('%Y', "order_date")"#),
        "Năm của thời điểm đặt hàng (order_date)"
    );
}

/// Điều kiện lọc theo quyền xem không kéo cả danh sách giá trị của mô tả vào nhãn (đã gặp: "thị trường nơi giao hàng:
/// Africa, Europe, LATAM, Pacific Asia, USCA (market) là Europe").
#[test]
fn row_filter_label_uses_the_short_description() {
    let profile = [column(
        "market",
        "Thị trường nơi giao hàng: Africa, Europe, LATAM, Pacific Asia, USCA",
    )];
    assert_eq!(
        build_row_filter_label(r#""market" IN ('Europe')"#, &profile),
        "Lọc theo quyền xem: thị trường nơi giao hàng (market) là Europe"
    );
}
