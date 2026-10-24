// SPDX-License-Identifier: Apache-2.0
/* engine/store/columns.rs - hồ sơ cột của một phiên bản dataset: bảng columns và column_values.
Lớp cấu trúc (kiểu, số ô trống, min, max, vai trò, tần suất giá trị) do service/profile.rs tính và ghi một lần lúc nạp;
mô tả và bí danh (#23) sửa sau. Như mọi hàm của store, câu SQL lọc theo workspace_id lấy từ WorkspaceId: đọc hồ sơ của
phiên bản thuộc workspace khác ra rỗng. */

use std::time::Duration;

use sqlx::PgConnection;
use uuid::Uuid;

use super::{db_error, is_query_canceled, ProfileStatus, Store};
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
    pub null_count: i64,
    pub distinct_count: i64,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub time_format: Option<String>,
    /// Ghi vào column_values; rỗng với cột nhiều giá trị.
    pub values: Vec<NewColumnValue>,
}

// Số đo (null_count, distinct_count, min, max) chưa ai đọc ngoài test: chờ câu "tại sao" (#38) và màn xem hồ sơ. Gỡ
// allow khi có người đọc để clippy bắt lại mã thừa.
/// Một dòng của bảng columns: lớp cấu trúc, cộng mô tả, bí danh và cờ nhạy cảm của lớp ngữ nghĩa (#23 ghi; chưa ghi
/// thì mô tả null, bí danh rỗng, không nhạy cảm).
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfileColumn {
    pub id: Uuid,
    pub position: i32,
    pub name: String,
    pub data_type: String,
    pub role: String,
    pub null_count: Option<i64>,
    pub distinct_count: Option<i64>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub time_format: Option<String>,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    pub is_sensitive: bool,
}

// frequency chưa ai đọc ngoài test (thứ tự đã theo nó); gỡ allow khi có người đọc.
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ColumnValue {
    pub column_id: Uuid,
    pub value: String,
    pub frequency: Option<i64>,
}

/// Một giá trị trong column_values khớp thô một cụm của câu hỏi (find_values): bằng nhau sau bỏ dấu, hoặc khớp gần
/// cùng số từ. Điểm cuối cùng, và việc so có dấu, do service/ask_match.rs làm.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ValueHit {
    /// Cụm của câu hỏi, đúng chuỗi người gọi truyền vào.
    pub phrase: String,
    pub column_name: String,
    pub value: String,
    /// Chữ đã khớp: chính giá trị hoặc một bí danh của nó.
    pub matched_text: String,
    /// Bằng nhau sau khi bỏ dấu, chữ thường, bỏ dấu câu.
    pub same_folded: bool,
}

/// Số dòng tối đa find_values trả về. Chặn trên cho câu hỏi dài khớp nhiều giá trị; service chỉ giữ vài giá trị mỗi
/// cụm, nên cắt ở đây không làm mất khớp tốt (câu SQL xếp khớp bằng nhau trước).
const FIND_VALUES_MAX_ROWS: i64 = 500;

