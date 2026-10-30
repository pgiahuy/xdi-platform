// SPDX-License-Identifier: Apache-2.0
/* engine/store/columns.rs - hồ sơ cột của một phiên bản dataset: bảng columns và column_values.
Lớp cấu trúc (kiểu, số ô trống, min, max, vai trò, tần suất giá trị) do service/profile.rs tính và ghi một lần lúc nạp;
mô tả và bí danh (#23) sửa sau. Như mọi hàm của store, câu SQL lọc theo workspace_id lấy từ WorkspaceId: đọc hồ sơ của
phiên bản thuộc workspace khác ra rỗng. */

use std::time::Duration;

use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, is_query_canceled, ColumnAggregation, ProfileStatus, Store};
use crate::{auth::WorkspaceId, error::AppError};

/// Vai trò của một cột trong dataset, khớp CHECK của columns.role (0001_init.sql). Luật chọn ở service/profile.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRole {
    Dimension,
    Measure,
    Time,
    Key,
    Text,
    Ignore,
}

impl ProfileRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileRole::Dimension => "dimension",
            ProfileRole::Measure => "measure",
            ProfileRole::Time => "time",
            ProfileRole::Key => "key",
            ProfileRole::Text => "text",
            ProfileRole::Ignore => "ignore",
        }
    }
}

/// Đơn vị của một cột số đo, khớp CHECK của columns.unit (0007_column_unit.sql). Lớp ngữ nghĩa đề xuất; luật biểu đồ
/// so đơn vị để biết vẽ hai số đo chung một thang đo hay tách thành hai biểu đồ (ARCHITECTURE mục 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnUnit {
    Money,
    /// Số lượng hàng, sản phẩm.
    Quantity,
    Percent,
    /// Thời lượng: số ngày, số giờ.
    Duration,
    /// Đơn vị khác danh sách (điểm, cân nặng...): không so được với cột nào, kể cả cột cũng là other.
    Other,
}

impl ColumnUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            ColumnUnit::Money => "money",
            ColumnUnit::Quantity => "quantity",
            ColumnUnit::Percent => "percent",
            ColumnUnit::Duration => "duration",
            ColumnUnit::Other => "other",
        }
    }

    pub fn parse(text: &str) -> Option<ColumnUnit> {
        match text {
            "money" => Some(ColumnUnit::Money),
            "quantity" => Some(ColumnUnit::Quantity),
            "percent" => Some(ColumnUnit::Percent),
            "duration" => Some(ColumnUnit::Duration),
            "other" => Some(ColumnUnit::Other),
            _ => None,
        }
    }
}

/// Một giá trị của cột ít giá trị, kèm số dòng mang giá trị đó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewColumnValue {
    pub value: String,
    pub frequency: i64,
}

/// Hồ sơ lớp cấu trúc của một cột, chờ ghi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewColumn {
    pub position: i32,
    pub name: String,
    pub data_type: String,
    pub role: ProfileRole,
    /// Cờ nhạy cảm do luật của service/profile.rs đặt ngay lúc lập hồ sơ cấu trúc, để giá trị cột nhạy cảm không bao
    /// giờ rời máy chủ, kể cả trước khi lớp ngữ nghĩa (#23) chạy.
    pub is_sensitive: bool,
    pub null_count: i64,
    pub distinct_count: i64,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub time_format: Option<String>,
    /// Ghi vào column_values; rỗng với cột nhiều giá trị.
    pub values: Vec<NewColumnValue>,
    /// schema.bảng của bảng nối nhiều-một mà cột chép từ đó (bản chụp database, service/sources/); None với cột của
    /// bảng chính và cột nạp từ file.
    pub origin_table: Option<String>,
}

