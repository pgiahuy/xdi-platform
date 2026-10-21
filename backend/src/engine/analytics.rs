// SPDX-License-Identifier: Apache-2.0
/* engine/analytics.rs - DuckDB nhúng trong tiến trình backend: đọc CSV, Parquet; đổi file người dùng nạp lên thành
Parquet. Chạy SQL đã qua guard thêm ở issue chạy truy vấn. Không có container riêng, không giữ dữ liệu bền: mỗi lần
dùng mở một kết nối DuckDB trong bộ nhớ, đọc và ghi file trên đĩa.

DuckDB là thư viện đồng bộ và tốn CPU, nên các hàm ở đây là hàm thường (không async); service gọi chúng trong
tokio::task::spawn_blocking để không chặn các request khác.

Nhiều file cùng cột (ví dụ mỗi tháng một file) gộp thành một bảng: mỗi file ghi thành một file Parquet trong cùng thư
mục, kèm cột SOURCE_COLUMN ghi tên file gốc; đọc lại mọi file .parquet trong thư mục như một bảng (parquet_glob). */

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use duckdb::Connection;

use crate::error::AppError;

/// Cột thêm vào mỗi dòng, ghi tên file gốc của dòng đó. Truy dòng nguồn dùng cột này để chỉ ra dòng đến từ file nào.
pub const SOURCE_COLUMN: &str = "_source_file";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Csv,
    Parquet,
}

impl FileFormat {
    /// Đoán theo đuôi tên file người dùng đặt. None nếu không phải định dạng nhận được.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.ends_with(".csv") {
            Some(FileFormat::Csv)
        } else if lower.ends_with(".parquet") {
            Some(FileFormat::Parquet)
        } else {
            None
        }
    }
}

/// Một file đã nằm trên đĩa máy chủ, chờ đổi sang Parquet.
pub struct InputFile {
    /// Đường dẫn trên máy chủ (do backend đặt, không phải tên người dùng gửi).
    pub path: PathBuf,
    /// Tên file người dùng gửi, ghi vào SOURCE_COLUMN và vào thông báo lỗi.
    pub name: String,
    pub format: FileFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// Kiểu theo DuckDB, ví dụ BIGINT, DOUBLE, VARCHAR, DATE.
    pub data_type: String,
}

/// Kết quả đổi một bộ file sang Parquet.
#[derive(Debug)]
pub struct Materialized {
    /// Cột của bảng gộp, gồm cả SOURCE_COLUMN ở cuối.
    pub columns: Vec<Column>,
    pub row_count: i64,
}

