// SPDX-License-Identifier: Apache-2.0
/* service/explain/claims_tests.rs - test cổng 1 (explain/claims.rs, #201) trên bảng phân rã của mẫu why
(ask/examples.rs). Khẳng định của lời mẫu khớp bảng thì qua. Khẳng định cố ý sai thì trượt: số gán nhầm nhóm, sai
dấu (sai chiều), nguyên nhân chính ngược chiều hay ngoài ba nhóm đầu, nhóm kéo lại cùng chiều, nhóm hay ô không có
trong bảng, phần trăm chép sai. ask/narrate_tests.rs dùng build_example_reply cho bước narrate. */

use serde_json::json;

use crate::service::ask::contract::Explanation;
use crate::service::ask::examples::build_why_example;
use crate::service::explain::annotate::GroupPresence;
use crate::service::explain::build_number_check;
use crate::service::explain::claims::{
    build_claim_check, build_narrate_schema, parse_narrate_reply, Claim, ClaimField, NarrateReply,
};

fn example() -> Explanation {
    build_why_example().explanation.expect("why example")
}

fn claim(field: &str, column: &str, group: &str, value: &str) -> serde_json::Value {
    json!({ "field": field, "column": column, "group": group, "value": value })
}

/// Insight của mẫu why (cũng là câu trả lời narrate của mock) kèm một khẳng định cho mỗi số và mỗi nhóm insight dùng.
pub(crate) fn build_example_reply() -> serde_json::Value {
    let e = example();
    let reasons: Vec<serde_json::Value> = e
        .summary_points
        .iter()
        .map(|p| json!({ "lead": p.lead, "text": p.text }))
        .collect();
    json!({
        "headline": e.summary_title,
        "reasons": reasons,
        "claims": [
            claim("change", "", "", "-1.283.400,00"),
            claim("percent_change", "", "", "-13,6"),
            claim("base_value", "", "", "9.402.500,00"),
            claim("compare_value", "", "", "8.119.100,00"),
            claim("base_volume", "", "", "5.000"),
            claim("compare_volume", "", "", "4.400"),
            claim("volume_effect", "", "", "-1.117.725,00"),
            claim("volume_share", "", "", "87,1"),
            claim("base_price", "", "", "1.880,50"),
            claim("compare_price", "", "", "1.845,25"),
            claim("price_effect", "", "", "-165.675,00"),
            claim("contribution", "Market", "Europe", "-639.780,25"),
            claim("cause", "Market", "Europe", ""),
            claim("contribution", "Market", "LATAM", "-339.049,50"),
        ]
    })
}

fn claims(value: serde_json::Value) -> Vec<Claim> {
    serde_json::from_value(value).expect("claims shape")
}

fn check_on(value: serde_json::Value, explanation: &Explanation) -> Result<usize, Vec<String>> {
    build_claim_check(&claims(value), explanation)
}

fn check(value: serde_json::Value) -> Result<usize, Vec<String>> {
    check_on(value, &example())
}

#[test]
fn example_claims_pass() {
    let reply: NarrateReply = serde_json::from_value(build_example_reply()).unwrap();
    assert_eq!(build_claim_check(&reply.claims, &example()), Ok(14));
    // Nối lại đúng như summary của mẫu: chữ mô hình chấm đọc là chữ người dùng thấy.
    assert_eq!(reply.build_text(), example().summary);
}

/// Số của LATAM khai cho Europe: bước kiểm số cũ cho lời qua vì số có trong bảng; cổng 1 so đúng ô nên trượt.
#[test]
fn number_of_another_group_fails() {
    assert!(build_number_check("Thị trường Europe giảm 339.049,50.", &example()).is_ok());
    let problems = check(json!([claim(
        "contribution",
        "Market",
        "Europe",
        "-339.049,50"
    )]))
    .unwrap_err();
    assert_eq!(
        problems,
        ["contribution của Europe (Market): giá trị chép là -339.049,50, bảng ghi -639.780,25"]
    );
}

/// Chiều nằm ở dấu: khai Europe tăng (số dương) khi bảng ghi giảm thì trượt.
#[test]
fn wrong_sign_fails() {
    let problems = check(json!([claim(
        "contribution",
        "Market",
        "Europe",
        "639.780,25"
    )]))
    .unwrap_err();
    assert!(problems[0].contains("bảng ghi -639.780,25"), "{problems:?}");
}

/// Dấu cộng trước số dương không đổi nghĩa nên vẫn qua.
#[test]
fn plus_sign_on_a_positive_value_passes() {
    assert_eq!(
        check(json!([claim("base_value", "", "", "+9.402.500,00")])),
        Ok(1)
    );
}