// position chỉ để sắp xếp trong SQL, description và aliases đợi ngữ cảnh prompt (#29) đọc ngoài test; gỡ allow khi
// có người đọc để clippy bắt lại mã thừa.
/// Một dòng của bảng columns: lớp cấu trúc (kiểu, số đo, vai trò; cờ nhạy cảm do luật của service/profile.rs đặt
/// ngay lúc lập hồ sơ), cộng mô tả và bí danh của lớp ngữ nghĩa (#23 ghi sau; chưa ghi thì mô tả null, bí danh rỗng).
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfileColumn {
    pub id: Uuid,
    pub position: i32,
    pub name: String,
    pub data_type: String,
    pub role: String,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    pub is_sensitive: bool,
    pub null_count: Option<i64>,
    pub distinct_count: Option<i64>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub time_format: Option<String>,
    /// Đơn vị của cột số đo (ColumnUnit::as_str); None khi lớp ngữ nghĩa chưa chạy hay không biết.
    pub unit: Option<String>,
    /// Bảng nối nhiều-một mà cột chép từ đó (NewColumn::origin_table).
    pub origin_table: Option<String>,
    /// Cách gộp mặc định (ColumnAggregation::as_str): sum hay avg; chỉ có nghĩa với cột measure.
    pub aggregation: String,
}

/// Một dòng của bảng column_values.
// aliases do #23 ghi, ngữ cảnh prompt (#28) đọc qua ValueRow; frequency chưa ai đọc ngoài test (thứ tự đã theo nó).
// Gỡ allow khi có người đọc trực tiếp.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ColumnValue {
    pub column_id: Uuid,
    pub value: String,
    #[allow(dead_code)]
    pub aliases: Vec<String>,
    #[allow(dead_code)]
    pub frequency: Option<i64>,
}

/// Một giá trị của một cột chiều không nhạy cảm, kèm bí danh: thứ service/ask/values.rs so với câu hỏi.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ValueRow {
    pub column_id: Uuid,
    pub column_name: String,
    pub value: String,
    pub aliases: Vec<String>,
}

/// Phần ngữ nghĩa của một cột, đã qua kiểm tra của service/semantic/mod.rs: tên cột đã đổi ra id, giới hạn độ dài đã áp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticColumnUpdate {
    pub column_id: Uuid,
    /// None khi mô hình không có mô tả hợp lệ cho cột này.
    pub description: Option<String>,
    pub aliases: Vec<String>,
    /// Vai trò mới khi mô hình đề xuất và luật cho phép (xem service/semantic/mod.rs); None là giữ vai trò cũ.
    pub role: Option<ProfileRole>,
    /// Đơn vị mô hình đề xuất cho cột số đo; None khi không có hay không hợp lệ.
    pub unit: Option<ColumnUnit>,
    /// Cách gộp mô hình đề xuất cho cột số đo; None là giữ cách đang có (mặc định sum).
    pub aggregation: Option<ColumnAggregation>,
}

/// Bí danh của một giá trị đã qua kiểm tra (giá trị có thật trong column_values của cột).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticValueUpdate {
    pub column_id: Uuid,
    pub value: String,
    pub aliases: Vec<String>,
}

impl Store {
    /// Ghi hồ sơ cột cho một phiên bản đã có mà còn pending (việc nền lúc khởi động; lúc nạp thì create_version ghi hồ
    /// sơ cùng phiên bản) và chuyển profile_status sang structure, trong một giao dịch: không bao giờ có phiên bản
    /// structure mà thiếu cột, hay cột ghi dở. Trả false, không ghi gì, khi phiên bản không còn pending hoặc không thuộc
    /// workspace này. UPDATE chạy trước và khóa dòng phiên bản, nên hai lần ghi cùng lúc cho một phiên bản thì lần sau
    /// chờ, thấy structure và bỏ qua.
    pub async fn insert_profile(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        columns: &[NewColumn],
    ) -> Result<bool, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin profile", e))?;
        let claimed = sqlx::query(
            "UPDATE dataset_versions SET profile_status = $3
             WHERE workspace_id = $1 AND id = $2 AND profile_status = $4",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(ProfileStatus::Structure.as_str())
        .bind(ProfileStatus::Pending.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("claim version for profile", e))?
        .rows_affected();
        // Ra khỏi hàm mà chưa commit thì sqlx tự rollback giao dịch.
        if claimed == 0 {
            return Ok(false);
        }

        insert_columns(&mut tx, ws, version_id, columns).await?;
        tx.commit()
            .await
            .map_err(|e| db_error("commit profile", e))?;
        Ok(true)
    }

