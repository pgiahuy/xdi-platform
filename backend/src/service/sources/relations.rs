// SPDX-License-Identifier: Apache-2.0
/* service/sources/relations.rs - tìm quan hệ khóa ngoại quanh một bảng của nguồn Postgres, để chụp bảng chính (bảng sự
kiện, ví dụ order_items) kèm các bảng nó trỏ tới (orders, products) thành một dataset. Dựng câu SELECT nối các bảng ở
join_sql.rs; service/sources/mod.rs gọi cả hai lúc chụp.

Chỉ nối nhiều-một: đi theo khóa ngoại RỜI bảng chính, tới bảng mà cột đích là khóa chính hay có ràng buộc UNIQUE. Mỗi
dòng của bảng chính khi đó khớp tối đa một dòng bên kia, nên số dòng không đổi và mọi tính năng một bảng (hồ sơ, guard,
hỏi, "tại sao", truy dòng nguồn) chạy y như cũ. Đi thêm được một bước từ bảng vừa nối (order_items -> orders ->
customers), không xa hơn: hai bước đủ cho mô hình sao và bông tuyết thường gặp, và mỗi bước thêm cột làm prompt dài ra.
Khóa ngoại TRỎ VÀO bảng chính (orders <- order_items khi chọn orders) là một-nhiều: nối sẽ nhân dòng, nên chỉ liệt kê
kèm lý do, không bao giờ nối.

Mọi thông tin đọc từ pg_catalog (tên ràng buộc, bảng, cột, quyền), không đọc dòng dữ liệu nào. pg_catalog cho mọi tài
khoản xem tên bảng và ràng buộc, kể cả bảng không có quyền đọc; bảng như vậy được liệt kê mà không nối. */

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnection;

use super::postgres::{query_error, SourceTable};
use crate::error::AppError;

/// Số bước tối đa đi theo khóa ngoại từ bảng chính.
pub const MAX_DEPTH: u8 = 2;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RelationsRequest {
    /// `schema.table` from the table list.
    pub table: String,
}

/// One foreign key around the chosen table, as the UI lists it.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SourceRelation {
    /// Stable id to send back in the snapshot request: `schema.table/constraint`, two hops joined with `>`.
    pub id: String,
    /// 1 for a foreign key of the chosen table, 2 for a foreign key of a table joined at depth 1.
    pub depth: u8,
    /// Id of the depth-1 relation this one continues; null at depth 1.
    #[schema(required)]
    pub parent: Option<String>,
    pub constraint: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
    /// True when the join keeps one row per row of the chosen table and the account can read the target.
    pub joinable: bool,
    /// Why the relation is not joined, in Vietnamese; null when joinable.
    #[schema(required)]
    pub reason: Option<String>,
    /// Columns the join would add (the target table's readable columns minus the key columns).
    pub column_count: usize,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RelationsResponse {
    pub table: String,
    /// Readable columns of the chosen table.
    pub column_count: usize,
    /// Foreign keys leaving the chosen table (depth 1) or a table joined at depth 1 (depth 2), in join order.
    pub many_to_one: Vec<SourceRelation>,
    /// Foreign keys of other tables pointing into the chosen table: never joined, they would repeat its rows.
    pub one_to_many: Vec<SourceRelation>,
}

/// Một khóa ngoại đọc từ pg_constraint, cột theo đúng thứ tự của khóa (khóa nhiều cột ghép theo cặp).
#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub constraint: String,
    pub from: SourceTable,
    pub from_columns: Vec<String>,
    pub to: SourceTable,
    pub to_columns: Vec<String>,
    /// Cột đích đúng bằng tập cột của khóa chính hay một ràng buộc UNIQUE của bảng đích.
    pub unique_target: bool,
    /// Tài khoản có quyền SELECT trên bảng đích.
    pub readable_target: bool,
}

/// Một quan hệ đã tìm, kèm cột của bảng đích để dựng câu SELECT.
#[derive(Debug, Clone)]
pub struct Relation {
    pub info: SourceRelation,
    pub key: ForeignKey,
    /// Cột đọc được của bảng đích, theo thứ tự trong bảng.
    pub target_columns: Vec<String>,
}

/// Kết quả tìm quanh một bảng: cột của bảng chính và các quan hệ.
pub struct Discovered {
    pub fact_columns: Vec<String>,
    pub many_to_one: Vec<Relation>,
    pub one_to_many: Vec<SourceRelation>,
}

impl Discovered {
    pub fn build_response(&self, table: &SourceTable) -> RelationsResponse {
        RelationsResponse {
            table: table.full_name(),
            column_count: self.fact_columns.len(),
            many_to_one: self.many_to_one.iter().map(|r| r.info.clone()).collect(),
            one_to_many: self.one_to_many.clone(),
        }
    }

