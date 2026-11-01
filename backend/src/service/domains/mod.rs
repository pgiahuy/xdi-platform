// SPDX-License-Identifier: Apache-2.0
/* service/domains/mod.rs - màn Khái niệm ngành (#229): mọi thành viên xem các gói của workspace cùng mọi khái niệm;
admin thêm, sửa, tắt khái niệm. API ở api/domains.rs, kiểm trước khi lưu ở check.rs, ghi xuống Postgres ở
engine/store/domains.rs.

Gói của workspace nạp khởi tạo từ các file .json trong data/domains/ nhúng vào binary (engine/domains.rs), lúc workspace
lần đầu cần gói: mở màn này hay hỏi trên dataset có gói (load_for_prompt). Không nạp lúc tạo workspace vì workspace
được tạo ở nhiều chỗ (thiết lập lần đầu, tạo từ giao diện, test); nạp lúc đọc thì mọi workspace, kể cả workspace có từ
trước migration, đều có gói mà không phải nhớ gọi ở từng chỗ. Mỗi lần đọc tốn một câu SELECT để so khóa; chỉ khi thiếu
mới ghi (ensure_seeded).

Khái niệm admin lưu vào prompt của câu hỏi kế tiếp trên dataset gắn gói đó (sources.domain), không cần build lại. Gắn gói
cho dataset ở assign.rs. */

pub mod assign;
pub mod check;
pub mod contract;

use std::collections::HashSet;

use uuid::Uuid;

use crate::auth::{Member, WorkspaceId};
use crate::engine::domains::{list_domain_packages, DomainPackage};
use crate::engine::store::{DomainConceptRecord, DomainPackRecord};
use crate::error::AppError;
use crate::service::domains::check::build_new_concept;
use crate::service::domains::contract::{Concept, ConceptPack, ConceptRequest, ConceptsResponse};
use crate::state::AppState;

/// Mọi gói và khái niệm của workspace; mọi thành viên xem được.
pub async fn list(state: &AppState, member: &Member) -> Result<ConceptsResponse, AppError> {
    let ws = &member.workspace;
    ensure_seeded(state, ws).await?;
    let packs = state.store.list_domain_packs(ws).await?;
    let concepts = state.store.list_domain_concepts(ws).await?;
    Ok(ConceptsResponse {
        packs: build_packs(packs, concepts),
    })
}

/// Thêm một khái niệm vào cuối gói (chỉ admin).
pub async fn create(
    state: &AppState,
    member: &Member,
    pack_key: &str,
    request: ConceptRequest,
) -> Result<Concept, AppError> {
    member.require_admin()?;
    let (pack, concepts) = load_pack(state, member, pack_key).await?;
    let concept = build_new_concept(request, &concepts, None)?;
    let id = state
        .store
        .add_domain_concept(&member.workspace, pack.id, &concept, member.user.id)
        .await?;
    load_concept(state, member, id).await
}

/// Ghi đè một khái niệm của gói (chỉ admin); tắt là gửi enabled false.
pub async fn update(
    state: &AppState,
    member: &Member,
    pack_key: &str,
    concept_id: &str,
    request: ConceptRequest,
) -> Result<Concept, AppError> {
    member.require_admin()?;
    let not_found = || AppError::NotFound("Không có khái niệm này trong gói".to_string());
    let id = Uuid::parse_str(concept_id.trim()).map_err(|_| not_found())?;
    let (pack, concepts) = load_pack(state, member, pack_key).await?;
    if !concepts.iter().any(|c| c.id == id) {
        return Err(not_found());
    }
    let concept = build_new_concept(request, &concepts, Some(id))?;
    let updated = state
        .store
        .update_domain_concept(&member.workspace, pack.id, id, &concept, member.user.id)
        .await?;
    if !updated {
        return Err(not_found());
    }
    load_concept(state, member, id).await
}

/// Gói có khóa này của workspace kèm các khái niệm đang bật, cho prompt sinh SQL và form câu "tại sao"
/// (service/ask/mod.rs). None khi không có gói đó, ví dụ sources.domain trỏ tới gói đã bỏ khỏi data/domains/.
pub async fn load_for_prompt(
    state: &AppState,
    ws: &WorkspaceId,
    key: &str,
) -> Result<Option<DomainPackage>, AppError> {
    ensure_seeded(state, ws).await?;
    state.store.load_domain_package(ws, key).await
}

