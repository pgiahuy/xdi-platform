// SPDX-License-Identifier: Apache-2.0
/* service/sources/relations.rs - quan hệ khóa ngoại quanh một bảng của nguồn Postgres. Hai việc:
- discover: liệt kê bảng nối với bảng đang chọn theo cả hai chiều, để người dùng chọn chụp kèm (route
  POST /sources/{id}/relations).
- build_group_keys: lúc chụp một nhóm bảng (snapshot.rs), giữ các khóa ngoại nối hai bảng trong nhóm để ghi thành quan
  hệ giữa các dataset (engine/store/relations.rs).

Mỗi bảng chụp thành một dataset riêng, không nối thành bảng phẳng, nên cả hai chiều đều chụp kèm được: nhiều-một (khóa
ngoại rời bảng đang chọn, order_items -> orders) và một-nhiều (khóa ngoại của bảng khác trỏ vào nó, orders <-
order_items). Chỉ đi một bước quanh bảng đang chọn; bảng xa hơn thì chọn tiếp từ bảng đã thêm.

Chỉ ghi quan hệ khi cột đích là khóa chính hay ràng buộc UNIQUE: mỗi dòng bên nhiều khớp tối đa một dòng bên một, nên
sau này JOIN theo quan hệ không nhân dòng phía một. Mọi thông tin đọc từ pg_catalog (tên ràng buộc, bảng, cột, quyền),
không đọc dòng dữ liệu nào. pg_catalog cho mọi tài khoản xem tên bảng và ràng buộc, kể cả bảng không có quyền đọc; bảng
như vậy được liệt kê kèm lý do mà không chụp được. */

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnection;

use super::postgres::{query_error, SourceTable};
use crate::error::AppError;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RelationsRequest {
    /// `schema.table` from the table list.
    pub table: String,
}

/// A table linked to the chosen table by one foreign key.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RelatedTable {
    /// `schema.table` that can be snapshotted together with the chosen table.
    pub table: String,
    pub constraint: String,
    /// Table holding the foreign key (the many side) and its key columns.
    pub from_table: String,
    pub from_columns: Vec<String>,
    /// Table the foreign key points to (the one side) and its key columns.
    pub to_table: String,
    pub to_columns: Vec<String>,
    /// Readable columns of `table`; 0 when the account cannot read it.
    pub column_count: usize,
    /// True when `table` can be snapshotted with the chosen table and the relation recorded.
    pub addable: bool,
    /// Why the table cannot be added, in Vietnamese; null when addable.
    #[schema(required)]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RelationsResponse {
    pub table: String,
    /// Readable columns of the chosen table.
    pub column_count: usize,
    /// Tables the chosen table points to (its foreign keys).
    pub many_to_one: Vec<RelatedTable>,
    /// Tables pointing to the chosen table (their foreign keys).
    pub one_to_many: Vec<RelatedTable>,
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
    /// Cột khóa ngoại cũng là khóa chính hay UNIQUE của bảng giữ nó: quan hệ một-một.
    pub unique_source: bool,
    /// Tài khoản có quyền SELECT trên bảng giữ khóa và trên bảng đích.
    pub readable_source: bool,
    pub readable_target: bool,
}

impl ForeignKey {
    /// Chữ ghi vào dataset_relations.cardinality (CHECK của migration 0009).
    pub fn cardinality(&self) -> &'static str {
        if self.unique_source {
            "one_to_one"
        } else {
            "many_to_one"
        }
    }
}