/// Đổi các file sang Parquet trong out_dir (thư mục phải có sẵn và trống), kiểm các file có cùng tên cột.
/// Lỗi do dữ liệu người dùng (file hỏng, lệch cột, kiểu không gộp được) trả BadRequest kèm tên file; lỗi khác là Internal.
/// Lỗi giữa chừng thì out_dir có thể còn file Parquet dở; người gọi xóa cả thư mục.
pub fn materialize(files: &[InputFile], out_dir: &Path) -> Result<Materialized, AppError> {
    if files.is_empty() {
        return Err(AppError::BadRequest("Chưa có file nào".to_string()));
    }
    let conn = open()?;

    // Đọc cột của từng file trước khi ghi gì: lệch cột thì từ chối cả bộ, không để lại nửa chừng.
    let mut readers = Vec::with_capacity(files.len());
    let mut first: Option<(String, BTreeSet<String>)> = None;
    for file in files {
        // DuckDB đọc file rỗng thành một cột "column0" không có dòng nào, không báo lỗi; chặn trước cho rõ.
        let size = std::fs::metadata(&file.path)
            .map_err(|e| AppError::Internal(format!("stat {}: {e}", file.path.display())))?
            .len();
        if size == 0 {
            return Err(AppError::BadRequest(format!("File {} rỗng", file.name)));
        }
        let (reader, columns) = open_reader(&conn, file)?;
        let names: BTreeSet<String> = columns.iter().map(|c| c.name.clone()).collect();
        if names.iter().any(|n| n.eq_ignore_ascii_case(SOURCE_COLUMN)) {
            return Err(AppError::BadRequest(format!(
                "File {} có cột tên {SOURCE_COLUMN}, trùng tên cột hệ thống dùng để ghi file nguồn; đổi tên cột rồi nạp lại",
                file.name
            )));
        }
        match &first {
            None => first = Some((file.name.clone(), names)),
            Some((first_name, first_names)) => {
                check_same_columns(first_name, first_names, &file.name, &names)?
            }
        }
        readers.push(reader);
    }

    for (i, (file, reader)) in files.iter().zip(&readers).enumerate() {
        let target = out_dir.join(format!("part-{:04}.parquet", i + 1));
        let target_text = path_text(&target)?;
        match copy_to_parquet(&conn, file, reader, &target_text) {
            Ok(()) => {}
            // Bước đọc cột chỉ xem phần đầu file. Byte không phải UTF-8 nằm sâu trong file CSV thì tới đây mới lộ ra;
            // đọc lại cả file theo latin-1.
            Err(e) if file.format == FileFormat::Csv && is_unicode_error(&e) => {
                let _ = std::fs::remove_file(&target);
                let latin1 = csv_reader(&file.path, true)?;
                copy_to_parquet(&conn, file, &latin1, &target_text)
                    .map_err(|e| user_file_error(&file.name, "đổi sang Parquet", e))?;
            }
            Err(e) => return Err(user_file_error(&file.name, "đổi sang Parquet", e)),
        }
    }

    // Đọc lại cả thư mục như bảng gộp: DuckDB gộp theo tên cột và nâng kiểu (BIGINT với DOUBLE thành DOUBLE). Kiểu
    // không gộp được (số với ngày) thì báo ở đây.
    let all = parquet_glob(out_dir)?;
    let columns = describe(&conn, &all).map_err(|e| {
        tracing::info!(error = %e, "union of parquet parts failed");
        AppError::BadRequest(
            "Các file có cùng tên cột nhưng kiểu dữ liệu không gộp được (ví dụ một file là số, file khác là ngày)"
                .to_string(),
        )
    })?;
    let row_count: i64 = conn
        .query_row(&format!("SELECT count(*) FROM {all}"), [], |row| row.get(0))
        .map_err(|e| AppError::Internal(format!("count rows: {e}")))?;
    if row_count == 0 {
        return Err(AppError::BadRequest(
            "Các file không có dòng dữ liệu nào (chỉ có dòng tiêu đề?)".to_string(),
        ));
    }
    Ok(Materialized { columns, row_count })
}

/// Biểu thức FROM đọc mọi phần Parquet của một phiên bản dataset như một bảng.
pub fn parquet_glob(dir: &Path) -> Result<String, AppError> {
    let pattern = path_text(&dir.join("*.parquet"))?;
    Ok(format!(
        "read_parquet({}, union_by_name = true)",
        sql_string(&pattern)
    ))
}

/// Kết nối DuckDB trong bộ nhớ. Tắt tự cài và tự nạp extension: Parquet đã biên dịch sẵn vào binary, nên không có lý do
/// gì để DuckDB tải code từ Internet lúc chạy (máy chủ không có mạng cũng chạy được).
fn open() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()
        .map_err(|e| AppError::Internal(format!("open duckdb: {e}")))?;
    conn.execute_batch(
        "SET autoinstall_known_extensions = false; SET autoload_known_extensions = false;",
    )
    .map_err(|e| AppError::Internal(format!("configure duckdb: {e}")))?;
    Ok(conn)
}

/// Ghi một file người dùng thành một phần Parquet, thêm cột SOURCE_COLUMN mang tên file gốc.
fn copy_to_parquet(
    conn: &Connection,
    file: &InputFile,
    reader: &str,
    target: &str,
) -> Result<(), duckdb::Error> {
    conn.execute_batch(&format!(
        "COPY (SELECT *, {} AS {SOURCE_COLUMN} FROM {reader}) TO {} (FORMAT parquet)",
        sql_string(&file.name),
        sql_string(target),
    ))
}

fn csv_reader(path: &Path, latin1: bool) -> Result<String, AppError> {
    let path = sql_string(&path_text(path)?);
    Ok(if latin1 {
        format!("read_csv({path}, header = true, encoding = 'latin-1')")
    } else {
        format!("read_csv({path}, header = true)")
    })
}