    /// Cột của một phiên bản theo thứ tự trong bảng. Rỗng nếu phiên bản chưa có hồ sơ hoặc thuộc workspace khác.
    pub async fn list_columns(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
    ) -> Result<Vec<ProfileColumn>, AppError> {
        sqlx::query_as(
            "SELECT id, position, name, data_type, role, description, aliases, is_sensitive,
                 null_count, distinct_count, min_value, max_value, time_format, unit, origin_table,
                 COALESCE(aggregation, 'sum') AS aggregation
             FROM columns WHERE workspace_id = $1 AND version_id = $2 ORDER BY position",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list columns", e))
    }

    /// Mọi giá trị và bí danh của các cột chiều (role dimension) không nhạy cảm của một phiên bản, để so với câu hỏi
    /// trong bộ nhớ (service/ask/values.rs giữ sẵn theo phiên bản). Rỗng khi phiên bản thuộc workspace khác.
    ///
    /// Trả None khi chạy quá timeout (ghi log), không trả lỗi: khi đó prompt không có danh sách giá trị, mô hình chỉ thấy
    /// schema (tên cột, kiểu, mô tả, bí danh) mà câu hỏi vẫn trả lời được; trả lỗi thì chặn cả câu hỏi.
    pub async fn list_value_rows(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        timeout: Duration,
    ) -> Result<Option<Vec<ValueRow>>, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin list value rows", e))?;
        // set_config(..., true) như SET LOCAL: chỉ có hiệu lực tới hết giao dịch này, không lan sang kết nối khác
        // của pool. Giao dịch chỉ đọc, nên kết thúc bằng rollback (khi tx bị hủy) cũng như commit.
        sqlx::query("SELECT set_config('statement_timeout', $1, true)")
            .bind(format!("{}ms", timeout.as_millis().max(1)))
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("set list value rows timeout", e))?;
        let rows = sqlx::query_as(
            "SELECT c.id AS column_id, c.name AS column_name, v.value, v.aliases
             FROM columns c
             JOIN column_values v ON v.workspace_id = c.workspace_id AND v.column_id = c.id
             WHERE c.workspace_id = $1 AND c.version_id = $2 AND c.role = $3 AND NOT c.is_sensitive
             ORDER BY c.position, v.frequency DESC, v.value",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(ProfileRole::Dimension.as_str())
        .fetch_all(&mut *tx)
        .await;
        match rows {
            Ok(rows) => Ok(Some(rows)),
            Err(e) if is_query_canceled(&e) => {
                tracing::warn!(%version_id, timeout_ms = timeout.as_millis() as u64,
                    "loading column values timed out; answering without mapped values");
                Ok(None)
            }
            Err(e) => Err(db_error("list value rows", e)),
        }
    }

    /// Giá trị của các cột cho trước, mỗi cột giá trị nhiều dòng nhất trước. Id cột của workspace khác bị bỏ qua.
    pub async fn list_column_values(
        &self,
        ws: &WorkspaceId,
        column_ids: &[Uuid],
    ) -> Result<Vec<ColumnValue>, AppError> {
        sqlx::query_as(
            "SELECT column_id, value, aliases, frequency FROM column_values
             WHERE workspace_id = $1 AND column_id = ANY($2)
             ORDER BY column_id, frequency DESC, value",
        )
        .bind(ws.as_uuid())
        .bind(column_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list column values", e))
    }

    /// Bật cờ nhạy cảm cho các cột (chỉ chiều false -> true). Hai người gọi: lượt quét lúc khởi động cho phiên
    /// bản cũ (service/profile.rs) và cột do mô hình đề xuất thêm trong câu trả lời ngữ nghĩa (service/semantic/mod.rs,
    /// qua apply_semantic_profile).
    pub async fn mark_columns_sensitive(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        column_ids: &[Uuid],
    ) -> Result<(), AppError> {
        if column_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "UPDATE columns SET is_sensitive = true
             WHERE workspace_id = $1 AND version_id = $2 AND id = ANY($3)",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(column_ids)
        .execute(&self.pool)
        .await
        .map_err(|e| db_error("mark sensitive columns", e))?;
        Ok(())
    }

    /// Ghi hồ sơ lớp ngữ nghĩa (#23) cho một phiên bản: mô tả, bí danh và vai trò mới của cột, cột nhạy cảm do mô
    /// hình đề xuất thêm (chỉ chiều false -> true), bí danh giá trị, câu hỏi gợi ý, rồi chuyển profile_status sang
    /// semantic, trong một giao dịch. Như insert_profile, câu UPDATE đầu khóa dòng phiên bản và chỉ nhận phiên bản
    /// đang ở structure, nên hai lần ghi cùng lúc thì lần sau thấy semantic và bỏ qua (trả false, không ghi gì).
    /// Câu hỏi gợi ý nằm trên dataset_versions vì chúng đi theo phiên bản.
    pub async fn apply_semantic_profile(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        columns: &[SemanticColumnUpdate],
        values: &[SemanticValueUpdate],
        sensitive_columns: &[Uuid],
        suggested_questions: &[String],
    ) -> Result<bool, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin semantic profile", e))?;
        let claimed = sqlx::query(
            "UPDATE dataset_versions SET profile_status = $3, suggested_questions = $4
             WHERE workspace_id = $1 AND id = $2 AND profile_status = $5",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(ProfileStatus::Semantic.as_str())
        .bind(sqlx::types::Json(suggested_questions))
        .bind(ProfileStatus::Structure.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error("claim version for semantic profile", e))?
        .rows_affected();
        // Ra khỏi hàm mà chưa commit thì sqlx tự rollback giao dịch.
        if claimed == 0 {
            return Ok(false);
        }

        // Cách gộp chỉ điền khi còn NULL (chưa ai chọn): admin có thể đổi ở màn Chỉ số ngay sau khi nạp, trước khi lượt
        // chạy nền này xong; ghi đè thì lựa chọn đó mất mà không ai báo.
        for column in columns {
            let role = column.role.map(|r| r.as_str());
            let unit = column.unit.map(|u| u.as_str());
            let aggregation = column.aggregation.map(|a| a.as_str());
            sqlx::query(
                "UPDATE columns SET description = $3, aliases = $4, role = COALESCE($5, role), unit = $6,
                                    aggregation = COALESCE(aggregation, $7)
                 WHERE workspace_id = $1 AND id = $2",
            )
            .bind(ws.as_uuid())
            .bind(column.column_id)
            .bind(&column.description)
            .bind(&column.aliases)
            .bind(role)
            .bind(unit)
            .bind(aggregation)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("update column semantics", e))?;
        }
        for value in values {
            sqlx::query(
                "UPDATE column_values SET aliases = $4 WHERE workspace_id = $1 AND column_id = $2 AND value = $3",
            )
            .bind(ws.as_uuid())
            .bind(value.column_id)
            .bind(&value.value)
            .bind(&value.aliases)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("update value aliases", e))?;
        }
        // Cột nhạy cảm do mô hình đề xuất thêm: chỉ đặt, không bao giờ gỡ (mô hình không nói gì thì giữ nguyên).
        if !sensitive_columns.is_empty() {
            sqlx::query(
                "UPDATE columns SET is_sensitive = true WHERE workspace_id = $1 AND id = ANY($2)",
            )
            .bind(ws.as_uuid())
            .bind(sensitive_columns)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("mark llm-suggested sensitive columns", e))?;
        }
        tx.commit()
            .await
            .map_err(|e| db_error("commit semantic profile", e))?;
        Ok(true)
    }
}

