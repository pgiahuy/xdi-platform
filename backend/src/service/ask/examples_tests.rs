// SPDX-License-Identifier: Apache-2.0
/* service/ask/examples_tests.rs - test cho response mẫu của ask (ask/examples.rs): file JSON của frontend khớp code,
mẫu khớp schema trong đặc tả OpenAPI, và số trong mẫu phân rã cộng khớp. */

use serde_json::Value;

use crate::service::ask::contract::AskStatus;
use crate::service::ask::examples::*;

/// frontend/src/api/examples/ask.json phải khớp mẫu sinh từ code. Lệch tức là đổi struct của ask (hoặc mẫu) mà quên
/// chạy make openapi: giao diện sẽ dựng trên hình dạng cũ.
#[test]
fn ask_examples_json_is_up_to_date() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../frontend/src/api/examples/ask.json"
    );
    let committed = std::fs::read_to_string(path).expect("read frontend/src/api/examples/ask.json");
    assert!(
        committed == build_examples_json(),
        "frontend/src/api/examples/ask.json is out of date with the code: run `make openapi` and commit it"
    );
}

/// Mẫu phải khớp schema AskResponse trong đặc tả, tức là khớp kiểu schema.ts của giao diện. Test trên chỉ so file với
/// code; nếu serde và đặc tả lệch nhau (ví dụ skip_serializing_if trên trường đặc tả ghi là bắt buộc), chỉ test này bắt
/// được. Giao diện không tự kiểm được: TypeScript nới kiểu chuỗi của file JSON thành string, không so với enum.
#[test]
fn ask_examples_match_the_spec() {
    let spec: Value = serde_json::from_str(&crate::api::build_openapi_json()).expect("parse spec");
    let examples: Value = serde_json::from_str(&build_examples_json()).expect("parse examples");
    let schema = serde_json::json!({ "$ref": "#/components/schemas/AskResponse" });
    for (name, example) in examples.as_object().expect("examples object") {
        if let Err(error) = check_schema(&spec, &schema, example, name) {
            panic!("example {error}");
        }
    }
}

// Kiểm vừa đủ những gì đặc tả của ask dùng: $ref, oneOf, enum, type, required, properties, items. Trường lạ không có
// trong properties cũng là lỗi, vì schema.ts sẽ không có nó.
fn check_schema(spec: &Value, schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(reference) = schema["$ref"].as_str() {
        let name = reference.trim_start_matches("#/components/schemas/");
        return check_schema(spec, &spec["components"]["schemas"][name], value, path);
    }
    if let Some(branches) = schema["oneOf"].as_array() {
        // Báo lỗi của nhánh không phải null, để biết trường nào sai thay vì in cả object.
        let mut last_error = format!("{path}: {value} matches no branch of oneOf");
        for branch in branches {
            match check_schema(spec, branch, value, path) {
                Ok(()) => return Ok(()),
                Err(error) if branch["type"] != "null" => last_error = error,
                Err(_) => {}
            }
        }
        return Err(last_error);
    }
    if let Some(allowed) = schema["enum"].as_array() {
        if !allowed.contains(value) {
            return Err(format!("{path}: {value} is not one of {allowed:?}"));
        }
    }
    let types: Vec<&str> = match &schema["type"] {
        Value::String(t) => vec![t.as_str()],
        Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    if !types.is_empty() && !types.iter().any(|t| has_type(value, t)) {
        return Err(format!("{path}: {value} is not of type {types:?}"));
    }
    if let Value::Object(fields) = value {
        let properties = &schema["properties"];
        for required in schema["required"].as_array().into_iter().flatten() {
            let key = required.as_str().unwrap_or_default();
            if !fields.contains_key(key) {
                return Err(format!("{path}: missing required field {key}"));
            }
        }
        for (key, field) in fields {
            let field_schema = &properties[key];
            if field_schema.is_null() {
                return Err(format!("{path}: field {key} is not in the spec"));
            }
            check_schema(spec, field_schema, field, &format!("{path}.{key}"))?;
        }
    }
    if let Value::Array(items) = value {
        for (i, item) in items.iter().enumerate() {
            check_schema(spec, &schema["items"], item, &format!("{path}[{i}]"))?;
        }
    }
    Ok(())
}

fn has_type(value: &Value, json_type: &str) -> bool {
    match json_type {
        "null" => value.is_null(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        _ => false,
    }
}

// Đổi chuỗi "-1283400.00" thành số xu để cộng đúng, không qua f64. Mẫu chỉ có tối đa hai chữ số thập phân.
fn cents(value: &str) -> i64 {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, "00"));
    let negative = whole.starts_with('-');
    let units: i64 = whole.trim_start_matches('-').parse().expect("integer part");
    let fraction: i64 = format!("{fraction:0<2}").parse().expect("fraction part");
    assert!(fraction < 100, "more than two decimals in {value}");
    let total = units * 100 + fraction;
    if negative {
        -total
    } else {
        total
    }
}

/// Mẫu phải đúng các ràng buộc giao diện sẽ dựa vào: mỗi nhóm có đóng góp bằng sau trừ trước, các nhóm cộng lại bằng
/// giá trị hai lát cắt, tổng đóng góp của mỗi chiều bằng chênh lệch (#39), lượng cộng giá bằng chênh lệch (#40).
#[test]
fn why_example_adds_up() {
    let example = build_why_example();
    assert_eq!(example.status, AskStatus::Answered);
    let explanation = example.explanation.expect("explanation");
    let change = cents(&explanation.change);
    assert_eq!(
        change,
        cents(&explanation.compare.value) - cents(&explanation.base.value)
    );
    for dimension in &explanation.dimensions {
        let (mut base, mut compare, mut sum) = (0, 0, 0);
        for group in &dimension.groups {
            let contribution = cents(&group.contribution);
            assert_eq!(
                contribution,
                cents(&group.compare_value) - cents(&group.base_value)
            );
            base += cents(&group.base_value);
            compare += cents(&group.compare_value);
            sum += contribution;
        }
        let name = &dimension.column;
        assert_eq!(base, cents(&explanation.base.value), "dimension {name}");
        assert_eq!(
            compare,
            cents(&explanation.compare.value),
            "dimension {name}"
        );
        assert_eq!(sum, change, "dimension {name}");
    }
    let split = explanation.volume_price.expect("volume_price");
    // volume_effect = (compare_volume - base_volume) * (base_price + compare_price) / 2, tính bằng xu.
    let volume_change = (cents(&split.compare_volume) - cents(&split.base_volume)) / 100;
    let price_sum = cents(&split.base_price) + cents(&split.compare_price);
    assert_eq!(volume_change * price_sum / 2, cents(&split.volume_effect));
    assert_eq!(
        cents(&split.volume_effect) + cents(&split.price_effect),
        change
    );
    // base_volume * compare_price; giá có hai chữ số thập phân nên tích tính bằng xu.
    let unchanged = cents(&split.base_volume) / 100 * cents(&split.compare_price);
    assert_eq!(unchanged, cents(&split.if_volume_unchanged));
}
