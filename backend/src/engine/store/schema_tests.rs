// SPDX-License-Identifier: Apache-2.0
/* engine/store/schema_tests.rs - test cho chính schema: migration chạy lại được, cách ly workspace bằng khóa ngoại,
quy tắc xóa. Chạy với Postgres thật; mỗi test làm trong transaction rồi rollback. */

use super::test_support::database_url;
use super::*;

#[tokio::test]
async fn migrations_apply_and_rerun() {
    let Some(url) = database_url() else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("first run");
    store.migrate().await.expect("second run");

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables WHERE table_schema = 'public' ORDER BY 1",
    )
    .fetch_all(&store.pool)
    .await
    .expect("list tables");
    for t in [
        "workspaces",
        "users",
        "memberships",
        "sources",
        "datasets",
        "dataset_versions",
        "columns",
        "column_values",
        "metrics",
        "rls_policies",
        "queries",
        "settings",
    ] {
        assert!(tables.iter().any(|x| x == t), "missing table {t}");
    }
}

#[tokio::test]
async fn every_business_table_has_workspace_id() {
    let Some(url) = database_url() else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    // Ngoại lệ có chủ đích: workspaces là chính nó; users và sessions thuộc về người dùng, dùng chung mọi workspace.
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT t.table_name::text FROM information_schema.tables t
         WHERE t.table_schema = 'public'
           AND t.table_name NOT IN ('workspaces', 'users', 'sessions', '_sqlx_migrations')
           AND NOT EXISTS (SELECT 1 FROM information_schema.columns c
                           WHERE c.table_name = t.table_name AND c.column_name = 'workspace_id')",
    )
    .fetch_all(&store.pool)
    .await
    .expect("query");
    assert!(
        missing.is_empty(),
        "tables without workspace_id: {missing:?}"
    );
}

#[tokio::test]
async fn cross_workspace_reference_is_rejected() {
    let Some(url) = database_url() else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");

    // Mọi thứ trong một transaction rồi rollback, để test không để lại dữ liệu và chạy song song được.
    let mut tx = store.pool.begin().await.expect("begin");
    let ws_a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO workspaces (name, slug) VALUES ('A', 'test-a-' || gen_random_uuid()) RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("workspace a");
    let ws_b: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO workspaces (name, slug) VALUES ('B', 'test-b-' || gen_random_uuid()) RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("workspace b");
    let source_a: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO sources (workspace_id, kind) VALUES ($1, 'local_file') RETURNING id",
    )
    .bind(ws_a)
    .fetch_one(&mut *tx)
    .await
    .expect("source in a");

    // Dataset của workspace B trỏ tới nguồn của workspace A: database phải từ chối.
    let result = sqlx::query(
        "INSERT INTO datasets (workspace_id, source_id, name, logical_table) VALUES ($1, $2, 'x', 'x')",
    )
    .bind(ws_b)
    .bind(source_a)
    .execute(&mut *tx)
    .await;
    let err = result.expect_err("cross-workspace reference must fail");
    assert!(
        err.to_string().contains("foreign key"),
        "unexpected error: {err}"
    );
    tx.rollback().await.expect("rollback");
}

/// Tạo một workspace có nguồn, một dataset với hai phiên bản (phiên bản 2 là hiện hành) và một câu hỏi.
/// Trả về id của workspace, dataset, phiên bản 1, phiên bản 2.
async fn seed(tx: &mut sqlx::PgConnection) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let ws: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO workspaces (name, slug) VALUES ('W', 'test-' || left(gen_random_uuid()::text, 8)) RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("workspace");
    let src: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO sources (workspace_id, kind) VALUES ($1, 'local_file') RETURNING id",
    )
    .bind(ws)
    .fetch_one(&mut *tx)
    .await
    .expect("source");
    let ds: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO datasets (workspace_id, source_id, name, logical_table) VALUES ($1, $2, 'Sales', 'sales') RETURNING id",
    )
    .bind(ws)
    .bind(src)
    .fetch_one(&mut *tx)
    .await
    .expect("dataset");
    let mut versions = Vec::new();
    for n in [1, 2] {
        let v: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO dataset_versions (workspace_id, dataset_id, version, source_kind) VALUES ($1, $2, $3, 'csv') RETURNING id",
        )
        .bind(ws)
        .bind(ds)
        .bind(n)
        .fetch_one(&mut *tx)
        .await
        .expect("version");
        versions.push(v);
    }
    sqlx::query("UPDATE datasets SET current_version_id = $1 WHERE id = $2")
        .bind(versions[1])
        .bind(ds)
        .execute(&mut *tx)
        .await
        .expect("set current");
    sqlx::query(
        "INSERT INTO queries (workspace_id, dataset_id, version_id, question, status) VALUES ($1, $2, $3, 'q', 'ok')",
    )
    .bind(ws)
    .bind(ds)
    .bind(versions[1])
    .execute(&mut *tx)
    .await
    .expect("query");
    (ws, ds, versions[0], versions[1])
}

