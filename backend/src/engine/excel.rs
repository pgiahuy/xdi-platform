// SPDX-License-Identifier: Apache-2.0
/* engine/excel.rs - đọc file Excel (.xlsx) bằng crate calamine: liệt kê sheet, đổi sheet thành file CSV. Sau bước này
sheet đi tiếp đường CSV của analytics.rs (đổi sang Parquet, gộp nhiều phần cùng cột), nên ở đây chỉ có phần riêng của
Excel.

Không dùng extension excel của DuckDB: extension đó không biên dịch sẵn vào crate duckdb, phải tải từ Internet lúc
chạy (máy chủ không có mạng thì không nạp được), và không có hàm liệt kê sheet.

Quy ước đọc một sheet (ô gộp, tiêu đề lệch dòng chưa hỗ trợ):
- Dòng đầu tiên có dữ liệu là dòng tiêu đề. Các cột tính từ ô tiêu đề trái nhất đến ô tiêu đề phải nhất.
- Tên cột bỏ khoảng trắng hai đầu; ô tiêu đề trống lấy tên theo chữ cột Excel (C, AA); tên trùng (không phân biệt hoa
  thường) thêm hậu tố _2, _3. Nhờ vậy tên cột ở danh sách sheet trùng với tên cột của dataset sau khi nạp.
- Dòng trống hoàn toàn bị bỏ qua. Ô lỗi công thức (#DIV/0!) thành ô trống, nên dòng chỉ có ô lỗi cũng bị bỏ.
- Ô có dữ liệu nằm ngoài các cột của tiêu đề thì báo lỗi, không lặng lẽ bỏ.
- Ô ngày giờ ghi dạng 2009-12-01 07:45:00 (ô chỉ có ngày: 2009-12-01, ô chỉ có giờ: 07:45:00) để DuckDB nhận ra kiểu
  TIMESTAMP, DATE, TIME. Cột có cả ô chỉ ngày lẫn ô có giờ được báo cho DuckDB là TIMESTAMP (SheetCsv), vì DuckDB đoán
  kiểu theo ô đầu và bỏ về chữ khi gặp dạng khác. Cột trộn ô chỉ có giờ với ô ngày thành cột chữ.

Bộ nhớ và đĩa. calamine đọc ô trong sheet theo luồng nhưng nạp cả bảng chuỗi dùng chung (sharedStrings.xml) vào RAM lúc
mở file, đọc trọn một ô trước khi trả về, và tin những con số file tự khai. Vì vậy các hàm ở đây chỉ chạy trong tiến
trình con có trần RAM và thời gian (engine/excel_worker.rs). Ở đây thêm các trần cho lỗi rõ ràng, trả 413 hay 400 kèm
lời giải thích thay vì để tiến trình con chạm trần: kích thước giải nén mà file tự ghi cho từng phần (crate zip dừng
đọc phần nào bung ra lớn hơn kích thước nó tự ghi, nên không khai gian được), số sheet, độ dài một dòng, dung lượng CSV
ghi ra.

Các hàm ở đây là hàm đồng bộ và tốn CPU. */

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use calamine::{
    open_workbook, DataRef, ExcelDateTime, Reader, SheetType, SheetVisible, Xlsx, XlsxError,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Excel có tối đa 16384 cột (A đến XFD). File ghi cột lớn hơn là file hỏng hoặc cố ý.
const MAX_COLUMNS: u32 = 16_384;
/// Một dòng có tổng chữ dài hơn thì từ chối: chặn kiểu một chuỗi dùng chung lớn được nhiều ô trong cùng dòng trỏ tới.
const MAX_ROW_BYTES: u64 = 16 * 1024 * 1024;
/// Số sheet tối đa của một file: mỗi sheet tốn một lần đọc, nhiều sheet có thể cùng trỏ tới một phần dữ liệu.
const MAX_SHEETS: usize = 100;
/// Tổng độ dài tên sheet và tên cột của mọi sheet trong danh sách sheet trả về.
const MAX_LISTING_BYTES: u64 = 1024 * 1024;
/// Excel giới hạn tên sheet 31 ký tự; dài hơn nhiều là file tự ghép.
const MAX_SHEET_NAME_CHARS: usize = 100;

/// Trần cho một file Excel; service đặt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Limits {
    /// Tổng dung lượng các phần trong file sau khi giải nén.
    pub unzipped: u64,
    /// Riêng bảng chuỗi dùng chung, phần calamine nạp hết vào RAM.
    pub shared_strings: u64,
    /// Tổng dung lượng CSV ghi ra cho mọi sheet của một lần nạp (chuỗi dùng chung lặp lại nhiều ô làm CSV lớn hơn cả
    /// phần giải nén).
    pub csv: u64,
}

/// Một sheet như người dùng thấy lúc chọn sheet để nạp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSummary {
    pub name: String,
    /// Sheet bị ẩn trong Excel. Không chọn sheet thì sheet ẩn không được tính.
    pub hidden: bool,
    /// Tên cột theo dòng tiêu đề, đã chuẩn hóa như khi nạp; rỗng nếu sheet không có dữ liệu.
    pub columns: Vec<String>,
    /// Số dòng dưới dòng tiêu đề, theo vùng dữ liệu file Excel tự ghi (thẻ dimension). Là số ước lượng: có thể tính cả
    /// dòng trống; 0 nếu file không ghi. Số dòng thật có sau khi nạp.
    pub approx_rows: u64,
}

/// Tên một sheet chứa bảng và sheet đó có bị ẩn không.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetName {
    pub name: String,
    pub hidden: bool,
}

