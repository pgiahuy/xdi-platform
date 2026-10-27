// SPDX-License-Identifier: Apache-2.0
/* service/ask/other_text.rs - chữ của ô dạng other (ValueKind::Other: TIME, INTERVAL, danh sách, struct, map, UUID,
BLOB...), viết như DuckDB in khi đổi giá trị sang VARCHAR, để người dùng thấy giống khi tự chạy câu trong DuckDB.
ask/result.rs gọi build_other_text cho các cột này. ask/run_tests.rs so với chính DuckDB cho từng loại kể trên.

Cột ngoài cùng kiểu TIME_NS, TIMETZ, BIT, BIGNUM không tới đây: engine/query.rs báo lỗi để mô hình đổi sang VARCHAR, vì
duckdb-rs đọc chúng sai hoặc thiếu. Còn khác DuckDB khi các kiểu đó nằm trong danh sách, struct hay map: BIT, BIGNUM ra
byte thô dạng \xAA, TIMETZ mất múi giờ; ngày giờ trong danh sách in theo chrono (năm ngoài 0000 tới 9999 có dấu); số
thực rất lớn in 1e20 thay vì 1e+20. */

use duckdb::types::{TimeUnit, Value};

use crate::service::ask::result::{build_date_text, build_timestamp_text};

pub fn build_other_text(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Boolean(flag) => flag.to_string(),
        Value::TinyInt(n) => n.to_string(),
        Value::SmallInt(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::HugeInt(n) => n.to_string(),
        Value::UTinyInt(n) => n.to_string(),
        Value::USmallInt(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::UBigInt(n) => n.to_string(),
        Value::UHugeInt(n) => n.to_string(),
        // {:?} giữ ".0" của số chẵn (2.0) như DuckDB; {} in "2".
        Value::Float(n) => format!("{n:?}"),
        Value::Double(n) => format!("{n:?}"),
        Value::Decimal(n) => n.to_string(),
        Value::Text(text) | Value::Enum(text) => text.clone(),
        Value::Blob(bytes) => build_blob_text(bytes),
        Value::Date32(days) => build_date_text(*days).unwrap_or_else(|| days.to_string()),
        Value::Timestamp(unit, time) => build_timestamp_text(*unit, *time, false)
            .map(|text| text.replacen('T', " ", 1))
            .unwrap_or_else(|| time.to_string()),
        Value::Time64(unit, time) => build_time_text(to_micros(*unit, *time)),
        Value::Interval {
            months,
            days,
            nanos,
        } => build_interval_text(*months, *days, nanos / 1_000),
        Value::List(items) | Value::Array(items) => {
            let items: Vec<String> = items.iter().map(build_other_text).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Struct(fields) => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("'{name}': {}", build_other_text(value)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        Value::Map(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(key, value)| {
                    format!("{}={}", build_other_text(key), build_other_text(value))
                })
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        Value::Union(inner) => build_other_text(inner),
        // Value có thể thêm biến thể ở bản crate sau; in dạng gỡ lỗi còn hơn bỏ mất ô.
        other => format!("{other:?}"),
    }
}

fn to_micros(unit: TimeUnit, value: i64) -> i64 {
    match unit {
        TimeUnit::Second => value.saturating_mul(1_000_000),
        TimeUnit::Millisecond => value.saturating_mul(1_000),
        TimeUnit::Microsecond => value,
        TimeUnit::Nanosecond => value / 1_000,
    }
}

/// `HH:MM:SS`, thêm phần lẻ giây bỏ số 0 ở cuối như DuckDB (`12:34:56.5`). Giờ có thể quá 24 khi in phần giờ của
/// INTERVAL (`30:00:00`).
fn build_time_text(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let micros = micros.unsigned_abs();
    let seconds = micros / 1_000_000;
    let mut text = format!(
        "{sign}{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    let fraction = micros % 1_000_000;
    if fraction != 0 {
        let digits = format!("{fraction:06}");
        text.push('.');
        text.push_str(digits.trim_end_matches('0'));
    }
    text
}

/// Như DuckDB: `1 year 2 months 3 days 04:05:06`, bỏ phần bằng 0, cả khoảng bằng 0 là `00:00:00`.
fn build_interval_text(months: i32, days: i32, micros: i64) -> String {
    let mut parts = Vec::new();
    for (count, unit) in [(months / 12, "year"), (months % 12, "month"), (days, "day")] {
        if count != 0 {
            let plural = if count.abs() == 1 { "" } else { "s" };
            parts.push(format!("{count} {unit}{plural}"));
        }
    }
    if micros != 0 || parts.is_empty() {
        parts.push(build_time_text(micros));
    }
    parts.join(" ")
}

/// Byte in được giữ nguyên, byte khác viết `\xAA` như DuckDB.
fn build_blob_text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if (0x20..0x7f).contains(&byte) && !matches!(byte, b'\\' | b'\'' | b'"') {
                char::from(byte).to_string()
            } else {
                format!("\\x{byte:02X}")
            }
        })
        .collect()
}
