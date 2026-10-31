// SPDX-License-Identifier: Apache-2.0
/* engine/store/policies_tests.rs - test phần ghi của engine/store/policies.rs trên Postgres thật: gán vai, thay bộ
luật, luật "vai đang gán phải còn luật", và khóa workspace. Mỗi test một database mới (fresh_database) vì test tạo tài
khoản, mà bảng users dùng chung cho mọi workspace. */

use std::time::Duration;

use uuid::Uuid;

use super::test_support::{fresh_database, TestDb};
use super::{DataRoleOutcome, PoliciesOutcome, RowPolicy, Store, WorkspaceKind};
use crate::auth::{Role, WorkspaceId};
use crate::engine::guard::RowFilter;

struct Fixture {
    db: TestDb,
    ws: WorkspaceId,
    sales: Uuid,
    orders: Uuid,
    member: Uuid,
}

/// Workspace có hai dataset và một thành viên chưa có vai dữ liệu.
async fn fixture() -> Option<Fixture> {
    let db = fresh_database().await?;
    let store = &db.store;
    let ws_id = store
        .create_workspace("Shop", "shop", WorkspaceKind::Team)
        .await
        .unwrap();
    let ws = WorkspaceId::for_test(ws_id);
    let source = store.create_local_source(&ws, "Test").await.unwrap();
    let sales = store
        .create_dataset(&ws, source, "Sales", "sales", None)
        .await
        .unwrap()
        .id;
    let orders = store
        .create_dataset(&ws, source, "Orders", "orders", None)
        .await
        .unwrap()
        .id;
    let member: Uuid = sqlx::query_scalar("INSERT INTO users (email) VALUES ($1) RETURNING id")
        .bind("binh@example.com")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert!(store.add_member(&ws, member, Role::Member).await.unwrap());
    Some(Fixture {
        db,
        ws,
        sales,
        orders,
        member,
    })
}

fn rule(data_role: &str, column: &str, values: &[&str]) -> RowPolicy {
    RowPolicy {
        data_role: data_role.to_string(),
        column: column.to_string(),
        allowed_values: values.iter().map(|v| v.to_string()).collect(),
    }
}

async fn replace(store: &Store, f: &Fixture, dataset: Uuid, rules: &[RowPolicy]) {
    let outcome = store
        .replace_row_policies(&f.ws, dataset, rules)
        .await
        .unwrap();
    assert!(matches!(outcome, PoliciesOutcome::Done));
}

async fn set_role(store: &Store, f: &Fixture, role: Option<&str>) -> DataRoleOutcome {
    store.set_data_role(&f.ws, f.member, role).await.unwrap()
}

async fn stored_role(store: &Store, f: &Fixture) -> Option<String> {
    sqlx::query_scalar("SELECT data_role FROM memberships WHERE workspace_id = $1 AND user_id = $2")
        .bind(f.ws.as_uuid())
        .bind(f.member)
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

async fn filter_columns(store: &Store, f: &Fixture, dataset: Uuid) -> Vec<String> {
    store
        .list_row_filters(&f.ws, dataset, f.member)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|filter| match filter {
            RowFilter::Column { column, .. } => Some(column),
            RowFilter::Parent { .. } => None,
        })
        .collect()
}

/// Gán vai chưa có luật nào (gõ sai tên) bị từ chối: nhận thì người đó thấy hết dữ liệu mà không ai biết.
#[tokio::test]
async fn role_without_rules_cannot_be_assigned() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;

    // Đọc thẳng cột data_role: vai sai tên không có luật nên list_row_filters ra rỗng dù tên đó đã bị lưu hay chưa.
    assert!(matches!(
        set_role(store, &f, Some("sale_south")).await,
        DataRoleOutcome::UnknownRole
    ));
    assert_eq!(stored_role(store, &f).await, None);

    assert!(matches!(
        set_role(store, &f, Some("sales_south")).await,
        DataRoleOutcome::Done
    ));
    assert_eq!(filter_columns(store, &f, f.sales).await, ["region"]);

    // Bị từ chối thì vai đang có giữ nguyên.
    assert!(matches!(
        set_role(store, &f, Some("sale_south")).await,
        DataRoleOutcome::UnknownRole
    ));
    assert_eq!(stored_role(store, &f).await.as_deref(), Some("sales_south"));

    // Gỡ vai: không lọc nữa.
    assert!(matches!(
        set_role(store, &f, None).await,
        DataRoleOutcome::Done
    ));
    assert_eq!(stored_role(store, &f).await, None);
    assert!(filter_columns(store, &f, f.sales).await.is_empty());

    let stranger = store
        .set_data_role(&f.ws, Uuid::new_v4(), Some("sales_south"))
        .await
        .unwrap();
    assert!(matches!(stranger, DataRoleOutcome::NotMember));
    f.db.drop().await;
}