/// Một sheet cần đổi và chỗ ghi file CSV của nó.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetJob {
    pub sheet: String,
    pub out: PathBuf,
}

/// Kết quả đổi một sheet sang CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetCsv {
    /// Cột cần báo DuckDB đọc là TIMESTAMP: mọi ô có dữ liệu đều là ngày hoặc ngày giờ, có cả hai dạng.
    pub timestamp_columns: Vec<String>,
}

type Workbook = Xlsx<BufReader<File>>;

/// Các sheet chứa bảng, theo thứ tự trong file (bỏ sheet chỉ có biểu đồ).
pub fn sheet_names(
    path: &Path,
    file_name: &str,
    limits: Limits,
) -> Result<Vec<SheetName>, AppError> {
    let workbook = open(path, file_name, limits)?;
    worksheets(&workbook, file_name)
}

/// Mọi sheet kèm cột và số dòng ước lượng. Chỉ đọc tới hết dòng tiêu đề của mỗi sheet, để người dùng không phải chờ
/// đọc cả file chỉ để chọn sheet.
pub fn list_sheets(
    path: &Path,
    file_name: &str,
    limits: Limits,
) -> Result<Vec<SheetSummary>, AppError> {
    let mut workbook = open(path, file_name, limits)?;
    let mut sheets = Vec::new();
    let mut listed_bytes: u64 = 0;
    for sheet in worksheets(&workbook, file_name)? {
        let mut columns = Vec::new();
        let (header_row, last_row) = read_rows(&mut workbook, file_name, &sheet.name, |row| {
            columns = row.to_vec();
            Ok(Next::Stop)
        })?;
        listed_bytes +=
            sheet.name.len() as u64 + columns.iter().map(|c| c.len() as u64).sum::<u64>();
        if listed_bytes > MAX_LISTING_BYTES {
            return Err(AppError::BadRequest(format!(
                "File {file_name} có tên sheet và tên cột quá dài (tổng hơn {} MB)",
                MAX_LISTING_BYTES / 1024 / 1024
            )));
        }
        let approx_rows = header_row.map_or(0, |row| u64::from(last_row.saturating_sub(row)));
        sheets.push(SheetSummary {
            name: sheet.name,
            hidden: sheet.hidden,
            columns,
            approx_rows,
        });
    }
    Ok(sheets)
}

/// Ghi từng sheet thành file CSV (UTF-8, dấu phẩy, nháy kép). Mở file Excel một lần cho mọi sheet.
pub fn sheets_to_csv(
    path: &Path,
    file_name: &str,
    jobs: &[SheetJob],
    limits: Limits,
) -> Result<Vec<SheetCsv>, AppError> {
    let mut workbook = open(path, file_name, limits)?;
    let mut written: u64 = 0;
    let mut results = Vec::with_capacity(jobs.len());
    for job in jobs {
        let before = written;
        results.push(write_csv(
            &mut workbook,
            file_name,
            job,
            limits.csv,
            &mut written,
        )?);
        if written == before {
            return Err(AppError::BadRequest(format!(
                "Sheet {} trong file {file_name} không có dữ liệu",
                job.sheet
            )));
        }
    }
    Ok(results)
}

/// Ghi một sheet; written cộng dồn số byte qua mọi sheet để so với trần chung.
fn write_csv(
    workbook: &mut Workbook,
    file_name: &str,
    job: &SheetJob,
    max_bytes: u64,
    written: &mut u64,
) -> Result<SheetCsv, AppError> {
    let out = &job.out;
    let file = File::create(out)
        .map_err(|e| AppError::Internal(format!("create {}: {e}", out.display())))?;
    let mut writer = BufWriter::new(file);
    let mut header: Vec<String> = Vec::new();
    let mut kinds: Vec<ColumnKinds> = Vec::new();
    read_typed_rows(workbook, file_name, &job.sheet, |row| {
        if header.is_empty() {
            header = row.iter().map(|(text, _)| text.clone()).collect();
            kinds = vec![ColumnKinds::default(); header.len()];
        } else {
            for (kinds, (_, kind)) in kinds.iter_mut().zip(row) {
                kinds.add(*kind);
            }
        }
        let line = csv_line(row);
        *written += line.len() as u64;
        if *written > max_bytes {
            return Err(AppError::TooLarge(format!(
                "File {file_name} quá lớn sau khi giải nén: dữ liệu dạng CSV vượt {} MB",
                max_bytes / 1024 / 1024
            )));
        }
        writer
            .write_all(line.as_bytes())
            .map_err(|e| AppError::Internal(format!("write {}: {e}", out.display())))?;
        Ok(Next::Continue)
    })?;
    writer
        .flush()
        .map_err(|e| AppError::Internal(format!("flush {}: {e}", out.display())))?;
    let timestamp_columns = header
        .into_iter()
        .zip(&kinds)
        .filter(|(_, kinds)| kinds.needs_timestamp())
        .map(|(name, _)| name)
        .collect();
    Ok(SheetCsv { timestamp_columns })
}

/// Loại giá trị của một ô, để biết cột nào cần báo kiểu cho DuckDB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Empty,
    Date,
    DateTime,
    Time,
    Other,
}

/// Các loại giá trị đã gặp trong một cột.
#[derive(Debug, Clone, Copy, Default)]
struct ColumnKinds {
    date: bool,
    datetime: bool,
    time: bool,
    other: bool,
}

impl ColumnKinds {
    fn add(&mut self, kind: Kind) {
        match kind {
            Kind::Empty => {}
            Kind::Date => self.date = true,
            Kind::DateTime => self.datetime = true,
            Kind::Time => self.time = true,
            Kind::Other => self.other = true,
        }
    }

