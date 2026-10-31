// SPDX-License-Identifier: Apache-2.0
/* api/excel_tests.rs - test nạp file Excel qua router thật (issue #21): liệt kê sheet, chọn sheet, gộp sheet cùng cột,
sheet khác cột thành dataset riêng, chống trùng theo sheet. Mỗi test dùng database và thư mục dữ liệu riêng. */

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
use serde_json::{json, Value};

use crate::api::test_support::{app_with, call, multipart, send_raw, CODE};
use crate::engine::store::test_support::fresh_database;

const DATASETS: &str = "/api/workspaces/cua-hang/datasets";
const SHEETS: &str = "/api/workspaces/cua-hang/datasets/sheets";

/// File book.xlsx: Jan và Feb cùng cột (region, amount), Notes khác cột.
fn book() -> Vec<u8> {
    let mut book = Workbook::new();
    for (name, rows) in [
        ("Jan", [("South", 10.0), ("North", 20.0)]),
        ("Feb", [("South", 30.0), ("East", 40.0)]),
    ] {
        let sheet = book.add_worksheet();
        sheet.set_name(name).unwrap();
        sheet.write_string(0, 0, "region").unwrap();
        sheet.write_string(0, 1, "amount").unwrap();
        for (i, (region, amount)) in rows.iter().enumerate() {
            let row = i as u32 + 1;
            sheet.write_string(row, 0, *region).unwrap();
            sheet.write_number(row, 1, *amount).unwrap();
        }
    }
    let notes = book.add_worksheet();
    notes.set_name("Notes").unwrap();
    notes.write_string(0, 0, "note").unwrap();
    notes.write_string(1, 0, "giá đã gồm VAT").unwrap();
    book.save_to_buffer().unwrap()
}

async fn setup(app: &axum::Router) -> String {
    let body = json!({"setup_code": CODE, "email": "lan@example.com", "password": "a long password",
                      "workspace_name": "Cửa hàng"});
    let (status, session, _) = call(app, "POST", "/api/auth/setup", None, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    session.unwrap()
}

async fn send(
    app: &axum::Router,
    session: &str,
    path: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &[u8])],
) -> (StatusCode, Value) {
    let (content_type, body) = multipart(fields, files);
    send_raw(app, path, session, &content_type, body).await
}

/// Giá trị khác nhau của cột _source_file trong mọi file Parquet của một dataset, đọc thẳng bằng DuckDB.
fn source_files(data: &Path, workspace_id: &str, dataset_id: &str) -> Vec<String> {
    let pattern = data
        .join(workspace_id)
        // <workspace>/<dataset>/<bảng>/<phiên bản> (#257); dataset của một file chỉ có bảng này.
        .join("*")
        .join(dataset_id)
        .join("*")
        .join("*.parquet");
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let sql = format!(
        "SELECT DISTINCT _source_file FROM read_parquet('{}') ORDER BY 1",
        pattern.display()
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt.query_map([], |row| row.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

struct DataDir(PathBuf);

impl Drop for DataDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn excel_sheets_are_listed_chosen_and_merged() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data =
        DataDir(std::env::temp_dir().join(format!("xdi-excel-api-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&data.0).unwrap();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;
    let xlsx = book();
    let file = [("book.xlsx", xlsx.as_slice())];

    // Liệt kê sheet: tên, cột, số dòng ước lượng.
    let (status, sheets) = send(&app, &lan, SHEETS, &[], &file).await;
    assert_eq!(status, StatusCode::OK, "{sheets}");
    assert_eq!(
        sheets,
        json!([
            {"name": "Jan", "hidden": false, "columns": ["region", "amount"], "approx_rows": 2},
            {"name": "Feb", "hidden": false, "columns": ["region", "amount"], "approx_rows": 2},
            {"name": "Notes", "hidden": false, "columns": ["note"], "approx_rows": 1},
        ])
    );

    // Nhiều sheet mà không chọn: báo tên các sheet để chọn.
    let (status, body) = send(&app, &lan, DATASETS, &[], &file).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Jan, Feb, Notes"),
        "{body}"
    );

    // Hai sheet cùng cột: một dataset, cột _source_file ghi tên sheet.
    let both = [("name", "Bán hàng"), ("sheet", "Jan"), ("sheet", "Feb")];
    let (status, body) = send(&app, &lan, DATASETS, &both, &file).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["row_count"], 4);
    assert_eq!(body["version"]["source_kind"], "xlsx");
    let columns: Vec<&str> = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(columns, ["region", "amount", "_source_file"]);
    let (_, _, list) = call(&app, "GET", "/api/workspaces", Some(&lan), None).await;
    let workspace_id = list[0]["workspace_id"].as_str().unwrap().to_string();
    let dataset_id = body["dataset"]["id"].as_str().unwrap();
    assert_eq!(
        source_files(&data.0, &workspace_id, dataset_id),
        ["Feb", "Jan"]
    );

    // Cùng file, cùng sheet theo thứ tự khác: trùng, không tạo gì mới.
    let reversed = [("sheet", "Feb"), ("sheet", "Jan")];
    let (status, body) = send(&app, &lan, DATASETS, &reversed, &file).await;
    assert_eq!(
        (status, body["created"].as_bool()),
        (StatusCode::OK, Some(false))
    );

    // Cùng file nhưng chỉ một sheet: dữ liệu khác, nạp thành dataset mới.
    let (status, body) = send(&app, &lan, DATASETS, &[("sheet", "Jan")], &file).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["row_count"], 2);

    // Sheet khác cột: chọn chung thì từ chối, nạp riêng thì thành dataset riêng.
    let mixed = [("sheet", "Jan"), ("sheet", "Notes")];
    let (status, body) = send(&app, &lan, DATASETS, &mixed, &file).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"].as_str().unwrap().contains("Sheet Notes"),
        "{body}"
    );
    let (status, body) = send(&app, &lan, DATASETS, &[("sheet", "Notes")], &file).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["version"]["row_count"], 1);
    let (_, _, datasets) = call(&app, "GET", DATASETS, Some(&lan), None).await;
    assert_eq!(datasets.as_array().unwrap().len(), 3);

    db.drop().await;
}

