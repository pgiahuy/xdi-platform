// SPDX-License-Identifier: Apache-2.0
/* engine/store/mod.rs - tầng lưu trữ Postgres: kết nối, migration, và các hàm truy vấn chia theo bảng.

Migration nằm ở backend/migrations/ và được nhúng vào binary lúc biên dịch (sqlx::migrate!), nên build không cần
Postgres. Câu SQL viết dạng chuỗi với sqlx::query_as, không dùng macro query! kiểm lúc biên dịch: đỡ phải có
thư mục .sqlx/ và sqlx-cli; đổi lại, mọi câu SQL được kiểm bằng test chạy trên Postgres thật trong CI.

Mọi hàm chạm bảng nghiệp vụ nhận &WorkspaceId và lọc theo nó. Ngoại lệ có chủ đích: tạo và tìm workspace (workspaces.rs),
vì lúc đó chưa có workspace để lọc; tài khoản, phiên (accounts.rs), vì chúng dùng chung cho mọi workspace; và danh sách
phiên bản chờ lập hồ sơ lúc khởi động (datasets.rs, list_pending_versions). */

mod accounts;
mod aggregation;
mod columns;
mod conversations;
mod members;
// Phần ghi (set_data_role, list_row_policies, replace_row_policies) chờ service và API của #45 gọi; gỡ allow khi có
// người gọi để clippy bắt lại.
#[allow(dead_code)]
mod policies;
#[cfg(test)]
mod policies_tests;
mod users;
// Khung: các hàm truy vấn dưới đây được gọi từ issue nạp dữ liệu (#20), endpoint hỏi (#33) và chế độ xác thực (#42).
// Chưa có code gọi nên tạm cho phép "chưa dùng" ở đúng ba module này; gỡ khi có người gọi để clippy bắt lại.
#[allow(dead_code)]
mod datasets;
mod metric_edits;
mod metrics;
mod queries;
mod relations;
#[cfg(test)]
mod relations_tests;
#[cfg(test)]
mod schema_tests;
mod sources;
#[cfg(test)]
pub mod test_support;
#[allow(dead_code)]
mod workspaces;

pub use accounts::{Membership, NewAdmin, User};
pub use aggregation::ColumnAggregation;
pub use columns::{
    ColumnUnit, ColumnValue, NewColumn, NewColumnValue, ProfileColumn, ProfileRole,
    SemanticColumnUpdate, SemanticValueUpdate, ValueRow,
};
pub use conversations::{Conversation, StoredTurn};
#[allow(unused_imports)]
pub use datasets::{
    Dataset, DatasetVersion, NewVersion, PendingVersion, ProfileStatus, SemanticCandidate,
    SourceKind,
};
pub use members::{ChangeOutcome, MemberChange, MemberRow, WorkspaceOverview};
#[allow(unused_imports)]
pub use metric_edits::MetricRecord;
#[allow(unused_imports)]
pub use metrics::{MetricDefinition, NewMetric};
#[allow(unused_imports)]
pub use policies::{DataRoleOutcome, PoliciesOutcome, RowPolicy};
#[allow(unused_imports)]
pub use queries::{NewQuery, PromptExample, QueryStatus, StoredQuery};
pub use relations::{build_related, DatasetRelation, NewRelation};
pub use sources::SourceRow;
pub use users::{Deactivated, UserSummary};
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
    /// Mở kết nối thật, báo lỗi ngay nếu Postgres không với tới được. acquire_timeout: thời gian tối đa một request chờ
    /// lấy được kết nối (kể cả lần kết nối đầu); quá thì request nhận lỗi thay vì treo.
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
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
/// context là tiếng Anh, chỉ đi vào log; thông báo trả người dùng là tiếng Việt (AGENTS.md mục 1).
fn db_error(context: &str, e: sqlx::Error) -> AppError {
    match e {
        sqlx::Error::RowNotFound => {
            tracing::debug!(context, "row not found");
            AppError::NotFound("Không tìm thấy dữ liệu".to_string())
        }
        other => AppError::Internal(format!("{context}: {other}")),
    }
}

/// Lỗi vi phạm khóa ngoại (mã 23503 của Postgres): một id do người gọi đưa vào không trỏ tới dòng hợp lệ.
fn is_foreign_key_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23503"))
}

/// Câu lệnh bị Postgres hủy (mã 57014), ví dụ vì quá statement_timeout.
fn is_query_canceled(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("57014"))
}
