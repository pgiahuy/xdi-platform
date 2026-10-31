// SPDX-License-Identifier: Apache-2.0
/* engine/store/policies.rs - chính sách lọc dòng (rls_policies) theo vai dữ liệu (memberships.data_role): đọc thành
RowFilter cho guard (engine/guard/mod.rs, bước 3), và phần ghi của #45 (gán vai cho thành viên, thay bộ luật của một
dataset). Ai được ghi (admin), kiểm và chuẩn hóa dữ liệu vào do service quyết định; ai không bị lọc (admin) cũng vậy.

Schema chưa có bảng định nghĩa vai: tên vai là chữ tự do, và vai không có luật nào thì không lọc. Gõ sai tên khi gán
thì người đó thấy hết, nên hai hàm ghi ở đây giữ một luật: vai đang gán cho ai đó phải còn ít nhất một luật trong
workspace. Gán vai chưa có luật bị từ chối; thay bộ luật mà làm một vai đang gán mất luật cuối cùng cũng bị từ chối.
Xóa dataset thì luật của nó bị xóa theo (khóa ngoại) mà không qua luật này, nên vai có thể còn gán mà hết luật; người
mang vai đó không thấy thêm dòng nào, vì vai vốn không có luật ở các dataset còn lại. Bảng định nghĩa vai sẽ thay luật
này bằng khóa ngoại (#167). */

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, engine::guard::RowFilter, error::AppError};

// Một luật lọc dòng của một vai trên một dataset, đúng một dòng của rls_policies.
/// A row rule of a data role on a dataset: members with this data role see only the rows whose `column` is one of
/// `allowed_values`; admins are not filtered. Rules of one data role on one dataset all apply together (AND).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct RowPolicy {
    pub data_role: String,
    /// A column of the dataset.
    pub column: String,
    /// Values of `column` the data role sees; empty means no rows.
    pub allowed_values: Vec<String>,
}

pub enum DataRoleOutcome {
    Done,
    NotMember,
    /// Từ chối: vai chưa có luật nào trong workspace.
    UnknownRole,
}

pub enum PoliciesOutcome {
    Done,
    /// Từ chối: sau khi thay, các vai này còn đang gán cho người mà không còn luật nào trong workspace.
    RolesLeftWithoutRules(Vec<String>),
}

impl Store {
    /// Chính sách của dataset cho data_role của người dùng trong workspace. Rỗng khi người đó không có data_role, hay
    /// dataset không có chính sách nào cho vai đó: guard không lọc. Đọc mỗi request, không giữ lại, để đổi vai có hiệu
    /// lực ngay request kế tiếp.
    pub async fn list_row_filters(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<RowFilter>, AppError> {
        let rows: Vec<(String, Vec<String>)> = sqlx::query_as(
            "SELECT p.column_name, p.allowed_values FROM memberships m
             JOIN rls_policies p ON p.workspace_id = m.workspace_id AND p.data_role = m.data_role
             WHERE m.workspace_id = $1 AND m.user_id = $2 AND p.dataset_id = $3
             ORDER BY p.column_name",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list row filters", e))?;
        Ok(rows
            .into_iter()
            .map(|(column, allowed_values)| RowFilter::Column {
                column,
                allowed_values,
            })
            .collect())
    }

