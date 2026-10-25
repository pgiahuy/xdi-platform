// SPDX-License-Identifier: Apache-2.0
/* engine/analytics.rs - DuckDB nhúng trong tiến trình backend: đọc CSV, Parquet; đổi file người dùng nạp lên thành
Parquet. Sheet Excel đã được engine/excel.rs đổi sang CSV trước khi tới đây. Chạy SQL đã qua guard ở engine/query.rs.
Không có container riêng, không giữ dữ liệu bền: mỗi lần dùng mở một kết nối DuckDB trong bộ nhớ, đọc và ghi file trên
đĩa.

DuckDB là thư viện đồng bộ và tốn CPU, nên các hàm ở đây là hàm thường (không async); service gọi chúng trong
tokio::task::spawn_blocking để không chặn các request khác.

Nhiều file cùng cột (ví dụ mỗi tháng một file) gộp thành một bảng: mỗi file ghi thành một file Parquet trong cùng thư
mục, kèm cột SOURCE_COLUMN ghi tên file gốc; đọc lại mọi file .parquet trong thư mục như một bảng (parquet_glob). */

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use duckdb::Connection;

use super::csv_encoding::{self, Utf8Copy};
use crate::error::AppError;

/// Cột thêm vào mỗi dòng, ghi tên file gốc của dòng đó. Truy dòng nguồn dùng cột này để chỉ ra dòng đến từ file nào.
pub const SOURCE_COLUMN: &str = "_source_file";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Csv,
    Parquet,
    /// Một sheet Excel. Trong InputFile, path là file CSV do engine/excel.rs ghi từ sheet đó, không phải file .xlsx.
    Xlsx,
}

impl FileFormat {
    /// Đoán theo đuôi tên file người dùng đặt. None nếu không phải định dạng nhận được.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.ends_with(".csv") {
            Some(FileFormat::Csv)
        } else if lower.ends_with(".parquet") {
            Some(FileFormat::Parquet)
        } else if lower.ends_with(".xlsx") {
            Some(FileFormat::Xlsx)
        } else {
            None
        }
    }
}

/// Một file đã nằm trên đĩa máy chủ, chờ đổi sang Parquet.
pub struct InputFile {
    /// Đường dẫn trên máy chủ (do backend đặt, không phải tên người dùng gửi).
    pub path: PathBuf,
    /// Tên file người dùng gửi (với Xlsx: tên sheet), ghi vào SOURCE_COLUMN và vào thông báo lỗi.
    pub name: String,
    pub format: FileFormat,
    /// Chỉ dùng với Xlsx: cột báo DuckDB đọc là TIMESTAMP (xem engine/excel.rs, SheetCsv). Rỗng với CSV, Parquet.
    pub timestamp_columns: Vec<String>,
}

