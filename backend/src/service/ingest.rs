// SPDX-License-Identifier: Apache-2.0
/* service/ingest.rs - nạp dữ liệu, ba bước độc lập: đăng ký nguồn, lập hồ sơ cột, chuyển sang Parquet.
File đi cả ba bước; nguồn database (sau này) đi hai bước đầu. Xem docs/ARCHITECTURE.md mục 5. */

use crate::{error::AppError, state::AppState};

pub async fn list_datasets(_state: &AppState) -> Result<(), AppError> {
    Err(AppError::NotImplemented("liệt kê dataset"))
}

pub async fn create_dataset(_state: &AppState) -> Result<(), AppError> {
    Err(AppError::NotImplemented("nạp dataset"))
}

pub async fn dataset_detail(_state: &AppState, _id: &str) -> Result<(), AppError> {
    Err(AppError::NotImplemented("xem dataset"))
}