    /// Quan hệ theo id người dùng chọn, giữ thứ tự tìm thấy (bước 1 trước bước 2 của nó), để tên cột và câu SQL như
    /// nhau mỗi lần chụp. Chọn bước 2 thì bước 1 của nó được nối theo, vì câu JOIN cần bảng ở giữa. Id lạ, hay quan hệ
    /// không nối được, thì từ chối cả lần chụp: chụp thiếu một bảng mà không báo thì dataset khác điều người dùng chọn.
    pub fn pick(&self, ids: &[String]) -> Result<Vec<&Relation>, AppError> {
        let mut wanted: HashSet<&str> = HashSet::new();
        for id in ids {
            let relation = self
                .many_to_one
                .iter()
                .find(|r| &r.info.id == id && r.info.joinable)
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Quan hệ {id} không còn trong database hoặc không nối được: mở lại danh sách quan hệ \
                         của bảng rồi chọn lại"
                    ))
                })?;
            wanted.insert(&relation.info.id);
            if let Some(parent) = &relation.info.parent {
                wanted.insert(parent);
            }
        }
        Ok(self
            .many_to_one
            .iter()
            .filter(|r| wanted.contains(r.info.id.as_str()))
            .collect())
    }
}

/// Tìm quan hệ quanh bảng chính: khóa ngoại rời nó (bước 1), khóa ngoại rời mỗi bảng nối được ở bước 1 (bước 2), và
/// khóa ngoại của bảng khác trỏ vào nó (một-nhiều, không nối).
pub async fn discover(c: &mut PgConnection, fact: &SourceTable) -> Result<Discovered, AppError> {
    let fact_columns = list_columns(c, fact).await?;
    let mut many_to_one = Vec::new();
    let mut one_to_many = Vec::new();
    for key in list_foreign_keys(c, fact).await? {
        if key.from.same(fact) {
            let relation = build_relation(c, key, None).await?;
            many_to_one.push(relation);
        } else {
            let id = build_hop_id(&key);
            one_to_many.push(build_info(
                id,
                1,
                None,
                &key,
                0,
                Some(incoming_reason(&key)),
            ));
        }
    }
    // Bước 2 chỉ đi tiếp từ bảng nối được, và không quay về bảng chính: nối bảng chính vào chính nó qua một bảng khác
    // không thêm thông tin gì mà dễ làm người đọc nhầm.
    let first: Vec<Relation> = many_to_one.clone();
    let mut ordered = Vec::new();
    for parent in first {
        let target = parent.key.to.clone();
        let parent_id = parent.info.id.clone();
        let joinable = parent.info.joinable;
        ordered.push(parent);
        if !joinable || MAX_DEPTH < 2 {
            continue;
        }
        for key in list_foreign_keys(c, &target).await? {
            if !key.from.same(&target) || key.to.same(fact) {
                continue;
            }
            ordered.push(build_relation(c, key, Some(&parent_id)).await?);
        }
    }
    many_to_one = ordered;
    Ok(Discovered {
        fact_columns,
        many_to_one,
        one_to_many,
    })
}

async fn build_relation(
    c: &mut PgConnection,
    key: ForeignKey,
    parent: Option<&str>,
) -> Result<Relation, AppError> {
    let hop = build_hop_id(&key);
    let (id, depth) = match parent {
        Some(p) => (format!("{p}>{hop}"), 2),
        None => (hop, 1),
    };
    let reason = outgoing_reason(&key);
    // Bảng không đọc được thì không hỏi cột (danh sách sẽ rỗng): quan hệ đằng nào cũng không nối.
    let target_columns = if reason.is_none() {
        list_columns(c, &key.to).await?
    } else {
        Vec::new()
    };
    let added = target_columns
        .iter()
        .filter(|col| !key.to_columns.contains(col))
        .count();
    let info = build_info(id, depth, parent.map(str::to_string), &key, added, reason);
    Ok(Relation {
        info,
        key,
        target_columns,
    })
}

fn build_info(
    id: String,
    depth: u8,
    parent: Option<String>,
    key: &ForeignKey,
    column_count: usize,
    reason: Option<String>,
) -> SourceRelation {
    SourceRelation {
        id,
        depth,
        parent,
        constraint: key.constraint.clone(),
        from_table: key.from.full_name(),
        from_columns: key.from_columns.clone(),
        to_table: key.to.full_name(),
        to_columns: key.to_columns.clone(),
        joinable: reason.is_none(),
        reason,
        column_count,
    }
}

