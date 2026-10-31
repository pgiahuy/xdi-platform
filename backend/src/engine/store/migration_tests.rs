// SPDX-License-Identifier: Apache-2.0
/* engine/store/migration_tests.rs - test chuyển dữ liệu cũ của migration 0017 (#257): áp các migration trước nó lên
một database trống, ghi dữ liệu kiểu cũ (mỗi bảng một dataset, gói ngành trên bảng, chỉ số trùng tên giữa hai bảng
cùng database), áp 0017 rồi kiểm dữ liệu sau khi chuyển. Không có DATABASE_URL thì bỏ qua. */

use uuid::Uuid;

use super::test_support::{empty_database, fetch_count, fetch_text};
use super::Store;
use crate::auth::WorkspaceId;

/// Áp các migration có số từ `from` tới `to` (kể cả hai đầu), đúng nội dung file như sqlx sẽ áp.
async fn apply_migrations(store: &Store, from: i64, to: i64) {
    for m in sqlx::migrate!("./migrations").iter() {
        if (from..=to).contains(&m.version) {
            sqlx::raw_sql(&m.sql)
                .execute(&store.pool)
                .await
                .unwrap_or_else(|e| panic!("apply migration {}: {e}", m.version));
        }
    }
}

async fn run(store: &Store, sql: &str) {
    sqlx::raw_sql(sql)
        .execute(&store.pool)
        .await
        .unwrap_or_else(|e| panic!("{e}: {sql}"));
}

// Một workspace có một database (shop: orders, order_items) và một file cũng tên orders (bảng logic orders_2 như
// luật cũ đặt). Thời điểm tạo cố định để thứ tự "bảng tạo trước" của migration là đã biết.
const OLD_DATA: &str = "
INSERT INTO workspaces (id, name, slug) VALUES ('00000000-0000-0000-0000-0000000000a1', 'Shop', 'shop');
INSERT INTO sources (id, workspace_id, kind, config) VALUES
  ('00000000-0000-0000-0000-0000000000b1', '00000000-0000-0000-0000-0000000000a1', 'postgres',
   '{\"database\": \"shop_db\", \"tables\": {}}'),
  ('00000000-0000-0000-0000-0000000000b2', '00000000-0000-0000-0000-0000000000a1', 'local_file', '{}');
INSERT INTO datasets (id, workspace_id, source_id, name, logical_table, domain, created_at) VALUES
  ('00000000-0000-0000-0000-0000000000c1', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000b1', 'orders', 'orders', 'retail', '2026-01-01'),
  ('00000000-0000-0000-0000-0000000000c2', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000b1', 'order_items', 'order_items', NULL, '2026-01-02'),
  ('00000000-0000-0000-0000-0000000000c3', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000b2', 'Orders file', 'orders_2', NULL, '2026-01-03');
INSERT INTO metrics (workspace_id, dataset_id, name, expression, origin) VALUES
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000c1', 'Doanh thu', 'SUM(total)', 'ui'),
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000c2', 'Doanh thu', 'SUM(amount)', 'ui'),
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000c3', 'Doanh thu', 'SUM(x)', 'ui');
INSERT INTO queries (workspace_id, dataset_id, question, sql_generated, status, feedback, example_approved_at)
  VALUES ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000c1', 'Số đơn',
          'SELECT count(*) FROM orders', 'ok', 'correct', now());
INSERT INTO lessons (workspace_id, dataset_id, text, enabled, approved_at)
  VALUES ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000c1', 'Đơn hủy không tính',
          true, now());
";

#[tokio::test]
async fn migration_0017_moves_tables_into_datasets() {
    let Some(db) = empty_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &db.store;
    apply_migrations(store, 1, 16).await;
    run(store, OLD_DATA).await;
    apply_migrations(store, 17, 17).await;

    // Dataset của database mang tên database, của file mang tên bảng; gói ngành lên dataset.
    let names = fetch_text(
        store,
        "SELECT string_agg(kind || '=' || name || '/' || COALESCE(domain, '-'), ',' ORDER BY kind) FROM sources",
    )
    .await;
    assert_eq!(names, "local_file=Orders file/-,postgres=shop_db/retail");

    // Chỉ số trùng tên trong một dataset: bảng tạo sau thêm tên bảng; chỉ số của file (dataset khác) giữ tên.
    let metrics = fetch_text(
        store,
        "SELECT string_agg(d.logical_table || '=' || m.name, ',' ORDER BY d.created_at) FROM metrics m
         JOIN datasets d ON d.id = m.dataset_id AND d.source_id = m.source_id",
    )
    .await;
    assert_eq!(
        metrics,
        "orders=Doanh thu,order_items=Doanh thu (order_items),orders_2=Doanh thu"
    );

    // Tên bảng logic chỉ không trùng trong một dataset: file được có bảng orders, database thì không có hai.
    run(
        store,
        "INSERT INTO datasets (workspace_id, source_id, name, logical_table) VALUES
           ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000b2', 'x', 'orders')",
    )
    .await;
    let twice = sqlx::raw_sql(
        "INSERT INTO datasets (workspace_id, source_id, name, logical_table) VALUES
           ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000b1', 'x', 'ORDERS')",
    )
    .execute(&store.pool)
    .await;
    assert!(twice.is_err(), "two tables named orders in one dataset");

    // Câu mẫu và bài học viết trên orders có ở câu hỏi bắt đầu từ order_items (cùng database), không ở file.
    let ws = WorkspaceId::for_test(Uuid::from_u128(0xa1));
    let order_items = Uuid::from_u128(0xc2);
    let file = Uuid::from_u128(0xc3);
    assert_eq!(
        store
            .list_approved_examples(&ws, order_items, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .list_approved_examples(&ws, file, 10)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .list_active_lessons(&ws, order_items, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .list_active_lessons(&ws, file, 10)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        fetch_count(
            store,
            "SELECT count(*) FROM queries WHERE dataset_id IS NOT NULL"
        )
        .await,
        1
    );
    db.drop().await;
}
