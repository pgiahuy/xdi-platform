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

// Dữ liệu vai kiểu cũ (trước 0020): Binh mang 'south', Chi mang 'South' (cùng vai, khác hoa thường), Dung không có vai;
// luật của 'south' và 'South' trên cùng cột region, luật 'north' không ai mang. Thời điểm tạo khác nhau để biết dòng
// nào được giữ khi gộp.
const OLD_ROLES: &str = "
INSERT INTO workspaces (id, name, slug) VALUES ('00000000-0000-0000-0000-0000000000a1', 'Shop', 'shop');
INSERT INTO sources (id, workspace_id, kind, name) VALUES
  ('00000000-0000-0000-0000-0000000000b1', '00000000-0000-0000-0000-0000000000a1', 'local_file', 'sales');
INSERT INTO datasets (id, workspace_id, source_id, name, logical_table) VALUES
  ('00000000-0000-0000-0000-0000000000c1', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000b1', 'sales', 'sales');
INSERT INTO users (id, email) VALUES
  ('00000000-0000-0000-0000-0000000000d1', 'binh@example.com'),
  ('00000000-0000-0000-0000-0000000000d2', 'chi@example.com'),
  ('00000000-0000-0000-0000-0000000000d3', 'dung@example.com');
INSERT INTO memberships (workspace_id, user_id, role, data_role) VALUES
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000d1', 'member', 'south'),
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000d2', 'member', 'South'),
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-0000000000d3', 'member', NULL);
INSERT INTO rls_policies (id, workspace_id, dataset_id, data_role, column_name, allowed_values) VALUES
  ('00000000-0000-0000-0000-0000000000e1', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000c1', 'south', 'region', ARRAY['South']),
  ('00000000-0000-0000-0000-0000000000e2', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000c1', 'South', 'region', ARRAY['East', 'South']),
  ('00000000-0000-0000-0000-0000000000e3', '00000000-0000-0000-0000-0000000000a1',
   '00000000-0000-0000-0000-0000000000c1', 'north', 'region', ARRAY['North']);
";

/// 0020 (#45, #167): mỗi tên vai thành một dòng data_roles (tên chỉ khác hoa thường gộp làm một), người đang mang vai
/// vẫn mang vai đó, luật giữ nguyên (luật trùng cột của hai tên gộp giá trị), cột cũ bị bỏ.
#[tokio::test]
async fn migration_0020_keeps_every_member_scope() {
    let Some(db) = empty_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &db.store;
    apply_migrations(store, 1, 18).await;
    run(store, OLD_ROLES).await;
    apply_migrations(store, 19, 20).await;

    let roles = fetch_text(
        store,
        "SELECT string_agg(lower(name), ',' ORDER BY lower(name)) FROM data_roles",
    )
    .await;
    assert_eq!(roles, "north,south");
    let held = fetch_text(
        store,
        "SELECT string_agg(u.email || '=' || lower(r.name), ',' ORDER BY u.email) FROM member_data_roles m
         JOIN users u ON u.id = m.user_id JOIN data_roles r ON r.id = m.data_role_id",
    )
    .await;
    assert_eq!(held, "binh@example.com=south,chi@example.com=south");
    let rules = fetch_text(
        store,
        "SELECT string_agg(lower(r.name) || ':' || p.column_name || '=' || array_to_string(p.allowed_values, '|'), ','
                           ORDER BY lower(r.name))
         FROM rls_policies p JOIN data_roles r ON r.id = p.data_role_id",
    )
    .await;
    assert_eq!(rules, "north:region=North,south:region=East|South");
    let old_columns = fetch_count(
        store,
        "SELECT count(*) FROM information_schema.columns
         WHERE column_name = 'data_role' AND table_name IN ('memberships', 'rls_policies')",
    )
    .await;
    assert_eq!(old_columns, 0);

    // Store đọc lại đúng phạm vi: Binh và Chi có luật South, East; Dung không có luật nào.
    let ws = WorkspaceId::for_test(Uuid::from_u128(0xa1));
    let sales = Uuid::from_u128(0xc1);
    for (user, expected) in [(0xd1, 1), (0xd2, 1), (0xd3, 0)] {
        let policies = store
            .list_member_policies(&ws, Uuid::from_u128(user), &[sales])
            .await
            .unwrap();
        assert_eq!(policies.len(), expected, "user {user:x}");
    }
    db.drop().await;
}
