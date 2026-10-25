// SPDX-License-Identifier: Apache-2.0
/* engine/store/policies.rs - đọc chính sách lọc dòng (rls_policies) theo vai dữ liệu (memberships.data_role) của một
thành viên, thành RowFilter cho guard (engine/guard/mod.rs, bước 3). Truy dòng nguồn (#34) gọi với người đang bấm.
Chỉ đọc; đặt data_role và chính sách qua API là việc của #45. Ai không bị lọc (admin) do service quyết định. */

use uuid::Uuid;

use super::{db_error, Store};
use crate::{auth::WorkspaceId, engine::guard::RowFilter, error::AppError};

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
            .map(|(column, allowed_values)| RowFilter {
                column,
                allowed_values,
            })
            .collect())
    }
}
