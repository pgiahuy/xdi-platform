// SPDX-License-Identifier: Apache-2.0
/* service/ask/summary_tests.rs - test của đoạn trả lời câu thường (ask/summary.rs): câu khuôn, danh sách dòng chỉ có
câu khuôn, lời của phần giải thích dùng lại khi đã qua bước kiểm, ANSWER_TEXT=false thì luôn là câu khuôn. Prompt duy
nhất có số của kết quả là prompt phần giải thích: test cuối dựng nó cho một bảng có cột nhạy cảm và xem không có ô nào
lọt vào. */

use crate::engine::sql_shape::build_query_shape;
use crate::engine::store::ProfileColumn;
use crate::service::ask::contract::{
    CellValue, ColumnRole, Insight, InsightKind, ResultColumn, ResultTable, Understanding,
    UnderstoodItem, ValueKind,
};
use crate::service::ask::summary::{
    build_answer, build_rows_answer, build_template_answer, SummaryInput, NO_DATA_ANSWER,
};
use crate::service::insight::facts::{build_group_parts, MeasureText, Point};
use crate::service::insight::find_unsendable_key;
use crate::service::insight::narrate::build_insight_prompt;
use crate::service::insight::plan::PlainTable;

fn column(name: &str, kind: ValueKind, role: ColumnRole) -> ResultColumn {
    ResultColumn {
        name: name.to_string(),
        kind,
        role,
    }
}

fn item(label: &str) -> UnderstoodItem {
    UnderstoodItem {
        label: label.to_string(),
        sql: String::new(),
        from_previous: false,
    }
}

fn understanding() -> Understanding {
    Understanding {
        measures: vec![item("Tổng doanh thu (Sales)")],
        filters: vec![],
        group_by: vec![item("Danh mục (Category Name)")],
        order_by: vec![],
        limit: None,
        time_range: None,
        assumptions: vec![],
    }
}

fn table(rows: &[(&str, f64)]) -> ResultTable {
    ResultTable {
        columns: vec![
            column("category", ValueKind::Text, ColumnRole::Dimension),
            column("sales", ValueKind::Number, ColumnRole::Measure),
        ],
        rows: rows
            .iter()
            .map(|(name, value)| {
                vec![
                    Some(CellValue::String(name.to_string())),
                    Some(CellValue::Number(*value)),
                ]
            })
            .collect(),
        total_rows: rows.len() as u64,
        truncated: false,
        limit_reached: false,
    }
}

fn sales() -> ResultTable {
    table(&[
        ("Fishing", 6929656.49),
        ("Cleats", 4431942.09),
        ("Camping & Hiking", 3667633.2),
    ])
}

fn input<'a>(
    result: &'a ResultTable,
    understanding: Option<&'a Understanding>,
    insight: Option<&'a Insight>,
) -> SummaryInput<'a> {
    SummaryInput {
        understanding,
        result,
        insight,
        enabled: true,
    }
}

fn insight(kind: InsightKind, summary: &str, by_model: bool) -> Insight {
    Insight {
        kind,
        title: "Fishing đứng đầu với 6.929.656,49.".to_string(),
        points: vec![],
        summary: summary.to_string(),
        summary_by_model: by_model,
        summary_warnings: vec![],
        summary_verdict: None,
        notable: true,
    }
}

#[test]
fn template_names_the_top_group_when_sorted() {
    let result = sales();
    assert_eq!(
        build_template_answer(&input(&result, None, None)),
        "Fishing có sales cao nhất: khoảng 6,93 triệu (6.929.656,49), tiếp theo là Cleats và Camping & Hiking."
    );
}

/// Một dòng ít cột: câu kể từng ô, số lớn có dạng rút gọn.
#[test]
fn template_of_one_row_lists_each_cell() {
    let result = table(&[("Fishing", 6929656.49)]);
    assert_eq!(
        build_template_answer(&input(&result, None, None)),
        "Kết quả: category là Fishing, sales là khoảng 6,93 triệu (6.929.656,49)."
    );
}

/// Tên độ đo chỉ bỏ ngoặc cuối khi đó là tên cột tiếng Anh; ngoặc tiếng Việt của tên chỉ số giữ nguyên.
#[test]
fn template_keeps_a_vietnamese_parenthesis_of_the_measure() {
    let result = sales();
    let mut u = understanding();
    u.measures = vec![item("Lợi nhuận (đã trừ chiết khấu)")];
    assert_eq!(
        build_template_answer(&input(&result, Some(&u), None)),
        "Fishing có lợi nhuận (đã trừ chiết khấu) cao nhất: khoảng 6,93 triệu (6.929.656,49), tiếp theo là Cleats và \
         Camping & Hiking."
    );
}

