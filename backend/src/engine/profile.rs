// SPDX-License-Identifier: Apache-2.0
/* engine/profile.rs - đo từng cột của một phiên bản dataset bằng DuckDB: số ô trống, số giá trị khác nhau, min, max,
tần suất từng giá trị của cột ít giá trị, và định dạng ngày của cột ngày lưu dạng chữ. Đây chỉ là phần đo; luật chọn
vai trò cột từ các con số này ở service/profile.rs, ghi vào Postgres ở engine/store/columns.rs.

Như analytics.rs, hàm ở đây đồng bộ; service gọi trong spawn_blocking. Mỗi cột một câu SQL, đọc lại thư mục Parquet
của phiên bản qua một view. Đếm phân biệt dùng COUNT(DISTINCT), không dùng approx_unique: số này quyết định cột có
thành chiều và có ghi column_values hay không, số ước lượng lệch vài phần trăm quanh ngưỡng thì hai lần nạp cùng dữ liệu
có thể ra hai vai trò khác nhau. */

use std::collections::HashMap;
use std::path::Path;

use duckdb::Connection;

use super::analytics::{self, sql_string, Column};
use crate::error::AppError;

/// Định dạng strptime thử cho cột VARCHAR (xem find_time_formats). Lúc đọc CSV (cả sheet Excel, vốn được đổi sang CSV)
/// DuckDB đã tự nhận một số dạng ngày thành DATE hay TIMESTAMP, nhưng không phải mọi dạng, nên cột ngày vẫn có thể tới
/// đây dưới dạng chữ; DataCo ghi 1/31/2018 22:56. Ngày/tháng đứng trước tháng/ngày: giá trị như 3/4/2018 đọc được cả hai
/// cách thì theo cách viết ở Việt Nam, trừ khi phiên bản trước đã chọn cách kia. Dạng có giờ đứng trước dạng không giờ
/// chỉ để dễ đọc: strptime từ chối phần chữ thừa, nên "1/31/2018 22:56" không khớp '%m/%d/%Y'.
const TIME_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d",
    "%d/%m/%Y %H:%M:%S",
    "%d/%m/%Y %H:%M",
    "%d/%m/%Y",
    "%m/%d/%Y %H:%M:%S",
    "%m/%d/%Y %H:%M",
    "%m/%d/%Y",
];

/// strptime nhận năm 1 tới 4 chữ số, nên '1/2/3' đọc ra năm 0003 và cột điểm số hay số nhà dạng 12/3/45 thành cột
/// ngày. Chỉ tính là đọc được khi năm nằm trong khoảng này.
const YEAR_MIN: i32 = 1900;
const YEAR_MAX: i32 = 2100;

/// Tên view trỏ tới bảng gộp của phiên bản, chỉ sống trong kết nối DuckDB của một lần đo.
const VIEW: &str = "profiled";

/// Một giá trị của cột và số dòng mang giá trị đó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCount {
    pub value: String,
    pub frequency: i64,
}

/// Để luật đánh dấu nhạy cảm (service/profile.rs) xét danh sách giá trị mà không phải tách chuỗi ra.
impl AsRef<str> for ValueCount {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

/// Số đo của một cột. Giá trị (min, max, value) ghi dạng chữ như DuckDB CAST sang VARCHAR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnStats {
    pub name: String,
    /// Kiểu theo DuckDB, như Column::data_type.
    pub data_type: String,
    pub null_count: i64,
    pub distinct_count: i64,
    /// Với cột có time_format: thời điểm sớm nhất, muộn nhất sau khi đọc theo định dạng đó, dạng ISO
    /// (2015-01-01 00:00:00). So chữ thì "10/1/2017" đứng trước "9/30/2017", nên min, max của chữ vô nghĩa ở cột này.
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    /// Định dạng strptime đọc được mọi giá trị của cột VARCHAR (xem find_time_formats, choose_time_format); None với
    /// cột khác.
    pub time_format: Option<String>,
    /// Tần suất từng giá trị, nhiều nhất trước, khi cột có từ 1 tới values_max giá trị khác nhau; rỗng khi nhiều hơn.
    pub values: Vec<ValueCount>,
}

