// SPDX-License-Identifier: Apache-2.0
/* engine/store/policies_tests.rs - test engine/store/data_roles.rs và engine/store/policies.rs trên Postgres thật: tạo
vai, gán nhiều vai, thay bộ luật của một vai, các luật giữ trong giao dịch (vai đang gán phải còn luật, không xóa vai
đang gán, không hai vai trùng bộ luật, tên không trùng) và khóa workspace. Mỗi test một database mới (fresh_database)
vì test tạo tài khoản, mà bảng users dùng chung cho mọi workspace. */

use std::time::Duration;

use uuid::Uuid;

use super::test_support::{fresh_database, TestDb};
use super::{
    MemberRolesOutcome, PoliciesOutcome, RoleDeleteOutcome, RolePolicy, RoleWriteOutcome, Store,
    WorkspaceKind,
};
use crate::auth::{Role, WorkspaceId};

struct Fixture {
    db: TestDb,
    ws: WorkspaceId,
    sales: Uuid,
    orders: Uuid,
    member: Uuid,
}

/// Workspace có hai bảng và một thành viên chưa có vai dữ liệu.
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

fn rule(table_id: Uuid, column: &str, values: &[&str]) -> RolePolicy {
    RolePolicy {
        table_id,
        column: column.to_string(),
        allowed_values: values.iter().map(|v| v.to_string()).collect(),
    }
}

async fn create(store: &Store, f: &Fixture, name: &str) -> Uuid {
    match store.create_data_role(&f.ws, name, None).await.unwrap() {
        RoleWriteOutcome::Done(row) => row.id,
        _ => panic!("create data role {name}"),
    }
}

async fn replace(store: &Store, f: &Fixture, role: Uuid, rules: &[RolePolicy]) -> PoliciesOutcome {
    let mut rules = rules.to_vec();
    rules.sort();
    store
        .replace_role_policies(&f.ws, role, &rules)
        .await
        .unwrap()
}

async fn assign(store: &Store, f: &Fixture, roles: &[Uuid]) -> MemberRolesOutcome {
    store
        .set_member_data_roles(&f.ws, f.member, roles)
        .await
        .unwrap()
}

async fn held(store: &Store, f: &Fixture) -> Vec<Uuid> {
    let mut roles: Vec<Uuid> = store
        .list_member_data_roles(&f.ws)
        .await
        .unwrap()
        .into_iter()
        .filter(|(user, _)| *user == f.member)
        .map(|(_, role)| role)
        .collect();
    roles.sort();
    roles
}

/// Vai chưa có luật không gán được; vai có luật gán được, nhiều vai một lúc, và gán lại thay hẳn bộ cũ.
#[tokio::test]
async fn member_holds_several_roles_with_rules() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    let south = create(store, &f, "Miền Nam").await;
    let north = create(store, &f, "Miền Bắc").await;
    let outcome = assign(store, &f, &[south]).await;
    assert!(
        matches!(&outcome, MemberRolesOutcome::RolesWithoutRules(names) if names == &["Miền Nam"])
    );
    assert!(held(store, &f).await.is_empty());

    replace(store, &f, south, &[rule(f.sales, "region", &["South"])]).await;
    replace(store, &f, north, &[rule(f.sales, "region", &["North"])]).await;
    assert!(matches!(
        assign(store, &f, &[south, north]).await,
        MemberRolesOutcome::Done
    ));
    let mut both = vec![south, north];
    both.sort();
    assert_eq!(held(store, &f).await, both);

    let policies = store
        .list_member_policies(&f.ws, f.member, &[f.sales, f.orders])
        .await
        .unwrap();
    assert_eq!(policies.len(), 2);

    assert!(matches!(
        assign(store, &f, &[north]).await,
        MemberRolesOutcome::Done
    ));
    assert_eq!(held(store, &f).await, [north]);
    assert!(matches!(
        assign(store, &f, &[Uuid::new_v4()]).await,
        MemberRolesOutcome::UnknownRole
    ));
    assert!(matches!(
        store
            .set_member_data_roles(&f.ws, Uuid::new_v4(), &[north])
            .await
            .unwrap(),
        MemberRolesOutcome::NotMember
    ));
    f.db.drop().await;
}

