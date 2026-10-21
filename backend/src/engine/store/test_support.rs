// SPDX-License-Identifier: Apache-2.0
/* engine/store/test_support.rs - phần dùng chung cho test cần Postgres thật. */

use std::time::{SystemTime, UNIX_EPOCH};

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
    let store = Store::connect(&url, 5).await.expect("connect");
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
    let url = database_url()?;
    let admin = Store::connect(&url, 2).await.expect("connect");
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
    let store = Store::connect(&format!("{base}/{name}"), 5)
        .await
        .expect("connect test database");
    store.migrate().await.expect("migrate test database");
    Some(TestDb { store, name, admin })
}

impl TestDb {
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
