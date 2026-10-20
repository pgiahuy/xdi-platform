// SPDX-License-Identifier: Apache-2.0
/* engine/store.rs - kết nối Postgres và chạy migration.
Migration nằm ở backend/migrations/ và được nhúng vào binary lúc biên dịch (sqlx::migrate!), nên build không cần
Postgres và mọi bản cài đều có cùng schema. Các hàm truy vấn theo workspace thêm ở issue tầng lưu trữ. */

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::error::AppError;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Mở kết nối thật, báo lỗi ngay nếu Postgres không với tới được.
    pub async fn connect(database_url: &str) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
            .map_err(|e| AppError::Internal(format!("connect to postgres: {e}")))?;
        Ok(Self { pool })
    }

    /// Tạo pool chưa kết nối; dùng trong test không cần Postgres.
    #[cfg(test)]
    pub fn lazy(database_url: &str) -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy(database_url)
            .expect("invalid database url");
        Self { pool }
    }

    /// Áp các migration chưa chạy. Chạy lại nhiều lần không sao: sqlx ghi lại migration đã áp.
    pub async fn migrate(&self) -> Result<(), AppError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AppError::Internal(format!("run migrations: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cần Postgres thật. CI đặt DATABASE_URL; trên máy, chạy `docker compose up -d postgres` rồi đặt biến này.
    // Trên CI (biến CI do GitHub Actions đặt) mà thiếu DATABASE_URL thì báo lỗi, để test không lặng lẽ bị bỏ qua.
    fn database_url() -> Option<String> {
        match std::env::var("DATABASE_URL") {
            Ok(url) => Some(url),
            Err(_) if std::env::var("CI").is_ok() => panic!("DATABASE_URL must be set on CI"),
            Err(_) => None,
        }
    }

    #[tokio::test]
    async fn migrations_apply_and_rerun() {
        let Some(url) = database_url() else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let store = Store::connect(&url).await.expect("connect");
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
        let store = Store::connect(&url).await.expect("connect");
        store.migrate().await.expect("migrate");
        let missing: Vec<String> = sqlx::query_scalar(
            "SELECT t.table_name::text FROM information_schema.tables t
             WHERE t.table_schema = 'public'
               AND t.table_name NOT IN ('workspaces', 'users', '_sqlx_migrations')
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
        let store = Store::connect(&url).await.expect("connect");
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
        let store = Store::connect(&url).await.expect("connect");
        store.migrate().await.expect("migrate");

        // Phiên bản hiện hành không xóa được; phiên bản khác thì được.
        let mut tx = store.pool.begin().await.expect("begin");
        let (_, _, v1, v2) = seed(&mut tx).await;
        sqlx::query("SAVEPOINT s").execute(&mut *tx).await.unwrap();
        let blocked = sqlx::query("DELETE FROM dataset_versions WHERE id = $1")
            .bind(v2)
            .execute(&mut *tx)
            .await;
        assert!(blocked.is_err(), "deleting the current version must fail");
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
        let store = Store::connect(&url).await.expect("connect");
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
        assert!(
            result.is_err(),
            "a query must not point to another dataset's version"
        );
        tx.rollback().await.unwrap();
    }
}