    /// Mọi luật của một dataset, cho admin xem và sửa.
    pub async fn list_row_policies(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
    ) -> Result<Vec<RowPolicy>, AppError> {
        // "column" là từ khóa của SQL nên phải để trong nháy kép.
        sqlx::query_as(
            "SELECT data_role, column_name AS \"column\", allowed_values FROM rls_policies
             WHERE workspace_id = $1 AND dataset_id = $2 ORDER BY data_role, column_name",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list row policies", e))
    }

    /// Đặt hoặc gỡ (None) vai dữ liệu của một thành viên. Vai phải đã có ít nhất một luật trong workspace.
    pub async fn set_data_role(
        &self,
        ws: &WorkspaceId,
        user_id: Uuid,
        data_role: Option<&str>,
    ) -> Result<DataRoleOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin set data role", e))?;
        lock_workspace(&mut tx, ws).await?;
        if let Some(role) = data_role {
            let has_rules: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM rls_policies WHERE workspace_id = $1 AND data_role = $2)",
            )
            .bind(ws.as_uuid())
            .bind(role)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_error("check data role", e))?;
            if !has_rules {
                return Ok(DataRoleOutcome::UnknownRole);
            }
        }
        let result = sqlx::query(
            "UPDATE memberships SET data_role = $3 WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(ws.as_uuid())
        .bind(user_id)
        .bind(data_role)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("set data role", e))?;
        if result.rows_affected() == 0 {
            return Ok(DataRoleOutcome::NotMember);
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit set data role", e))?;
        Ok(DataRoleOutcome::Done)
    }

    /// Thay toàn bộ luật của một dataset bằng policies, trong một giao dịch: lỗi giữa chừng hay bị từ chối thì luật cũ
    /// còn nguyên. Dataset phải thuộc workspace; service kiểm trước để trả 404 thay vì lỗi khóa ngoại.
    pub async fn replace_row_policies(
        &self,
        ws: &WorkspaceId,
        dataset_id: Uuid,
        policies: &[RowPolicy],
    ) -> Result<PoliciesOutcome, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin replace row policies", e))?;
        lock_workspace(&mut tx, ws).await?;
        let removed_roles: Vec<String> = sqlx::query_scalar(
            "DELETE FROM rls_policies WHERE workspace_id = $1 AND dataset_id = $2 RETURNING data_role",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_error("delete row policies", e))?;
        for policy in policies {
            sqlx::query(
                "INSERT INTO rls_policies (workspace_id, dataset_id, data_role, column_name, allowed_values)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(ws.as_uuid())
            .bind(dataset_id)
            .bind(&policy.data_role)
            .bind(&policy.column)
            .bind(&policy.allowed_values)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("insert row policy", e))?;
        }
        // Chỉ xét vai vừa mất luật trên dataset này. Vai đã không còn luật từ trước (ví dụ dataset chứa luật của nó vừa
        // bị xóa) không làm lần sửa luật một dataset khác bị từ chối.
        let left_without_rules: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT m.data_role FROM memberships m
             WHERE m.workspace_id = $1 AND m.data_role = ANY($2)
               AND NOT EXISTS (SELECT 1 FROM rls_policies p
                               WHERE p.workspace_id = m.workspace_id AND p.data_role = m.data_role)
             ORDER BY m.data_role",
        )
        .bind(ws.as_uuid())
        .bind(&removed_roles)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_error("check roles left without rules", e))?;
        if !left_without_rules.is_empty() {
            // tx bị hủy khi ra khỏi hàm mà chưa commit, nên các lệnh DELETE, INSERT ở trên không được ghi.
            return Ok(PoliciesOutcome::RolesLeftWithoutRules(left_without_rules));
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit replace row policies", e))?;
        Ok(PoliciesOutcome::Done)
    }
}

/// Khóa dòng workspace để gán vai và thay luật của cùng workspace chạy lần lượt. Không khóa thì một lần gán vai có thể
/// thấy luật cũ còn đó trong khi một lần thay luật đang xóa luật cuối của vai, và người được gán vai thấy hết dữ liệu.
/// FOR NO KEY UPDATE thay cho FOR UPDATE: không chặn các lệnh ghi bảng khác có khóa ngoại tới workspace (lưu câu hỏi,
/// nạp dataset) trong lúc giao dịch này chạy.
async fn lock_workspace(conn: &mut PgConnection, ws: &WorkspaceId) -> Result<(), AppError> {
    sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
        .bind(ws.as_uuid())
        .execute(conn)
        .await
        .map_err(|e| db_error("lock workspace", e))?;
    Ok(())
}