#[tokio::test]
async fn delete_rules_keep_data_consistent() {
    let Some(url) = database_url() else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");

    // Phiên bản hiện hành không xóa được; phiên bản khác thì được.
    let mut tx = store.pool.begin().await.expect("begin");
    let (_, _, v1, v2) = seed(&mut tx).await;
    sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
    let blocked = sqlx::query("DELETE FROM dataset_versions WHERE id = $1")
        .bind(v2)
        .execute(&mut *tx)
        .await;
    let err = blocked.expect_err("deleting the current version must fail");
    assert!(
        err.to_string().contains("foreign key"),
        "unexpected error: {err}"
    );
    sqlx::query("ROLLBACK TO SAVEPOINT s")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM dataset_versions WHERE id = $1")
        .bind(v1)
        .execute(&mut *tx)
        .await
        .expect("deleting a non-current version must succeed");
    tx.rollback().await.unwrap();

    // Xóa dataset: lịch sử câu hỏi còn, cả hai cột trỏ tới đều null, workspace_id giữ nguyên.
    let mut tx = store.pool.begin().await.expect("begin");
    let (ws, ds, _, _) = seed(&mut tx).await;
    sqlx::query("DELETE FROM datasets WHERE id = $1")
        .bind(ds)
        .execute(&mut *tx)
        .await
        .expect("delete dataset");
    let kept: (i64, bool) = sqlx::query_as(
        "SELECT count(*), bool_and(dataset_id IS NULL AND version_id IS NULL) FROM queries WHERE workspace_id = $1",
    )
    .bind(ws)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(kept, (1, true));
    tx.rollback().await.unwrap();

    // Xóa workspace: mọi thứ của nó mất theo, kể cả nguồn đang có dataset.
    let mut tx = store.pool.begin().await.expect("begin");
    let (ws, _, _, _) = seed(&mut tx).await;
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws)
        .execute(&mut *tx)
        .await
        .expect("delete workspace");
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM sources WHERE workspace_id = $1)
              + (SELECT count(*) FROM datasets WHERE workspace_id = $1)
              + (SELECT count(*) FROM queries WHERE workspace_id = $1)",
    )
    .bind(ws)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(left, 0);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn query_version_must_belong_to_its_dataset() {
    let Some(url) = database_url() else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    let mut tx = store.pool.begin().await.expect("begin");
    let (ws, _, _, v2) = seed(&mut tx).await;
    let src: uuid::Uuid = sqlx::query_scalar("SELECT id FROM sources WHERE workspace_id = $1")
        .bind(ws)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let other: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO datasets (workspace_id, source_id, name, logical_table) VALUES ($1, $2, 'Other', 'other') RETURNING id",
    )
    .bind(ws)
    .bind(src)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let result = sqlx::query(
        "INSERT INTO queries (workspace_id, dataset_id, version_id, question, status) VALUES ($1, $2, $3, 'q', 'ok')",
    )
    .bind(ws)
    .bind(other)
    .bind(v2)
    .execute(&mut *tx)
    .await;
    // Kiểm đúng lỗi khóa ngoại, để lỗi khác (cú pháp, kết nối) không làm test xanh nhầm.
    let err = result.expect_err("a query must not point to another dataset's version");
    assert!(
        err.to_string().contains("foreign key"),
        "unexpected error: {err}"
    );
    tx.rollback().await.unwrap();
}
