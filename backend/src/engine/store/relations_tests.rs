// SPDX-License-Identifier: Apache-2.0
/* engine/store/relations_tests.rs - test engine/store/relations.rs trên Postgres thật: thay quan hệ của một nhóm, đọc cả
hai chiều, xóa dataset thì quan hệ mất theo, không nối được sang dataset của workspace khác. Không có DATABASE_URL thì
bỏ qua. */

use uuid::Uuid;

use super::test_support::{drop_workspace, store, temp_workspace};
use super::{build_related, NewRelation, Store};
use crate::auth::WorkspaceId;
use crate::error::AppError;

async fn dataset(store: &Store, ws: &WorkspaceId, name: &str) -> Uuid {
    let source = store.create_local_source(ws).await.unwrap();
    store
        .create_dataset(ws, source, name, name, None)
        .await
        .unwrap()
        .id
}

fn relation(from: Uuid, to: Uuid, column: &str, constraint: &str) -> NewRelation {
    NewRelation {
        from_dataset_id: from,
        from_columns: vec![column.to_string()],
        to_dataset_id: to,
        to_columns: vec!["id".to_string()],
        constraint_name: constraint.to_string(),
        cardinality: "many_to_one",
    }
}

/// order_items -> orders -> customers; thay nhóm thì quan hệ cũ trong nhóm mất, quan hệ với dataset ngoài nhóm giữ.
#[tokio::test]
async fn replace_group_and_list_both_directions() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    let items = dataset(&store, &ws, "order_items").await;
    let orders = dataset(&store, &ws, "orders").await;
    let customers = dataset(&store, &ws, "customers").await;
    let returns = dataset(&store, &ws, "returns").await;
    let group = [items, orders, customers];
    let first = [
        relation(items, orders, "order_id", "items_order_fkey"),
        relation(orders, customers, "customer_id", "orders_customer_fkey"),
    ];
    store.replace_relations(&ws, &group, &first).await.unwrap();
    // returns chụp ở nhóm khác, trỏ vào orders.
    let outside = [relation(returns, orders, "order_id", "returns_order_fkey")];
    store
        .replace_relations(&ws, &[returns, orders], &outside)
        .await
        .unwrap();

    let around = store.list_relations(&ws, Some(orders)).await.unwrap();
    assert_eq!(around.len(), 3, "{around:?}");
    let related = build_related(orders, &around);
    let directions: Vec<(&str, &str)> = related
        .iter()
        .map(|r| (r.name.as_str(), r.direction.as_str()))
        .collect();
    assert_eq!(
        directions,
        [
            ("order_items", "one_to_many"),
            ("customers", "many_to_one"),
            ("returns", "one_to_many")
        ]
    );
    assert_eq!(related[1].columns, ["customer_id"]);
    assert_eq!(related[1].related_columns, ["id"]);

    // Lần chụp lại thấy khóa orders -> customers đã bỏ: chỉ còn một quan hệ trong nhóm, quan hệ của returns giữ nguyên.
    let second = [relation(items, orders, "order_id", "items_order_fkey")];
    store.replace_relations(&ws, &group, &second).await.unwrap();
    let all = store.list_relations(&ws, None).await.unwrap();
    let names: Vec<&str> = all.iter().map(|r| r.constraint_name.as_str()).collect();
    assert_eq!(names, ["items_order_fkey", "returns_order_fkey"]);
    drop_workspace(&store, ws).await;
}

/// Xóa dataset phía một thì mọi quan hệ trỏ vào nó mất theo (ON DELETE CASCADE).
#[tokio::test]
async fn deleting_a_dataset_removes_its_relations() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    let items = dataset(&store, &ws, "order_items").await;
    let orders = dataset(&store, &ws, "orders").await;
    let rel = [relation(items, orders, "order_id", "items_order_fkey")];
    store
        .replace_relations(&ws, &[items, orders], &rel)
        .await
        .unwrap();
    store.delete_dataset(&ws, orders).await.unwrap();
    assert!(store.list_relations(&ws, None).await.unwrap().is_empty());
    drop_workspace(&store, ws).await;
}

/// Quan hệ trỏ sang dataset của workspace khác: khóa ngoại theo cặp từ chối, không ghi gì.
#[tokio::test]
async fn relation_to_another_workspace_is_rejected() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    let other = temp_workspace(&store).await;
    let items = dataset(&store, &ws, "order_items").await;
    let foreign = dataset(&store, &other, "orders").await;
    let rel = [relation(items, foreign, "order_id", "items_order_fkey")];
    let err = store
        .replace_relations(&ws, &[items, foreign], &rel)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    assert!(store.list_relations(&ws, None).await.unwrap().is_empty());
    assert!(store.list_relations(&other, None).await.unwrap().is_empty());
    drop_workspace(&store, ws).await;
    drop_workspace(&store, other).await;
}
