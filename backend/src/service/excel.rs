// SPDX-License-Identifier: Apache-2.0
/* service/excel.rs - phần riêng của việc nạp file Excel: chọn sheet, đổi các sheet đã chọn thành CSV để ingest.rs đi
tiếp như CSV, và liệt kê sheet cho người dùng chọn. Đọc file Excel nằm ở engine/excel.rs.

Mỗi lần nạp một file Excel. Các sheet được chọn phải cùng cột và gộp thành một dataset, cột _source_file ghi tên
sheet. Sheet khác cột thì nạp riêng từng lần (mỗi lần một dataset): route nạp luôn trả về đúng một dataset.

Mọi lần đọc file Excel chạy trong tiến trình con có trần RAM và thời gian (engine/excel_worker.rs), và mỗi lúc chỉ một
lần (wait_turn), để một file độc hại không làm chết hay nghẽn backend. */

use std::collections::HashSet;
use std::path::Path;

use tokio::sync::SemaphorePermit;

pub use crate::engine::excel::SheetSummary;

use crate::engine::analytics::{FileFormat, InputFile};
use crate::engine::excel::{Limits, SheetJob};
use crate::engine::excel_worker;
use crate::error::AppError;
use crate::service::ingest::{StagedFile, Staging, Upload};
use crate::state::AppState;

/// File xlsx là file nén. Phần giải nén và CSV đổi ra được phép lớn tới ngần này lần MAX_UPLOAD_MB: Online Retail II
/// (45 MB) giải nén ra 317 MB, tức khoảng 7 lần. Lớn hơn là file nén bất thường, từ chối để không đầy đĩa.
const MAX_EXPANSION: u64 = 10;

/// Trần riêng cho bảng chuỗi dùng chung, phần nằm hết trong RAM (RAM thật có thể gấp vài lần). File thật thường chỉ
/// vài MB (Online Retail II: 0,5 MB).
const MAX_SHARED_STRINGS: u64 = 64 * 1024 * 1024;

fn limits(max_upload_bytes: u64) -> Limits {
    let expanded = max_upload_bytes.saturating_mul(MAX_EXPANSION);
    Limits {
        unzipped: expanded,
        shared_strings: MAX_SHARED_STRINGS.min(max_upload_bytes),
        csv: expanded,
    }
}

/// Chờ tới lượt đọc file Excel (AppState::excel_jobs). Giữ giá trị trả về tới khi đọc xong. Request bị hủy thì lượt
/// được trả cùng lúc tiến trình con bị tắt, nên không có lần đọc nào chạy ngoài lượt.
async fn wait_turn(state: &AppState) -> Result<SemaphorePermit<'_>, AppError> {
    state
        .excel_jobs
        .acquire()
        .await
        .map_err(|e| AppError::Internal(format!("excel semaphore closed: {e}")))
}

/// Các sheet của file Excel kèm cột và số dòng ước lượng, để người dùng chọn trước khi nạp. Không lưu gì: staging
/// xóa khi xong.
pub async fn list_sheets(
    state: &AppState,
    staging: Staging,
    upload: Upload,
) -> Result<Vec<SheetSummary>, AppError> {
    let file = single_excel(&upload.files)?;
    let _turn = wait_turn(state).await?;
    let sheets = excel_worker::list_sheets(
        &state.config.excel_worker,
        &file.path,
        &file.name,
        limits(state.config.max_upload_bytes),
    )
    .await;
    // Thư mục tạm còn tới khi đọc xong; xóa ở đây.
    drop(staging);
    sheets
}

/// Sheet sẽ nạp, theo thứ tự trong file. Người dùng không chọn mà file chỉ có một sheet đang hiện thì lấy sheet đó.
pub async fn choose_sheets(state: &AppState, upload: &Upload) -> Result<Vec<String>, AppError> {
    let file = single_excel(&upload.files)?;
    let sheets = {
        let _turn = wait_turn(state).await?;
        excel_worker::sheet_names(
            &state.config.excel_worker,
            &file.path,
            &file.name,
            limits(state.config.max_upload_bytes),
        )
        .await?
    };

    let names: HashSet<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    let mut chosen = HashSet::new();
    for sheet in &upload.sheets {
        if !chosen.insert(sheet.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Sheet {sheet} được chọn hai lần"
            )));
        }
        if !names.contains(sheet.as_str()) {
            return Err(AppError::BadRequest(format!(
                "File {} không có sheet {sheet}",
                file.name
            )));
        }
    }
    if chosen.is_empty() {
        let visible: Vec<&str> = sheets
            .iter()
            .filter(|s| !s.hidden)
            .map(|s| s.name.as_str())
            .collect();
        return match visible.as_slice() {
            [] => Err(AppError::BadRequest(format!(
                "File {} không có sheet dữ liệu nào đang hiện",
                file.name
            ))),
            [only] => Ok(vec![only.to_string()]),
            _ => Err(AppError::BadRequest(format!(
                "File {} có {} sheet: {}. Chọn sheet cần nạp (trường sheet)",
                file.name,
                visible.len(),
                visible.join(", ")
            ))),
        };
    }
    Ok(sheets
        .into_iter()
        .filter(|s| chosen.contains(s.name.as_str()))
        .map(|s| s.name)
        .collect())
}

/// Đổi từng sheet đã chọn thành CSV trong out_dir, trả danh sách file cho analytics::materialize.
pub async fn sheet_inputs(
    state: &AppState,
    file: &StagedFile,
    sheets: &[String],
    out_dir: &Path,
) -> Result<Vec<InputFile>, AppError> {
    let jobs: Vec<SheetJob> = sheets
        .iter()
        .enumerate()
        .map(|(i, sheet)| SheetJob {
            sheet: sheet.clone(),
            out: out_dir.join(format!("{:04}.csv", i + 1)),
        })
        .collect();
    let converted = {
        let _turn = wait_turn(state).await?;
        excel_worker::sheets_to_csv(
            &state.config.excel_worker,
            &file.path,
            &file.name,
            jobs.clone(),
            limits(state.config.max_upload_bytes),
        )
        .await?
    };
    Ok(jobs
        .into_iter()
        .zip(converted)
        .map(|(job, csv)| InputFile {
            path: job.out,
            name: job.sheet,
            format: FileFormat::Xlsx,
            timestamp_columns: csv.timestamp_columns,
        })
        .collect())
}

/// File Excel duy nhất của lần nạp.
pub fn single_excel(files: &[StagedFile]) -> Result<&StagedFile, AppError> {
    match files {
        [file] if FileFormat::from_file_name(&file.name) == Some(FileFormat::Xlsx) => Ok(file),
        [file] => Err(AppError::BadRequest(format!(
            "File {} không phải file Excel .xlsx",
            file.name
        ))),
        [] => Err(AppError::BadRequest("Chưa chọn file nào".to_string())),
        _ => Err(AppError::BadRequest(
            "Mỗi lần nạp chỉ một file Excel; nạp từng file riêng".to_string(),
        )),
    }
}
