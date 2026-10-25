// SPDX-License-Identifier: Apache-2.0
/* service/ask_result.rs - đổi kết quả DuckDB (engine/query.rs) thành ResultTable của hợp đồng (ask_contract.rs): tên
cột duy nhất, dạng ô theo kiểu cột, cờ truncated và limit_reached. Luật dạng ô ở ValueKind và docs/ARCHITECTURE.md mục
8, bảng "Giá trị ô"; chữ của dạng other ở ask_other_text.rs. ask_run.rs gọi build_result_table sau mỗi lần chạy
thành công; test ở ask_run_tests.rs.

Dạng ô quyết định theo kiểu cột, không theo từng giá trị: cùng một cột luôn cùng một dạng, kể cả cột toàn null. */

use chrono::DateTime;
use duckdb::core::LogicalTypeId;
use duckdb::types::{TimeUnit, Value};

use super::ask_contract::{
    build_unique_names, CellValue, ColumnRole, ResultColumn, ResultTable, ValueKind,
};
use super::ask_other_text::build_other_text;
use crate::engine::query::QueryOutput;

/// Số dòng tối đa gửi cho giao diện (ResultTable).
pub const SENT_ROWS: usize = 1000;

/// max_rows là trần số dòng guard đã đặt vào câu: kết quả có đúng số dòng đó thì có thể còn dòng bị cắt.
pub fn build_result_table(output: QueryOutput, max_rows: u64) -> ResultTable {
    let QueryOutput {
        columns,
        rows,
        total_rows,
    } = output;
    let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
    let kinds: Vec<ValueKind> = columns
        .iter()
        .map(|c| build_value_kind(c.type_id))
        .collect();
    let rows: Vec<Vec<Option<CellValue>>> = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .enumerate()
                .map(|(i, value)| build_cell(columns[i].type_id, kinds[i], value))
                .collect()
        })
        .collect();
    let columns = build_unique_names(&names)
        .into_iter()
        .zip(kinds)
        .map(|(name, kind)| ResultColumn {
            name,
            kind,
            // Vai trò cột lấy từ cây cú pháp của câu (#35); tới khi đó mọi cột là other.
            role: ColumnRole::Other,
        })
        .collect();
    ResultTable {
        truncated: (rows.len() as u64) < total_rows,
        limit_reached: total_rows == max_rows,
        columns,
        rows,
        total_rows,
    }
}

/// Số nguyên tới 32 bit và số thực là number vì số JavaScript giữ đúng chúng; số nguyên 64 bit trở lên và DECIMAL thì
/// JavaScript làm tròn, nên là decimal (chuỗi).
pub fn build_value_kind(type_id: LogicalTypeId) -> ValueKind {
    use LogicalTypeId as T;
    match type_id {
        T::Varchar | T::Enum => ValueKind::Text,
        T::Tinyint
        | T::Smallint
        | T::Integer
        | T::UTinyint
        | T::USmallint
        | T::UInteger
        | T::Float
        | T::Double => ValueKind::Number,
        T::Bigint | T::Hugeint | T::UBigint | T::UHugeint | T::Decimal => ValueKind::Decimal,
        T::Boolean => ValueKind::Boolean,
        T::Date => ValueKind::Date,
        T::Timestamp | T::TimestampS | T::TimestampMs | T::TimestampNs | T::TimestampTZ => {
            ValueKind::Timestamp
        }
        _ => ValueKind::Other,
    }
}

