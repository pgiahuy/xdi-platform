// SPDX-License-Identifier: Apache-2.0
/* service/ask.rs - luồng một câu hỏi: đọc hồ sơ, ghép ngữ cảnh, LLM sinh SQL, guard kiểm và tiêm phân quyền,
chạy trên DuckDB, giải thích nếu là câu "tại sao", ghi lịch sử. Xem docs/ARCHITECTURE.md mục 2.
Kết quả trả về theo hợp đồng ở ask_contract.rs; luồng chưa cài đặt (#33) nên hiện trả 501. */

use super::ask_contract::{AskResponse, ChosenOption};
use crate::{auth::Member, error::AppError, state::AppState};

pub async fn ask(
    _state: &AppState,
    _member: &Member,
    _dataset_id: &str,
    question: &str,
    _choices: &[ChosenOption],
) -> Result<AskResponse, AppError> {
    if question.trim().is_empty() {
        return Err(AppError::BadRequest("Câu hỏi đang trống".to_string()));
    }
    Err(AppError::NotImplemented("hỏi dữ liệu"))
}

pub async fn drill(_state: &AppState, _member: &Member, _dataset_id: &str) -> Result<(), AppError> {
    Err(AppError::NotImplemented("truy dòng nguồn"))
}
