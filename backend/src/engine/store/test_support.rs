// SPDX-License-Identifier: Apache-2.0
/* engine/store/test_support.rs - phần dùng chung cho test cần Postgres thật. */

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use super::Store;
use crate::auth::WorkspaceId;

/// CI đặt DATABASE_URL; trên máy, chạy `docker compose up -d postgres` rồi đặt biến này.
/// Trên CI (biến CI do GitHub Actions đặt) mà thiếu DATABASE_URL thì báo lỗi, để test không lặng lẽ bị bỏ qua.
pub fn database_url() -> Option<String> {
    match std::env::var("DATABASE_URL") {
        Ok(url) => Some(url),
        Err(_) if std::env::var("CI").is_ok() => panic!("DATABASE_URL must be set on CI"),
        Err(_) => None,
    }
}

/// Store đã migrate, hoặc None nếu không có Postgres (test tự bỏ qua).
pub async fn store() -> Option<Store> {
    let url = database_url()?;
    let store = Store::connect(&url, 5, Duration::from_secs(5))
        .await
        .expect("connect");
    store.migrate().await.expect("migrate");
    Some(store)
}

/// Một workspace riêng cho từng test. Test xóa nó ở cuối; khóa ngoại xóa theo mọi dữ liệu bên trong.
/// Nhờ vậy các test chạy song song không đụng nhau. Test thất bại giữa chừng thì không tới được bước xóa; Rust không
/// có hàm hủy bất đồng bộ, nên thay vào đó mỗi lần tạo workspace mới thì dọn các workspace test cũ hơn 10 phút.
pub async fn temp_workspace(store: &Store) -> WorkspaceId {
    sqlx::query(
        "DELETE FROM workspaces WHERE slug LIKE 'test-%' AND created_at < now() - interval '10 minutes'",
    )
    .execute(&store.pool)
    .await
    .expect("clean stale test workspaces");
    let slug = format!("test-{}", &Uuid::new_v4().simple().to_string()[..12]);
    let id = store
        .create_workspace("Test", &slug, super::WorkspaceKind::Team)
        .await
        .expect("create workspace");
    WorkspaceId::for_test(id)
}

pub async fn drop_workspace(store: &Store, ws: WorkspaceId) {
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws.as_uuid())
        .execute(&store.pool)
        .await
        .expect("drop workspace");
}

/// Một database Postgres mới tinh cho một test, đã migrate. Dùng khi test cần trạng thái "chưa có tài khoản nào"
/// (thiết lập lần đầu), điều không có được trên database dùng chung với các test khác.
pub struct TestDb {
    pub store: Store,
    name: String,
    admin: Store,
}

pub async fn fresh_database() -> Option<TestDb> {
    let db = empty_database().await?;
    db.store.migrate().await.expect("migrate test database");
    Some(db)
}

/// Như fresh_database nhưng chưa chạy migration nào: test migration (migration_tests.rs) tự áp từng file.
pub async fn empty_database() -> Option<TestDb> {
    let url = database_url()?;
    let admin = Store::connect(&url, 2, Duration::from_secs(5))
        .await
        .expect("connect");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();

    // Tên mang thời điểm tạo, để dọn được database của các test chết giữa chừng (cũ hơn 10 phút).
    let names: Vec<String> =
        sqlx::query_scalar("SELECT datname::text FROM pg_database WHERE datname LIKE 'xdi_test_%'")
            .fetch_all(&admin.pool)
            .await
            .expect("list test databases");
    for old in names {
        let created: u64 = old
            .split('_')
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if created + 600 < now {
            drop_database(&admin, &old).await;
        }
    }

    let name = format!(
        "xdi_test_{now}_{}",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    sqlx::query(&format!("CREATE DATABASE {name}"))
        .execute(&admin.pool)
        .await
        .expect("create test database");
    let (base, _) = url
        .split('?')
        .next()
        .unwrap_or(&url)
        .rsplit_once('/')
        .expect("database url has a path");
    let store = Store::connect(&format!("{base}/{name}"), 5, Duration::from_secs(5))
        .await
        .expect("connect test database");
    Some(TestDb { store, name, admin })
}

impl TestDb {
    /// Tên database của test, để test nguồn Postgres (api/sources_tests.rs) kết nối vào chính nó như một database ngoài.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gọi ở cuối test. Đóng kết nối trước, vì Postgres không xóa database đang có người dùng.
    pub async fn drop(self) {
        self.store.pool.close().await;
        drop_database(&self.admin, &self.name).await;
    }
}

async fn drop_database(admin: &Store, name: &str) {
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .execute(&admin.pool)
        .await
        .expect("drop test database");
}

/// Chạy một câu SQL thô, để test dựng những trạng thái khó tạo qua API (phiên hết hạn, tài khoản bị khóa).
pub async fn exec(store: &Store, sql: &str) {
    sqlx::query(sql)
        .execute(&store.pool)
        .await
        .expect("exec test sql");
}

/// Đọc một số từ câu SQL thô (thường là đếm dòng), để test kiểm những gì API chưa trả ra.
pub async fn fetch_count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(&store.pool)
        .await
        .expect("fetch test count")
}

/// Đọc một chuỗi từ câu SQL thô (ví dụ id dạng text), để test dựng lại kiểu của tầng trên.
pub async fn fetch_text(store: &Store, sql: &str) -> String {
    sqlx::query_scalar(sql)
        .fetch_one(&store.pool)
        .await
        .expect("fetch test text")
}

/// Cho tài khoản `email` mang vai dữ liệu `role` với các luật (id bảng, cột, giá trị), ghi thẳng database cho test của
/// luồng hỏi, truy dòng nguồn và tổng quan. Vai thuộc workspace của bảng đầu tiên; gọi lại với tên vai khác để người đó
/// mang nhiều vai. Đường qua API có test riêng ở api/data_roles_tests.rs.
pub async fn give_data_role(
    store: &Store,
    email: &str,
    role: &str,
    rules: &[(&str, &str, &[&str])],
) {
    let first: Uuid = rules[0].0.parse().expect("table id");
    let workspace_id: Uuid = sqlx::query_scalar("SELECT workspace_id FROM datasets WHERE id = $1")
        .bind(first)
        .fetch_one(&store.pool)
        .await
        .expect("table workspace");
    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO data_roles (workspace_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(workspace_id)
    .bind(role)
    .fetch_one(&store.pool)
    .await
    .expect("insert data role");
    sqlx::query(
        "INSERT INTO member_data_roles (workspace_id, user_id, data_role_id)
         SELECT $1, id, $3 FROM users WHERE email = $2",
    )
    .bind(workspace_id)
    .bind(email)
    .bind(role_id)
    .execute(&store.pool)
    .await
    .expect("give data role");
    for (table, column, values) in rules {
        let table: Uuid = table.parse().expect("table id");
        sqlx::query(
            "INSERT INTO rls_policies (workspace_id, dataset_id, data_role_id, column_name, allowed_values)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(workspace_id)
        .bind(table)
        .bind(role_id)
        .bind(column)
        .bind(values.to_vec())
        .execute(&store.pool)
        .await
        .expect("insert row policy");
    }
}
