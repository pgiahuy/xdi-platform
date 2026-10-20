// SPDX-License-Identifier: Apache-2.0
/* engine/store/mod.rs - tầng lưu trữ Postgres: kết nối, migration, và các hàm truy vấn chia theo bảng.

Migration nằm ở backend/migrations/ và được nhúng vào binary lúc biên dịch (sqlx::migrate!), nên build không cần
Postgres. Câu SQL viết dạng chuỗi với sqlx::query_as, không dùng macro query! kiểm lúc biên dịch: đỡ phải có
thư mục .sqlx/ và sqlx-cli; đổi lại, mọi câu SQL được kiểm bằng test chạy trên Postgres thật trong CI.

Mọi hàm chạm bảng nghiệp vụ nhận &WorkspaceId và lọc theo nó. Ngoại lệ có chủ đích: tạo và tìm workspace (workspaces.rs),
vì lúc đó chưa có workspace để lọc. */

// Khung: các hàm truy vấn dưới đây được gọi từ issue nạp dữ liệu (#20), endpoint hỏi (#33) và chế độ xác thực (#42).
// Chưa có code gọi nên tạm cho phép "chưa dùng" ở đúng ba module này; gỡ khi có người gọi để clippy bắt lại.
#[allow(dead_code)]
mod datasets;
#[allow(dead_code)]
mod queries;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod test_support;
#[allow(dead_code)]
mod workspaces;

#[allow(unused_imports)]
pub use datasets::{Dataset, DatasetVersion, NewVersion, SourceKind};
#[allow(unused_imports)]
pub use queries::{NewQuery, QueryStatus};
#[allow(unused_imports)]
pub use workspaces::WorkspaceKind;

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::error::AppError;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Mở kết nối thật, báo lỗi ngay nếu Postgres không với tới được.
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
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

/// Đổi lỗi sqlx thành AppError. Không tìm thấy dòng là lỗi của người gọi, còn lại là lỗi nội bộ.
fn db_error(context: &str, e: sqlx::Error) -> AppError {
    match e {
        sqlx::Error::RowNotFound => AppError::NotFound(format!("Không tìm thấy: {context}")),
        other => AppError::Internal(format!("{context}: {other}")),
    }
}
