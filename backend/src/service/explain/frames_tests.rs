// SPDX-License-Identifier: Apache-2.0
/* service/explain/frames_tests.rs - test phần thuần của các khuôn câu "tại sao" thêm ở #244: một nhóm so với các nhóm
còn lại (explain/group_base.rs), đọc và kiểm form, SQL dựng ra. Hồ sơ DataCo thu nhỏ của explain/check_tests.rs; trọn
đường trên Parquet tạm ở explain/frames_run_tests.rs. */

use std::collections::HashMap;

use serde_json::json;

use crate::service::explain::check::PlanSlices;
use crate::service::explain::check_tests::Fixture;
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::sql::{build_dimension_sql, build_why_sql};

fn rest_form() -> serde_json::Value {
    json!({
        "metric": "doanh thu",
        "groups": {"column": "Market", "compare": "Pacific Asia"},
        "filters": []
    })
}

/// "Vì sao Pacific Asia khác các thị trường khác": base vắng là mọi nhóm còn lại; dòng trống của Market không thuộc
/// lát nào.
#[test]
fn group_without_base_is_compared_with_the_rest() {
    let fixture = Fixture::new();
    let plan = fixture.plan(rest_form());
    let PlanSlices::Groups { base, compare, .. } = &plan.slices else {
        panic!("expected two groups, got {:?}", plan.slices);
    };
    assert_eq!(base, &GroupBase::Rest);
    assert_eq!(compare, "Pacific Asia");
    let sql = build_why_sql(&plan);
    assert_eq!(sql.base_filter, "\"Market\" <> 'Pacific Asia'");
    assert_eq!(sql.compare_filter, "\"Market\" = 'Pacific Asia'");
    assert!(
        sql.total_sql
            .contains("WHERE (\"Market\" <> 'Pacific Asia' OR \"Market\" = 'Pacific Asia')"),
        "{}",
        sql.total_sql
    );
    // Cột của hai lát không là chiều phân rã, như so hai nhóm.
    assert_eq!(plan.dimensions, ["Customer Segment"]);
    let segment = build_dimension_sql(&plan, "Customer Segment");
    assert!(
        segment.contains("CASE WHEN \"Market\" <> 'Pacific Asia' THEN"),
        "{segment}"
    );
    // Chuỗi rỗng cũng là phần còn lại: Gemini hay ghi "" thay cho bỏ trường.
    let mut empty = rest_form();
    empty["groups"]["base"] = json!("");
    assert_eq!(fixture.plan(empty), plan);
}

#[test]
fn rest_is_named_by_the_column_meaning() {
    let mut labels = HashMap::new();
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels),
        "các nhóm còn lại"
    );
    labels.insert("Market".to_string(), "Thị trường".to_string());
    assert_eq!(
        GroupBase::Rest.build_label("Market", &labels),
        "các thị trường còn lại"
    );
    assert_eq!(
        GroupBase::Value("LATAM".to_string()).build_label("Market", &labels),
        "LATAM"
    );
    // Dấu nháy trong giá trị nhóm in thành chuỗi SQL đúng, không cắt được câu.
    let condition = GroupBase::Rest.build_condition("Market", "Europe' OR 1=1 --");
    assert_eq!(condition.to_string(), "\"Market\" <> 'Europe'' OR 1=1 --'");
}

/// Nhóm đang so vẫn phải có trong danh sách giá trị khi so với phần còn lại.
#[test]
fn rest_needs_a_listed_compare_group() {
    let fixture = Fixture::new();
    let mut form = rest_form();
    form["groups"]["compare"] = json!("Asia");
    assert_eq!(fixture.fields(form), ["groups.compare"]);
}
