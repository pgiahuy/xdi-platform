// SPDX-License-Identifier: Apache-2.0
/* service/ask/related.rs - các bảng một câu hỏi được đọc (#213): dataset đang chọn cùng các dataset nối với nó qua
dataset_relations (engine/store/relations.rs, ghi lúc chụp nhiều bảng từ database), cả hai chiều, tối đa MAX_HOPS bước
và MAX_GROUP_TABLES bảng, chỉ dataset có phiên bản đang dùng và đã lập hồ sơ. ask/mod.rs gọi load_group trước khi dựng
ngữ cảnh: mỗi bảng thành một AllowedTable của guard với bộ lọc dòng của riêng nó; ask/group_prompt.rs dựng ngữ cảnh
của các bảng liên quan cho prompt SQL. Dataset nạp từ file
không có quan hệ nào, nên nhóm chỉ có một bảng và prompt như trước.

Người bị lọc dòng nhận đủ các bảng như mọi người: bảng trỏ tới bảng bị lọc mang điều kiện lan theo khóa ngoại
(ask/row_filters.rs, #215), nên đọc orders một mình hay JOIN với customers đều chỉ thấy đơn của khách được thấy. */

use std::collections::HashSet;

use uuid::Uuid;

use crate::auth::Member;
use crate::engine::guard::AllowedTable;
use crate::engine::store::{Dataset, DatasetRelation, DatasetVersion, ProfileColumn};
use crate::service::ask::build_allowed_table;
use crate::service::ask::context::check_profile_ready;
use crate::service::ask::contract::TableRead;
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::source::{build_related_tables, RelatedTables, SourceTable};
use crate::{error::AppError, state::AppState};

/// Số bảng tối đa một câu hỏi được đọc, kể cả bảng đang chọn: đủ cho chuỗi bảng sự kiện, bảng đơn, khách hàng, sản
/// phẩm, danh mục; nhiều hơn thì prompt dài và mô hình dễ nối sai.
pub const MAX_GROUP_TABLES: usize = 8;

/// Số bước tối đa đi theo quan hệ từ bảng đang chọn (order_items, orders, customers là hai bước).
pub const MAX_HOPS: usize = 2;

/// Một bảng của nhóm.
pub struct GroupTable {
    pub dataset: Dataset,
    pub version: DatasetVersion,
    /// Hồ sơ cột của phiên bản (Store::list_columns).
    pub columns: Vec<ProfileColumn>,
}

pub struct TableGroup {
    /// Bảng đang chọn ở đầu, rồi các bảng liên quan theo số bước.
    pub tables: Vec<GroupTable>,
    /// Đầu vào của guard, cùng thứ tự với tables.
    pub allowed: Vec<AllowedTable>,
    /// Quan hệ có cả hai đầu trong nhóm.
    pub relations: Vec<DatasetRelation>,
}

/// Nhóm bảng của câu hỏi. picked là AllowedTable của dataset đang chọn (ask/mod.rs đã dựng).
pub async fn load_group(
    state: &AppState,
    member: &Member,
    picked: (&Dataset, &DatasetVersion, &[ProfileColumn]),
    picked_table: AllowedTable,
) -> Result<TableGroup, AppError> {
    let (dataset, version, columns) = picked;
    let ws = &member.workspace;
    let relations = state.store.list_relations(ws, None).await?;
    let mut group = TableGroup {
        tables: vec![GroupTable {
            dataset: dataset.clone(),
            version: version.clone(),
            columns: columns.to_vec(),
        }],
        allowed: vec![picked_table],
        relations: Vec::new(),
    };
    // Đọc các bảng ứng viên; bảng chưa có phiên bản, chưa lập hồ sơ xong hay không có file Parquet thì bỏ.
    let candidates = build_group_ids(dataset.id, &relations, |_| true);
    let mut loaded: Vec<(GroupTable, AllowedTable)> = Vec::new();
    for id in candidates.into_iter().skip(1) {
        if let Some(found) = load_table(state, member, id).await? {
            loaded.push(found);
        }
    }
    // Tính lại sau khi bỏ bảng không dùng được: bảng chỉ nối được qua bảng đó không viết JOIN tới được.
    let usable: HashSet<Uuid> = loaded.iter().map(|(t, _)| t.dataset.id).collect();
    let kept = build_group_ids(dataset.id, &relations, |id| usable.contains(&id));
    for (table, allowed) in loaded {
        if kept.contains(&table.dataset.id) {
            group.tables.push(table);
            group.allowed.push(allowed);
        }
    }
    group.relations = relations
        .into_iter()
        .filter(|r| kept.contains(&r.from_dataset_id) && kept.contains(&r.to_dataset_id))
        .collect();
    Ok(group)
}