/// Danh sách dòng chỉ một dòng ngắn (câu "nhóm nào cao nhất" có LIMIT 1): đoạn trả lời là câu phần giải thích đã kể
/// dòng đó (insight/one_row.rs), hay lời của mô hình khi lời đó qua hai cổng.
#[test]
fn one_short_row_of_a_list_uses_the_insight_text() {
    let result = table(&[("Caguas", 74846.31)]);
    let mut list = insight(InsightKind::Rows, "Caguas cao nhất, với 74.846,31.", false);
    list.title = "Caguas có sales cao nhất: 74.846,31.".to_string();
    let outcome = build_answer(&input(&result, None, Some(&list))).unwrap();
    assert_eq!(outcome.text, "Caguas có sales cao nhất: 74.846,31.");
    assert!(!outcome.by_model);
    list.summary_by_model = true;
    let outcome = build_answer(&input(&result, None, Some(&list))).unwrap();
    assert_eq!(outcome.text, "Caguas cao nhất, với 74.846,31.");
    assert!(outcome.by_model);
}

/// Câu SQL đặt bí danh tiếng Việt cho cột tính ra thì câu khuôn gọi độ đo bằng bí danh đó, không bằng nhãn dựng từ công
/// thức; bí danh tiếng Anh thì giữ nhãn của thẻ "Cách tính".
#[test]
fn template_names_the_measure_by_its_vietnamese_alias() {
    let mut result = sales();
    result.columns[1].name = "Số lượng bán".to_string();
    let mut u = understanding();
    u.measures = vec![UnderstoodItem {
        label: "Số dòng có mã sản phẩm".to_string(),
        sql: "count(\"product_id\")".to_string(),
        from_previous: false,
    }];
    assert!(build_template_answer(&input(&result, Some(&u), None))
        .starts_with("Fishing có số lượng bán cao nhất:"));
    result.columns[1].name = "units".to_string();
    assert!(build_template_answer(&input(&result, Some(&u), None))
        .starts_with("Fishing có số dòng có mã sản phẩm cao nhất:"));
}

#[test]
fn template_without_order_only_counts_rows() {
    let result = table(&[("Cleats", 10.0), ("Fishing", 20.0)]);
    assert_eq!(
        build_template_answer(&input(&result, None, None)),
        "Kết quả có 2 dòng, xem bảng bên dưới."
    );
}

/// Kết quả chỉ có ô trống (tổng của năm không có dữ liệu): câu do code viết, không ra "Tổng ... là .".
#[test]
fn all_empty_result_gets_the_no_data_answer() {
    let result = ResultTable {
        columns: vec![column("total", ValueKind::Decimal, ColumnRole::Measure)],
        rows: vec![vec![None]],
        total_rows: 1,
        truncated: false,
        limit_reached: false,
    };
    let u = understanding();
    let outcome = build_answer(&input(&result, Some(&u), None)).unwrap();
    assert_eq!(outcome.text, NO_DATA_ANSWER);
    assert!(!outcome.by_model);
}

#[test]
fn template_of_one_cell_names_the_measure() {
    let result = ResultTable {
        columns: vec![column("total", ValueKind::Decimal, ColumnRole::Measure)],
        rows: vec![vec![Some(CellValue::String("36784735.01".to_string()))]],
        total_rows: 1,
        truncated: false,
        limit_reached: false,
    };
    let u = understanding();
    assert_eq!(
        build_template_answer(&input(&result, Some(&u), None)),
        "Tổng doanh thu là khoảng 36,8 triệu (36.784.735,01)."
    );
}

/// Lời của phần giải thích đã qua bước kiểm thì thành đoạn trả lời; lời của code hay ANSWER_TEXT=false thì câu khuôn.
#[test]
fn checked_insight_text_becomes_the_answer() {
    let result = sales();
    let text = "Fishing đứng đầu với 6.929.656,49, chiếm 46,4% tổng doanh thu.";
    let narrated = insight(InsightKind::Groups, text, true);
    let outcome = build_answer(&input(&result, None, Some(&narrated))).unwrap();
    assert_eq!(outcome.text, text);
    assert!(outcome.by_model);

    let code = insight(InsightKind::Groups, "câu của code", false);
    let outcome = build_answer(&input(&result, None, Some(&code))).unwrap();
    assert_eq!(
        outcome.text,
        "Fishing có sales cao nhất: khoảng 6,93 triệu (6.929.656,49), tiếp theo là Cleats và Camping & Hiking."
    );
    assert!(!outcome.by_model);

    let off = SummaryInput {
        enabled: false,
        ..input(&result, None, Some(&narrated))
    };
    assert!(!build_answer(&off).unwrap().by_model);
}

/// Lời của mô hình không được dùng mà câu khuôn chỉ đếm dòng (bảng không xếp theo số đo): đoạn trả lời là câu chính của
/// phần giải thích do code tính, không phải "Kết quả có 12 dòng, xem bảng bên dưới." (lỗi gặp với Gemini, 01/11).
#[test]
fn unsorted_table_without_model_text_uses_the_main_sentence() {
    let result = table(&[("Cleats", 10.0), ("Fishing", 20.0)]);
    let mut code = insight(InsightKind::TimeSeries, "câu của code", false);
    code.title =
        "Tháng 11/2017 đổi mạnh nhất so với kỳ trước: giảm 402.687,94 (41,7%).".to_string();
    let outcome = build_answer(&input(&result, None, Some(&code))).unwrap();
    assert_eq!(outcome.text, code.title);
    assert!(!outcome.by_model);
    // Không có câu chính thì vẫn là câu khuôn.
    code.title = String::new();
    let outcome = build_answer(&input(&result, None, Some(&code))).unwrap();
    assert_eq!(outcome.text, "Kết quả có 2 dòng, xem bảng bên dưới.");
}