// Câu SQL của find_values. Tham số: $1 workspace, $2 phiên bản, $3 cụm chỉ khớp bằng nhau, $4 cụm nhận khớp gần,
// $5 vai trò dimension, $6 ngưỡng similarity của bước lọc thô, $7 số dòng tối đa.
//
// Bỏ dấu cả hai phía bằng cùng một biểu thức: immutable_unaccent, lower, rồi đổi mỗi dãy ký tự không phải chữ hay số
// thành một dấu cách, để "Miền Nam", "mien nam" và "MIỀN-NAM" thành một chuỗi. Bỏ dấu trước rồi mới lower thì chữ đã
// là ASCII, locale nào cũng như nhau. So có dấu thì service/ask_match.rs làm ở Rust, vì lower() và cách pg_trgm tách
// từ theo locale của database.
//
// Lọc theo phiên bản trước (target: giá trị và bí danh của các cột chiều không nhạy cảm của phiên bản này, bỏ dấu một
// lần), rồi mới so: dữ liệu của workspace và phiên bản khác không làm truy vấn chậm theo. Không dùng chỉ mục
// column_values_trgm_idx: chỉ mục phủ mọi workspace, và với nhiều dữ liệu của workspace khác thì planner bỏ nó.
// Hai nhánh, vì gộp làm một (bằng nhau OR similarity) thì Postgres tính similarity cho cả cụm chỉ nhận khớp bằng nhau:
// - Bằng nhau: hash join trên chuỗi đã bỏ dấu.
// - Khớp gần: chỉ giữa cụm và chuỗi cùng số từ (hash join trên số từ, xem ask_match.rs vì sao), rồi similarity từ $6.
//
// Số đo ở chú thích của find_values.
const FIND_VALUES_SQL: &str = "
WITH phrase AS MATERIALIZED (
    SELECT p.text, p.fuzzy, f.folded, cardinality(string_to_array(f.folded, ' ')) AS words
    FROM (SELECT text, false AS fuzzy FROM UNNEST($3::text[]) AS e(text)
          UNION ALL
          SELECT text, true FROM UNNEST($4::text[]) AS f(text)) AS p
    CROSS JOIN LATERAL (
        SELECT btrim(regexp_replace(lower(immutable_unaccent(p.text)), '[^[:alnum:]]+', ' ', 'g')) AS folded
    ) AS f
),
target AS MATERIALIZED (
    SELECT c.name AS column_name, v.value, t.text AS matched_text, f.folded,
           cardinality(string_to_array(f.folded, ' ')) AS words
    FROM columns c
    JOIN column_values v ON v.workspace_id = c.workspace_id AND v.column_id = c.id
    CROSS JOIN LATERAL UNNEST(array_prepend(v.value, v.aliases)) AS t(text)
    CROSS JOIN LATERAL (
        SELECT btrim(regexp_replace(lower(immutable_unaccent(t.text)), '[^[:alnum:]]+', ' ', 'g')) AS folded
    ) AS f
    WHERE c.workspace_id = $1 AND c.version_id = $2 AND c.role = $5 AND NOT c.is_sensitive
),
hit AS (
    SELECT phrase.text AS phrase, target.column_name, target.value, target.matched_text, true AS same_folded
    FROM phrase JOIN target ON target.folded = phrase.folded
    UNION ALL
    SELECT phrase.text, target.column_name, target.value, target.matched_text, false
    FROM phrase JOIN target ON target.words = phrase.words
    WHERE phrase.fuzzy AND target.folded <> phrase.folded AND similarity(phrase.folded, target.folded) >= $6
)
SELECT phrase, column_name, value, matched_text, same_folded FROM hit
ORDER BY same_folded DESC, column_name, value
LIMIT $7";

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
            "SELECT id, position, name, data_type, role, null_count, distinct_count, min_value, max_value, time_format,
                 description, aliases, is_sensitive
             FROM columns WHERE workspace_id = $1 AND version_id = $2 ORDER BY position",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list columns", e))
    }

    /// Giá trị của các cột chiều (role dimension, không nhạy cảm) của một phiên bản khớp thô các cụm của câu hỏi, so
    /// trên giá trị và từng bí danh của nó. exact: cụm chỉ nhận khớp bằng nhau; fuzzy: cụm nhận cả khớp gần cùng số
    /// từ, similarity sau bỏ dấu từ min_similarity. Rỗng khi phiên bản thuộc workspace khác. Câu SQL: FIND_VALUES_SQL.
    ///
    /// Chạy quá timeout thì trả rỗng và ghi log, không trả lỗi: thiếu giá trị đã ánh xạ thì câu hỏi vẫn trả lời được
    /// (mô hình thấy giá trị mẫu), còn lỗi thì chặn cả câu hỏi.
    ///
    /// Đã đo (Postgres 17 của docker compose trên máy phát triển, \timing của psql, 5 lần; giá trị 1 đến 3 âm tiết sinh
    /// ngẫu nhiên, mỗi giá trị 2 bí danh; câu 40 từ không từ dừng, 40 cụm khớp gần và 150 cụm khớp bằng nhau như
    /// service/ask_context.rs gửi): 600 giá trị khoảng 25 ms; 20 000 giá trị (100 cột chiều, mỗi cột 200 giá trị)
    /// khoảng 470 ms, trong đó bỏ dấu 60 000 chuỗi khoảng 240 ms; không đổi khi bảng có thêm 1 triệu dòng của
    /// workspace khác. Cùng câu mà gửi cả 117 cụm khớp gần thì khoảng 2,3 giây, vì vậy có trần số cụm.
    pub async fn find_values(
        &self,
        ws: &WorkspaceId,
        version_id: Uuid,
        exact: &[String],
        fuzzy: &[String],
        min_similarity: f32,
        timeout: Duration,
    ) -> Result<Vec<ValueHit>, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_error("begin find values", e))?;
        // set_config(..., true) như SET LOCAL: chỉ có hiệu lực tới hết giao dịch này, không lan sang kết nối khác
        // của pool.
        sqlx::query("SELECT set_config('statement_timeout', $1, true)")
            .bind(format!("{}ms", timeout.as_millis()))
            .execute(&mut *tx)
            .await
            .map_err(|e| db_error("set find values timeout", e))?;
        let found = sqlx::query_as(FIND_VALUES_SQL)
            .bind(ws.as_uuid())
            .bind(version_id)
            .bind(exact)
            .bind(fuzzy)
            .bind(ProfileRole::Dimension.as_str())
            .bind(min_similarity)
            .bind(FIND_VALUES_MAX_ROWS)
            .fetch_all(&mut *tx)
            .await;
        // Chỉ đọc, nên kết thúc giao dịch bằng rollback (khi tx bị hủy) cũng như commit.
        match found {
            Ok(hits) => Ok(hits),
            Err(e) if is_query_canceled(&e) => {
                tracing::warn!(%version_id, timeout_ms = timeout.as_millis() as u64,
                    "value search timed out; answering without mapped values");
                Ok(Vec::new())
            }
            Err(e) => Err(db_error("find values", e)),
        }
    }

    /// Tối đa per_column giá trị nhiều dòng nhất của mỗi cột cho trước, theo cột rồi tần suất giảm dần. Id cột của
    /// workspace khác bị bỏ qua.
    pub async fn list_sample_values(
        &self,
        ws: &WorkspaceId,
        column_ids: &[Uuid],
        per_column: i64,
    ) -> Result<Vec<ColumnValue>, AppError> {
        sqlx::query_as(
            "SELECT column_id, value, frequency FROM (
                 SELECT column_id, value, frequency,
                        row_number() OVER (PARTITION BY column_id ORDER BY frequency DESC, value) AS rank
                 FROM column_values WHERE workspace_id = $1 AND column_id = ANY($2)
             ) AS ranked
             WHERE rank <= $3
             ORDER BY column_id, rank",
        )
        .bind(ws.as_uuid())
        .bind(column_ids)
        .bind(per_column)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list sample values", e))
    }

    /// Giá trị của các cột cho trước, mỗi cột giá trị nhiều dòng nhất trước. Id cột của workspace khác bị bỏ qua.
    // Chưa ai gọi ngoài test: kiểm form câu "tại sao" (#38) cần đủ danh sách. Gỡ allow khi có người gọi.
    #[allow(dead_code)]
    pub async fn list_column_values(
        &self,
        ws: &WorkspaceId,
        column_ids: &[Uuid],
    ) -> Result<Vec<ColumnValue>, AppError> {
        sqlx::query_as(
            "SELECT column_id, value, frequency FROM column_values
             WHERE workspace_id = $1 AND column_id = ANY($2)
             ORDER BY column_id, frequency DESC, value",
        )
        .bind(ws.as_uuid())
        .bind(column_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_error("list column values", e))
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
            "INSERT INTO columns (workspace_id, version_id, position, name, data_type, role, null_count,
                 distinct_count, min_value, max_value, time_format)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
        )
        .bind(ws.as_uuid())
        .bind(version_id)
        .bind(column.position)
        .bind(&column.name)
        .bind(&column.data_type)
        .bind(column.role.as_str())
        .bind(column.null_count)
        .bind(column.distinct_count)
        .bind(&column.min_value)
        .bind(&column.max_value)
        .bind(&column.time_format)
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
            null_count: 0,
            distinct_count: values.len() as i64,
            min_value: None,
            max_value: None,
            time_format: None,
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
}