impl InputFile {
    /// "file a.csv" hay "sheet Năm 2024", để thông báo lỗi gọi đúng thứ người dùng đã chọn.
    fn label(&self) -> String {
        match self.format {
            FileFormat::Xlsx => format!("sheet {}", self.name),
            _ => format!("file {}", self.name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct Column {
    pub name: String,
    // Kiểu theo DuckDB, ví dụ BIGINT, DOUBLE, VARCHAR, DATE.
    /// DuckDB type, for example `BIGINT`, `DOUBLE`, `VARCHAR`, `DATE`.
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
        let (reader, columns, utf8_copy) = open_reader(&conn, file)?;
        let names: BTreeSet<String> = columns.iter().map(|c| c.name.clone()).collect();
        if names.iter().any(|n| n.eq_ignore_ascii_case(SOURCE_COLUMN)) {
            return Err(AppError::BadRequest(format!(
                "Cột {SOURCE_COLUMN} trong {} trùng tên cột hệ thống dùng để ghi nguồn của từng dòng; đổi tên cột rồi nạp lại",
                file.label()
            )));
        }
        match &first {
            None => first = Some((file.name.clone(), names)),
            Some((first_name, first_names)) => {
                check_same_columns(file.format, first_name, first_names, &file.name, &names)?
            }
        }
        readers.push((reader, utf8_copy));
    }

    // utf8_copy được chuyển vào vòng lặp (zip(readers), không phải &readers) nên bản UTF-8 của mỗi file bị xóa ngay khi
    // file đó ghi xong Parquet, không đợi tới hết lần nạp.
    for (i, (file, (reader, utf8_copy))) in files.iter().zip(readers).enumerate() {
        let target = out_dir.join(format!("part-{:04}.parquet", i + 1));
        let target_text = path_text(&target)?;
        match copy_to_parquet(&conn, file, &reader, &target_text) {
            Ok(()) => {}
            // Bước đọc cột chỉ xem phần đầu file. Byte không phải UTF-8 nằm sâu trong file CSV thì tới đây mới lộ ra;
            // đổi cả file sang UTF-8 rồi đọc lại. Bản UTF-8 bị xóa khi ra khỏi nhánh này.
            Err(e) if file.format == FileFormat::Csv && is_invalid_unicode(&e.to_string()) => {
                let _ = std::fs::remove_file(&target);
                let copy = csv_encoding::build_utf8_copy(&file.path, &file.name)?;
                let converted = csv_reader(copy.path())?;
                copy_to_parquet(&conn, file, &converted, &target_text)
                    .map_err(|e| user_file_error(&file.label(), "đổi sang Parquet", e))?;
            }
            Err(e) => return Err(user_file_error(&file.label(), "đổi sang Parquet", e)),
        }
        drop(utf8_copy);
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
            "Không đọc được dòng dữ liệu nào: file chỉ có dòng tiêu đề, hoặc các dòng không cùng số cột với dòng tiêu đề".to_string(),
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
/// gì để DuckDB tải code từ Internet lúc chạy (máy chủ không có mạng cũng chạy được). Hồ sơ cột (profile.rs) dùng chung.
pub(super) fn open() -> Result<Connection, AppError> {
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

fn csv_reader(path: &Path) -> Result<String, AppError> {
    let path = sql_string(&path_text(path)?);
    Ok(format!("read_csv({path}, header = true)"))
}

/// DuckDB báo file không phải UTF-8. So cả cụm chứ không chỉ chữ "unicode": thông báo lỗi CSV của DuckDB chép nguyên
/// dòng lỗi và đường dẫn file, chữ "unicode" nằm trong dữ liệu sẽ làm lỗi khác (như thừa cột) đi nhầm sang bước đổi mã.
fn is_invalid_unicode(message: &str) -> bool {
    message
        .to_lowercase()
        .contains("invalid unicode (byte sequence mismatch)")
}

/// Biểu thức đọc một file, danh sách cột của nó, và bản UTF-8 nếu đã phải đổi mã (biểu thức đọc trỏ vào bản này, nên
/// phải giữ nó tới khi ghi Parquet xong). CSV thử UTF-8 trước; DuckDB báo lỗi mã hóa thì đổi file sang UTF-8
/// (engine/csv_encoding.rs), dạng thường gặp ở file xuất từ phần mềm cũ, rồi đọc file đã đổi.
fn open_reader(
    conn: &Connection,
    file: &InputFile,
) -> Result<(String, Vec<Column>, Option<Utf8Copy>), AppError> {
    let path = sql_string(&path_text(&file.path)?);
    match file.format {
        FileFormat::Parquet => {
            let reader = format!("read_parquet({path})");
            let columns = describe(conn, &reader)
                .map_err(|e| user_file_error(&file.label(), "đọc Parquet", e))?;
            Ok((reader, columns, None))
        }
        FileFormat::Csv => {
            let utf8 = csv_reader(&file.path)?;
            let (reader, columns, utf8_copy) = match describe(conn, &utf8) {
                Ok(columns) => (utf8, columns, None),
                Err(e) if is_invalid_unicode(&e.to_string()) => {
                    let copy = csv_encoding::build_utf8_copy(&file.path, &file.name)?;
                    let converted = csv_reader(copy.path())?;
                    let columns = describe(conn, &converted)
                        .map_err(|e| user_file_error(&file.label(), "đọc CSV", e))?;
                    (converted, columns, Some(copy))
                }
                Err(e) => return Err(user_file_error(&file.label(), "đọc CSV", e)),
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
            Ok((reader, columns, utf8_copy))
        }
        // CSV do chính backend ghi: biết chắc UTF-8, dấu phẩy, nháy kép, nên không để DuckDB đoán. sample_size = -1
        // cho DuckDB xem hết file trước khi chọn kiểu cột: cột như mã hàng có số ở đầu và chữ ở dòng thứ trăm nghìn
        // (85123A) mà chỉ xem phần đầu thì chọn kiểu số rồi lỗi khi gặp chữ.
        FileFormat::Xlsx => {
            let types = if file.timestamp_columns.is_empty() {
                String::new()
            } else {
                let pairs: Vec<String> = file
                    .timestamp_columns
                    .iter()
                    .map(|name| format!("{}: 'TIMESTAMP'", sql_string(name)))
                    .collect();
                format!(", types = {{{}}}", pairs.join(", "))
            };
            let reader = format!(
                "read_csv({path}, header = true, delim = ',', quote = '\"', escape = '\"', sample_size = -1{types})"
            );
            let columns =
                describe(conn, &reader).map_err(|e| user_file_error(&file.label(), "đọc", e))?;
            Ok((reader, columns, None))
        }
    }
}

pub(super) fn describe(conn: &Connection, from: &str) -> Result<Vec<Column>, duckdb::Error> {
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
    format: FileFormat,
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
    let parts = parts.join("; ");
    // Sheet Excel khác cột thì nạp riêng từng lần, mỗi lần thành một dataset.
    Err(AppError::BadRequest(match format {
        FileFormat::Xlsx => format!(
            "Sheet {name} không cùng cột với sheet {first_name}: {parts}. Các sheet nạp cùng lúc phải có cùng tên cột; sheet khác cột thì nạp riêng từng sheet"
        ),
        _ => format!(
            "File {name} không cùng cột với file {first_name}: {parts}. Các file nạp cùng lúc phải có cùng tên cột (phân biệt chữ hoa, chữ thường)"
        ),
    }))
}

/// Lỗi khi DuckDB đọc file người dùng: chi tiết vào log, người dùng nhận thông báo kèm tên file hay sheet (label).
/// Lỗi IO (đầy ổ đĩa, mất quyền ghi) là lỗi máy chủ, không phải lỗi của file người dùng.
fn user_file_error(label: &str, step: &str, e: duckdb::Error) -> AppError {
    let text = e.to_string();
    if text.contains("IO Error") {
        return AppError::Internal(format!("{step} {label}: {text}"));
    }
    tracing::info!(file = label, step, error = %text, "duckdb rejected user file");
    AppError::BadRequest(format!(
        "Không {step} được {label}: dữ liệu hỏng hoặc không đúng định dạng"
    ))
}

/// Chuỗi SQL: bọc nháy đơn, nhân đôi nháy đơn bên trong. Tên file người dùng đi vào SQL qua hàm này.
pub(super) fn sql_string(s: &str) -> String {
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
                timestamp_columns: Vec::new(),
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

    /// File xuất từ phần mềm Windows cũ: 0x92, 0x8a, 0x8e là ’, Š, Ž trong windows-1252, không có ký tự in được
    /// trong latin-1 (DataCo có đúng các byte này).
    #[test]
    fn windows_1252_csv_is_read() {
        let tmp = TempDir::new();
        let file = tmp.csv(
            "legacy.csv",
            b"city,amount\nCox\x92s Bazar,1\n\x8aiauliai,2\n\x8eilina,3\n",
        );
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 3);
        assert_eq!(
            query_strings(&tmp.out(), "city FROM {t} ORDER BY amount"),
            ["Cox\u{2019}s Bazar", "\u{160}iauliai", "\u{17d}ilina"]
        );
        // Bản UTF-8 đã bị xóa sau khi ghi Parquet, chỉ còn file gốc.
        assert_eq!(std::fs::read_dir(tmp.0.join("in")).unwrap().count(), 1);
    }

    /// File UTF-8 tiếng Việt có một byte hỏng: báo lỗi kèm số dòng, không đọc lại theo windows-1252 (sẽ làm sai mọi chữ
    /// có dấu). Byte hỏng ở đầu file lộ ra lúc đọc cột; ở sau 30.000 dòng thì lộ ra lúc ghi Parquet.
    #[test]
    fn utf8_csv_with_a_broken_byte_is_rejected() {
        let tmp = TempDir::new();
        let file = tmp.csv(
            "vn.csv",
            b"city,amount\nH\xc3\xa0 N\xe1\xbb\x99i,1\nCox\x92s,2\n",
        );
        let err = materialize(&[file], &tmp.out()).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("vn.csv") && m.contains("dòng 3")),
            "{err:?}"
        );

        let mut bytes = "city,amount\nHà Nội,1\n".as_bytes().to_vec();
        for i in 0..30_000 {
            bytes.extend_from_slice(format!("row{i},1\n").as_bytes());
        }
        bytes.extend_from_slice(b"Cox\x92s,5\n");
        let file = tmp.csv("big.csv", &bytes);
        let err = materialize(&[file], &tmp.out()).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("big.csv") && m.contains("dòng 30003")),
            "{err:?}"
        );
    }

    /// Chữ "unicode" trong dữ liệu không làm lỗi khác bị coi là lỗi mã hóa.
    #[test]
    fn only_duckdb_encoding_errors_count_as_invalid_unicode() {
        assert!(is_invalid_unicode(
            "Invalid Input Error: CSV Error on Line: 2\nOriginal Line: city,amount\nInvalid unicode (byte sequence mismatch) detected. This file is not utf-8 encoded."
        ));
        assert!(!is_invalid_unicode(
            "Invalid Input Error: CSV Error on Line: 30001\nOriginal Line: unicode,1,2,3\nExpected Number of Columns: 2 Found: 4"
        ));
    }

    /// File đã là UTF-8 thì đọc thẳng, không sinh bản đổi mã.
    #[test]
    fn utf8_csv_is_read_without_conversion() {
        let tmp = TempDir::new();
        let file = tmp.csv("utf8.csv", "city,amount\nŠiauliai,1\nHà Nội,2\n".as_bytes());
        materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(
            query_strings(&tmp.out(), "city FROM {t} ORDER BY amount"),
            ["Šiauliai", "Hà Nội"]
        );
        let in_dir: Vec<_> = std::fs::read_dir(tmp.0.join("in")).unwrap().collect();
        assert_eq!(in_dir.len(), 1);
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
            timestamp_columns: Vec::new(),
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
        assert_eq!(
            FileFormat::from_file_name("Book.XLSX"),
            Some(FileFormat::Xlsx)
        );
        assert_eq!(FileFormat::from_file_name("x.xls"), None);
    }

    /// Byte latin-1 nằm sau phần DuckDB đọc thử (khoảng 20.000 dòng đầu): bước ghi gặp lỗi mã hóa, đổi file sang UTF-8
    /// rồi đọc lại.
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

    /// Như trên với byte 0x92 của windows-1252, byte mà DuckDB đọc theo latin-1 từ chối.
    #[test]
    fn windows_1252_byte_deep_in_a_large_csv_is_read() {
        let tmp = TempDir::new();
        let mut bytes = b"name,amount\n".to_vec();
        for i in 0..30_000 {
            bytes.extend_from_slice(format!("row{i},1\n").as_bytes());
        }
        bytes.extend_from_slice(b"Cox\x92s Bazar,5\n");
        let file = tmp.csv("big.csv", &bytes);
        let result = materialize(&[file], &tmp.out()).unwrap();
        assert_eq!(result.row_count, 30_001);
        assert_eq!(
            query_strings(&tmp.out(), "name FROM {t} WHERE amount = 5"),
            ["Cox\u{2019}s Bazar"]
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