/// Thay luật chỉ đụng dataset được chỉ định, và bộ mới thay hẳn bộ cũ.
#[tokio::test]
async fn replace_touches_only_its_dataset() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;
    replace(
        store,
        &f,
        f.orders,
        &[rule("sales_south", "channel", &["web"])],
    )
    .await;
    replace(
        store,
        &f,
        f.sales,
        &[
            rule("sales_south", "segment", &["Consumer"]),
            rule("sales_north", "region", &["North"]),
        ],
    )
    .await;

    assert_eq!(
        store.list_row_policies(&f.ws, f.sales).await.unwrap(),
        [
            rule("sales_north", "region", &["North"]),
            rule("sales_south", "segment", &["Consumer"]),
        ]
    );
    assert_eq!(
        store.list_row_policies(&f.ws, f.orders).await.unwrap(),
        [rule("sales_south", "channel", &["web"])]
    );
    f.db.drop().await;
}

/// Thay luật làm một vai đang gán mất luật cuối cùng thì bị từ chối và luật cũ còn nguyên. Vai còn luật ở dataset
/// khác thì được.
#[tokio::test]
async fn last_rule_of_an_assigned_role_cannot_be_removed() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;
    assert!(matches!(
        set_role(store, &f, Some("sales_south")).await,
        DataRoleOutcome::Done
    ));

    let outcome = store
        .replace_row_policies(&f.ws, f.sales, &[])
        .await
        .unwrap();
    let PoliciesOutcome::RolesLeftWithoutRules(roles) = outcome else {
        panic!("removing the last rule of an assigned role must be refused");
    };
    assert_eq!(roles, ["sales_south"]);
    assert_eq!(filter_columns(store, &f, f.sales).await, ["region"]);

    replace(
        store,
        &f,
        f.orders,
        &[rule("sales_south", "channel", &["web"])],
    )
    .await;
    replace(store, &f, f.sales, &[]).await;
    assert!(filter_columns(store, &f, f.sales).await.is_empty());
    assert_eq!(filter_columns(store, &f, f.orders).await, ["channel"]);
    f.db.drop().await;
}

/// Vai đã mất hết luật vì dataset chứa luật của nó bị xóa không chặn việc sửa luật của dataset khác.
#[tokio::test]
async fn role_emptied_by_a_deleted_dataset_does_not_block_other_datasets() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;
    assert!(matches!(
        set_role(store, &f, Some("sales_south")).await,
        DataRoleOutcome::Done
    ));
    sqlx::query("DELETE FROM datasets WHERE id = $1")
        .bind(f.sales)
        .execute(&store.pool)
        .await
        .unwrap();

    replace(
        store,
        &f,
        f.orders,
        &[rule("sales_north", "channel", &["web"])],
    )
    .await;
    f.db.drop().await;
}

/// Gán vai phải chờ giao dịch khác đang giữ khóa workspace (một lần thay luật đang dở). Bỏ lock_workspace thì test hỏng:
/// set_data_role đọc thấy luật cũ còn đó và gán xong ngay, trong khi lần thay luật kia sắp xóa luật đó.
#[tokio::test]
async fn set_data_role_waits_for_the_workspace_lock() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;

    let mut other = store.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
        .bind(f.ws.as_uuid())
        .execute(&mut *other)
        .await
        .unwrap();
    sqlx::query("DELETE FROM rls_policies WHERE workspace_id = $1")
        .bind(f.ws.as_uuid())
        .execute(&mut *other)
        .await
        .unwrap();

    let assign = tokio::spawn({
        let store = store.clone();
        let ws = f.ws;
        let member = f.member;
        async move { store.set_data_role(&ws, member, Some("sales_south")).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !assign.is_finished(),
        "set_data_role must wait for the lock"
    );
    other.commit().await.unwrap();
    let outcome = assign.await.unwrap().unwrap();
    assert!(matches!(outcome, DataRoleOutcome::UnknownRole));
    f.db.drop().await;
}

/// Thay luật phải chờ giao dịch khác đang giữ khóa workspace (một lần gán vai đang dở). Bỏ lock_workspace trong
/// replace_row_policies thì test hỏng: lần thay luật không thấy vai vừa gán (chưa commit), xóa luật cuối của vai đó và
/// xong ngay, để lại một người mang vai không còn luật.
#[tokio::test]
async fn replace_row_policies_waits_for_the_workspace_lock() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    replace(
        store,
        &f,
        f.sales,
        &[rule("sales_south", "region", &["South"])],
    )
    .await;

    let mut other = store.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
        .bind(f.ws.as_uuid())
        .execute(&mut *other)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE memberships SET data_role = 'sales_south' WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(f.ws.as_uuid())
    .bind(f.member)
    .execute(&mut *other)
    .await
    .unwrap();

    let remove_rules = tokio::spawn({
        let store = store.clone();
        let ws = f.ws;
        let sales = f.sales;
        async move { store.replace_row_policies(&ws, sales, &[]).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !remove_rules.is_finished(),
        "replace_row_policies must wait for the lock"
    );
    other.commit().await.unwrap();
    let outcome = remove_rules.await.unwrap().unwrap();
    let PoliciesOutcome::RolesLeftWithoutRules(roles) = outcome else {
        panic!("the role assigned meanwhile must keep its last rule");
    };
    assert_eq!(roles, ["sales_south"]);
    f.db.drop().await;
}
