// SPDX-License-Identifier: Apache-2.0
/* service/eval_tests.rs - ca test của bộ đánh giá, thuần không cần Postgres: chấm một câu ra ba trạng thái
(build_answer_verdict, build_ask_error_verdict), và SQL
đúng của bộ câu JOIN eval/sales_joins.json và bộ câu edu eval/edu.json qua guard. */

use super::eval::load_cases;

/// Bộ câu JOIN trên schema sales của database mẫu (eval/sales_joins.json, #213): mỗi SQL đúng qua guard với sáu bảng như
/// ảnh chụp database mẫu, và đọc bảng của trường "dataset" (bảng người hỏi chọn). `make eval` chạy bộ này trên ảnh chụp
/// thật của database mẫu (data/postgres-demo, #230) khi nó đang chạy.
#[test]
fn sales_join_cases_pass_the_guard() {
    use crate::engine::guard::{build_guarded_sql, AllowedTable, JoinLink};

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
    // Khóa ngoại của schema sales trong data/postgres-demo/sql/sales_schema.sql.
    let link = |many: &str, many_column: &str, one: &str, one_column: &str| JoinLink {
        many: many.to_string(),
        many_columns: vec![many_column.to_string()],
        one: one.to_string(),
        one_columns: vec![one_column.to_string()],
        fans_out: true,
    };
    let links = [
        link(
            "categories",
            "department_id",
            "departments",
            "department_id",
        ),
        link("products", "category_id", "categories", "category_id"),
        link("orders", "customer_id", "customers", "customer_id"),
        link("order_items", "order_id", "orders", "order_id"),
        link("order_items", "product_id", "products", "product_id"),
    ];
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval/sales_joins.json");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() >= 8);
    for case in &cases {
        let picked = case["dataset"].as_str().unwrap();
        for sql in case["expected_sql"].as_array().unwrap() {
            let guarded = build_guarded_sql(sql.as_str().unwrap(), &tables, &links, 1000)
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

/// Bộ câu trên schema edu của database mẫu (eval/edu.json, #230): mỗi SQL đúng qua guard với các bảng edu như ảnh chụp
/// database mẫu và đọc bảng của trường "dataset". `make eval` chạy bộ này trên ảnh chụp thật của database mẫu khi nó đang chạy.
#[test]
fn edu_cases_pass_the_guard() {
    use crate::engine::guard::{build_guarded_sql, AllowedTable, JoinLink};

    let schema: [(&str, &[&str]); 5] = [
        (
            "students",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "gender",
                "region",
                "highest_education",
                "imd_band",
                "age_band",
                "num_of_prev_attempts",
                "studied_credits",
                "disability",
                "final_result",
            ],
        ),
        (
            "courses",
            &[
                "code_module",
                "code_presentation",
                "presentation_year",
                "presentation_start_month",
                "module_presentation_length",
            ],
        ),
        (
            "assessments",
            &[
                "id_assessment",
                "code_module",
                "code_presentation",
                "assessment_type",
                "date",
                "weight",
            ],
        ),
        (
            "student_assessments",
            &[
                "id_assessment",
                "id_student",
                "code_module",
                "code_presentation",
                "date_submitted",
                "is_banked",
                "score",
            ],
        ),
        (
            "student_daily_clicks",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "date",
                "sum_click",
                "num_sites",
            ],
        ),
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
    // Khóa ngoại của schema edu trong data/postgres-demo/sql/edu_schema.sql.
    let link = |many: &str, one: &str, columns: &[&str]| JoinLink {
        many: many.to_string(),
        many_columns: columns.iter().map(|c| c.to_string()).collect(),
        one: one.to_string(),
        one_columns: columns.iter().map(|c| c.to_string()).collect(),
        fans_out: true,
    };
    let course = ["code_module", "code_presentation"];
    let student = ["code_module", "code_presentation", "id_student"];
    let links = [
        link("students", "courses", &course),
        link("assessments", "courses", &course),
        link("student_assessments", "assessments", &["id_assessment"]),
        link("student_assessments", "students", &student),
        link("student_daily_clicks", "students", &student),
    ];
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval/edu.json");
    let cases = load_cases(std::path::Path::new(path)).unwrap();
    assert!(cases.len() >= 7);
    for case in &cases {
        let picked = case.dataset.as_deref().unwrap();
        for sql in &case.expected_sql {
            let guarded = build_guarded_sql(sql, &tables, &links, 1000)
                .unwrap_or_else(|e| panic!("{}: {e:?}", case.id));
            assert!(
                guarded.tables_read.iter().any(|t| t == picked),
                "{}: {:?}",
                case.id,
                guarded.tables_read
            );
        }
    }
}

/// Bộ câu OULAD trên database mẫu của #202 (eval/oulad_db.json, gồm cả câu phải từ chối và câu tiếp nối): mọi
/// expected_sql qua guard với sáu bảng edu như ảnh chụp database mẫu (data/samples/sample_database.json) và đúng khóa
/// ngoại của data/postgres-demo/sql/edu_schema.sql. Không đòi đọc bảng của trường "dataset" như edu_cases_pass_the_guard:
/// câu bẫy nhân dòng (kind plain_join_trap) hỏi trên một bảng mà đáp án đúng cố ý chỉ đọc bảng khác. load_cases đọc được
/// file dù có các trường chỉ người đọc dùng (author, reviewers, note), vì runner bỏ qua trường lạ. Câu tiếp nối trỏ tới
/// câu gốc đứng trước nó trong file (service/eval.rs chỉ đọc được cuộc của câu đã hỏi), câu phải từ chối không có đáp án.
#[test]
fn oulad_db_cases_pass_the_guard() {
    use crate::engine::guard::{build_guarded_sql, AllowedTable, JoinLink};

    let course = ["code_module", "code_presentation"];
    let student = ["code_module", "code_presentation", "id_student"];
    let schema: [(&str, &[&str]); 6] = [
        (
            "students",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "gender",
                "region",
                "highest_education",
                "imd_band",
                "age_band",
                "num_of_prev_attempts",
                "studied_credits",
                "disability",
                "final_result",
            ],
        ),
        (
            "courses",
            &[
                "code_module",
                "code_presentation",
                "presentation_year",
                "presentation_start_month",
                "module_presentation_length",
            ],
        ),
        (
            "registrations",
            &[
                "code_module",
                "code_presentation",
                "id_student",
                "date_registration",
                "date_unregistration",
            ],
        ),
        (
            "assessments",
            &[
                "id_assessment",
                "code_module",
                "code_presentation",
                "assessment_type",
                "date",
                "weight",
            ],
        ),
        (
            "student_assessments",
            &[
                "id_assessment",
                "id_student",
                "code_module",
                "code_presentation",
                "date_submitted",
                "is_banked",
                "score",
            ],
        ),
        (
            "vle_materials",
            &[
                "id_site",
                "code_module",
                "code_presentation",
                "activity_type",
                "week_from",
                "week_to",
            ],
        ),
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
    let link = |many: &str, one: &str, columns: &[&str], fans_out: bool| JoinLink {
        many: many.to_string(),
        many_columns: columns.iter().map(|c| c.to_string()).collect(),
        one: one.to_string(),
        one_columns: columns.iter().map(|c| c.to_string()).collect(),
        fans_out,
    };
    let links = [
        link("students", "courses", &course, true),
        // Mỗi dòng students có đúng một dòng registrations (comment của bảng trong edu_schema.sql).
        link("registrations", "students", &student, false),
        link("assessments", "courses", &course, true),
        link(
            "student_assessments",
            "assessments",
            &["id_assessment"],
            true,
        ),
        link("student_assessments", "students", &student, true),
        link("vle_materials", "courses", &course, true),
    ];
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval/");
    let main = format!("{dir}oulad_db.json");
    let cases = load_cases(std::path::Path::new(&main)).unwrap();
    assert!(cases.len() >= 50);
    for (i, case) in cases.iter().enumerate() {
        assert_eq!(
            case.must_refuse,
            case.expected_sql.is_empty(),
            "{}",
            case.id
        );
        if let Some(parent) = &case.follow_up_of {
            assert!(
                cases[..i].iter().any(|c| &c.id == parent),
                "{}: parent {parent} must come first",
                case.id
            );
        }
        for sql in &case.expected_sql {
            build_guarded_sql(sql, &tables, &links, 1000)
                .unwrap_or_else(|e| panic!("{}: {e:?}\n{sql}", case.id));
        }
    }
}