/// Thay luật chỉ đụng vai được chỉ định, bộ mới thay hẳn bộ cũ; vai đang gán không được thành không luật.
#[tokio::test]
async fn replace_touches_only_its_role_and_keeps_assigned_roles_ruled() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    let south = create(store, &f, "south").await;
    let north = create(store, &f, "north").await;
    replace(store, &f, south, &[rule(f.sales, "region", &["South"])]).await;
    replace(store, &f, north, &[rule(f.sales, "region", &["North"])]).await;
    let outcome = replace(
        store,
        &f,
        south,
        &[
            rule(f.sales, "region", &["South", "East"]),
            rule(f.orders, "channel", &["web"]),
        ],
    )
    .await;
    assert!(matches!(outcome, PoliciesOutcome::Done));
    let all = store.list_role_policies(&f.ws).await.unwrap();
    assert_eq!(all.iter().filter(|p| p.data_role_id == south).count(), 2);
    assert_eq!(all.iter().filter(|p| p.data_role_id == north).count(), 1);

    assert!(matches!(
        assign(store, &f, &[south]).await,
        MemberRolesOutcome::Done
    ));
    assert!(matches!(
        replace(store, &f, south, &[]).await,
        PoliciesOutcome::AssignedWithoutRules
    ));
    assert_eq!(store.list_role_policies(&f.ws).await.unwrap().len(), 3);
    // Vai chưa gán thì xóa hết luật được.
    assert!(matches!(
        replace(store, &f, north, &[]).await,
        PoliciesOutcome::Done
    ));
    assert!(matches!(
        replace(store, &f, Uuid::new_v4(), &[]).await,
        PoliciesOutcome::NotFound
    ));
    f.db.drop().await;
}

/// Hai vai không được có đúng cùng bộ luật (#167), kể cả khi gửi giá trị theo thứ tự khác; khác một giá trị thì được.
#[tokio::test]
async fn two_roles_cannot_have_the_same_rules() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    let first = create(store, &f, "sales_south").await;
    let second = create(store, &f, "z1").await;
    let rules = [
        rule(f.sales, "region", &["East", "South"]),
        rule(f.orders, "channel", &["web"]),
    ];
    replace(store, &f, first, &rules).await;
    let outcome = replace(store, &f, second, &rules).await;
    assert!(matches!(&outcome, PoliciesOutcome::SameRulesAs(name) if name == "sales_south"));
    let outcome = replace(
        store,
        &f,
        second,
        &[rule(f.sales, "region", &["East", "South"])],
    )
    .await;
    assert!(matches!(outcome, PoliciesOutcome::Done));
    f.db.drop().await;
}

/// Tên không trùng trong workspace, không phân biệt hoa thường; vai đang gán không xóa được, vai chưa gán xóa cùng luật.
#[tokio::test]
async fn names_are_unique_and_assigned_roles_are_not_deleted() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    let south = create(store, &f, "Miền Nam").await;
    let taken = store
        .create_data_role(&f.ws, "miền nam", None)
        .await
        .unwrap();
    assert!(matches!(taken, RoleWriteOutcome::NameTaken));
    let other = create(store, &f, "Miền Bắc").await;
    let renamed = store
        .update_data_role(&f.ws, other, "MIỀN NAM", None)
        .await
        .unwrap();
    assert!(matches!(renamed, RoleWriteOutcome::NameTaken));
    let renamed = store
        .update_data_role(&f.ws, other, "Miền Trung", Some("Đà Nẵng, Huế"))
        .await
        .unwrap();
    assert!(matches!(renamed, RoleWriteOutcome::Done(row) if row.name == "Miền Trung"));

    replace(store, &f, south, &[rule(f.sales, "region", &["South"])]).await;
    assign(store, &f, &[south]).await;
    let outcome = store.delete_data_role(&f.ws, south).await.unwrap();
    assert!(matches!(outcome, RoleDeleteOutcome::Assigned(1)));
    assign(store, &f, &[]).await;
    let outcome = store.delete_data_role(&f.ws, south).await.unwrap();
    assert!(matches!(outcome, RoleDeleteOutcome::Done));
    assert!(store.list_role_policies(&f.ws).await.unwrap().is_empty());
    let outcome = store.delete_data_role(&f.ws, south).await.unwrap();
    assert!(matches!(outcome, RoleDeleteOutcome::NotFound));
    f.db.drop().await;
}

/// Gán vai phải chờ giao dịch khác đang giữ khóa workspace (một lần thay luật đang dở). Bỏ lock_workspace thì test hỏng:
/// set_member_data_roles đọc thấy luật cũ còn đó và gán xong ngay, trong khi lần thay luật kia sắp xóa luật đó.
#[tokio::test]
async fn assigning_waits_for_the_workspace_lock() {
    let Some(f) = fixture().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let store = &f.db.store;
    let south = create(store, &f, "south").await;
    replace(store, &f, south, &[rule(f.sales, "region", &["South"])]).await;

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

    let task = tokio::spawn({
        let store = store.clone();
        let ws = f.ws;
        let member = f.member;
        async move { store.set_member_data_roles(&ws, member, &[south]).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "assigning must wait for the lock");
    other.commit().await.unwrap();
    let outcome = task.await.unwrap().unwrap();
    assert!(matches!(outcome, MemberRolesOutcome::RolesWithoutRules(_)));
    f.db.drop().await;
}
