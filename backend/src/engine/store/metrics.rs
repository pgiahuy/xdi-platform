// SPDX-License-Identifier: Apache-2.0
/* engine/store/metrics.rs - định nghĩa chỉ số của dataset (bảng metrics), để prompt sinh SQL (service/ask_prompt.rs)
dùng đúng công thức của tổ chức thay vì để mô hình tự đoán "doanh thu" là cột nào.
Sửa một định nghĩa là thêm dòng cùng tên với version lớn hơn, không ghi đè (0001_init.sql), nên đọc chỉ lấy bản mới
nhất của mỗi tên. Ghi định nghĩa là việc của bước seed (#25) và màn Thiết lập chỉ số, chưa có ở đây. */

use super::{db_error, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Bản mới nhất của một chỉ số.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MetricDefinition {
    pub name: String,
    pub description: String,
    /// Biểu thức tổng hợp trên cột của dataset, ví dụ SUM("Order Item Total").
    pub expression: String,
    /// Điều kiện đi kèm biểu thức, ví dụ "Order Status" <> 'CANCELED'.
    pub filter: Option<String>,
    pub time_column: Option<String>,
    pub unit: Option<String>,
    /// False khi chưa được admin xác nhận (status = 'unconfirmed'); prompt ghi rõ để mô hình biết.
    pub confirmed: bool,
}

impl Store {
    /// Chỉ số của một dataset, mỗi tên một dòng, xếp theo tên để prompt dựng ra luôn giống nhau với cùng dữ liệu.
    /// Mỗi tên lấy bản đã xác nhận mới nhất; chưa có bản nào được xác nhận mới lấy bản chưa xác nhận mới nhất. Một bản
    /// sửa đang chờ duyệt không được thay công thức đã duyệt: bản nháp chỉ dùng khi chưa có gì khác. Dataset của
    /// workspace khác trả rỗng.
    pub async fn list_metrics(
        &self,
        ws: &WorkspaceId,
        dataset_id: uuid::Uuid,
    ) -> Result<Vec<MetricDefinition>, AppError> {
        // DISTINCT ON giữ dòng đầu của mỗi tên theo ORDER BY: bản đã xác nhận trước (true xếp trước false khi DESC),
        // rồi version lớn nhất.
        sqlx::query_as(
            "SELECT DISTINCT ON (name)
                    name, description, expression, filter, time_column, unit, status = 'confirmed' AS confirmed
             FROM metrics
             WHERE workspace_id = $1 AND dataset_id = $2
             ORDER BY name, status = 'confirmed' DESC, version DESC",
        )
        .bind(ws.as_uuid())
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list metrics", e))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, exec, store, temp_workspace};
    use super::*;

    /// Chèn thẳng bằng SQL vì store chưa có hàm ghi chỉ số (#25).
    async fn insert_metric(
        store: &Store,
        ws: &WorkspaceId,
        dataset_id: uuid::Uuid,
        name: &str,
        expression: &str,
        status: &str,
        version: i32,
    ) {
        exec(
            store,
            &format!(
                "INSERT INTO metrics (workspace_id, dataset_id, name, expression, status, origin, version)
                 VALUES ('{}', '{dataset_id}', '{name}', '{expression}', '{status}', 'sample', {version})",
                ws.as_uuid()
            ),
        )
        .await;
    }

    #[tokio::test]
    async fn confirmed_version_first_then_latest_in_own_workspace_only() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws).await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "DataCo", "dataco", None)
            .await
            .unwrap();
        insert_metric(
            &store,
            &ws,
            ds.id,
            "doanh thu",
            "SUM(\"Sales\")",
            "confirmed",
            1,
        )
        .await;
        insert_metric(
            &store,
            &ws,
            ds.id,
            "doanh thu",
            "SUM(\"Order Item Total\")",
            "confirmed",
            2,
        )
        .await;
        insert_metric(
            &store,
            &ws,
            ds.id,
            "lợi nhuận",
            "SUM(\"Order Profit Per Order\")",
            "unconfirmed",
            1,
        )
        .await;
        // Bản sửa đang chờ duyệt, version lớn hơn: không thay công thức đã duyệt của doanh thu.
        insert_metric(
            &store,
            &ws,
            ds.id,
            "doanh thu",
            "SUM(\"Sales\") - SUM(\"Order Item Discount\")",
            "unconfirmed",
            3,
        )
        .await;
        // Lợi nhuận chưa có bản nào được duyệt: lấy bản nháp mới nhất.
        insert_metric(
            &store,
            &ws,
            ds.id,
            "lợi nhuận",
            "SUM(\"Benefit per order\")",
            "unconfirmed",
            2,
        )
        .await;

        let metrics = store.list_metrics(&ws, ds.id).await.unwrap();
        let got: Vec<(&str, &str, bool)> = metrics
            .iter()
            .map(|m| (m.name.as_str(), m.expression.as_str(), m.confirmed))
            .collect();
        assert_eq!(
            got,
            [
                ("doanh thu", "SUM(\"Order Item Total\")", true),
                ("lợi nhuận", "SUM(\"Benefit per order\")", false),
            ]
        );

        // Biết đúng id dataset vẫn không đọc được chỉ số từ workspace khác.
        let other = temp_workspace(&store).await;
        assert!(store.list_metrics(&other, ds.id).await.unwrap().is_empty());

        drop_workspace(&store, ws).await;
        drop_workspace(&store, other).await;
    }
}