/// Nguyên nhân chính phải cùng chiều chênh lệch và nằm trong ba nhóm đầu; nhóm kéo lại phải ngược chiều.
#[test]
fn named_groups_follow_the_direction_and_the_top_three() {
    let mut explanation = example();
    // LATAM đổi sang tăng: đi ngược chiều doanh thu giảm.
    explanation.dimensions[0].groups[1].contribution = "339049.50".to_string();
    let named = |field: &str, group: &str| {
        check_on(json!([claim(field, "Market", group, "")]), &explanation)
    };
    assert_eq!(named("cause", "Europe"), Ok(1));
    assert!(named("cause", "LATAM").is_err());
    assert_eq!(named("against", "LATAM"), Ok(1));
    assert!(named("against", "Europe").is_err());
    // USCA là nhóm thứ tư của Market.
    let problems = named("cause", "USCA").unwrap_err();
    assert!(problems[0].contains("3 nhóm"), "{problems:?}");
}

#[test]
fn unknown_group_or_cell_fails() {
    let problems = check(json!([claim(
        "contribution",
        "Market",
        "Antarctica",
        "-1.000,00"
    )]))
    .unwrap_err();
    assert!(
        problems[0].contains("không có nhóm Antarctica"),
        "{problems:?}"
    );
    // Tổng không có phần cơ cấu: mix_effect chỉ có ở nhóm hay ở chiều.
    let problems = check(json!([claim("mix_effect", "", "", "-1,00")])).unwrap_err();
    assert!(problems[0].contains("không có ô này"), "{problems:?}");
}

/// Nhóm nêu là chỉ có ở một bên phải đúng như ghi chú: nhóm có ở cả hai kỳ thì khai new hay gone đều trượt.
#[test]
fn presence_claims_follow_the_notes() {
    let mut explanation = example();
    // LATAM không còn ở kỳ so sánh.
    explanation.notes.dimensions[0].groups[1].presence = GroupPresence::Gone;
    let named = |field: &str, group: &str| {
        check_on(json!([claim(field, "Market", group, "")]), &explanation)
    };
    assert_eq!(named("gone", "LATAM"), Ok(1));
    assert!(named("new", "LATAM").is_err());
    assert!(named("gone", "Europe").is_err());
}

/// Cột nhận cả tên tiếng Việt của hồ sơ cột ("thị trường" của Market).
#[test]
fn column_label_finds_the_group() {
    assert_eq!(
        check(json!([claim(
            "contribution",
            "thị trường",
            "Europe",
            "-639.780,25"
        )])),
        Ok(1)
    );
}

/// Phần trăm thay đổi code đã tính (một chữ số lẻ): chép đúng kể cả dấu thì qua, kèm "%" hay không; chép sai số hay
/// sai dấu thì trượt.
#[test]
fn percentages_are_copied_as_printed() {
    let mut explanation = example();
    explanation.percent_change = Some("-13.6".to_string());
    let percent = |value: &str| {
        check_on(
            json!([claim("percent_change", "", "", value)]),
            &explanation,
        )
    };
    assert_eq!(percent("-13,6"), Ok(1));
    assert_eq!(percent("-13,6%"), Ok(1));
    assert!(percent("-14").is_err());
    assert!(percent("13,6").is_err());
}

/// Mỗi khẳng định sai có một lý do; cùng một lý do không lặp lại.
#[test]
fn problems_are_listed_once_each() {
    let wrong = claim("contribution", "Market", "Europe", "-1,00");
    let problems = check(json!([
        wrong.clone(),
        wrong,
        claim("change", "", "", "1.283.400,00")
    ]))
    .unwrap_err();
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn every_field_is_in_the_schema_and_reads_back() {
    let schema = build_narrate_schema();
    let names = schema["properties"]["claims"]["items"]["properties"]["field"]["enum"]
        .as_array()
        .expect("field enum");
    assert_eq!(names.len(), ClaimField::ALL.len());
    for field in ClaimField::ALL {
        let parsed: ClaimField = serde_json::from_value(json!(field.name())).unwrap();
        assert_eq!(parsed, field);
    }
}

#[test]
fn plain_text_reply_is_not_read() {
    assert!(parse_narrate_reply("Doanh thu giảm 1.283.400,00.").is_err());
    // Khuôn cũ {text, claims} không còn đọc được: thiếu câu mở đầu.
    assert!(parse_narrate_reply(r#"{"text": "Doanh thu giảm.", "claims": []}"#).is_err());
    assert!(parse_narrate_reply(&build_example_reply().to_string()).is_ok());
}
