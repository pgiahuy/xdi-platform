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
}
