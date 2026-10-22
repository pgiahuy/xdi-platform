// SPDX-License-Identifier: Apache-2.0
/* engine/csv_encoding.rs - đổi file CSV không phải UTF-8 sang UTF-8 để DuckDB đọc. engine/analytics.rs gọi
build_utf8_copy khi DuckDB báo file không phải UTF-8.

Đọc theo windows-1252 chứ không phải latin-1: file xuất từ phần mềm Windows cũ (như DataCo, preset chính) dùng
windows-1252, có các byte 0x80 đến 0x9F cho Š, Ž, dấu nháy cong; DuckDB đọc với encoding = 'latin-1' từ chối cả file vì
các byte đó. Ở mọi byte khác, windows-1252 và latin-1 cho cùng một ký tự, nên file latin-1 vẫn đọc đúng. encoding_rs
đổi được mọi byte (byte không có ký tự thành ký tự điều khiển cùng mã), nên bước đổi không báo lỗi mã hóa.

Ngoại lệ: file mở đầu bằng BOM UTF-16 thì đọc theo UTF-16 (decoder của encoding_rs tự nhận BOM). File có chữ UTF-8
lẫn byte không phải UTF-8 thì không đổi mà báo người dùng lưu lại (reject_broken_utf8).

Giới hạn: file UTF-16 thường không tới được đây. DuckDB dò định dạng gặp byte NUL của UTF-16 và báo lỗi tách cột chứ
không báo lỗi unicode, nên analytics.rs không gọi bước đổi mã; chỉ file UTF-16 nhỏ có BOM mới đi qua. Không coi là hỗ
trợ UTF-16.

Đọc và ghi từng khối, không nạp cả file vào bộ nhớ: file có thể tới MAX_UPLOAD_MB. */

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use encoding_rs::{CoderResult, Decoder, Encoding};

use crate::error::AppError;

const CHUNK_BYTES: usize = 64 * 1024;

/// Bản UTF-8 của một file CSV, nằm cạnh file gốc với đuôi ".utf8.csv". Tự xóa khi ra khỏi phạm vi (Drop): bản sao có
/// thể lớn gấp ba file gốc (’ hay € thành ba byte UTF-8), nên không để nó nằm trong thư mục tạm tới hết lần nạp.
pub struct Utf8Copy {
    path: PathBuf,
}

