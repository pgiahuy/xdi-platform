// SPDX-License-Identifier: Apache-2.0
/* engine/store/test_support.rs - phần dùng chung cho test cần Postgres thật. */

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
/// Nhờ vậy các test chạy song song không đụng nhau và không để lại dữ liệu.
pub async fn temp_workspace(store: &Store) -> WorkspaceId {
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