/// Id một bước: bảng chứa khóa ngoại và tên ràng buộc. Postgres không cho hai ràng buộc cùng tên trên một bảng, nên id
/// duy nhất, và không đổi khi bảng thêm cột hay khóa khác: "Chụp lại" tìm lại được đúng quan hệ.
fn build_hop_id(key: &ForeignKey) -> String {
    format!("{}/{}", key.from.full_name(), key.constraint)
}

/// Lý do không nối một khóa ngoại rời bảng; None khi nối được.
pub fn outgoing_reason(key: &ForeignKey) -> Option<String> {
    if !key.unique_target {
        return Some(format!(
            "Cột {} của {} không phải khóa chính hay có ràng buộc UNIQUE: một dòng có thể khớp nhiều dòng bên đó, nối \
             sẽ nhân dòng",
            key.to_columns.join(", "),
            key.to.full_name()
        ));
    }
    if !key.readable_target {
        return Some(format!(
            "Tài khoản không có quyền đọc {}: cấp GRANT SELECT nếu cần nối",
            key.to.full_name()
        ));
    }
    None
}

fn incoming_reason(key: &ForeignKey) -> String {
    format!(
        "{} trỏ vào bảng này: một dòng ở đây ứng với nhiều dòng bên đó, nối sẽ nhân dòng. Chụp {} làm bảng chính nếu \
         cần phân tích theo nó",
        key.from.full_name(),
        key.from.full_name()
    )
}

/// Khóa ngoại rời hay trỏ vào một bảng. conparentid = 0 bỏ bản sao của ràng buộc mà Postgres tạo cho từng partition.
/// Cột đích "có ràng buộc UNIQUE" khi tập cột của nó đúng bằng tập cột của khóa chính hay một ràng buộc UNIQUE (hai
/// chiều chứa nhau); chỉ mục UNIQUE không kèm ràng buộc không tính, để luật dễ kiểm bằng mắt trong \d của psql.
async fn list_foreign_keys(
    c: &mut PgConnection,
    table: &SourceTable,
) -> Result<Vec<ForeignKey>, AppError> {
    type Row = (
        String,
        String,
        String,
        Vec<String>,
        String,
        String,
        Vec<String>,
        bool,
        bool,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT c.conname::text, sn.nspname::text, s.relname::text,
                ARRAY(SELECT a.attname::text FROM unnest(c.conkey) WITH ORDINALITY AS k(num, ord)
                      JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.num ORDER BY k.ord),
                tn.nspname::text, t.relname::text,
                ARRAY(SELECT a.attname::text FROM unnest(c.confkey) WITH ORDINALITY AS k(num, ord)
                      JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.num ORDER BY k.ord),
                EXISTS (SELECT 1 FROM pg_constraint u
                        WHERE u.conrelid = c.confrelid AND u.contype IN ('p', 'u')
                          AND u.conkey @> c.confkey AND u.conkey <@ c.confkey),
                has_table_privilege(c.confrelid, 'SELECT')
         FROM pg_constraint c
         JOIN pg_class s ON s.oid = c.conrelid
         JOIN pg_namespace sn ON sn.oid = s.relnamespace
         JOIN pg_class t ON t.oid = c.confrelid
         JOIN pg_namespace tn ON tn.oid = t.relnamespace
         WHERE c.contype = 'f' AND c.conparentid = 0
           AND ((sn.nspname = $1 AND s.relname = $2) OR (tn.nspname = $1 AND t.relname = $2))
         ORDER BY 2, 3, 1",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))?;
    Ok(rows
        .into_iter()
        .map(|r| ForeignKey {
            constraint: r.0,
            from: SourceTable {
                schema: r.1,
                name: r.2,
                kind: "table".to_string(),
            },
            from_columns: r.3,
            to: SourceTable {
                schema: r.4,
                name: r.5,
                kind: "table".to_string(),
            },
            to_columns: r.6,
            unique_target: r.7,
            readable_target: r.8,
        })
        .collect())
}

/// Cột tài khoản đọc được của một bảng hay view, theo thứ tự trong bảng.
pub async fn list_columns(
    c: &mut PgConnection,
    table: &SourceTable,
) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar(
        "SELECT a.attname::text FROM pg_attribute a
         JOIN pg_class c ON c.oid = a.attrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped
           AND has_column_privilege(a.attrelid, a.attnum, 'SELECT')
         ORDER BY a.attnum",
    )
    .bind(&table.schema)
    .bind(&table.name)
    .fetch_all(&mut *c)
    .await
    .map_err(|e| query_error(&e))
}
