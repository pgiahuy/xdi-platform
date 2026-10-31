// SPDX-License-Identifier: Apache-2.0
/* engine/store/policies.rs - luật lọc dòng (rls_policies) của từng vai dữ liệu (data_roles.rs): đọc theo vai của người
hỏi cho guard (service/ask/row_filters.rs dựng RowFilter), và thay bộ luật của một vai cho admin. Ai được ghi (admin),
kiểm bảng, cột, giá trị và chuẩn hóa dữ liệu vào do service/data_roles.rs quyết định; ai không bị lọc (admin) cũng vậy.

Một luật gắn với một bảng (datasets.id, #266) và một cột: người mang vai chỉ thấy dòng có cột đó nằm trong
allowed_values. Luật giữ trong cùng giao dịch ghi:
- Vai đang gán cho ai đó không được còn không luật nào (vai không luật không giới hạn gì, xem data_roles.rs).
- Hai vai của một workspace không được có đúng cùng bộ luật (#167): khi đó hai tên chỉ là một quyền, và admin không
  biết gán tên nào. Service sắp xếp giá trị và luật trước khi gửi xuống, nên so bằng == là đủ. */

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

// Một luật của một vai, đúng một dòng của rls_policies.
/// A row rule of a data role: members with this role see only the rows of table `table_id` whose `column` is one of
/// `allowed_values`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
pub struct RolePolicy {
    /// Table id (`DatasetTable.id` in the catalog).
    pub table_id: Uuid,
    /// A dimension column of the table.
    pub column: String,
    /// Values of `column` the role sees; empty means no rows of the table.
    pub allowed_values: Vec<String>,
}

/// Một luật kèm vai của nó, đọc cho người hỏi hay cho admin.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct StoredPolicy {
    pub data_role_id: Uuid,
    pub table_id: Uuid,
    pub column: String,
    pub allowed_values: Vec<String>,
}

pub enum PoliciesOutcome {
    Done,
    NotFound,
    /// Từ chối: vai đang gán cho người mà bộ luật mới rỗng.
    AssignedWithoutRules,
    /// Từ chối: vai này đã có đúng bộ luật đó.
    SameRulesAs(String),
}

impl Store {
    /// Luật trên các bảng table_ids của mọi vai người dùng đang mang trong workspace. Đọc mỗi request, không giữ lại,
    /// để đổi vai hay luật có hiệu lực ngay request kế tiếp.
    pub async fn list_member_policies(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
        table_ids: &[Uuid],
    ) -> Result<Vec<StoredPolicy>, AppError> {
        // "column" là từ khóa của SQL nên phải để trong nháy kép.
        sqlx::query_as(
            "SELECT p.data_role_id, p.dataset_id AS table_id, p.column_name AS \"column\", p.allowed_values
             FROM member_data_roles m
             JOIN rls_policies p ON p.workspace_id = m.workspace_id AND p.data_role_id = m.data_role_id
             WHERE m.workspace_id = $1 AND m.user_id = $2 AND p.dataset_id = ANY($3)
             ORDER BY p.data_role_id, p.dataset_id, p.column_name",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .bind(table_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list member policies", e))
    }

    /// Mọi luật của mọi vai trong workspace, cho admin xem và sửa.
    pub async fn list_role_policies(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Vec<StoredPolicy>, AppError> {
        sqlx::query_as(
            "SELECT data_role_id, dataset_id AS table_id, column_name AS \"column\", allowed_values
             FROM rls_policies WHERE workspace_id = $1 ORDER BY data_role_id, dataset_id, column_name",
        )
        .bind(ws.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list role policies", e))
    }

    /// Thay toàn bộ luật của một vai bằng policies (đã sắp xếp, mỗi bảng và cột một lần), trong một giao dịch: lỗi giữa
    /// chừng hay bị từ chối thì luật cũ còn nguyên. Bảng phải thuộc workspace; service kiểm trước để báo lời rõ thay vì
    /// lỗi khóa ngoại.
    pub async fn replace_role_policies(
        &self,
        ws: &WorkspaceId,
        role_id: Uuid,
        policies: &[RolePolicy],
    ) -> Result<PoliciesOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin replace role policies", e))?;
        lock_workspace(&mut tx, ws).await?;
        let assigned: Option<i64> = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM member_data_roles m WHERE m.data_role_id = r.id)
             FROM data_roles r WHERE r.workspace_id = $1 AND r.id = $2",
        )
        .bind(ws.as_uuid())
        .bind(role_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| db_error("check data role", e))?;
        let Some(assigned) = assigned else {
            return Ok(PoliciesOutcome::NotFound);
        };
        if assigned > 0 && policies.is_empty() {
            return Ok(PoliciesOutcome::AssignedWithoutRules);
        }
        if let Some(name) = find_role_with_rules(&mut tx, ws, role_id, policies).await? {
            return Ok(PoliciesOutcome::SameRulesAs(name));
        }
        sqlx::query("DELETE FROM rls_policies WHERE workspace_id = $1 AND data_role_id = $2")
            .bind(ws.as_uuid())
            .bind(role_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("delete role policies", e))?;
        for policy in policies {
            sqlx::query(
                "INSERT INTO rls_policies (workspace_id, dataset_id, data_role_id, column_name, allowed_values)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(ws.as_uuid())
            .bind(policy.table_id)
            .bind(role_id)
            .bind(&policy.column)
            .bind(&policy.allowed_values)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("insert role policy", e))?;
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit replace role policies", e))?;
        Ok(PoliciesOutcome::Done)
    }
}

