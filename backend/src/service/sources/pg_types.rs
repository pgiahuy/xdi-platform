// SPDX-License-Identifier: Apache-2.0
/* service/sources/pg_types.rs - kiểu DuckDB cho từng cột của bản chụp bảng Postgres. postgres.rs gọi column_types
ngay trước lệnh COPY, với đúng câu SELECT được chép; danh sách đi theo file CSV qua service/ingest.rs tới
engine/analytics.rs, nơi read_csv nhận nó qua tham số types.

Vì sao không để DuckDB đoán kiểu như với file nạp tay: COPY ghi mọi giá trị thành chữ, DuckDB chỉ xem một phần đầu file
để chọn kiểu. Cột chữ chứa mã '00123' sẽ thành số 123 (lọc theo mã gốc không khớp nữa), cột mã toàn số ở vài chục
nghìn dòng đầu rồi mới có chữ sẽ làm cả lần chụp lỗi. Postgres đã biết kiểu từng cột, nên lấy luôn từ đó.

Định dạng chữ của COPY: sqlx mở kết nối với DateStyle ISO và TimeZone UTC, nên ngày là 2024-03-17, timestamptz là
2024-03-17 03:00:00+00, đúng dạng DuckDB đọc được. */

use sqlx::postgres::{PgColumn, PgConnection};
use sqlx::{Column, Executor, Statement, TypeInfo};

use super::postgres::query_error;
use crate::error::AppError;

/// Tên và kiểu DuckDB của từng cột mà câu SELECT trả về, theo đúng thứ tự cột.
pub async fn column_types(
    c: &mut PgConnection,
    select: &str,
) -> Result<Vec<(String, String)>, AppError> {
    // prepare chỉ nhờ Postgres phân tích câu SELECT và trả tên, kiểu từng cột; câu SELECT chưa chạy.
    let statement = c.prepare(select).await.map_err(|e| query_error(&e))?;
    let mut types = Vec::new();
    for column in statement.columns() {
        let pg_type = column.type_info().name();
        let numeric = if pg_type == "NUMERIC" {
            declared_numeric(c, column).await?
        } else {
            None
        };
        types.push((column.name().to_string(), duckdb_type(pg_type, numeric)));
    }
    Ok(types)
}

/// Độ chính xác và số chữ số sau dấu phẩy đã khai báo của một cột numeric(p,s). None khi cột khai báo numeric trơn, hoặc
/// là kết quả của một biểu thức chứ không phải cột của bảng nào.
async fn declared_numeric(
    c: &mut PgConnection,
    column: &PgColumn,
) -> Result<Option<(i32, i32)>, AppError> {
    let (Some(table), Some(position)) = (column.relation_id(), column.relation_attribute_no())
    else {
        return Ok(None);
    };
    // Postgres lưu numeric(p,s) trong atttypmod dưới dạng ((p << 16) | s) + 4, và -1 khi không khai báo.
    sqlx::query_as(
        "SELECT (atttypmod - 4) >> 16, (atttypmod - 4) & 65535
         FROM pg_catalog.pg_attribute
         WHERE attrelid = $1 AND attnum = $2 AND atttypmod >= 4",
    )
    .bind(table)
    .bind(position)
    .fetch_optional(&mut *c)
    .await
    .map_err(|e| query_error(&e))
}

/// Kiểu DuckDB để đọc lại một cột Postgres từ file CSV của COPY. pg_type là tên kiểu như sqlx ghi (INT4, NUMERIC,
/// TIMESTAMPTZ). Kiểu nào không có ở đây (text, varchar, char, uuid, json, mảng, enum, kiểu tự định nghĩa) đọc thành
/// VARCHAR: giữ nguyên chữ COPY ghi, không mất gì.
fn duckdb_type(pg_type: &str, numeric: Option<(i32, i32)>) -> String {
    let duckdb = match pg_type {
        "INT2" | "INT4" => "INTEGER",
        "INT8" => "BIGINT",
        "FLOAT4" | "FLOAT8" => "DOUBLE",
        // DuckDB chỉ nhận DECIMAL tới 38 chữ số và số chữ số sau dấu phẩy từ 0 tới p; Postgres cho rộng hơn. numeric
        // không khai báo (p,s) hay vượt mức đó thì đọc thành DOUBLE chứ không phải VARCHAR: cột tiền, số lượng vẫn cộng
        // được và vẫn là cột số đo trong hồ sơ, đổi lại chỉ giữ chính xác khoảng 15 chữ số.
        "NUMERIC" => {
            return match numeric {
                Some((precision, scale)) if precision <= 38 && (0..=precision).contains(&scale) => {
                    format!("DECIMAL({precision},{scale})")
                }
                _ => "DOUBLE".to_string(),
            }
        }
        "BOOL" => "BOOLEAN",
        "DATE" => "DATE",
        "TIMESTAMP" => "TIMESTAMP",
        "TIMESTAMPTZ" => "TIMESTAMPTZ",
        _ => "VARCHAR",
    };
    duckdb.to_string()
}

#[cfg(test)]
mod tests {
    use super::duckdb_type;

    #[test]
    fn known_types_map_and_the_rest_stay_text() {
        assert_eq!(duckdb_type("INT4", None), "INTEGER");
        assert_eq!(duckdb_type("INT8", None), "BIGINT");
        assert_eq!(duckdb_type("FLOAT8", None), "DOUBLE");
        assert_eq!(duckdb_type("BOOL", None), "BOOLEAN");
        assert_eq!(duckdb_type("TIMESTAMPTZ", None), "TIMESTAMPTZ");
        for text in [
            "TEXT",
            "VARCHAR",
            "BPCHAR",
            "UUID",
            "JSONB",
            "INT4[]",
            "order_status",
        ] {
            assert_eq!(duckdb_type(text, None), "VARCHAR", "{text}");
        }
    }

    #[test]
    fn numeric_keeps_declared_precision_when_duckdb_can() {
        assert_eq!(duckdb_type("NUMERIC", Some((10, 2))), "DECIMAL(10,2)");
        assert_eq!(duckdb_type("NUMERIC", None), "DOUBLE");
        assert_eq!(duckdb_type("NUMERIC", Some((60, 4))), "DOUBLE");
        // numeric(5,-2) của Postgres 15 trở lên: scale âm đọc ra thành số lớn qua phép & 65535.
        assert_eq!(duckdb_type("NUMERIC", Some((5, 65534))), "DOUBLE");
    }
}