#[tokio::test]
async fn bad_excel_uploads_are_rejected() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data =
        DataDir(std::env::temp_dir().join(format!("xdi-excel-api-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&data.0).unwrap();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;
    let xlsx = book();
    let csv: &[u8] = b"region,amount\nSouth,10\n";

    let xlsx = xlsx.as_slice();
    let cases = [
        // Sheet không có trong file; chọn một sheet hai lần.
        (DATASETS, vec![("sheet", "Mar")], vec![("book.xlsx", xlsx)]),
        (
            DATASETS,
            vec![("sheet", "Jan"), ("sheet", "Jan")],
            vec![("book.xlsx", xlsx)],
        ),
        // Trường sheet với file CSV; trộn Excel với CSV; hai file Excel một lần.
        (DATASETS, vec![("sheet", "Jan")], vec![("a.csv", csv)]),
        (
            DATASETS,
            vec![("sheet", "Jan")],
            vec![("book.xlsx", xlsx), ("a.csv", csv)],
        ),
        (
            DATASETS,
            vec![("sheet", "Jan")],
            vec![("a.xlsx", xlsx), ("b.xlsx", xlsx)],
        ),
        // File mang đuôi .xlsx mà không phải Excel.
        (SHEETS, Vec::new(), vec![("fake.xlsx", csv)]),
    ];
    for (path, fields, files) in cases {
        let (status, body) = send(&app, &lan, path, &fields, &files).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{fields:?} {body}");
    }
    let (_, _, datasets) = call(&app, "GET", DATASETS, Some(&lan), None).await;
    assert_eq!(datasets, json!([]));

    db.drop().await;
}

/// Không chọn sheet: sheet ẩn không tính, nên file có một sheet hiện và một sheet ẩn nạp được ngay. Ô ngày, giờ, ngày
/// giờ thành đúng kiểu DATE, TIME, TIMESTAMP.
#[tokio::test]
async fn hidden_sheets_are_skipped_and_dates_keep_their_kind() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data =
        DataDir(std::env::temp_dir().join(format!("xdi-excel-api-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&data.0).unwrap();
    let app = app_with(db.store.clone(), Some(CODE), |c| {
        c.data_dir = data.0.clone()
    });
    let lan = setup(&app).await;

    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Data").unwrap();
    let formats = [
        ("day", "yyyy-mm-dd", ["2024-01-31", "2024-02-01"]),
        ("start", "hh:mm", ["07:30:00", "18:00:00"]),
        // Ô đầu đúng nửa đêm, ghi dạng ngày: không báo TIMESTAMP thì DuckDB đọc cả cột thành chữ.
        (
            "at",
            "yyyy-mm-dd hh:mm",
            ["2024-02-01 00:00:00", "2024-01-31 07:30:00"],
        ),
    ];
    for (col, (title, num_format, values)) in formats.iter().enumerate() {
        let format = Format::new().set_num_format(*num_format);
        sheet.write_string(0, col as u16, *title).unwrap();
        for (row, value) in values.iter().enumerate() {
            let value = ExcelDateTime::parse_from_str(value).unwrap();
            sheet
                .write_datetime_with_format(row as u32 + 1, col as u16, value, &format)
                .unwrap();
        }
    }
    let lookup = book.add_worksheet();
    lookup.set_name("Lookup").unwrap();
    lookup.set_hidden(true);
    lookup.write_string(0, 0, "code").unwrap();
    let xlsx = book.save_to_buffer().unwrap();

    let (status, body) = send(&app, &lan, DATASETS, &[], &[("book.xlsx", &xlsx)]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let types: Vec<(&str, &str)> = body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap(),
                c["data_type"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        types,
        [
            ("day", "DATE"),
            ("start", "TIME"),
            ("at", "TIMESTAMP"),
            ("_source_file", "VARCHAR")
        ]
    );

    db.drop().await;
}