    /// Cột chỉ có ngày và ngày giờ, có cả hai dạng: DuckDB tự đoán sẽ thành chữ, nên phải báo TIMESTAMP.
    fn needs_timestamp(&self) -> bool {
        self.date && self.datetime && !self.time && !self.other
    }
}

fn open(path: &Path, file_name: &str, limits: Limits) -> Result<Workbook, AppError> {
    check_archive(path, file_name, limits)?;
    open_workbook(path).map_err(|e: XlsxError| workbook_error(file_name, e))
}

/// Kiểm kích thước khai cho từng phần trước khi giải nén thật.
fn check_archive(path: &Path, file_name: &str, limits: Limits) -> Result<(), AppError> {
    let file = File::open(path)
        .map_err(|e| AppError::Internal(format!("open {}: {e}", path.display())))?;
    let mut archive = match zip::ZipArchive::new(BufReader::new(file)) {
        Ok(archive) => archive,
        Err(_) => return Err(not_a_zip(path, file_name)),
    };
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        // by_index_raw chỉ đọc thông tin của phần đó, không giải nén.
        let (size, is_shared_strings) = {
            let entry = archive.by_index_raw(i).map_err(|_| broken(file_name))?;
            let name = entry.name().map(|n| n.to_string()).unwrap_or_default();
            (entry.size(), is_shared_strings(&name))
        };
        total = total.saturating_add(size);
        if is_shared_strings && size > limits.shared_strings {
            return Err(AppError::TooLarge(format!(
                "File {file_name} có bảng chữ dùng chung quá lớn ({} MB sau khi giải nén, tối đa {} MB)",
                size / 1024 / 1024,
                limits.shared_strings / 1024 / 1024
            )));
        }
    }
    if total > limits.unzipped {
        return Err(AppError::TooLarge(format!(
            "File {file_name} quá lớn sau khi giải nén ({} MB, tối đa {} MB)",
            total / 1024 / 1024,
            limits.unzipped / 1024 / 1024
        )));
    }
    Ok(())
}

/// calamine tìm bảng chuỗi dùng chung theo thư mục ghi trong file, không phân biệt dấu \ và /. Nên kiểm mọi phần có tên
/// kết thúc như vậy, không chỉ xl/sharedStrings.xml.
fn is_shared_strings(name: &str) -> bool {
    name.replace('\\', "/")
        .to_ascii_lowercase()
        .ends_with("sharedstrings.xml")
}

/// File không phải zip. File Excel có mật khẩu (và file .xls cũ) là dạng CFB, bắt đầu bằng D0 CF 11 E0: báo riêng để
/// người dùng biết cách sửa.
fn not_a_zip(path: &Path, file_name: &str) -> AppError {
    let mut magic = [0u8; 4];
    let is_cfb = File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == [0xD0, 0xCF, 0x11, 0xE0];
    if is_cfb {
        return AppError::BadRequest(format!(
            "File {file_name} có mật khẩu hoặc là file .xls cũ: bỏ mật khẩu, hoặc lưu lại dạng .xlsx trong Excel, rồi nạp lại"
        ));
    }
    broken(file_name)
}

fn broken(file_name: &str) -> AppError {
    AppError::BadRequest(format!(
        "Không đọc được file {file_name}: file hỏng hoặc không phải Excel .xlsx"
    ))
}

fn workbook_error(file_name: &str, e: XlsxError) -> AppError {
    match e {
        // Lỗi đọc đĩa của máy chủ, không phải lỗi của file người dùng.
        XlsxError::Io(e) => AppError::Internal(format!("read {file_name}: {e}")),
        e => {
            tracing::info!(file = file_name, error = %e, "calamine rejected workbook");
            broken(file_name)
        }
    }
}

fn worksheets(workbook: &Workbook, file_name: &str) -> Result<Vec<SheetName>, AppError> {
    let sheets: Vec<SheetName> = workbook
        .sheets_metadata()
        .iter()
        .filter(|s| s.typ == SheetType::WorkSheet)
        .map(|s| SheetName {
            name: s.name.clone(),
            hidden: s.visible != SheetVisible::Visible,
        })
        .collect();
    if sheets
        .iter()
        .any(|s| s.name.chars().count() > MAX_SHEET_NAME_CHARS)
    {
        return Err(AppError::BadRequest(format!(
            "File {file_name} có tên sheet dài hơn {MAX_SHEET_NAME_CHARS} ký tự: file hỏng"
        )));
    }
    if sheets.len() > MAX_SHEETS {
        return Err(AppError::BadRequest(format!(
            "File {file_name} có {} sheet, nhiều hơn {MAX_SHEETS}; tách bớt sheet ra file khác rồi nạp",
            sheets.len()
        )));
    }
    Ok(sheets)
}

/// on_row trả Next::Stop khi không cần đọc thêm (liệt kê sheet chỉ cần dòng tiêu đề).
enum Next {
    Continue,
    Stop,
}

/// Như read_typed_rows nhưng chỉ đưa chữ của từng ô.
fn read_rows(
    workbook: &mut Workbook,
    file_name: &str,
    sheet: &str,
    mut on_row: impl FnMut(&[String]) -> Result<Next, AppError>,
) -> Result<(Option<u32>, u32), AppError> {
    read_typed_rows(workbook, file_name, sheet, |row| {
        let texts: Vec<String> = row.iter().map(|(text, _)| text.clone()).collect();
        on_row(&texts)
    })
}

