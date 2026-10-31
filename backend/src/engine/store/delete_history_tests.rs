// SPDX-License-Identifier: Apache-2.0
/* engine/store/delete_history_tests.rs - xóa dataset thì lịch sử câu hỏi của nó mất theo (#187, migration
0011_delete_query_history.sql): các dòng queries, kể cả các lượt của cuộc trò chuyện, và cuộc trò chuyện. Lịch sử của
dataset khác cùng workspace giữ nguyên. Đi qua đúng hàm Store::delete_dataset mà API gọi. Không có DATABASE_URL thì
bỏ qua. */

use uuid::Uuid;

use super::test_support::{drop_workspace, fetch_count, store, temp_workspace};
use super::{NewQuery, NewVersion, QueryStatus, SourceKind, Store};
use crate::auth::WorkspaceId;

/// Một dataset có một phiên bản, một cuộc trò chuyện hai lượt và một câu hỏi ngoài cuộc trò chuyện.
/// Trả (id dataset, id cuộc trò chuyện).
async fn dataset_with_history(
    store: &Store,
    ws: &WorkspaceId,
    user: Uuid,
    name: &str,
) -> (Uuid, Uuid) {
    let source = store.create_local_source(ws, "Test").await.unwrap();
    let ds = store
        .create_dataset(ws, source, name, name, None)
        .await
        .unwrap();
    let version = store
        .create_version(
            ws,
            ds.id,
            &NewVersion {
                source_kind: SourceKind::Csv,
                file_hash: name,
                parquet_path: "x.parquet",
                row_count: 1,
                description: None,
            },
            &[],
        )
        .await
        .unwrap();
    let conversation = store
        .create_conversation(ws, ds.id, user, "revenue")
        .await
        .unwrap();
    for (question, in_conversation) in [
        ("first turn", true),
        ("second turn", true),
        ("alone", false),
    ] {
        let id = store
            .record_query(
                ws,
                &NewQuery {
                    target: Some((ds.id, version.id)),
                    user_id: Some(user),
                    question,
                    sql_generated: Some("SELECT 1"),
                    sql_guarded: Some("SELECT 1 LIMIT 10000"),
                    status: QueryStatus::Ok,
                    error: None,
                    timings: serde_json::json!({}),
                    row_count: Some(1),
                    explanation: Some(serde_json::json!({"segments": ["Europe"]})),
                    read_versions: vec![(ds.id, version.id)],
                },
            )
            .await
            .unwrap();
        if in_conversation {
            store
                .attach_turn(ws, id, conversation, Some("revenue by market"), None)
                .await
                .unwrap();
        }
    }
    (ds.id, conversation)
}

#[tokio::test]
async fn deleting_a_dataset_deletes_its_question_history() {
    let Some(store) = store().await else {
        return;
    };
    let ws = temp_workspace(&store).await;
    let user: Uuid = sqlx::query_scalar("INSERT INTO users (email) VALUES ($1) RETURNING id")
        .bind(format!("{}@example.com", Uuid::new_v4()))
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let (deleted, deleted_conversation) = dataset_with_history(&store, &ws, user, "sales").await;
    let (kept, kept_conversation) = dataset_with_history(&store, &ws, user, "orders").await;

    store.delete_dataset(&ws, deleted).await.unwrap();

    let ws_id = ws.as_uuid();
    // Không còn dòng nào trỏ tới dataset đã xóa, cũng không còn dòng mồ côi (dataset_id null) như trước #187.
    assert_eq!(
        fetch_count(
            &store,
            &format!("SELECT count(*) FROM queries WHERE dataset_id = '{deleted}'")
        )
        .await,
        0
    );
    assert_eq!(
        fetch_count(
            &store,
            &format!(
            "SELECT count(*) FROM queries WHERE workspace_id = '{ws_id}' AND dataset_id IS NULL"
        )
        )
        .await,
        0
    );
    assert_eq!(
        fetch_count(
            &store,
            &format!(
                "SELECT count(*) FROM queries WHERE conversation_id = '{deleted_conversation}'"
            )
        )
        .await,
        0
    );
    assert!(store
        .get_conversation(&ws, deleted_conversation)
        .await
        .unwrap()
        .is_none());

    // Dataset kia giữ đủ ba câu hỏi, cuộc trò chuyện và hai lượt của nó.
    assert_eq!(
        fetch_count(
            &store,
            &format!("SELECT count(*) FROM queries WHERE dataset_id = '{kept}'")
        )
        .await,
        3
    );
    assert!(store
        .get_conversation(&ws, kept_conversation)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        store
            .list_turns(&ws, kept_conversation, 10)
            .await
            .unwrap()
            .len(),
        2
    );
    // Cả workspace chỉ còn đúng ba câu hỏi của dataset kia.
    assert_eq!(
        fetch_count(
            &store,
            &format!("SELECT count(*) FROM queries WHERE workspace_id = '{ws_id}'")
        )
        .await,
        3
    );

    drop_workspace(&store, ws).await;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user)
        .execute(&store.pool)
        .await
        .unwrap();
}
