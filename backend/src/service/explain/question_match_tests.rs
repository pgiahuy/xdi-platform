// SPDX-License-Identifier: Apache-2.0
/* service/explain/question_match_tests.rs - test explain/question_match.rs (nhóm câu hỏi nêu tên, độ đo theo cột lượt
phân loại nêu) và phần form dùng cột của bảng liên quan (explain/joined.rs, explain/check.rs). Form là các form sai đã gặp
khi chạy thật với Gemini: so AAA với các môn còn lại khi câu hỏi nêu AAA và GGG, so AAA với BBB khi câu hỏi chỉ nêu AAA,
đo trọng số bài cho câu hỏi về tỷ lệ bỏ học. Hồ sơ là bản thu nhỏ DataCo của explain/check_tests.rs. */

use serde_json::json;
use uuid::Uuid;

use crate::engine::store::{ColumnAggregation, ValueRow};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::values::ValueIndex;
use crate::service::explain::check::{build_why_check, PlanSlices, WhyProfile};
use crate::service::explain::check_tests::{column, Fixture, TABLE};
use crate::service::explain::form::{FieldError, WhyForm};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::joined::{JoinedColumns, NO_JOINED};
use crate::service::explain::question_match::{check_measure_columns, check_named_groups};

fn form(value: serde_json::Value) -> WhyForm {
    serde_json::from_value(value).unwrap()
}

fn groups_form(compare: &str, base: Option<&str>) -> WhyForm {
    let mut groups = json!({"column": "Market", "compare": compare});
    if let Some(base) = base {
        groups["base"] = json!(base);
    }
    form(json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "groups": groups,
        "compare": "",
        "filters": []
    }))
}

fn named_errors(question: &str, form: &WhyForm) -> Vec<FieldError> {
    let fixture = Fixture::new();
    check_named_groups(question, form, &fixture.profile())
}

/// Câu nêu hai nhóm mà form chỉ có một (đã gặp: "giữa môn AAA và GGG" ra "AAA so với các môn còn lại"): lỗi chỉ đúng
/// hai nhóm cần so, giữ nhóm form đã điền làm nhóm đang so.
#[test]
fn two_named_groups_must_both_be_compared() {
    let question = "Vì sao doanh thu khác nhau giữa Europe và LATAM?";
    let errors = named_errors(question, &groups_form("Europe", None));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field, "groups.base");
    assert!(
        errors[0]
            .message
            .contains("set \"groups.compare\" to \"Europe\" and \"groups.base\" to \"LATAM\""),
        "{}",
        errors[0].message
    );
    // Form đã so đúng hai nhóm, theo thứ tự nào cũng được.
    assert!(named_errors(question, &groups_form("Europe", Some("LATAM"))).is_empty());
    assert!(named_errors(question, &groups_form("LATAM", Some("Europe"))).is_empty());
    // Form điền LATAM: LATAM giữ làm nhóm đang so, Europe là nhóm gốc.
    let errors = named_errors(question, &groups_form("LATAM", Some("Africa")));
    assert!(errors[0]
        .message
        .contains("\"groups.compare\" to \"LATAM\" and \"groups.base\" to \"Europe\""));
}

/// Câu chỉ nêu một nhóm (đã gặp: "vì sao AAA ít sinh viên" so với BBB): nhóm gốc phải để trống, nhóm đang so phải là
/// nhóm câu hỏi nêu. Không nêu nhóm nào thì không kiểm.
#[test]
fn one_named_group_is_compared_with_the_others() {
    let question = "Vì sao doanh thu Pacific Asia thấp vậy?";
    assert!(named_errors(question, &groups_form("Pacific Asia", None)).is_empty());
    let errors = named_errors(question, &groups_form("Pacific Asia", Some("Europe")));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0]
        .message
        .contains("names only \"Pacific Asia\" of \"Market\", not \"Europe\""));
    let errors = named_errors(question, &groups_form("USCA", None));
    assert_eq!(errors[0].field, "groups.compare");
    assert!(named_errors("Vì sao doanh thu lệch nhau?", &groups_form("USCA", None)).is_empty());
}

/// So theo từ trọn vẹn và không dấu: "latam" gõ thường vẫn là LATAM; giá trị là một phần của từ khác thì không tính.
#[test]
fn named_groups_match_whole_words() {
    let question = "vi sao doanh thu europe khac latam";
    let errors = named_errors(question, &groups_form("Europe", None));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(named_errors(
        "Vì sao doanh thu Europeans cao?",
        &groups_form("Europe", None)
    )
    .is_empty());
}