/// Nạp các gói và khái niệm của file mà workspace chưa có. Thường chỉ là một câu SELECT: ghi khi lần đầu, hay khi bản
/// mới thêm gói hay khái niệm vào data/domains/.
async fn ensure_seeded(state: &AppState, ws: &WorkspaceId) -> Result<(), AppError> {
    let present = state.store.list_domain_seed_keys(ws).await?;
    let files = list_domain_packages();
    if is_seeded(files, &present) {
        return Ok(());
    }
    state.store.seed_domain_packages(ws, files).await?;
    tracing::info!(workspace = %ws.as_uuid(), "domain packages seeded");
    Ok(())
}

/// true khi mọi khái niệm của mọi file đã có trong workspace (theo cặp khóa gói, khóa khái niệm).
fn is_seeded(files: &[DomainPackage], present: &[(String, Option<String>)]) -> bool {
    let present: HashSet<(&str, Option<&str>)> = present
        .iter()
        .map(|(pack, concept)| (pack.as_str(), concept.as_deref()))
        .collect();
    files.iter().all(|package| {
        package
            .concepts
            .iter()
            .all(|c| present.contains(&(package.id.as_str(), Some(c.id.as_str()))))
    })
}

/// Gói có khóa này kèm mọi khái niệm của nó (để kiểm trùng); 404 khi workspace không có gói đó.
async fn load_pack(
    state: &AppState,
    member: &Member,
    pack_key: &str,
) -> Result<(DomainPackRecord, Vec<DomainConceptRecord>), AppError> {
    let ws = &member.workspace;
    ensure_seeded(state, ws).await?;
    let pack = state
        .store
        .find_domain_pack(ws, pack_key.trim())
        .await?
        .ok_or_else(|| AppError::NotFound("Không có gói khái niệm này".to_string()))?;
    let concepts = state
        .store
        .list_domain_concepts(ws)
        .await?
        .into_iter()
        .filter(|c| c.pack_id == pack.id)
        .collect();
    Ok((pack, concepts))
}

async fn load_concept(state: &AppState, member: &Member, id: Uuid) -> Result<Concept, AppError> {
    state
        .store
        .list_domain_concepts(&member.workspace)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .map(Concept::from)
        .ok_or_else(|| AppError::Internal("saved domain concept is not in the list".to_string()))
}

/// Ghép khái niệm vào gói của nó, giữ thứ tự store trả về (thứ tự của prompt).
fn build_packs(
    packs: Vec<DomainPackRecord>,
    concepts: Vec<DomainConceptRecord>,
) -> Vec<ConceptPack> {
    packs
        .into_iter()
        .map(|pack| ConceptPack {
            concepts: concepts
                .iter()
                .filter(|c| c.pack_id == pack.id)
                .cloned()
                .map(Concept::from)
                .collect(),
            key: pack.key,
            name: pack.name,
            description: pack.description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present(pairs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        pairs
            .iter()
            .map(|(p, c)| (p.to_string(), c.map(str::to_string)))
            .collect()
    }

    #[test]
    fn seeding_is_needed_until_every_file_concept_is_present() {
        let files = list_domain_packages();
        assert!(!is_seeded(files, &[]));
        let all: Vec<(String, Option<String>)> = files
            .iter()
            .flat_map(|p| {
                p.concepts
                    .iter()
                    .map(|c| (p.id.clone(), Some(c.id.clone())))
            })
            .collect();
        assert!(is_seeded(files, &all));
        // Một khái niệm mới trong file (bản sau thêm vào) thì phải nạp lại.
        let missing_one = &all[1..];
        assert!(!is_seeded(files, missing_one));
        // Gói không có trong file (gói đã bỏ khỏi data/domains/) không làm sai kết quả.
        let mut extra = all.clone();
        extra.extend(present(&[("removed_pack", None)]));
        assert!(is_seeded(files, &extra));
    }
}
