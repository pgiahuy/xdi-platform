// SPDX-License-Identifier: Apache-2.0
/* service/domains/assign.rs - gắn gói ngành cho một dataset (cột sources.domain), hay bỏ gói. Gọi từ hai chỗ: admin
chọn gói trên màn Khái niệm ngành (PUT /catalog/{id}/domain, api/catalog.rs), và thiết lập lần đầu gắn gói cho dataset
mẫu (service/samples_domains.rs). Gói đã gắn thì prompt viết SQL và prompt form của câu hỏi nguyên nhân có phần khái
niệm ngành (service/ask/mod.rs đọc dataset.domain rồi gọi load_for_prompt). */

use uuid::Uuid;

use super::ensure_seeded;
use crate::auth::Member;
use crate::error::AppError;
use crate::state::AppState;

/// Gắn gói có khóa `key` cho dataset `source_id`; None hay chuỗi rỗng là bỏ gói. Chỉ admin. Khóa không có trong
/// workspace thì 400 với field `domain`; dataset không có thì 404.
pub async fn assign(
    state: &AppState,
    member: &Member,
    source_id: Uuid,
    key: Option<&str>,
) -> Result<(), AppError> {
    member.require_admin()?;
    let key = build_key(key);
    if let Some(key) = key {
        check_pack_exists(state, member, key).await?;
    }
    let saved = state
        .store
        .set_source_domain(&member.workspace, source_id, key)
        .await?;
    if !saved {
        return Err(AppError::NotFound("Không có dataset này".to_string()));
    }
    tracing::info!(%source_id, domain = ?key, "dataset domain pack set");
    Ok(())
}

/// Bỏ khoảng trắng hai đầu; chuỗi rỗng coi như không chọn gói, để ô chọn "Không dùng gói" gửi "" hay null đều được.
fn build_key(key: Option<&str>) -> Option<&str> {
    key.map(str::trim).filter(|k| !k.is_empty())
}

/// Gói phải có trong workspace: lưu một khóa lạ thì câu hỏi sau không có phần khái niệm mà không ai biết vì sao.
async fn check_pack_exists(state: &AppState, member: &Member, key: &str) -> Result<(), AppError> {
    // Workspace chưa mở màn Khái niệm ngành lần nào thì chưa có gói; chép từ file trước khi tìm.
    ensure_seeded(state, &member.workspace).await?;
    let found = state.store.find_domain_pack(&member.workspace, key).await?;
    if found.is_none() {
        return Err(AppError::InvalidField {
            field: "domain".to_string(),
            message: "Không có gói ngành này trong workspace".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_key_means_no_pack() {
        assert_eq!(build_key(None), None);
        assert_eq!(build_key(Some("  ")), None);
        assert_eq!(build_key(Some(" retail ")), Some("retail"));
    }
}