/// Tên một vai khác của workspace có đúng bộ luật policies; None khi không có. Bộ rỗng không so: nhiều vai mới tạo
/// cùng chưa có luật là chuyện thường.
async fn find_role_with_rules(
    conn: &mut PgConnection,
    ws: &WorkspaceId,
    role_id: Uuid,
    policies: &[RolePolicy],
) -> Result<Option<String>, AppError> {
    if policies.is_empty() {
        return Ok(None);
    }
    let rows: Vec<(String, Uuid, String, Vec<String>)> = sqlx::query_as(
        "SELECT r.name, p.dataset_id, p.column_name, p.allowed_values
         FROM rls_policies p JOIN data_roles r ON r.id = p.data_role_id
         WHERE p.workspace_id = $1 AND p.data_role_id <> $2
         ORDER BY lower(r.name), p.dataset_id, p.column_name",
    )
    .bind(ws.as_uuid())
    .bind(role_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| db_error("list other role policies", e))?;
    // Gom luật theo vai (hàng đã sắp theo vai), rồi sắp luật của từng vai bằng Rust như service sắp policies: thứ tự
    // chữ của Postgres theo collation, có thể khác thứ tự byte của Rust.
    let mut by_role: Vec<(String, Vec<RolePolicy>)> = Vec::new();
    for (name, table_id, column, mut allowed_values) in rows {
        allowed_values.sort();
        let policy = RolePolicy {
            table_id,
            column,
            allowed_values,
        };
        match by_role.last_mut() {
            Some((last, rules)) if *last == name => rules.push(policy),
            _ => by_role.push((name, vec![policy])),
        }
    }
    Ok(by_role
        .into_iter()
        .find(|(_, rules)| {
            let mut rules = rules.clone();
            rules.sort();
            rules.as_slice() == policies
        })
        .map(|(name, _)| name))
}

/// Khóa dòng workspace để gán vai, thay luật và xóa vai của cùng workspace chạy lần lượt. Không khóa thì một lần gán vai
/// có thể thấy vai còn luật trong khi một lần thay luật đang xóa luật cuối của nó, và người được gán mang vai không luật.
/// FOR NO KEY UPDATE thay cho FOR UPDATE: không chặn các lệnh ghi bảng khác có khóa ngoại tới workspace (lưu câu hỏi,
/// nạp dataset) trong lúc giao dịch này chạy.
pub(super) async fn lock_workspace(
    conn: &mut PgConnection,
    ws: &WorkspaceId,
) -> Result<(), AppError> {
    sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
        .bind(ws.as_uuid())
        .execute(conn)
        .await
        .map_err(|e| db_error("lock workspace", e))?;
    Ok(())
}