/// Danh sách dòng không gộp ("danh sách khách hàng ở Scotland"): chỉ câu khuôn số dòng và điều kiện lọc, kể cả khi
/// insight có lời của mô hình (insight/mod.rs không gọi mô hình cho loại này).
#[test]
fn row_list_gets_the_count_and_filters_only() {
    let mut result = ResultTable {
        columns: vec![
            column("Customer Email", ValueKind::Text, ColumnRole::Dimension),
            column("Customer City", ValueKind::Text, ColumnRole::Dimension),
        ],
        rows: (0..37)
            .map(|i| {
                vec![
                    Some(CellValue::String(format!("person{i}@example.com"))),
                    Some(CellValue::String("Glasgow".to_string())),
                ]
            })
            .collect(),
        total_rows: 37,
        truncated: false,
        limit_reached: false,
    };
    let mut u = understanding();
    u.filters = vec![item("Quốc gia (Customer Country) là Scotland")];
    let list = insight(InsightKind::Rows, "person0@example.com", true);
    let outcome = build_answer(&input(&result, Some(&u), Some(&list))).unwrap();
    assert_eq!(
        outcome.text,
        "Có 37 dòng khớp điều kiện: Quốc gia (Customer Country) là Scotland."
    );
    assert!(!outcome.by_model);
    result.limit_reached = true;
    assert_eq!(
        build_rows_answer(&result, None),
        "Có ít nhất 37 dòng kết quả."
    );
}

#[test]
fn empty_result_has_no_answer_text() {
    let result = table(&[]);
    assert!(build_answer(&input(&result, None, None)).is_none());
}

fn profile_column(name: &str, role: &str, sensitive: bool) -> ProfileColumn {
    ProfileColumn {
        id: uuid::Uuid::new_v4(),
        position: 0,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        description: None,
        description_source: None,
        aliases: Vec::new(),
        is_sensitive: sensitive,
        null_count: Some(0),
        distinct_count: Some(3),
        min_value: None,
        max_value: None,
        time_format: None,
        unit: None,
        aggregation: "sum".to_string(),
    }
}

/// Prompt duy nhất có số của kết quả câu thường là prompt lời của phần giải thích. Với bảng theo nhóm có một cột chữ
/// nhạy cảm đi kèm, prompt chỉ có câu do code tính: không ô nào của cột nhạy cảm, không dòng nào nguyên văn.
#[test]
fn insight_prompt_has_no_result_rows_or_sensitive_values() {
    let rows = [
        ("Fishing", "lan@example.com", 6929656.49),
        ("Cleats", "minh@example.com", 4431942.09),
        ("Camping & Hiking", "an@example.com", 3667633.2),
    ];
    let points: Vec<Point> = rows
        .iter()
        .map(|(name, _, value)| Point {
            label: name.to_string(),
            value: value.to_string().parse().unwrap(),
        })
        .collect();
    let parts = build_group_parts(MeasureText::total("Doanh thu"), "Danh mục", &points, None);
    let prompt = build_insight_prompt("Doanh thu theo danh mục", &parts);
    for (_, email, value) in rows {
        assert!(!prompt.contains(email), "{prompt}");
        assert!(!prompt.contains(&value.to_string()), "{prompt}");
    }
    assert!(!prompt.contains("Cleats"), "{prompt}");
    assert!(
        prompt.contains("Fishing đứng đầu với 6.929.656,49"),
        "{prompt}"
    );
}

/// Cột khóa đọc từ cột nhạy cảm (nhóm theo email khách hàng) hay cột khóa (mã đơn): insight/mod.rs dùng câu của code,
/// không gọi mô hình. Cột phân loại không nhạy cảm thì được gửi.
#[test]
fn group_key_from_a_sensitive_column_is_found() {
    let columns = [
        profile_column("Customer Email", "dimension", true),
        profile_column("Category Name", "dimension", false),
        profile_column("Sales", "measure", false),
        profile_column("Order Id", "key", false),
    ];
    let tables = [PlainTable {
        table: "sales",
        columns: &columns,
        row_count: Some(3),
    }];
    let by_email = build_query_shape(
        r#"SELECT lower("Customer Email") AS who, SUM("Sales") FROM sales GROUP BY 1"#,
    )
    .unwrap();
    assert_eq!(
        find_unsendable_key(&tables, &by_email, 0).as_deref(),
        Some("Customer Email")
    );
    let by_category =
        build_query_shape(r#"SELECT "Category Name", SUM("Sales") FROM sales GROUP BY 1"#).unwrap();
    assert_eq!(find_unsendable_key(&tables, &by_category, 0), None);
    let by_order =
        build_query_shape(r#"SELECT "Order Id", SUM("Sales") FROM sales GROUP BY 1"#).unwrap();
    assert_eq!(
        find_unsendable_key(&tables, &by_order, 0).as_deref(),
        Some("Order Id")
    );
}