/// Đọc lần lượt các dòng có dữ liệu của sheet: dòng đầu là tiêu đề đã chuẩn hóa, các dòng sau đã cắt theo các cột của
/// tiêu đề; mỗi ô kèm loại giá trị. Trả (số dòng của tiêu đề, dòng cuối theo thẻ dimension), tính từ 0.
fn read_typed_rows(
    workbook: &mut Workbook,
    file_name: &str,
    sheet: &str,
    mut on_row: impl FnMut(&[(String, Kind)]) -> Result<Next, AppError>,
) -> Result<(Option<u32>, u32), AppError> {
    let mut cells = workbook
        .worksheet_cells_reader(sheet)
        .map_err(|e| sheet_error(file_name, sheet, e))?;
    let last_row = cells.dimensions().end.0;
    let mut rows = RowBuilder {
        sheet,
        columns: None,
        header_row: None,
        row: None,
        cells: Vec::new(),
        bytes: 0,
    };
    // File xlsx ghi ô theo thứ tự dòng; ô trống thường không được ghi. Trong một dòng, ô có thể không theo thứ tự cột.
    while let Some(cell) = cells
        .next_cell()
        .map_err(|e| sheet_error(file_name, sheet, e))?
    {
        let (row, col) = cell.get_position();
        match rows.row {
            Some(current) if row < current => {
                return Err(AppError::BadRequest(format!(
                    "Sheet {sheet}: file ghi dòng {} sau dòng {}, không theo thứ tự; mở và lưu lại file trong Excel rồi nạp lại",
                    row + 1,
                    current + 1
                )));
            }
            Some(current) if row == current => {}
            _ => {
                if let Next::Stop = rows.finish_row(&mut on_row)? {
                    return Ok((rows.header_row, last_row));
                }
                rows.row = Some(row);
            }
        }
        let (text, kind) = cell_value(cell.get_value());
        if !text.is_empty() {
            rows.bytes += text.len() as u64;
            if rows.bytes > MAX_ROW_BYTES {
                return Err(AppError::BadRequest(format!(
                    "Sheet {sheet}: dòng {} dài quá {} MB",
                    row + 1,
                    MAX_ROW_BYTES / 1024 / 1024
                )));
            }
            rows.cells.push((col, text, kind));
        }
    }
    rows.finish_row(&mut on_row)?;
    Ok((rows.header_row, last_row))
}

/// Gom các ô của dòng đang đọc thành một dòng đủ cột.
struct RowBuilder<'a> {
    sheet: &'a str,
    /// Cột trái nhất và phải nhất (tính từ 0) của dòng tiêu đề; None khi chưa gặp dòng tiêu đề.
    columns: Option<(u32, u32)>,
    /// Số dòng (tính từ 0) của dòng tiêu đề.
    header_row: Option<u32>,
    /// Số dòng (tính từ 0) đang đọc; None trước ô đầu tiên.
    row: Option<u32>,
    /// Các ô có dữ liệu của dòng đang đọc: (cột, chữ, loại giá trị).
    cells: Vec<(u32, String, Kind)>,
    /// Tổng số byte chữ của dòng đang đọc.
    bytes: u64,
}

impl RowBuilder<'_> {
    fn finish_row(
        &mut self,
        on_row: &mut impl FnMut(&[(String, Kind)]) -> Result<Next, AppError>,
    ) -> Result<Next, AppError> {
        // take lấy danh sách ô ra và để lại danh sách rỗng cho dòng sau.
        let cells = std::mem::take(&mut self.cells);
        self.bytes = 0;
        let row_number = self.row.unwrap_or(0) + 1;
        if cells.is_empty() {
            return Ok(Next::Continue); // dòng trống
        }
        let Some((start, end)) = self.columns else {
            // Dòng có dữ liệu đầu tiên là dòng tiêu đề: các cột của nó là các cột của bảng.
            let (start, end, names) = header_columns(self.sheet, &cells)?;
            self.columns = Some((start, end));
            self.header_row = self.row;
            let header: Vec<(String, Kind)> =
                names.into_iter().map(|name| (name, Kind::Other)).collect();
            return on_row(&header);
        };
        let mut row = vec![(String::new(), Kind::Empty); (end - start + 1) as usize];
        for (col, text, kind) in cells {
            if col < start || col > end {
                return Err(AppError::BadRequest(format!(
                    "Sheet {}: ô {}{row_number} có dữ liệu nhưng cột {} không có tiêu đề. Đặt tên cho cột đó ở dòng tiêu \
                     đề (ví dụ cột chỉ số pandas ghi với ô tiêu đề trống), hoặc xóa dữ liệu thừa, rồi nạp lại",
                    self.sheet,
                    column_letters(col),
                    column_letters(col)
                )));
            }
            row[(col - start) as usize] = (text, kind);
        }
        on_row(&row)
    }
}