/// Giá trị không đúng dạng của kind (không xảy ra khi kind lấy từ chính kiểu cột) thì rơi xuống dạng other.
fn build_cell(type_id: LogicalTypeId, kind: ValueKind, value: Value) -> Option<CellValue> {
    match (kind, value) {
        (_, Value::Null) => None,
        (ValueKind::Text, Value::Text(text) | Value::Enum(text)) => Some(CellValue::String(text)),
        (ValueKind::Number, value) => build_number(&value).map(CellValue::Number),
        (ValueKind::Decimal, value) => build_decimal_text(&value).map(CellValue::String),
        (ValueKind::Boolean, Value::Boolean(flag)) => Some(CellValue::Boolean(flag)),
        // Ngày, giờ ngoài khoảng chrono biểu diễn được (DuckDB có 'infinity') gửi null, như NaN của số.
        (ValueKind::Date, Value::Date32(days)) => build_date_text(days).map(CellValue::String),
        (ValueKind::Timestamp, Value::Timestamp(unit, time)) => {
            let utc = type_id == LogicalTypeId::TimestampTZ;
            build_timestamp_text(unit, time, utc).map(CellValue::String)
        }
        (_, value) => Some(CellValue::String(build_other_text(&value))),
    }
}

/// NaN và vô cực không có trong JSON, gửi null.
fn build_number(value: &Value) -> Option<f64> {
    let number = match value {
        Value::TinyInt(n) => f64::from(*n),
        Value::SmallInt(n) => f64::from(*n),
        Value::Int(n) => f64::from(*n),
        Value::UTinyInt(n) => f64::from(*n),
        Value::USmallInt(n) => f64::from(*n),
        Value::UInt(n) => f64::from(*n),
        // f64::from(0.1f32) là 0.10000000149011612; đi qua chữ ngắn nhất của f32 ("0.1") để gửi đúng số người ta thấy.
        Value::Float(n) => n.to_string().parse().unwrap_or(f64::from(*n)),
        Value::Double(n) => *n,
        _ => return None,
    };
    number.is_finite().then_some(number)
}

/// Chữ số thập phân đúng như DuckDB giữ: DECIMAL giữ số chữ số sau dấu phẩy của kiểu ("1234.50").
fn build_decimal_text(value: &Value) -> Option<String> {
    match value {
        Value::BigInt(n) => Some(n.to_string()),
        Value::HugeInt(n) => Some(n.to_string()),
        Value::UBigInt(n) => Some(n.to_string()),
        Value::UHugeInt(n) => Some(n.to_string()),
        Value::Decimal(n) => Some(n.to_string()),
        _ => None,
    }
}

/// DuckDB giữ DATE là số ngày kể từ 1970-01-01. Năm ngoài 0000 tới 9999 theo dạng mở rộng của ISO 8601 (chrono in
/// vậy), như chú thích của ValueKind: "+10000-01-01", "0000-12-31" là năm 1 trước Công nguyên. DuckDB tự in thì khác
/// ("10000-01-01", "0001-12-31 (BC)").
pub(super) fn build_date_text(days: i32) -> Option<String> {
    let midnight = DateTime::from_timestamp(i64::from(days) * 86_400, 0)?;
    Some(midnight.format("%Y-%m-%d").to_string())
}

/// `YYYY-MM-DDTHH:MM:SS[.ffffff]`, thêm `Z` khi utc. TIMESTAMPTZ DuckDB giữ sẵn theo UTC nên chỉ cần thêm `Z`. Phần lẻ
/// giây chỉ có khi khác 0, đúng 6 chữ số; TIMESTAMP_NS bỏ phần dưới micro giây cho khớp dạng đó.
pub(super) fn build_timestamp_text(unit: TimeUnit, value: i64, utc: bool) -> Option<String> {
    let micros = match unit {
        TimeUnit::Second => value.checked_mul(1_000_000)?,
        TimeUnit::Millisecond => value.checked_mul(1_000)?,
        TimeUnit::Microsecond => value,
        TimeUnit::Nanosecond => value.div_euclid(1_000),
    };
    let time = DateTime::from_timestamp_micros(micros)?;
    let mut text = time.format("%Y-%m-%dT%H:%M:%S").to_string();
    let fraction = micros.rem_euclid(1_000_000);
    if fraction != 0 {
        text.push_str(&format!(".{fraction:06}"));
    }
    if utc {
        text.push('Z');
    }
    Some(text)
}