impl Utf8Copy {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Utf8Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Đổi file sang UTF-8, ghi cạnh file gốc. file_name là tên người dùng đặt, chỉ dùng trong thông báo lỗi.
pub fn build_utf8_copy(path: &Path, file_name: &str) -> Result<Utf8Copy, AppError> {
    if !starts_with_utf16_bom(path)? {
        reject_broken_utf8(path, file_name)?;
    }
    let mut target = path.as_os_str().to_owned();
    target.push(".utf8.csv");
    // Tạo guard trước khi ghi: lỗi giữa chừng thì file dở bị xóa theo.
    let copy = Utf8Copy {
        path: PathBuf::from(target),
    };
    let file = File::create(copy.path()).map_err(|e| io_error(path, e))?;
    let mut output = BufWriter::new(file);
    // new_decoder (có dò BOM) chứ không phải new_decoder_without_bom_handling: file có BOM UTF-16 được đọc theo
    // UTF-16, BOM không thành ba ký tự rác ở đầu tên cột đầu tiên.
    let decoder = encoding_rs::WINDOWS_1252.new_decoder();
    decode_file(path, decoder, |text| {
        output
            .write_all(text.as_bytes())
            .map_err(|e| io_error(path, e))
    })?;
    output.flush().map_err(|e| io_error(path, e))?;
    Ok(copy)
}

fn starts_with_utf16_bom(path: &Path) -> Result<bool, AppError> {
    let mut head = [0u8; 3];
    let mut input = File::open(path).map_err(|e| io_error(path, e))?;
    let read = input.read(&mut head).map_err(|e| io_error(path, e))?;
    Ok(match Encoding::for_bom(&head[..read]) {
        Some((encoding, _)) => {
            encoding == encoding_rs::UTF_16LE || encoding == encoding_rs::UTF_16BE
        }
        None => false,
    })
}

/// File có ít nhất một chữ UTF-8 nhiều byte hợp lệ (chữ có dấu, BOM UTF-8) mà vẫn có byte không phải UTF-8 thì gần như
/// chắc là file UTF-8 bị hỏng vài byte. Đổi theo windows-1252 sẽ làm sai mọi chữ có dấu trong file mà không báo gì
/// ("Hà Nội" thành "HÃ  Ná»™i"): UTF-8 tiếng Việt có byte trong khoảng 0x80 đến 0x9F, mà windows-1252 nhận hết. Thay
/// byte hỏng bằng ký tự thay thế thì làm mất chữ mà người dùng không biết. Nên báo kèm số dòng để người dùng sửa file.
/// Có thể báo nhầm hiếm: file windows-1252 có hai byte liền nhau tình cờ thành một chữ UTF-8 hợp lệ (ví dụ "É’" là
/// C9 92); lưu lại dạng UTF-8 vẫn sửa được.
fn reject_broken_utf8(path: &Path, file_name: &str) -> Result<(), AppError> {
    let mut line = 1;
    let mut first_bad_line: Option<usize> = None;
    let mut has_multibyte = false;
    let decoder = encoding_rs::UTF_8.new_decoder_without_bom_handling();
    decode_file(path, decoder, |text| {
        for c in text.chars() {
            match c {
                '\n' => line += 1,
                // Decoder thay mỗi chuỗi byte không hợp lệ bằng U+FFFD.
                '\u{FFFD}' => {
                    first_bad_line.get_or_insert(line);
                }
                c if !c.is_ascii() => has_multibyte = true,
                _ => {}
            }
        }
        Ok(())
    })?;
    if !has_multibyte {
        return Ok(());
    }
    let place = match first_bad_line {
        Some(n) => format!(" (dòng {n})"),
        None => String::new(),
    };
    Err(AppError::BadRequest(format!(
        "File {file_name} có chữ UTF-8 lẫn byte không phải UTF-8{place}. Mở file bằng trình soạn thảo hoặc Excel, lưu lại dạng UTF-8 rồi nạp lại"
    )))
}

/// Đọc file từng khối, đổi sang UTF-8 bằng decoder, đưa từng đoạn chữ đã đổi cho on_text.
fn decode_file(
    path: &Path,
    mut decoder: Decoder,
    mut on_text: impl FnMut(&str) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let mut input = File::open(path).map_err(|e| io_error(path, e))?;
    let mut chunk = vec![0u8; CHUNK_BYTES];
    let mut text = String::with_capacity(CHUNK_BYTES * 3);
    loop {
        let read = input.read(&mut chunk).map_err(|e| io_error(path, e))?;
        // read = 0 là hết file: báo decoder đây là khối cuối.
        let last = read == 0;
        let mut rest = &chunk[..read];
        loop {
            // Trả về: đã đổi hết khối hay chưa, số byte đã đọc, có thay byte hỏng hay không.
            // Hết chỗ trong text (OutputFull: decoder dừng trước khi hết khối) thì đưa phần đã đổi cho on_text rồi đổi
            // tiếp phần còn lại. Bước kiểm UTF-8 gặp chuyện này khi khối toàn byte hỏng: mỗi byte thành U+FFFD ba byte.
            let (result, used, _) = decoder.decode_to_string(rest, &mut text, last);
            on_text(&text)?;
            text.clear();
            rest = &rest[used..];
            if result == CoderResult::InputEmpty {
                break;
            }
        }
        if last {
            return Ok(());
        }
    }
}

fn io_error(path: &Path, e: std::io::Error) -> AppError {
    AppError::Internal(format!("convert {} to UTF-8: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("xdi-encoding-{}", uuid::Uuid::new_v4()));
            std::fs::write(&path, bytes).unwrap();
            TempFile(path)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn convert(bytes: &[u8]) -> Result<String, AppError> {
        let source = TempFile::new(bytes);
        let copy = build_utf8_copy(&source.0, "f.csv")?;
        Ok(std::fs::read_to_string(copy.path()).unwrap())
    }

    #[test]
    fn windows_1252_bytes_become_utf8() {
        assert_eq!(
            convert(b"Cox\x92s,\x8aiauliai,\x8eilina,Caf\xe9").unwrap(),
            "Cox\u{2019}s,\u{160}iauliai,\u{17d}ilina,Caf\u{e9}"
        );
    }

    #[test]
    fn copy_is_removed_when_dropped() {
        let source = TempFile::new(b"a\x92b");
        let copy = build_utf8_copy(&source.0, "f.csv").unwrap();
        let path = copy.path().to_path_buf();
        assert!(path.is_file());
        drop(copy);
        assert!(!path.exists());
    }

    #[test]
    fn utf8_with_a_broken_byte_is_rejected_with_line() {
        let err = convert(&["city\nHà Nội\nCox".as_bytes(), b"\x92s\n"].concat()).unwrap_err();
        let AppError::BadRequest(msg) = err else {
            panic!("expected BadRequest, got {err:?}")
        };
        assert!(msg.contains("f.csv") && msg.contains("dòng 3"), "{msg}");
    }

    #[test]
    fn utf16_with_bom_is_read_as_utf16() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "city\nHà Nội\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(convert(&bytes).unwrap(), "city\nHà Nội\n");
    }

    /// Khối toàn byte 0x92 thành gấp ba số byte, lấp đầy bộ đệm đầu ra: không mất chữ nào.
    #[test]
    fn output_three_times_the_input_is_not_truncated() {
        let bytes = vec![0x92u8; CHUNK_BYTES * 3 + 7];
        let text = convert(&bytes).unwrap();
        assert_eq!(text.chars().count(), bytes.len());
        assert!(text.chars().all(|c| c == '\u{2019}'));
    }

    /// Bước kiểm UTF-8 với một khối toàn byte hỏng: đầu ra gấp ba khối, decoder trả OutputFull. Bỏ vòng lặp trong
    /// decode_file thì phần sau (chữ "à") bị bỏ sót và file lọt qua bước kiểm.
    #[test]
    fn utf8_check_reads_past_a_full_output_buffer() {
        let bytes = [vec![0x92u8; CHUNK_BYTES - 1], "à\n".as_bytes().to_vec()].concat();
        assert!(matches!(convert(&bytes), Err(AppError::BadRequest(_))));
    }

    /// Nhiều khối: chữ đổi đúng qua ranh giới khối và số dòng đếm tiếp qua các khối.
    #[test]
    fn large_files_are_handled_across_chunks() {
        let mut bytes = Vec::new();
        for _ in 0..30_000 {
            bytes.extend_from_slice(b"row,\x92\n");
        }
        let text = convert(&bytes).unwrap();
        assert_eq!(text.lines().count(), 30_000);
        assert!(text.lines().all(|l| l == "row,\u{2019}"));

        let mut broken = "Hà Nội\n".as_bytes().to_vec();
        for _ in 0..30_000 {
            broken.extend_from_slice(b"row\n");
        }
        broken.extend_from_slice(b"\x92\n");
        let err = convert(&broken).unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("dòng 30002")),
            "{err:?}"
        );
    }
}
