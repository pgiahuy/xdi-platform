// SPDX-License-Identifier: Apache-2.0
/* service/eval_prepare.rs - chuẩn bị dữ liệu cho các bộ đánh giá (service/eval.rs, service/eval_classify.rs): tài khoản
và workspace tạm, nạp file như khi người dùng gửi lên, chờ hồ sơ ngữ nghĩa xong.

--data là một file (DataCo mẫu) thì nạp với tên dataset DataCo, bảng logic dataco, như trước. --data là một thư mục
(OULAD, #202) thì mỗi file CSV là một dataset riêng, tên dataset là tên file bỏ đuôi, đúng như khi người dùng nạp từng
file mà không đặt tên: studentInfo.csv thành bảng studentinfo. Chỉ nạp file có bảng mà bộ câu cần, để không phải nạp
studentVle.csv (454 MB) khi không câu nào hỏi tới. */

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::auth::{self, Member};
use crate::error::AppError;
use crate::service::ingest::logical_table_from_name;
use crate::service::samples::{upload_local_file, SAMPLE_NAME};
use crate::service::workspaces;
use crate::state::AppState;

/// Bảng logic của từng dataset đã nạp, trỏ tới id dataset.
pub type Datasets = BTreeMap<String, String>;

/// Một dataset mẫu cho bộ câu chỉ hỏi trên một bảng (eval-classify, eval DataCo). Trả id của dataset để hỏi.
pub(crate) async fn prepare(state: &AppState, data: &Path) -> Result<(Member, String), AppError> {
    let (member, datasets) = prepare_datasets(state, data, &BTreeSet::new()).await?;
    match datasets.into_values().next() {
        Some(id) => Ok((member, id)),
        None => Err(AppError::Internal(format!(
            "không nạp được dataset nào từ {}",
            data.display()
        ))),
    }
}

/// Tài khoản và workspace tạm, với các dataset đã nạp và hồ sơ ngữ nghĩa xong (mock thì có ngay). tables là các bảng
/// bộ câu cần; rỗng thì nạp mọi file. Workspace bị xóa sau khi chạy xong (run_eval), kéo theo user rác trong bảng users:
/// chấp nhận được vì eval nhắm vào database thử, không phải bản cài thật.
pub(crate) async fn prepare_datasets(
    state: &AppState,
    data: &Path,
    tables: &BTreeSet<String>,
) -> Result<(Member, Datasets), AppError> {
    let files = build_data_files(data, tables)?;
    let email = format!("eval-{}@example.com", uuid::Uuid::new_v4());
    let user = state.store.create_user(&email, "Eval", "unused").await?;
    let membership = workspaces::create(state, &user, "Eval").await?;
    let member = auth::member_of(&state.store, user, &membership.slug).await?;

    let mut datasets = Datasets::new();
    for (path, name) in &files {
        let result = upload_local_file(state, &member, path, name).await?;
        datasets.insert(result.dataset.logical_table, result.dataset.id.to_string());
    }
    for id in datasets.values() {
        wait_for_semantic(state, &member, id).await?;
    }
    Ok((member, datasets))
}

/// Các file cần nạp, kèm tên dataset. Thư mục thì lấy file .csv có bảng logic nằm trong tables (rỗng là mọi file),
/// thiếu bảng nào thì báo ngay, trước khi tạo workspace: lỗi hạ tầng, không phải câu trả lời sai.
pub fn build_data_files(
    data: &Path,
    tables: &BTreeSet<String>,
) -> Result<Vec<(PathBuf, String)>, AppError> {
    if !data.is_dir() {
        return Ok(vec![(data.to_path_buf(), SAMPLE_NAME.to_string())]);
    }
    let entries = std::fs::read_dir(data)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", data.display())))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| AppError::Internal(format!("read {}: {e}", data.display())))?
            .path();
        let is_csv = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"));
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if is_csv && (tables.is_empty() || tables.contains(&logical_table_from_name(&stem))) {
            files.push((path, stem));
        }
    }
    files.sort();
    let found: BTreeSet<String> = files
        .iter()
        .map(|(_, stem)| logical_table_from_name(stem))
        .collect();
    let missing: Vec<&str> = tables.difference(&found).map(String::as_str).collect();
    if !missing.is_empty() || files.is_empty() {
        return Err(AppError::Internal(format!(
            "{} thiếu file CSV cho bảng: {}",
            data.display(),
            missing.join(", ")
        )));
    }
    Ok(files)
}

/// Hồ sơ ngữ nghĩa chạy nền; hỏi khi chưa xong thì ngữ cảnh thiếu bí danh và câu gợi ý. Mô hình đã thử mà không lập
/// được (semantic_unavailable, ví dụ mock với bảng không phải DataCo) thì hỏi luôn với hồ sơ cấu trúc, như người dùng
/// hỏi trên giao diện; chờ thêm cũng không đổi được gì.
async fn wait_for_semantic(state: &AppState, member: &Member, id: &str) -> Result<(), AppError> {
    let id = uuid::Uuid::parse_str(id)
        .map_err(|e| AppError::Internal(format!("eval dataset id: {e}")))?;
    let provider = state.llm.name();
    for _ in 0..120 {
        let mut dataset = state
            .store
            .get_dataset(&member.workspace, id)
            .await?
            .ok_or_else(|| AppError::Internal("eval dataset vanished".to_string()))?;
        dataset.apply_semantic_unavailable(&provider);
        if dataset.current_profile_status.as_deref() == Some("semantic")
            || dataset.semantic_unavailable
        {
            if dataset.semantic_unavailable {
                tracing::warn!(%id, "semantic profile unavailable, asking with the structural profile");
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(AppError::Internal(format!(
        "hồ sơ ngữ nghĩa của dataset {id} chưa xong sau 120 giây"
    )))
}