fn is_unicode_error(e: &duckdb::Error) -> bool {
    e.to_string().to_lowercase().contains("unicode")
}

/// Biểu thức đọc một file và danh sách cột của nó. CSV thử UTF-8 trước; DuckDB báo lỗi mã hóa thì thử latin-1, dạng
/// thường gặp ở file xuất từ phần mềm cũ.
fn open_reader(conn: &Connection, file: &InputFile) -> Result<(String, Vec<Column>), AppError> {
    let path = sql_string(&path_text(&file.path)?);
    match file.format {
        FileFormat::Parquet => {
            let reader = format!("read_parquet({path})");
            let columns = describe(conn, &reader)
                .map_err(|e| user_file_error(&file.name, "đọc Parquet", e))?;
            Ok((reader, columns))
        }
        FileFormat::Csv => {
            let utf8 = csv_reader(&file.path, false)?;
            let (reader, columns) = match describe(conn, &utf8) {
                Ok(columns) => (utf8, columns),
                Err(e) if is_unicode_error(&e) => {
                    let latin1 = csv_reader(&file.path, true)?;
                    let columns = describe(conn, &latin1)
                        .map_err(|e| user_file_error(&file.name, "đọc CSV", e))?;
                    (latin1, columns)
                }
                Err(e) => return Err(user_file_error(&file.name, "đọc CSV", e)),
            };
            // Dòng thiếu hay thừa cột làm DuckDB bỏ cuộc tách cột: cả dòng thành một cột, tên cột là nguyên dòng tiêu
            // đề ("region,amount"). Không chặn thì một bảng sai được lưu mà không ai biết.
            if let [only] = columns.as_slice() {
                if only.name.contains([',', ';', '\t', '|']) {
                    return Err(AppError::BadRequest(format!(
                        "File {} có dòng thiếu hoặc thừa cột so với dòng tiêu đề, không tách được cột",
                        file.name
                    )));
                }
            }
            Ok((reader, columns))
        }
    }
}

fn describe(conn: &Connection, from: &str) -> Result<Vec<Column>, duckdb::Error> {
    let mut stmt = conn.prepare(&format!("DESCRIBE SELECT * FROM {from}"))?;
    let rows = stmt.query_map([], |row| {
        Ok(Column {
            name: row.get(0)?,
            data_type: row.get(1)?,
        })
    })?;
    rows.collect()
}

fn check_same_columns(
    first_name: &str,
    first: &BTreeSet<String>,
    name: &str,
    columns: &BTreeSet<String>,
) -> Result<(), AppError> {
    let missing: Vec<&str> = first.difference(columns).map(String::as_str).collect();
    let extra: Vec<&str> = columns.difference(first).map(String::as_str).collect();
    if missing.is_empty() && extra.is_empty() {
        return Ok(());
    }
    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("thiếu cột {}", missing.join(", ")));
    }
    if !extra.is_empty() {
        parts.push(format!("thừa cột {}", extra.join(", ")));
    }
    Err(AppError::BadRequest(format!(
        "File {name} không cùng cột với file {first_name}: {}. Các file nạp cùng lúc phải có cùng tên cột (phân biệt chữ hoa, chữ thường)",
        parts.join("; ")
    )))
}

/// Lỗi khi DuckDB đọc file người dùng: chi tiết vào log, người dùng nhận thông báo kèm tên file.
/// Lỗi IO (đầy ổ đĩa, mất quyền ghi) là lỗi máy chủ, không phải lỗi của file người dùng.
fn user_file_error(file_name: &str, step: &str, e: duckdb::Error) -> AppError {
    let text = e.to_string();
    if text.contains("IO Error") {
        return AppError::Internal(format!("{step} {file_name}: {text}"));
    }
    tracing::info!(file = file_name, step, error = %text, "duckdb rejected user file");
    AppError::BadRequest(format!(
        "Không {step} được file {file_name}: file hỏng hoặc không đúng định dạng"
    ))
}