fn context_column(name: &str, role: &str, picked: bool) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        time_format: None,
        description: None,
        aliases: Vec::new(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: picked,
        unit: None,
        aggregation: ColumnAggregation::Sum,
        join_key: false,
        metric_column: false,
        picked,
        brief: false,
    }
}

/// "Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B": lượt phân loại nêu final_result và code_presentation, form đo
/// trọng số (weight) mà không dùng final_result: lỗi chỉ sang tỷ lệ dòng có một giá trị của final_result.
#[test]
fn a_measure_the_classify_step_did_not_name_is_an_error() {
    let context = [
        context_column("code_presentation", "dimension", true),
        context_column("final_result", "dimension", true),
        context_column("weight", "measure", false),
    ];
    let wrong = form(json!({
        "terms": [],
        "measure": {"column": "weight", "agg": "avg"},
        "groups": {"column": "code_presentation", "compare": "2014J", "base": "2014B"},
        "compare": "",
        "filters": []
    }));
    let errors = check_measure_columns(&wrong, &context);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].field, "measure.column");
    assert!(errors[0].message.contains("\"final_result\""));
    assert!(errors[0].message.contains("\"agg\": \"share\""));
    let right = form(json!({
        "terms": [],
        "measure": {"column": "final_result", "agg": "share", "value": "Withdrawn"},
        "groups": {"column": "code_presentation", "compare": "2014J", "base": "2014B"},
        "compare": "",
        "filters": []
    }));
    assert!(check_measure_columns(&right, &context).is_empty());
    // Lượt phân loại không nêu cột nào: không có gì để so.
    let silent = [context_column("weight", "measure", false)];
    assert!(check_measure_columns(&wrong, &silent).is_empty());
}

/// Cột của bảng liên quan ("orders.market", "orders.order_date") dùng được cho nhóm, kỳ và bộ lọc; độ đo trên cột của
/// bảng liên quan là lỗi.
#[test]
fn joined_columns_serve_groups_but_not_the_measure() {
    let fixture = Fixture::new();
    let mut market = column(101, "orders.market", "VARCHAR", "dimension", 5);
    market.aliases = vec!["thị trường".to_string()];
    let mut date = column(102, "orders.order_date", "TIMESTAMP", "time", 900);
    date.min_value = Some("2015-01-01 00:00:00".to_string());
    date.max_value = Some("2018-01-31 00:00:00".to_string());
    let profit = column(103, "orders.order_profit", "DOUBLE", "measure", 900);
    let rows: Vec<ValueRow> = ["Europe", "LATAM"]
        .iter()
        .map(|v| ValueRow {
            column_id: Uuid::from_u128(101),
            column_name: "orders.market".to_string(),
            value: v.to_string(),
            aliases: Vec::new(),
        })
        .collect();
    let joined = JoinedColumns {
        profile: vec![market, date, profit],
        prompt: Vec::new(),
        values: ValueIndex::build(&rows).columns,
    };
    let profile = WhyProfile {
        joined: &joined,
        ..fixture.profile()
    };
    let europe = form(json!({
        "terms": [],
        "measure": {"column": "Order Item Total", "agg": "sum"},
        "groups": {"column": "orders.market", "compare": "Europe", "base": "LATAM"},
        "time": {"column": "orders.order_date", "grain": "year"},
        "compare": "2017",
        "filters": []
    }));
    let plan = build_why_check(&europe, &profile, None).unwrap();
    let PlanSlices::Groups {
        column,
        base,
        period,
        ..
    } = &plan.slices
    else {
        panic!("{:?}", plan.slices);
    };
    assert_eq!(column, "orders.market");
    assert_eq!(base, &GroupBase::Value("LATAM".to_string()));
    assert!(period.is_some());
    assert_eq!(plan.labels["orders.market"], "thị trường");
    assert!(check_named_groups(
        "Vì sao doanh thu năm 2017 khác nhau giữa Europe và LATAM?",
        &europe,
        &profile
    )
    .is_empty());

    let on_parent = form(json!({
        "terms": [],
        "measure": {"column": "orders.order_profit", "agg": "sum"},
        "groups": {"column": "orders.market", "compare": "Europe", "base": "LATAM"},
        "compare": "",
        "filters": []
    }));
    let errors = build_why_check(&on_parent, &profile, None).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.field == "measure.column" && e.message.contains("joined table")),
        "{errors:?}"
    );
    // Không có bảng liên quan thì cột "orders.market" là cột lạ như trước.
    let alone = WhyProfile {
        joined: &NO_JOINED,
        ..fixture.profile()
    };
    assert!(build_why_check(&europe, &alone, None).is_err());
    assert_eq!(profile.table, TABLE);
}
