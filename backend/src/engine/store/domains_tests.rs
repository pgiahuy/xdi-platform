// SPDX-License-Identifier: Apache-2.0
/* engine/store/domains_tests.rs - test engine/store/domains.rs trên Postgres thật: nạp khởi tạo chạy lại không đổi gì
và không ghi đè bản đã sửa, gói đọc từ bảng của workspace chưa sửa giống hệt gói trong file, khái niệm tắt không vào
gói của prompt, workspace khác không thấy gì. Không có DATABASE_URL thì bỏ qua. */

use super::test_support::{drop_workspace, fetch_count, store, temp_workspace};
use super::NewDomainConcept;
use crate::engine::domains::list_domain_packages;

fn edited(name: &str, enabled: bool) -> NewDomainConcept {
    NewDomainConcept {
        name: name.to_string(),
        description: "Edited description".to_string(),
        synonyms: vec!["edited synonym".to_string()],
        calculations: Vec::new(),
        dimensions: Vec::new(),
        sanity_checks: Vec::new(),
        enabled,
    }
}

#[tokio::test]
async fn seeding_is_idempotent_and_matches_the_files() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    let other = temp_workspace(&store).await;
    let files = list_domain_packages();
    store.seed_domain_packages(&ws, files).await.unwrap();
    let count_sql = format!(
        "SELECT count(*) FROM domain_concepts WHERE workspace_id = '{}'",
        ws.as_uuid()
    );
    let first = fetch_count(&store, &count_sql).await;
    let expected: usize = files.iter().map(|p| p.concepts.len()).sum();
    assert_eq!(first, expected as i64);
    store.seed_domain_packages(&ws, files).await.unwrap();
    assert_eq!(fetch_count(&store, &count_sql).await, first);

    // Workspace chưa sửa gì: gói đọc từ bảng giống hệt file, cả thứ tự khái niệm, nên prompt không đổi một chữ.
    for file in files {
        let loaded = store.load_domain_package(&ws, &file.id).await.unwrap();
        assert_eq!(loaded.as_ref(), Some(file));
    }
    assert!(store
        .load_domain_package(&other, "retail")
        .await
        .unwrap()
        .is_none());
    assert!(store.list_domain_concepts(&other).await.unwrap().is_empty());

    drop_workspace(&store, ws).await;
    drop_workspace(&store, other).await;
}

#[tokio::test]
async fn seeding_keeps_edits_and_disabled_concepts_leave_the_prompt() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    // Người sửa phải là tài khoản có thật (khóa ngoại updated_by); email riêng mỗi lần chạy vì bảng users dùng chung.
    let email = format!("domains-{}@example.com", uuid::Uuid::new_v4().simple());
    let user = store
        .create_user(&email, "Test", "unused")
        .await
        .unwrap()
        .id;
    let files = list_domain_packages();
    store.seed_domain_packages(&ws, files).await.unwrap();
    let pack = store
        .find_domain_pack(&ws, "retail")
        .await
        .unwrap()
        .unwrap();
    let concepts = store.list_domain_concepts(&ws).await.unwrap();
    let revenue = concepts
        .iter()
        .find(|c| c.key.as_deref() == Some("revenue"))
        .unwrap();
    let orders = concepts
        .iter()
        .find(|c| c.key.as_deref() == Some("order_count"))
        .unwrap();
    let updated = store
        .update_domain_concept(&ws, pack.id, revenue.id, &edited("Doanh số", true), user)
        .await
        .unwrap();
    assert!(updated);
    store
        .update_domain_concept(&ws, pack.id, orders.id, &edited("Số đơn", false), user)
        .await
        .unwrap();
    store
        .add_domain_concept(&ws, pack.id, &edited("Khách quay lại", true), user)
        .await
        .unwrap();

    store.seed_domain_packages(&ws, files).await.unwrap();
    let loaded = store
        .load_domain_package(&ws, "retail")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.concepts[0].name, "Doanh số");
    assert_eq!(loaded.concepts[0].description, "Edited description");
    assert!(loaded.concepts.iter().all(|c| c.name != "Số đơn"));
    assert_eq!(loaded.concepts.last().unwrap().name, "Khách quay lại");
    let retail = files.iter().find(|p| p.id == "retail").unwrap();
    assert_eq!(loaded.concepts.len(), retail.concepts.len());

    drop_workspace(&store, ws).await;
}