/// Chuỗi SQL: bọc nháy đơn, nhân đôi nháy đơn bên trong. Tên file người dùng đi vào SQL qua hàm này.
fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn path_text(path: &Path) -> Result<String, AppError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Internal(format!("path is not valid UTF-8: {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thư mục tạm cho một test, tự xóa khi ra khỏi phạm vi kể cả khi test thất bại giữa chừng.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("xdi-analytics-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(dir.join("in")).unwrap();
            std::fs::create_dir_all(dir.join("out")).unwrap();
            TempDir(dir)
        }

        fn csv(&self, name: &str, bytes: &[u8]) -> InputFile {
            let path = self.0.join("in").join(name);
            std::fs::write(&path, bytes).unwrap();
            InputFile {
                path,
                name: name.to_string(),
                format: FileFormat::Csv,
            }
        }

        fn out(&self) -> PathBuf {
            self.0.join("out")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Chạy "SELECT <select>" trên bảng gộp của dir ({t} trong select thay bằng biểu thức đọc bảng), trả cột đầu.
    fn query_strings(dir: &Path, select: &str) -> Vec<String> {
        let conn = open().unwrap();
        let sql = format!(
            "SELECT {}",
            select.replace("{t}", &parquet_glob(dir).unwrap())
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
        rows.map(Result::unwrap).collect()
    }

    #[test]
    fn single_csv_becomes_parquet_with_source_column() {
        let tmp = TempDir::new();
        let file = tmp.csv("orders.csv", b"region,amount\nSouth,10\nNorth,20\n");
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 2);
        let names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["region", "amount", SOURCE_COLUMN]);
        assert!(tmp.out().join("part-0001.parquet").is_file());
        let sources = query_strings(&tmp.out(), "DISTINCT _source_file FROM {t}");
        assert_eq!(sources, ["orders.csv"]);
    }

    #[test]
    fn files_with_same_columns_are_stacked_in_any_column_order() {
        let tmp = TempDir::new();
        let files = [
            tmp.csv("thang1.csv", b"region,amount\nSouth,10\nNorth,20\n"),
            tmp.csv("thang2.csv", b"amount,region\n30,South\n"),
            tmp.csv("thang3.csv", b"region,amount\nEast,40\nWest,50\nSouth,60\n"),
        ];
        let result = materialize(&files, &tmp.out()).unwrap();
        assert_eq!(result.row_count, 6);
        let per_file = query_strings(
            &tmp.out(),
            "_source_file || ':' || count(*) FROM {t} GROUP BY _source_file ORDER BY 1",
        );
        assert_eq!(per_file, ["thang1.csv:2", "thang2.csv:1", "thang3.csv:3"]);
        let south = query_strings(
            &tmp.out(),
            "CAST(sum(amount) AS VARCHAR) FROM {t} WHERE region = 'South'",
        );
        assert_eq!(south, ["100"]);
    }

    #[test]
    fn mismatched_columns_are_rejected_with_file_and_column_names() {
        let tmp = TempDir::new();
        let files = [
            tmp.csv("thang1.csv", b"region,amount\nSouth,10\n"),
            tmp.csv("thang3.csv", b"region,qty\nSouth,1\n"),
        ];
        let err = materialize(&files, &tmp.out()).unwrap_err();
        let AppError::BadRequest(msg) = err else {
            panic!("expected BadRequest, got {err:?}")
        };
        assert!(
            msg.contains("thang3.csv") && msg.contains("thang1.csv"),
            "{msg}"
        );
        assert!(
            msg.contains("thiếu cột amount") && msg.contains("thừa cột qty"),
            "{msg}"
        );
        // Từ chối trước khi ghi: không để lại Parquet nửa chừng.
        assert_eq!(std::fs::read_dir(tmp.out()).unwrap().count(), 0);
    }

    #[test]
    fn latin1_csv_is_read() {
        let tmp = TempDir::new();
        // "Café" với é là byte 0xE9 của latin-1, không hợp lệ trong UTF-8.
        let file = tmp.csv("legacy.csv", b"name,amount\nCaf\xe9,5\n");
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 1);
        assert_eq!(query_strings(&tmp.out(), "name FROM {t}"), ["Café"]);
    }

    #[test]
    fn parquet_input_is_accepted() {
        let tmp = TempDir::new();
        let source = tmp.0.join("in").join("data.parquet");
        let conn = open().unwrap();
        let sql = format!(
            "COPY (SELECT 'South' AS region, 10 AS amount) TO {} (FORMAT parquet)",
            sql_string(&path_text(&source).unwrap())
        );
        conn.execute_batch(&sql).unwrap();
        let file = InputFile {
            path: source,
            name: "data.parquet".to_string(),
            format: FileFormat::Parquet,
        };
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 1);
    }

    #[test]
    fn integer_and_decimal_files_stack_as_numbers() {
        let tmp = TempDir::new();
        let files = [
            tmp.csv("a.csv", b"amount\n10\n"),
            tmp.csv("b.csv", b"amount\n2.5\n"),
        ];
        let result = materialize(&files, &tmp.out()).unwrap();
        let amount = result.columns.iter().find(|c| c.name == "amount").unwrap();
        assert_eq!(amount.data_type, "DOUBLE");
        assert_eq!(
            query_strings(&tmp.out(), "CAST(sum(amount) AS VARCHAR) FROM {t}"),
            ["12.5"]
        );
    }

    #[test]
    fn bad_inputs_are_user_errors() {
        let tmp = TempDir::new();
        let reserved = tmp.csv("r.csv", b"_source_file,amount\nx,1\n");
        assert!(matches!(
            materialize(&[reserved], &tmp.out()),
            Err(AppError::BadRequest(_))
        ));
        let empty = tmp.csv("empty.csv", b"");
        let err = materialize(&[empty], &tmp.out()).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("empty.csv")),
            "{err:?}"
        );
        assert!(matches!(
            materialize(&[], &tmp.out()),
            Err(AppError::BadRequest(_))
        ));
        let header_only = tmp.csv("header.csv", b"region,amount\n");
        assert!(matches!(
            materialize(&[header_only], &tmp.out()),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn quotes_in_file_names_do_not_break_sql() {
        let tmp = TempDir::new();
        let file = tmp.csv("it's.csv", b"amount\n1\n");
        materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(
            query_strings(&tmp.out(), "_source_file FROM {t}"),
            ["it's.csv"]
        );
    }

    #[test]
    fn format_from_file_name() {
        assert_eq!(FileFormat::from_file_name("A.CSV"), Some(FileFormat::Csv));
        assert_eq!(
            FileFormat::from_file_name("x.parquet"),
            Some(FileFormat::Parquet)
        );
        assert_eq!(FileFormat::from_file_name("x.xlsx"), None);
    }

    /// Byte latin-1 nằm sau phần DuckDB đọc thử (khoảng 20.000 dòng đầu): bước ghi gặp lỗi mã hóa và đọc lại theo latin-1.
    #[test]
    fn latin1_byte_deep_in_a_large_csv_is_read() {
        let tmp = TempDir::new();
        let mut bytes = b"name,amount\n".to_vec();
        for i in 0..30_000 {
            bytes.extend_from_slice(format!("row{i},1\n").as_bytes());
        }
        bytes.extend_from_slice(b"Caf\xe9,5\n");
        let file = tmp.csv("big.csv", &bytes);
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 30_001);
        assert_eq!(
            query_strings(&tmp.out(), "name FROM {t} WHERE amount = 5"),
            ["Café"]
        );
    }

    /// Một dòng thiếu cột làm DuckDB gộp cả dòng thành một cột; phải báo lỗi chứ không lưu bảng sai.
    #[test]
    fn rows_with_missing_columns_are_rejected() {
        let tmp = TempDir::new();
        let mut bytes = b"region,amount\n".to_vec();
        for _ in 0..100 {
            bytes.extend_from_slice(b"South,1\n");
        }
        bytes.extend_from_slice(b"onlyone\n");
        let file = tmp.csv("ragged.csv", &bytes);
        let err = materialize(&[file], &tmp.out()).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("ragged.csv")),
            "{err:?}"
        );
    }

    #[test]
    fn semicolon_csv_is_split_into_columns() {
        let tmp = TempDir::new();
        let file = tmp.csv("vn.csv", b"region;amount\nSouth;10\n");
        let result = materialize(&[file], &tmp.out()).unwrap();
        let names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["region", "amount", SOURCE_COLUMN]);
    }

    #[test]
    fn extensions_are_not_auto_installed() {
        let conn = open().unwrap();
        let value: String = conn
            .query_row(
                "SELECT current_setting('autoinstall_known_extensions')::VARCHAR",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, "false");
    }
}