/// Một bảng liên quan dùng được, hay None khi chưa hỏi được trên nó.
async fn load_table(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<Option<(GroupTable, AllowedTable)>, AppError> {
    let ws = &member.workspace;
    let Some(dataset) = state.store.get_dataset(ws, id).await? else {
        return Ok(None);
    };
    let Some(version) = state.store.get_current_version(ws, id).await? else {
        return Ok(None);
    };
    if check_profile_ready(&version).is_err() || version.parquet_path.is_none() {
        return Ok(None);
    }
    let columns = state.store.list_columns(ws, version.id).await?;
    if columns.is_empty() {
        return Ok(None);
    }
    let allowed = build_allowed_table(state, member, &dataset, &version, &columns).await?;
    let table = GroupTable {
        dataset,
        version,
        columns,
    };
    Ok(Some((table, allowed)))
}

/// Dataset đi được từ picked theo quan hệ, hai chiều, tối đa MAX_HOPS bước và MAX_GROUP_TABLES dataset, picked ở đầu,
/// gần trước xa sau. Chỉ đi qua dataset usable nhận.
pub fn build_group_ids(
    picked: Uuid,
    relations: &[DatasetRelation],
    usable: impl Fn(Uuid) -> bool,
) -> Vec<Uuid> {
    let mut group = vec![picked];
    let mut frontier = vec![picked];
    for _ in 0..MAX_HOPS {
        let mut next = Vec::new();
        for &id in &frontier {
            for r in relations {
                let other = if r.from_dataset_id == id {
                    r.to_dataset_id
                } else if r.to_dataset_id == id {
                    r.from_dataset_id
                } else {
                    continue;
                };
                if group.contains(&other) || !usable(other) {
                    continue;
                }
                if group.len() == MAX_GROUP_TABLES {
                    return group;
                }
                group.push(other);
                next.push(other);
            }
        }
        frontier = next;
    }
    group
}

/// Bảng liên quan của nhóm nối nhiều-một vào bảng đang chọn (explain/source.rs), cho phân rã. Nhóm một bảng thì rỗng.
pub fn build_group_related(group: &TableGroup, links: &[JoinLink]) -> RelatedTables {
    let tables: Vec<SourceTable> = group
        .tables
        .iter()
        .skip(1)
        .map(|t| SourceTable {
            table: &t.dataset.logical_table,
            columns: &t.columns,
        })
        .collect();
    let base = &group.tables[0].dataset.logical_table;
    build_related_tables(base, &tables, links)
}

impl TableGroup {
    /// Cột khóa của dataset id trong các quan hệ của nhóm.
    pub fn join_keys(&self, id: Uuid) -> Vec<String> {
        let mut keys = Vec::new();
        for r in &self.relations {
            if r.from_dataset_id == id {
                keys.extend(r.from_columns.iter().cloned());
            }
            if r.to_dataset_id == id {
                keys.extend(r.to_columns.iter().cloned());
            }
        }
        keys
    }

    /// Có bảng nào của nhóm lọc dòng với người hỏi.
    pub fn is_restricted(&self) -> bool {
        self.allowed.iter().any(|t| !t.row_filters.is_empty())
    }

    /// Bảng ứng với các tên guard báo đã đọc (GuardedSql::tables_read), theo thứ tự đó.
    pub fn build_tables_read(&self, names: &[String]) -> Vec<TableRead> {
        names
            .iter()
            .filter_map(|name| {
                self.tables
                    .iter()
                    .find(|t| t.dataset.logical_table == *name)
            })
            .map(|t| TableRead {
                dataset_id: t.dataset.id,
                name: t.dataset.name.clone(),
                logical_table: t.dataset.logical_table.clone(),
            })
            .collect()
    }

    /// Cặp (dataset, phiên bản) ghi vào queries.read_versions: các bảng câu đã đọc, hay mọi bảng đưa cho guard khi chưa
    /// có câu nào qua guard (câu bị từ chối, mô hình báo không trả lời được).
    pub fn build_read_versions(&self, read: &[TableRead]) -> Vec<(Uuid, Uuid)> {
        self.tables
            .iter()
            .filter(|t| read.is_empty() || read.iter().any(|r| r.dataset_id == t.dataset.id))
            .map(|t| (t.dataset.id, t.version.id))
            .collect()
    }

    pub(super) fn logical_table(&self, id: Uuid) -> Option<&str> {
        self.tables
            .iter()
            .find(|t| t.dataset.id == id)
            .map(|t| t.dataset.logical_table.as_str())
    }
}