/// Cột trái nhất, phải nhất và tên các cột của dòng tiêu đề (xem quy ước ở đầu file).
fn header_columns(
    sheet: &str,
    cells: &[(u32, String, Kind)],
) -> Result<(u32, u32, Vec<String>), AppError> {
    // cells không rỗng (finish_row đã kiểm), nên min và max luôn có.
    let start = cells.iter().map(|(col, _, _)| *col).min().unwrap_or(0);
    let end = cells.iter().map(|(col, _, _)| *col).max().unwrap_or(0);
    if end >= MAX_COLUMNS {
        return Err(AppError::BadRequest(format!(
            "Sheet {sheet} ghi dữ liệu ở cột thứ {}, quá {MAX_COLUMNS} cột của Excel: file hỏng",
            u64::from(end) + 1
        )));
    }
    let mut names = vec![String::new(); (end - start + 1) as usize];
    for (col, text, _) in cells {
        // DuckDB bỏ BOM (U+FEFF) ở đầu file CSV và không đọc được tiêu đề có CR đứng riêng; làm giống trước để tên ở
        // danh sách sheet trùng tên cột thật.
        names[(col - start) as usize] = text
            .replace('\r', " ")
            .trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
            .to_string();
    }
    // seen: tên đã dùng (chữ thường). next_suffix: hậu tố kế tiếp sẽ thử cho mỗi tên gốc, để 16384 cột cùng tên không
    // phải thử lại từ _2 mỗi lần.
    let mut seen = HashSet::new();
    let mut next_suffix: HashMap<String, usize> = HashMap::new();
    for (i, name) in names.iter_mut().enumerate() {
        if name.is_empty() {
            *name = column_letters(start + i as u32);
        }
        if seen.insert(name.to_lowercase()) {
            continue;
        }
        let base = name.clone();
        let suffix = next_suffix.entry(base.to_lowercase()).or_insert(2);
        loop {
            let candidate = format!("{base}_{suffix}");
            *suffix += 1;
            if seen.insert(candidate.to_lowercase()) {
                *name = candidate;
                break;
            }
        }
    }
    Ok((start, end, names))
}

fn cell_value(value: &DataRef<'_>) -> (String, Kind) {
    match value {
        DataRef::Int(i) => (i.to_string(), Kind::Other),
        // Display của f64 không dùng dạng 1e5 và bỏ ".0" ở số nguyên (536365.0 thành "536365").
        DataRef::Float(f) => (f.to_string(), Kind::Other),
        DataRef::String(s) => (s.clone(), Kind::Other),
        DataRef::SharedString(s) => (s.to_string(), Kind::Other),
        DataRef::Bool(b) => (b.to_string(), Kind::Other),
        DataRef::DateTime(dt) => datetime_value(dt),
        DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => (s.clone(), Kind::Other),
        // Ô lỗi công thức (#DIV/0!, #N/A) không có giá trị: để trống, DuckDB đọc thành NULL.
        DataRef::Error(_) | DataRef::Empty => (String::new(), Kind::Empty),
    }
}

/// Excel lưu ngày giờ là số ngày tính từ 1900 (hoặc 1904), phần lẻ là giờ trong ngày; calamine đổi sang ngày giờ thật.
/// Số nhỏ hơn 1 là ô chỉ có giờ; không có phần lẻ là ô chỉ có ngày. Khoảng thời gian (ô định dạng [h]:mm) không phải
/// mốc thời gian, nên giữ số ngày. %.f in phần lẻ của giây khi có (mili giây), không in gì khi không có.
fn datetime_value(dt: &ExcelDateTime) -> (String, Kind) {
    let number = (dt.as_f64().to_string(), Kind::Other);
    if dt.is_duration() {
        return number;
    }
    let Some(value) = dt.as_datetime() else {
        return number;
    };
    let serial = dt.as_f64();
    if serial < 1.0 {
        (value.format("%H:%M:%S%.f").to_string(), Kind::Time)
    } else if serial.fract() == 0.0 {
        (value.format("%Y-%m-%d").to_string(), Kind::Date)
    } else {
        (
            value.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
            Kind::DateTime,
        )
    }
}

/// Một dòng CSV: bọc nháy kép ô có dấu phẩy, nháy kép hay xuống dòng; nháy kép bên trong nhân đôi.
fn csv_line(row: &[(String, Kind)]) -> String {
    let fields: Vec<String> = row
        .iter()
        .map(|(field, _)| {
            if field.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", field.replace('"', "\"\""))
            } else {
                field.clone()
            }
        })
        .collect();
    format!("{}\n", fields.join(","))
}

/// Tên cột kiểu Excel từ số cột tính từ 0: 0 thành A, 25 thành Z, 26 thành AA.
fn column_letters(col: u32) -> String {
    // u64 để col + 1 không tràn khi file ghi số cột rất lớn.
    let mut n = u64::from(col) + 1;
    let mut letters = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(char::from(b'A' + rem as u8));
        n = (n - 1) / 26;
    }
    letters.iter().rev().collect()
}

