// SPDX-License-Identifier: Apache-2.0
/* tests/excel_worker.rs - chạy binary xdi-backend thật ở chế độ excel-worker (engine/excel_worker.rs): file hợp lệ trả
JSON, file độc hại chỉ làm chết tiến trình con, nhanh, không kéo theo tiến trình chạy test. Test trong crate không làm
được việc này vì binary của test không phải xdi-backend. */

use std::io::{Cursor, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// File xlsx tối thiểu một sheet tên S, bảng chuỗi dùng chung có thuộc tính sst_attrs.
fn xlsx(sst_attrs: &str) -> Vec<u8> {
    let ns = "http://schemas.openxmlformats.org";
    let parts = [
        (
            "[Content_Types].xml",
            format!(
                "<Types xmlns=\"{ns}/package/2006/content-types\">\
                 <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
                 <Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>"
            ),
        ),
        (
            "_rels/.rels",
            format!(
                "<Relationships xmlns=\"{ns}/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"{ns}/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
                 </Relationships>"
            ),
        ),
        (
            "xl/workbook.xml",
            format!(
                "<workbook xmlns=\"{ns}/spreadsheetml/2006/main\" xmlns:r=\"{ns}/officeDocument/2006/relationships\">\
                 <sheets><sheet name=\"S\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            format!(
                "<Relationships xmlns=\"{ns}/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"{ns}/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
                 <Relationship Id=\"rId2\" Type=\"{ns}/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>\
                 </Relationships>"
            ),
        ),
        (
            "xl/worksheets/sheet1.xml",
            format!(
                "<worksheet xmlns=\"{ns}/spreadsheetml/2006/main\"><sheetData>\
                 <row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row></sheetData></worksheet>"
            ),
        ),
        (
            "xl/sharedStrings.xml",
            format!("<sst xmlns=\"{ns}/spreadsheetml/2006/main\" {sst_attrs}><si><t>a</t></si></sst>"),
        ),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in parts {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Gửi một yêu cầu cho tiến trình con, trả (thành công không, stdout, thời gian chạy).
fn ask(mut request: serde_json::Value, file: &std::path::Path) -> (bool, String, Duration) {
    request["path"] = serde_json::json!(file);
    request["file_name"] = serde_json::json!("book.xlsx");
    request["limits"] = serde_json::json!(
        {"unzipped": 100_000_000u64, "shared_strings": 10_000_000u64, "csv": 100_000_000u64}
    );
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_xdi-backend"))
        .arg("excel-worker")
        // Máy nhiều nhân: runtime tokio 64 luồng giữ hàng GB vùng nhớ ảo. Tiến trình con phải rẽ ra trước khi dựng
        // runtime, nếu không thì trần RAM làm cả file hợp lệ cũng bị từ chối.
        .env("TOKIO_WORKER_THREADS", "64")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        started.elapsed(),
    )
}

#[test]
fn worker_reads_a_valid_file_and_dies_alone_on_a_hostile_one() {
    let dir = std::env::temp_dir().join(format!("xdi-worker-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let good = dir.join("good.xlsx");
    std::fs::write(&good, xlsx("uniqueCount=\"1\"")).unwrap();
    let (ok, stdout, _) = ask(serde_json::json!({"action": "sheet_names"}), &good);
    assert!(ok, "{stdout}");
    let reply: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(reply["sheet_names"][0]["name"], "S");

    // calamine cấp phát sẵn theo uniqueCount: 10 tỷ chuỗi là hàng trăm GB. Tiến trình con chạm trần RAM và dừng; kiểu
    // viết có khoảng trắng và số 0 ở đầu vẫn được calamine đọc.
    let hostile = dir.join("hostile.xlsx");
    std::fs::write(&hostile, xlsx("uniqueCount ='00010000000000'")).unwrap();
    let (ok, stdout, elapsed) = ask(serde_json::json!({"action": "sheet_names"}), &hostile);
    assert!(!ok, "{stdout}");
    assert!(elapsed < Duration::from_secs(30), "{elapsed:?}");

    // Liệt kê sheet và đổi sang CSV cũng chạy được qua tiến trình con.
    let (ok, stdout, _) = ask(serde_json::json!({"action": "list_sheets"}), &good);
    assert!(ok, "{stdout}");
    let reply: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(reply["sheets"][0]["columns"], serde_json::json!(["a"]));
    let out = dir.join("s.csv");
    let request =
        serde_json::json!({"action": "sheets_to_csv", "jobs": [{"sheet": "S", "out": out}]});
    let (ok, stdout, _) = ask(request, &good);
    assert!(ok, "{stdout}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "a\n");

    let _ = std::fs::remove_dir_all(&dir);
}