/// Đo mọi cột của bảng gộp trong parquet_dir, theo thứ tự cột của bảng (cùng thứ tự với Materialized::columns).
/// values_max: cột có không quá ngần này giá trị khác nhau thì lấy tần suất từng giá trị.
/// previous_formats: tên cột và time_format của phiên bản trước cùng dataset, để cột ngày đọc được nhiều cách giữ cách
/// đã chọn lần trước; rỗng khi không có phiên bản trước.
pub fn build_column_stats(
    parquet_dir: &Path,
    values_max: i64,
    previous_formats: &HashMap<String, String>,
) -> Result<Vec<ColumnStats>, AppError> {
    let conn = analytics::open()?;
    let table = analytics::parquet_glob(parquet_dir)?;
    conn.execute_batch(&format!("CREATE VIEW {VIEW} AS SELECT * FROM {table}"))
        .map_err(|e| AppError::Internal(format!("create profile view: {e}")))?;
    let columns = analytics::describe(&conn, VIEW)
        .map_err(|e| AppError::Internal(format!("describe parquet for profile: {e}")))?;
    columns
        .iter()
        .map(|column| build_one(&conn, column, values_max, previous_formats))
        .collect()
}

fn build_one(
    conn: &Connection,
    column: &Column,
    values_max: i64,
    previous_formats: &HashMap<String, String>,
) -> Result<ColumnStats, AppError> {
    let c = sql_ident(&column.name);
    let fail =
        |e: duckdb::Error| AppError::Internal(format!("profile column {}: {e}", column.name));
    let sql = format!(
        "SELECT count(*), count({c}), count(DISTINCT {c}), CAST(min({c}) AS VARCHAR), CAST(max({c}) AS VARCHAR) \
         FROM {VIEW}"
    );
    let (total, non_null, distinct_count, min_value, max_value): (i64, i64, i64, _, _) = conn
        .query_row(&sql, [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(fail)?;
    let mut stats = ColumnStats {
        name: column.name.clone(),
        data_type: column.data_type.clone(),
        null_count: total - non_null,
        distinct_count,
        min_value,
        max_value,
        time_format: None,
        values: Vec::new(),
    };

    if column.data_type == "VARCHAR" && non_null > 0 {
        let fits = find_time_formats(conn, &c, non_null).map_err(fail)?;
        let previous = previous_formats.get(&column.name);
        if let Some(format) = choose_time_format(&fits, previous) {
            if fits.len() > 1 {
                tracing::info!(column = %column.name, chosen = format, fits = ?fits, previous = ?previous,
                    "text dates fit more than one format");
            }
            let parsed = format!("strptime({c}, {})", sql_string(format));
            let sql = format!(
                "SELECT CAST(min({parsed}) AS VARCHAR), CAST(max({parsed}) AS VARCHAR) FROM {VIEW}"
            );
            let (min, max) = conn
                .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(fail)?;
            stats.min_value = min;
            stats.max_value = max;
            stats.time_format = Some(format.to_string());
        }
    }

    if distinct_count > 0 && distinct_count <= values_max {
        stats.values = value_counts(conn, &c).map_err(fail)?;
    }
    Ok(stats)
}

/// Các định dạng trong TIME_FORMATS, theo thứ tự đó, đọc được đúng non_null giá trị (mọi ô không trống) với năm trong
/// khoảng YEAR_MIN tới YEAR_MAX. Một câu SQL đếm cho mọi định dạng cùng lúc, nên chỉ đọc cột một lần.
fn find_time_formats(
    conn: &Connection,
    c: &str,
    non_null: i64,
) -> Result<Vec<&'static str>, duckdb::Error> {
    let counts: Vec<String> = TIME_FORMATS
        .iter()
        .map(|f| {
            format!(
                "count(CASE WHEN year(try_strptime({c}, {})) BETWEEN {YEAR_MIN} AND {YEAR_MAX} THEN 1 END)",
                sql_string(f)
            )
        })
        .collect();
    let sql = format!("SELECT {} FROM {VIEW}", counts.join(", "));
    let parsed: Vec<i64> = conn.query_row(&sql, [], |r| {
        (0..TIME_FORMATS.len()).map(|i| r.get(i)).collect()
    })?;
    Ok(TIME_FORMATS
        .iter()
        .zip(parsed)
        .filter(|(_, n)| *n == non_null)
        .map(|(f, _)| *f)
        .collect())
}

/// Định dạng của phiên bản trước nếu nó vẫn đọc được mọi giá trị, không thì định dạng đầu tiên đọc được. Phiên bản
/// mới mà mọi ngày đều không quá 12 sẽ đọc được cả hai cách; giữ cách cũ để cùng cột không đổi nghĩa giữa hai lần nạp.
fn choose_time_format(fits: &[&'static str], previous: Option<&String>) -> Option<&'static str> {
    previous
        .and_then(|p| fits.iter().find(|f| **f == p.as_str()))
        .or(fits.first())
        .copied()
}

/// Tần suất từng giá trị khác null. Nhóm theo giá trị đã đổi sang chữ (không theo giá trị gốc), để hai giá trị gốc
/// khác nhau mà ra cùng một chữ không thành hai dòng trùng nhau, điều ràng buộc UNIQUE (column_id, value) của
/// column_values sẽ từ chối. GROUP BY 1 thay vì tên bí danh: bảng có cột trùng tên bí danh thì DuckDB nhóm theo cột đó.
fn value_counts(conn: &Connection, c: &str) -> Result<Vec<ValueCount>, duckdb::Error> {
    let sql = format!(
        "SELECT CAST({c} AS VARCHAR), count(*) FROM {VIEW} WHERE {c} IS NOT NULL GROUP BY 1 ORDER BY 2 DESC, 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(ValueCount {
            value: r.get(0)?,
            frequency: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// Tên cột trong SQL: bọc nháy kép, nhân đôi nháy kép bên trong. Tên cột lấy từ file người dùng nên có thể chứa dấu
/// cách, nháy đơn, nháy kép.
fn sql_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::engine::analytics::{materialize, FileFormat, InputFile};

    /// Nạp một file CSV vào thư mục tạm rồi đo; thư mục xóa trước khi trả kết quả.
    fn stats_of(csv: &str, values_max: i64) -> Vec<ColumnStats> {
        stats_after(csv, values_max, &HashMap::new())
    }

    /// Như stats_of, khi phiên bản trước đã chọn định dạng cho một số cột ngày.
    fn stats_after(
        csv: &str,
        values_max: i64,
        previous: &HashMap<String, String>,
    ) -> Vec<ColumnStats> {
        let dir = std::env::temp_dir().join(format!("xdi-profile-{}", uuid::Uuid::new_v4()));
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let path: PathBuf = dir.join("data.csv");
        std::fs::write(&path, csv).unwrap();
        let file = InputFile {
            path,
            name: "data.csv".to_string(),
            format: FileFormat::Csv,
            timestamp_columns: Vec::new(),
        };
        let result =
            materialize(&[file], &out).and_then(|_| build_column_stats(&out, values_max, previous));
        std::fs::remove_dir_all(&dir).unwrap();
        result.unwrap()
    }

    fn column<'a>(stats: &'a [ColumnStats], name: &str) -> &'a ColumnStats {
        stats.iter().find(|s| s.name == name).unwrap()
    }

    #[test]
    fn counts_nulls_distinct_min_max_and_values() {
        let stats = stats_of(
            "region,amount,note\nSouth,10,\nNorth,20,\nSouth,30,\nEast,,\n",
            200,
        );
        let names: Vec<&str> = stats.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["region", "amount", "note", "_source_file"]);

        let region = column(&stats, "region");
        assert_eq!((region.null_count, region.distinct_count), (0, 3));
        assert_eq!(region.min_value.as_deref(), Some("East"));
        assert_eq!(region.max_value.as_deref(), Some("South"));
        let values: Vec<(&str, i64)> = region
            .values
            .iter()
            .map(|v| (v.value.as_str(), v.frequency))
            .collect();
        assert_eq!(values, [("South", 2), ("East", 1), ("North", 1)]);

        let amount = column(&stats, "amount");
        assert_eq!((amount.null_count, amount.distinct_count), (1, 3));
        assert_eq!(amount.min_value.as_deref(), Some("10"));

        // Cột toàn ô trống: không giá trị nào, không có tần suất.
        let note = column(&stats, "note");
        assert_eq!((note.null_count, note.distinct_count), (4, 0));
        assert!(note.values.is_empty() && note.min_value.is_none());

        let source = column(&stats, "_source_file");
        assert_eq!(source.values.len(), 1);
        assert_eq!(source.values[0].frequency, 4);
    }

    #[test]
    fn many_distinct_values_have_no_value_list() {
        let mut csv = String::from("code\n");
        for i in 0..500 {
            csv.push_str(&format!("C{i:03}\n"));
        }
        let stats = stats_of(&csv, 200);
        let code = column(&stats, "code");
        assert_eq!(code.distinct_count, 500);
        assert!(code.values.is_empty());
    }

    #[test]
    fn text_dates_get_a_format_and_parsed_min_max() {
        // Dạng của DataCo: tháng/ngày/năm giờ:phút, không đệm số 0. So chữ thì "10/1/2017" nhỏ hơn "9/30/2017".
        let stats = stats_of(
            "order date,ship,both,day\n\
             1/31/2018 22:56,3/4/2018 7:05,3/4/2018 7:05,25/12/2017\n\
             10/1/2017 0:05,25/12/2017 22:56,5/6/2018 8:00,03/04/2018\n\
             9/30/2017 7:00,,1/2/2018 0:00,\n",
            200,
        );
        let order = column(&stats, "order date");
        assert_eq!(order.data_type, "VARCHAR");
        assert_eq!(order.time_format.as_deref(), Some("%m/%d/%Y %H:%M"));
        assert_eq!(order.min_value.as_deref(), Some("2017-09-30 07:00:00"));
        assert_eq!(order.max_value.as_deref(), Some("2018-01-31 22:56:00"));

        // 25/12/2017 chỉ khớp ngày/tháng; ô trống không tính.
        let ship = column(&stats, "ship");
        assert_eq!(ship.time_format.as_deref(), Some("%d/%m/%Y %H:%M"));
        // Mọi giá trị đọc được cả hai cách: theo ngày/tháng.
        let both = column(&stats, "both");
        assert_eq!(both.time_format.as_deref(), Some("%d/%m/%Y %H:%M"));
        assert_eq!(both.min_value.as_deref(), Some("2018-02-01 00:00:00"));

        // Ngày có đệm số 0 thì DuckDB tự đọc thành DATE lúc nạp CSV, không cần định dạng.
        let day = column(&stats, "day");
        assert_eq!(day.data_type, "DATE");
        assert_eq!(day.time_format, None);
        assert_eq!(day.min_value.as_deref(), Some("2017-12-25"));
    }

    /// Parquet người dùng nạp có thể có kiểu lồng nhau; đo không được làm hỏng cả lần nạp.
    #[test]
    fn nested_parquet_types_can_be_measured() {
        let dir = std::env::temp_dir().join(format!("xdi-profile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("part-0001.parquet");
        let conn = analytics::open().unwrap();
        conn.execute_batch(&format!(
            "COPY (SELECT [i, i + 1] AS list, {{'a': i}} AS struct, MAP {{'k': i}} AS map, \
             CAST('x' AS BLOB) AS blob, INTERVAL 1 DAY * i AS span FROM range(3) t(i)) TO {} (FORMAT parquet)",
            sql_string(file.to_str().unwrap())
        ))
        .unwrap();
        let result = build_column_stats(&dir, 200, &HashMap::new());
        std::fs::remove_dir_all(&dir).unwrap();
        let stats = result.unwrap();
        assert_eq!(stats.len(), 5);
        assert!(stats.iter().all(|s| s.null_count == 0), "{stats:?}");
    }

    #[test]
    fn text_that_is_not_all_dates_has_no_format() {
        let stats = stats_of("when\n1/31/2018\nsoon\n", 200);
        assert_eq!(column(&stats, "when").time_format, None);
    }

    /// Năm một tới hai chữ số (điểm số, số nhà) không phải ngày, kể cả khi strptime đọc được.
    #[test]
    fn short_years_are_not_dates() {
        let stats = stats_of(
            "score,house,ok\n1/2/3,12/3/45,1/2/2003 7:00\n4/5/6,7/8/99,4/5/2006 8:00\n",
            200,
        );
        assert_eq!(column(&stats, "score").time_format, None);
        assert_eq!(column(&stats, "house").time_format, None);
        assert_eq!(
            column(&stats, "ok").time_format.as_deref(),
            Some("%d/%m/%Y %H:%M")
        );
    }

    /// Ngày đọc được cả hai cách thì giữ cách của phiên bản trước; cách đó không đọc được nữa thì chọn lại.
    #[test]
    fn ambiguous_dates_keep_the_previous_format() {
        let csv = "ambiguous,clear\n3/4/2018 7:05,1/31/2018 22:56\n5/6/2018 8:00,2/3/2018 1:00\n";
        let previous: HashMap<String, String> = [
            ("ambiguous".to_string(), "%m/%d/%Y %H:%M".to_string()),
            ("clear".to_string(), "%d/%m/%Y %H:%M".to_string()),
        ]
        .into();
        let stats = stats_after(csv, 200, &previous);
        let ambiguous = column(&stats, "ambiguous");
        assert_eq!(ambiguous.time_format.as_deref(), Some("%m/%d/%Y %H:%M"));
        assert_eq!(ambiguous.min_value.as_deref(), Some("2018-03-04 07:05:00"));
        assert_eq!(
            column(&stats, "clear").time_format.as_deref(),
            Some("%m/%d/%Y %H:%M")
        );
    }

    #[test]
    fn quotes_and_spaces_in_column_names_do_not_break_sql() {
        let stats = stats_of(
            "\"Order Item Total\",\"say \"\"hi\"\"\",it's\n1.5,a,x\n2.5,b,y\n",
            200,
        );
        let names: Vec<&str> = stats.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["Order Item Total", "say \"hi\"", "it's", "_source_file"]
        );
        assert_eq!(column(&stats, "say \"hi\"").distinct_count, 2);
        assert_eq!(column(&stats, "it's").values.len(), 2);
        assert_eq!(
            column(&stats, "Order Item Total").max_value.as_deref(),
            Some("2.5")
        );
    }
}