fn sheet_error(file_name: &str, sheet: &str, e: XlsxError) -> AppError {
    match e {
        XlsxError::WorksheetNotFound(_) => {
            AppError::BadRequest(format!("File {file_name} không có sheet {sheet}"))
        }
        XlsxError::Io(e) => AppError::Internal(format!("read {file_name} sheet {sheet}: {e}")),
        e => {
            tracing::info!(file = file_name, sheet, error = %e, "calamine rejected sheet");
            AppError::BadRequest(format!(
                "Không đọc được sheet {sheet} trong file {file_name}: file hỏng"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rust_xlsxwriter::{ExcelDateTime as Date, Format, Workbook as Book, Worksheet};

    use super::*;

    const LIMITS: Limits = Limits {
        unzipped: 100_000_000,
        shared_strings: 10_000_000,
        csv: 1_000_000,
    };

    /// Hàm ghi ô vào một sheet của file test.
    type FillSheet = dyn Fn(&mut Worksheet);

    /// Thư mục tạm cho một test, tự xóa khi ra khỏi phạm vi.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("xdi-excel-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        /// Tạo file book.xlsx với các sheet (tên, hàm ghi ô) cho trước.
        fn xlsx(&self, sheets: &[(&str, &FillSheet)]) -> PathBuf {
            let mut book = Book::new();
            for (name, fill) in sheets {
                let sheet = book.add_worksheet();
                sheet.set_name(*name).unwrap();
                fill(sheet);
            }
            self.write("book.xlsx", &book.save_to_buffer().unwrap())
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }

        /// Đổi các sheet sang CSV trong thư mục tạm, trả nội dung CSV của từng sheet.
        fn csv(
            &self,
            path: &Path,
            sheets: &[&str],
            limits: Limits,
        ) -> Result<Vec<String>, AppError> {
            let jobs: Vec<SheetJob> = sheets
                .iter()
                .enumerate()
                .map(|(i, sheet)| SheetJob {
                    sheet: sheet.to_string(),
                    out: self.0.join(format!("{i}.csv")),
                })
                .collect();
            sheets_to_csv(path, "book.xlsx", &jobs, limits)?;
            Ok(jobs
                .iter()
                .map(|job| std::fs::read_to_string(&job.out).unwrap())
                .collect())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// File xlsx tối thiểu ghép tay từ XML của một sheet tên S (và bảng chuỗi dùng chung nếu có), để thử những gì
    /// rust_xlsxwriter không bao giờ ghi ra: ô không theo thứ tự cột, dòng ghi lùi, cột ngoài giới hạn.
    fn raw_xlsx(rows: &str, shared: Option<&str>) -> Vec<u8> {
        let ns = "http://schemas.openxmlformats.org";
        let mut types = format!(
            "<Types xmlns=\"{ns}/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
             <Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        );
        let mut book_rels = format!(
            "<Relationships xmlns=\"{ns}/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"{ns}/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>"
        );
        if shared.is_some() {
            types.push_str("<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>");
            book_rels.push_str(&format!(
                "<Relationship Id=\"rId2\" Type=\"{ns}/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>"
            ));
        }
        types.push_str("</Types>");
        book_rels.push_str("</Relationships>");
        let mut parts = vec![
            ("[Content_Types].xml", types),
            ("_rels/.rels", format!(
                "<Relationships xmlns=\"{ns}/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"{ns}/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
                 </Relationships>"
            )),
            ("xl/workbook.xml", format!(
                "<workbook xmlns=\"{ns}/spreadsheetml/2006/main\" xmlns:r=\"{ns}/officeDocument/2006/relationships\">\
                 <sheets><sheet name=\"S\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
            )),
            ("xl/_rels/workbook.xml.rels", book_rels),
            ("xl/worksheets/sheet1.xml", format!(
                "<worksheet xmlns=\"{ns}/spreadsheetml/2006/main\"><sheetData>{rows}</sheetData></worksheet>"
            )),
        ];
        if let Some(strings) = shared {
            parts.push((
                "xl/sharedStrings.xml",
                format!("<sst xmlns=\"{ns}/spreadsheetml/2006/main\">{strings}</sst>"),
            ));
        }
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, xml) in parts {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// Ô chữ ghi thẳng trong XML (không qua bảng chuỗi dùng chung).
    fn text(cell: &str, value: &str) -> String {
        format!("<c r=\"{cell}\" t=\"inlineStr\"><is><t>{value}</t></is></c>")
    }

    fn number(cell: &str, value: f64) -> String {
        format!("<c r=\"{cell}\"><v>{value}</v></c>")
    }

    fn orders(sheet: &mut Worksheet) {
        let datetime = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
        let date = Format::new().set_num_format("yyyy-mm-dd");
        let time = Format::new().set_num_format("hh:mm:ss");
        let titles = [
            "Invoice",
            "InvoiceDate",
            "Day",
            "Start",
            "Price",
            "Paid",
            "Note",
        ];
        for (col, title) in titles.iter().enumerate() {
            sheet.write_string(0, col as u16, *title).unwrap();
        }
        sheet.write_string(1, 0, "489434").unwrap();
        let at = |s| Date::parse_from_str(s).unwrap();
        sheet
            .write_datetime_with_format(1, 1, at("2009-12-01 07:45:00"), &datetime)
            .unwrap();
        sheet
            .write_datetime_with_format(1, 2, at("2009-12-01"), &date)
            .unwrap();
        sheet
            .write_datetime_with_format(1, 3, at("07:30:00"), &time)
            .unwrap();
        sheet.write_number(1, 4, 6.95).unwrap();
        sheet.write_boolean(1, 5, true).unwrap();
        sheet.write_string(1, 6, "a, \"b\"").unwrap();
        // Dòng 3 để trống; dòng 4 thiếu vài ô, ngày giờ có mili giây.
        sheet.write_string(3, 0, "C489449").unwrap();
        sheet
            .write_datetime_with_format(3, 1, at("2010-01-02 08:00:00.250"), &datetime)
            .unwrap();
        sheet.write_number(3, 4, 12.0).unwrap();
    }

    #[test]
    fn lists_sheets_with_columns_and_rows() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[
            ("Year 2009", &orders),
            ("Empty", &|_: &mut Worksheet| {}),
            ("Lookup", &|s: &mut Worksheet| {
                s.set_hidden(true);
                s.write_string(0, 0, "code").unwrap();
            }),
        ]);
        let names = sheet_names(&path, "book.xlsx", LIMITS).unwrap();
        let hidden: Vec<(&str, bool)> = names.iter().map(|s| (s.name.as_str(), s.hidden)).collect();
        assert_eq!(
            hidden,
            [("Year 2009", false), ("Empty", false), ("Lookup", true)]
        );
        let sheets = list_sheets(&path, "book.xlsx", LIMITS).unwrap();
        assert_eq!(
            sheets[0],
            SheetSummary {
                name: "Year 2009".to_string(),
                hidden: false,
                columns: [
                    "Invoice",
                    "InvoiceDate",
                    "Day",
                    "Start",
                    "Price",
                    "Paid",
                    "Note"
                ]
                .map(String::from)
                .to_vec(),
                // Vùng dữ liệu tới dòng 4, gồm cả dòng 3 trống.
                approx_rows: 3,
            }
        );
        assert_eq!((sheets[1].columns.len(), sheets[1].approx_rows), (0, 0));
        assert!(sheets[2].hidden);
    }

    #[test]
    fn sheet_becomes_csv_with_dates_and_quoting() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[("Year 2009", &orders)]);
        let csv = dir.csv(&path, &["Year 2009"], LIMITS).unwrap();
        assert_eq!(
            csv,
            ["Invoice,InvoiceDate,Day,Start,Price,Paid,Note\n\
              489434,2009-12-01 07:45:00,2009-12-01,07:30:00,6.95,true,\"a, \"\"b\"\"\"\n\
              C489449,2010-01-02 08:00:00.250,,,12,,\n"]
        );
    }

    /// Tên cột bỏ khoảng trắng, ô trống lấy chữ cột, tên trùng thêm hậu tố: giống tên DuckDB giữ nguyên khi đọc lại.
    #[test]
    fn header_names_are_normalized() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[("Data", &|s: &mut Worksheet| {
            for (col, title) in [(0, " Name "), (2, "name"), (3, "Name"), (4, "C")] {
                s.write_string(0, col, title).unwrap();
            }
            s.write_number(1, 0, 1.0).unwrap();
        })]);
        let sheets = list_sheets(&path, "book.xlsx", LIMITS).unwrap();
        assert_eq!(sheets[0].columns, ["Name", "B", "name_2", "Name_3", "C"]);
        let csv = dir.csv(&path, &["Data"], LIMITS).unwrap();
        assert!(csv[0].starts_with("Name,B,name_2,Name_3,C\n"), "{}", csv[0]);
    }

    /// BOM (U+FEFF) và CR đứng riêng trong tên cột: bỏ BOM, CR thành dấu cách, để khớp tên DuckDB giữ.
    #[test]
    fn header_bom_and_carriage_return() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[("Data", &|s: &mut Worksheet| {
            s.write_string(0, 0, "\u{feff}id").unwrap();
            s.write_string(0, 1, "a\rb").unwrap();
            s.write_number(1, 0, 1.0).unwrap();
            s.write_number(1, 1, 2.0).unwrap();
        })]);
        assert_eq!(
            list_sheets(&path, "book.xlsx", LIMITS).unwrap()[0].columns,
            ["id", "a b"]
        );
    }

    /// Bảng không bắt đầu ở ô A1: các cột tính theo dòng tiêu đề; ô ngoài các cột đó bị từ chối.
    #[test]
    fn columns_follow_the_header_row() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[("Data", &|s: &mut Worksheet| {
            s.write_string(2, 1, "region").unwrap();
            s.write_string(2, 2, "amount").unwrap();
            s.write_string(3, 1, "South").unwrap();
            s.write_number(3, 2, 10.0).unwrap();
            s.write_string(4, 3, "stray").unwrap();
        })]);
        let err = dir.csv(&path, &["Data"], LIMITS).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("D5")),
            "{err:?}"
        );
    }

    /// Ô trong dòng không theo thứ tự cột (C1 trước A1) vẫn đọc đúng. Trước đây làm tràn số khi tính số cột.
    #[test]
    fn cells_out_of_column_order() {
        let dir = TempDir::new();
        let rows = format!(
            "<row r=\"1\">{}{}</row><row r=\"2\">{}{}</row>",
            text("C1", "c"),
            text("A1", "a"),
            number("C2", 3.0),
            number("A2", 1.0)
        );
        let path = dir.write("book.xlsx", &raw_xlsx(&rows, None));
        assert_eq!(
            list_sheets(&path, "book.xlsx", LIMITS).unwrap()[0].columns,
            ["a", "B", "c"]
        );
        assert_eq!(dir.csv(&path, &["S"], LIMITS).unwrap(), ["a,B,c\n1,,3\n"]);
    }

    #[test]
    fn malformed_sheets_are_rejected() {
        let dir = TempDir::new();
        let backwards = format!(
            "<row r=\"2\">{}</row><row r=\"1\">{}</row>",
            number("A2", 1.0),
            text("A1", "a")
        );
        let too_wide = format!("<row r=\"1\">{}</row>", text("XFE1", "a"));
        for rows in [backwards, too_wide] {
            let path = dir.write("book.xlsx", &raw_xlsx(&rows, None));
            assert!(
                matches!(dir.csv(&path, &["S"], LIMITS), Err(AppError::BadRequest(_))),
                "{rows}"
            );
        }
    }

    /// Các trần chặn file nén nhỏ bung ra rất lớn: tổng giải nén, bảng chuỗi dùng chung, một dòng, tổng CSV.
    #[test]
    fn size_limits_hold() {
        let dir = TempDir::new();
        // Một chuỗi 1 MB, 20 ô cùng một dòng trỏ tới: dòng 20 MB, file nén chỉ vài KB.
        let big = "a".repeat(1024 * 1024);
        let shared = format!("<si><t>h</t></si><si><t>{big}</t></si>");
        let header: String = (0..20)
            .map(|c| format!("<c r=\"{}1\" t=\"s\"><v>0</v></c>", column_letters(c)))
            .collect();
        let row: String = (0..20)
            .map(|c| format!("<c r=\"{}2\" t=\"s\"><v>1</v></c>", column_letters(c)))
            .collect();
        let rows = format!("<row r=\"1\">{header}</row><row r=\"2\">{row}</row>");
        let path = dir.write("book.xlsx", &raw_xlsx(&rows, Some(&shared)));
        let roomy = Limits {
            unzipped: 100_000_000,
            shared_strings: 10_000_000,
            csv: 100_000_000,
        };
        assert!(
            matches!(dir.csv(&path, &["S"], roomy), Err(AppError::BadRequest(m)) if m.contains("dài quá"))
        );
        let small_shared = Limits {
            shared_strings: 1000,
            ..roomy
        };
        assert!(matches!(
            sheet_names(&path, "book.xlsx", small_shared),
            Err(AppError::TooLarge(_))
        ));
        let small_unzipped = Limits {
            unzipped: 1000,
            ..roomy
        };
        assert!(matches!(
            list_sheets(&path, "book.xlsx", small_unzipped),
            Err(AppError::TooLarge(_))
        ));

        let path = dir.xlsx(&[("Year 2009", &orders)]);
        let small_csv = Limits { csv: 50, ..roomy };
        assert!(matches!(
            dir.csv(&path, &["Year 2009"], small_csv),
            Err(AppError::TooLarge(_))
        ));
    }

    #[test]
    fn bad_input_is_a_user_error() {
        let dir = TempDir::new();
        let not_excel = dir.write("fake.xlsx", b"region,amount\n");
        assert!(matches!(
            sheet_names(&not_excel, "fake.xlsx", LIMITS),
            Err(AppError::BadRequest(_))
        ));
        // File có mật khẩu là dạng CFB.
        let locked = dir.write(
            "locked.xlsx",
            &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        );
        assert!(matches!(
            sheet_names(&locked, "locked.xlsx", LIMITS),
            Err(AppError::BadRequest(m)) if m.contains("mật khẩu")
        ));

        let path = dir.xlsx(&[("Year 2009", &orders), ("Empty", &|_: &mut Worksheet| {})]);
        for sheet in ["Missing", "Empty"] {
            assert!(
                matches!(
                    dir.csv(&path, &[sheet], LIMITS),
                    Err(AppError::BadRequest(_))
                ),
                "{sheet}"
            );
        }
    }

    /// calamine tìm bảng chuỗi dùng chung theo thư mục ghi trong file và chấp nhận dấu \, nên mọi tên kiểu đó đều phải
    /// qua trần.
    #[test]
    fn shared_strings_names() {
        for name in [
            "xl/sharedStrings.xml",
            "x/SharedStrings.xml",
            "xl\\sharedStrings.xml",
        ] {
            assert!(is_shared_strings(name), "{name}");
        }
        assert!(!is_shared_strings("xl/worksheets/sheet1.xml"));
    }

    #[test]
    fn too_many_sheets_are_rejected() {
        let dir = TempDir::new();
        let mut book = Book::new();
        for i in 0..=MAX_SHEETS {
            book.add_worksheet().set_name(format!("S{i}")).unwrap();
        }
        let path = dir.write("book.xlsx", &book.save_to_buffer().unwrap());
        assert!(matches!(
            list_sheets(&path, "book.xlsx", LIMITS),
            Err(AppError::BadRequest(_))
        ));
    }

    /// Tên trùng với hậu tố đã sinh vẫn ra tên riêng; 16384 cột cùng tên vẫn nhanh.
    #[test]
    fn duplicate_suffixes_stay_unique() {
        let cells = |names: &[&str]| -> Vec<(u32, String, Kind)> {
            names
                .iter()
                .enumerate()
                .map(|(i, n)| (i as u32, n.to_string(), Kind::Other))
                .collect()
        };
        let (_, _, names) = header_columns("S", &cells(&["a", "A", "a_2"])).unwrap();
        assert_eq!(names, ["a", "A_2", "a_2_2"]);
        let same = vec!["x"; MAX_COLUMNS as usize];
        let (_, _, names) = header_columns("S", &cells(&same)).unwrap();
        assert_eq!(names.last().map(String::as_str), Some("x_16384"));
    }

    /// Cột có cả ô chỉ ngày (nửa đêm) lẫn ô có giờ được báo là TIMESTAMP; cột chỉ có ngày thì không.
    #[test]
    fn mixed_date_columns_are_reported() {
        let dir = TempDir::new();
        let path = dir.xlsx(&[("Data", &|s: &mut Worksheet| {
            let datetime = Format::new().set_num_format("yyyy-mm-dd hh:mm");
            s.write_string(0, 0, "at").unwrap();
            s.write_string(0, 1, "day").unwrap();
            for (row, at, day) in [
                (1, "2024-02-01 00:00:00", "2024-02-01"),
                (2, "2024-01-31 07:30:00", "2024-02-02"),
            ] {
                let at = Date::parse_from_str(at).unwrap();
                let day = Date::parse_from_str(day).unwrap();
                s.write_datetime_with_format(row, 0, at, &datetime).unwrap();
                s.write_datetime_with_format(row, 1, day, &datetime)
                    .unwrap();
            }
        })]);
        let jobs = [SheetJob {
            sheet: "Data".to_string(),
            out: dir.0.join("0.csv"),
        }];
        let result = sheets_to_csv(&path, "book.xlsx", &jobs, LIMITS).unwrap();
        assert_eq!(result[0].timestamp_columns, ["at"]);
    }

    #[test]
    fn column_letters_match_excel() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(701), "ZZ");
        assert_eq!(column_letters(702), "AAA");
        assert_eq!(column_letters(MAX_COLUMNS - 1), "XFD");
        // Số cột rất lớn (file hỏng) không làm tràn số.
        assert_eq!(column_letters(u32::MAX).len(), 7);
    }
}