/// Bảng nối với `chosen` theo cả hai chiều, kèm lý do khi một bảng không chụp kèm được.
pub async fn discover(
    c: &mut PgConnection,
    chosen: &SourceTable,
) -> Result<RelationsResponse, AppError> {
    let column_count = list_columns(c, chosen).await?.len();
    let mut many_to_one = Vec::new();
    let mut one_to_many = Vec::new();
    for key in list_foreign_keys(c, chosen).await? {
        // Khóa trỏ về chính bảng (cột quản lý trỏ tới nhân viên): không có bảng nào khác để thêm. Lúc chụp, quan hệ này
        // vẫn được ghi (build_group_keys).
        if key.from.same(&key.to) {
            continue;
        }
        let outgoing = key.from.same(chosen);
        let (other, readable) = if outgoing {
            (key.to.clone(), key.readable_target)
        } else {
            (key.from.clone(), key.readable_source)
        };
        let reason = build_reason(&key, &other, readable);
        // Bảng không đọc được thì không hỏi cột (danh sách sẽ rỗng).
        let column_count = if readable {
            list_columns(c, &other).await?.len()
        } else {
            0
        };
        let item = RelatedTable {
            table: other.full_name(),
            constraint: key.constraint.clone(),
            from_table: key.from.full_name(),
            from_columns: key.from_columns.clone(),
            to_table: key.to.full_name(),
            to_columns: key.to_columns.clone(),
            column_count,
            addable: reason.is_none(),
            reason,
        };
        if outgoing {
            many_to_one.push(item);
        } else {
            one_to_many.push(item);
        }
    }
    Ok(RelationsResponse {
        table: chosen.full_name(),
        column_count,
        many_to_one,
        one_to_many,
    })
}

/// Lý do không chụp kèm `other` theo khóa này; None khi được.
pub fn build_reason(key: &ForeignKey, other: &SourceTable, readable: bool) -> Option<String> {
    if !key.unique_target {
        return Some(format!(
            "Cột {} của {} không phải khóa chính hay có ràng buộc UNIQUE: một dòng có thể khớp nhiều dòng bên đó, nên \
             quan hệ không được ghi",
            key.to_columns.join(", "),
            key.to.full_name()
        ));
    }
    if !readable {
        return Some(format!(
            "Tài khoản không có quyền đọc {}: cấp GRANT SELECT nếu cần chụp bảng này",
            other.full_name()
        ));
    }
    None
}

/// Khóa ngoại nối hai bảng trong nhóm `chosen`, mỗi khóa một lần (list_foreign_keys của hai đầu đều trả nó). Chỉ giữ
/// khóa có cột đích là khóa chính hay UNIQUE, như discover cho phép.
pub fn build_group_keys(keys: Vec<ForeignKey>, chosen: &[SourceTable]) -> Vec<ForeignKey> {
    let in_group = |t: &SourceTable| chosen.iter().any(|c| c.same(t));
    let mut seen: HashSet<(String, String)> = HashSet::new();
    keys.into_iter()
        .filter(|k| k.unique_target && in_group(&k.from) && in_group(&k.to))
        .filter(|k| seen.insert((k.from.full_name(), k.constraint.clone())))
        .collect()
}

/// Khóa ngoại rời hay trỏ vào một bảng. conparentid = 0 bỏ bản sao của ràng buộc mà Postgres tạo cho từng partition.
/// Cột "có ràng buộc UNIQUE" khi tập cột của nó đúng bằng tập cột của khóa chính hay một ràng buộc UNIQUE (hai chiều
/// chứa nhau); chỉ mục UNIQUE không kèm ràng buộc không tính, để luật dễ kiểm bằng mắt trong \d của psql.
pub async fn list_foreign_keys(
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
                EXISTS (SELECT 1 FROM pg_constraint u
                        WHERE u.conrelid = c.conrelid AND u.contype IN ('p', 'u')
                          AND u.conkey @> c.conkey AND u.conkey <@ c.conkey),
                has_table_privilege(c.conrelid, 'SELECT'),
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
    let table_of = |schema: String, name: String| SourceTable {
        schema,
        name,
        kind: "table".to_string(),
    };
    Ok(rows
        .into_iter()
        .map(|r| ForeignKey {
            constraint: r.0,
            from: table_of(r.1, r.2),
            from_columns: r.3,
            to: table_of(r.4, r.5),
            to_columns: r.6,
            unique_target: r.7,
            unique_source: r.8,
            readable_source: r.9,
            readable_target: r.10,
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