/// Ghi các cột và giá trị của chúng cho một phiên bản, trên kết nối của giao dịch người gọi đã mở: insert_profile ở
/// đây và create_version (datasets.rs) dùng chung, để lỗi ở cột nào cũng rollback cả phiên bản lẫn hồ sơ.
pub(super) async fn insert_columns(
    conn: &mut PgConnection,
    ws: &WorkspaceId,
    version_id: Uuid,
    columns: &[NewColumn],
) -> Result<(), AppError> {
    for column in columns {
        let column_id: Uuid = sqlx::query_scalar(
            "INSERT INTO columns (workspace_id, version_id, position, name, data_type, role, is_sensitive, null_count,
                 distinct_count, min_value, max_value, time_format, origin_table)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(column.position)
        .bind(&column.name)
        .bind(&column.data_type)
        .bind(column.role.as_str())
        .bind(column.is_sensitive)
        .bind(column.null_count)
        .bind(column.distinct_count)
        .bind(&column.min_value)
        .bind(&column.max_value)
        .bind(&column.time_format)
        .bind(&column.origin_table)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| db_error("insert column", e))?;
        if column.values.is_empty() {
            continue;
        }
        // Mọi giá trị của một cột trong một câu: hai mảng song song, UNNEST ghép lại thành từng dòng.
        let values: Vec<&str> = column.values.iter().map(|v| v.value.as_str()).collect();
        let frequencies: Vec<i64> = column.values.iter().map(|v| v.frequency).collect();
        sqlx::query(
            "INSERT INTO column_values (workspace_id, column_id, value, frequency)
             SELECT $1, $2, v.value, v.frequency FROM UNNEST($3::text[], $4::bigint[]) AS v(value, frequency)",
        )
        .bind(ws.as_uuid())
        .bind(column_id)
        .bind(&values)
        .bind(&frequencies)
        .execute(&mut *conn)
        .await
        .map_err(|e| db_error("insert column values", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{drop_workspace, store, temp_workspace};
    use super::super::{NewVersion, SourceKind};
    use super::*;

    fn new_column(
        position: i32,
        name: &str,
        role: ProfileRole,
        values: &[(&str, i64)],
    ) -> NewColumn {
        NewColumn {
            position,
            name: name.to_string(),
            data_type: "VARCHAR".to_string(),
            role,
            is_sensitive: false,
            null_count: 0,
            distinct_count: values.len() as i64,
            min_value: None,
            max_value: None,
            time_format: None,
            origin_table: None,
            values: values
                .iter()
                .map(|(value, frequency)| NewColumnValue {
                    value: value.to_string(),
                    frequency: *frequency,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn profile_is_written_once_and_read_per_workspace() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let a = temp_workspace(&store).await;
        let b = temp_workspace(&store).await;
        let src = store.create_local_source(&a).await.unwrap();
        let ds = store
            .create_dataset(&a, src, "Sales", "sales", None)
            .await
            .unwrap();
        let new = NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: "h1",
            parquet_path: "x",
            row_count: 3,
            source_model: None,
        };
        let v = store.create_version(&a, ds.id, &new, &[]).await.unwrap();
        assert_eq!(v.profile_status, "pending");

        let mut date = new_column(1, "order date", ProfileRole::Time, &[]);
        date.time_format = Some("%m/%d/%Y %H:%M".to_string());
        let columns = [
            new_column(
                0,
                "it's \"Market\"",
                ProfileRole::Dimension,
                &[("Europe", 2), ("LATAM", 1)],
            ),
            date,
            new_column(2, "_source_file", ProfileRole::Ignore, &[("a.csv", 3)]),
        ];
        // Workspace khác không ghi được vào phiên bản này.
        assert!(!store.insert_profile(&b, v.id, &columns).await.unwrap());
        assert!(store.insert_profile(&a, v.id, &columns).await.unwrap());
        // Lần ghi thứ hai (nạp trùng, việc nền lúc khởi động) không ghi lại.
        assert!(!store.insert_profile(&a, v.id, &columns).await.unwrap());

        let current = store
            .find_version_by_hash(&a, ds.id, "h1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.profile_status, "structure");

        let read = store.list_columns(&a, v.id).await.unwrap();
        let names: Vec<(&str, &str)> = read
            .iter()
            .map(|c| (c.name.as_str(), c.role.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("it's \"Market\"", "dimension"),
                ("order date", "time"),
                ("_source_file", "ignore")
            ]
        );
        assert_eq!(read[1].time_format.as_deref(), Some("%m/%d/%Y %H:%M"));
        let ids: Vec<Uuid> = read.iter().map(|c| c.id).collect();
        let values = store.list_column_values(&a, &ids).await.unwrap();
        let market: Vec<(&str, Option<i64>)> = values
            .iter()
            .filter(|v| v.column_id == read[0].id)
            .map(|v| (v.value.as_str(), v.frequency))
            .collect();
        assert_eq!(market, [("Europe", Some(2)), ("LATAM", Some(1))]);
        assert_eq!(values.len(), 3);

        // Workspace khác đọc ra rỗng, kể cả khi biết đúng id.
        assert!(store.list_columns(&b, v.id).await.unwrap().is_empty());
        assert!(store.list_column_values(&b, &ids).await.unwrap().is_empty());

        drop_workspace(&store, a).await;
        drop_workspace(&store, b).await;
    }

    /// Một cột ghi lỗi giữa chừng (ở đây: hai cột cùng vị trí, vi phạm UNIQUE (version_id, position)) thì cả giao dịch
    /// rollback: create_version không để lại phiên bản, insert_profile để phiên bản ở pending và không có cột nào.
    #[tokio::test]
    async fn failed_profile_writes_roll_back() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws).await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "Sales", "sales", None)
            .await
            .unwrap();
        let new = |hash| NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: hash,
            parquet_path: "x",
            row_count: 3,
            source_model: None,
        };
        let clash = [
            new_column(0, "a", ProfileRole::Dimension, &[("x", 1)]),
            new_column(0, "b", ProfileRole::Text, &[]),
        ];

        assert!(store
            .create_version(&ws, ds.id, &new("h1"), &clash)
            .await
            .is_err());
        assert!(store
            .find_version_by_hash(&ws, ds.id, "h1")
            .await
            .unwrap()
            .is_none());
        // Không còn phiên bản nào, nên dataset vừa tạo xóa được (như ingest làm khi nạp lỗi).
        store
            .delete_dataset_without_versions(&ws, ds.id)
            .await
            .unwrap();
        assert!(store.get_dataset(&ws, ds.id).await.unwrap().is_none());

        let ds = store
            .create_dataset(
                &ws,
                store.create_local_source(&ws).await.unwrap(),
                "Sales",
                "sales",
                None,
            )
            .await
            .unwrap();
        let v = store
            .create_version(&ws, ds.id, &new("h2"), &[])
            .await
            .unwrap();
        assert!(store.insert_profile(&ws, v.id, &clash).await.is_err());
        let v = store
            .find_version_by_hash(&ws, ds.id, "h2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.profile_status, "pending");
        assert!(store.list_columns(&ws, v.id).await.unwrap().is_empty());
        // Dataset đã có phiên bản thì không bị xóa.
        store
            .delete_dataset_without_versions(&ws, ds.id)
            .await
            .unwrap();
        assert!(store.get_dataset(&ws, ds.id).await.unwrap().is_some());

        drop_workspace(&store, ws).await;
    }

    /// Hồ sơ ngữ nghĩa: chỉ ghi khi phiên bản đang ở structure (ghi lần hai hay của workspace khác ra false, không
    /// đổi gì), ghi đủ mô tả, bí danh, vai trò mới, bí danh giá trị và câu gợi ý trong một giao dịch.
    #[tokio::test]
    async fn semantic_profile_is_written_once_and_per_workspace() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let a = temp_workspace(&store).await;
        let b = temp_workspace(&store).await;
        let src = store.create_local_source(&a).await.unwrap();
        let ds = store
            .create_dataset(&a, src, "Sales", "sales", None)
            .await
            .unwrap();
        let new = |hash| NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: hash,
            parquet_path: "x",
            row_count: 3,
            source_model: None,
        };
        // Không có cột thì pending (chưa qua lớp cấu trúc): lớp ngữ nghĩa không ghi được.
        let pending = store
            .create_version(&a, ds.id, &new("h0"), &[])
            .await
            .unwrap();
        let columns = [
            new_column(
                0,
                "Market",
                ProfileRole::Dimension,
                &[("Europe", 2), ("LATAM", 1)],
            ),
            new_column(1, "Order Id", ProfileRole::Measure, &[]),
        ];
        let v = store
            .create_version(&a, ds.id, &new("h1"), &columns)
            .await
            .unwrap();

        let read = store.list_columns(&a, v.id).await.unwrap();
        let updates = vec![
            SemanticColumnUpdate {
                column_id: read[0].id,
                description: Some("Thị trường".to_string()),
                aliases: vec!["thị trường".to_string()],
                role: None,
                unit: Some(ColumnUnit::Money),
                aggregation: None,
            },
            SemanticColumnUpdate {
                column_id: read[1].id,
                description: None,
                aliases: Vec::new(),
                role: Some(ProfileRole::Key),
                unit: None,
                aggregation: Some(ColumnAggregation::Avg),
            },
        ];
        let values = vec![SemanticValueUpdate {
            column_id: read[0].id,
            value: "Europe".to_string(),
            aliases: vec!["châu Âu".to_string()],
        }];
        let questions = vec!["Doanh thu theo thị trường".to_string()];
        // Mô hình đề xuất đánh dấu thêm cột nhạy cảm (chỉ chiều false -> true).
        let sensitive = vec![read[1].id];

        // Phiên bản pending và workspace khác đều bị từ chối.
        assert!(!store
            .apply_semantic_profile(&a, pending.id, &updates, &values, &sensitive, &questions)
            .await
            .unwrap());
        assert!(!store
            .apply_semantic_profile(&b, v.id, &updates, &values, &sensitive, &questions)
            .await
            .unwrap());

        assert!(store
            .apply_semantic_profile(&a, v.id, &updates, &values, &sensitive, &questions)
            .await
            .unwrap());
        // Ghi lần hai (hai lượt chạy song song) không đổi gì.
        assert!(!store
            .apply_semantic_profile(&a, v.id, &[], &[], &[], &[])
            .await
            .unwrap());

        let read = store.list_columns(&a, v.id).await.unwrap();
        assert_eq!(read[0].description.as_deref(), Some("Thị trường"));
        assert_eq!(read[0].aliases, ["thị trường"]);
        assert_eq!(read[0].unit.as_deref(), Some("money"));
        assert_eq!(read[0].role, "dimension");
        assert!(!read[0].is_sensitive);
        assert_eq!(read[1].role, "key");
        assert!(read[1].is_sensitive);
        // Không đề xuất cách gộp thì giữ mặc định của migration.
        assert_eq!(read[0].aggregation, "sum");
        assert_eq!(read[1].aggregation, "avg");
        let vals = store.list_column_values(&a, &[read[0].id]).await.unwrap();
        let europe = vals.iter().find(|v| v.value == "Europe").unwrap();
        assert_eq!(europe.aliases, ["châu Âu"]);
        let latam = vals.iter().find(|v| v.value == "LATAM").unwrap();
        assert!(latam.aliases.is_empty());

        let v = store
            .find_version_by_hash(&a, ds.id, "h1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.profile_status, "semantic");
        store.set_current_version(&a, ds.id, v.id).await.unwrap();
        let ds = store.get_dataset(&a, ds.id).await.unwrap().unwrap();
        assert_eq!(ds.suggested_questions, ["Doanh thu theo thị trường"]);
        // Workspace khác không đọc được phần đã ghi.
        assert!(store.list_columns(&b, v.id).await.unwrap().is_empty());

        drop_workspace(&store, a).await;
        drop_workspace(&store, b).await;
    }

    /// Admin đổi cách gộp khi lớp ngữ nghĩa còn chạy nền: lượt chạy xong sau đó chỉ điền cột chưa ai chọn, không ghi
    /// đè lựa chọn của admin, kể cả khi admin chọn sum.
    #[tokio::test]
    async fn semantic_profile_keeps_an_aggregation_the_admin_set() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let src = store.create_local_source(&ws).await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "Credits", "credits", None)
            .await
            .unwrap();
        let columns = [
            new_column(0, "Price", ProfileRole::Measure, &[]),
            new_column(1, "Credits", ProfileRole::Measure, &[]),
        ];
        let version = NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: "h1",
            parquet_path: "x",
            row_count: 3,
            source_model: None,
        };
        let v = store
            .create_version(&ws, ds.id, &version, &columns)
            .await
            .unwrap();
        let read = store.list_columns(&ws, v.id).await.unwrap();
        // Chưa ai chọn thì đọc là sum.
        assert_eq!(read[0].aggregation, "sum");
        assert!(store
            .set_column_aggregation(&ws, v.id, "Price", ColumnAggregation::Sum)
            .await
            .unwrap());

        let suggest_avg = |column_id| SemanticColumnUpdate {
            column_id,
            description: None,
            aliases: Vec::new(),
            role: None,
            unit: None,
            aggregation: Some(ColumnAggregation::Avg),
        };
        let updates = vec![suggest_avg(read[0].id), suggest_avg(read[1].id)];
        assert!(store
            .apply_semantic_profile(&ws, v.id, &updates, &[], &[], &[])
            .await
            .unwrap());

        let read = store.list_columns(&ws, v.id).await.unwrap();
        assert_eq!(read[0].aggregation, "sum");
        assert_eq!(read[1].aggregation, "avg");

        drop_workspace(&store, ws).await;
    }
}
