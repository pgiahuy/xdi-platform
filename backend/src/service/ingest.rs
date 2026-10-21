// SPDX-License-Identifier: Apache-2.0
/* service/ingest.rs - nạp dữ liệu, ba bước độc lập: đăng ký nguồn, lập hồ sơ cột, chuyển sang Parquet.
File đi cả ba bước; nguồn database (sau này) đi hai bước đầu. Xem docs/ARCHITECTURE.md mục 5. */

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::store::Dataset;
use crate::{error::AppError, state::AppState};

pub async fn list_datasets(state: &AppState, member: &Member) -> Result<Vec<Dataset>, AppError> {
    state.store.list_datasets(&member.workspace).await
}

pub async fn create_dataset(_state: &AppState, _member: &Member) -> Result<(), AppError> {
    Err(AppError::NotImplemented("nạp dataset"))
}

/// Dataset của chính workspace này. Id của workspace khác trả 404 như id không tồn tại: store lọc theo WorkspaceId.
pub async fn dataset_detail(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<Dataset, AppError> {
    let not_found = || AppError::NotFound("Không có dataset này".to_string());
    let id = Uuid::parse_str(id).map_err(|_| not_found())?;
    state
        .store
        .get_dataset(&member.workspace, id)
        .await?
        .ok_or_else(not_found)
}
