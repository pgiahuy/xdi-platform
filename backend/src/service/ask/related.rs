// SPDX-License-Identifier: Apache-2.0
/* service/ask/related.rs - các bảng một câu hỏi được đọc (#213, #249): mọi bảng đã chụp của database chứa bảng bắt
đầu (cùng dataset, Dataset::parent_id), chỉ bảng có phiên bản đang dùng, đã lập hồ sơ và có file Parquet, cùng mọi quan
hệ dataset_relations (engine/store/relations.rs, ghi lúc chụp) giữa chúng. Không giới hạn số bước theo quan hệ: câu
"theo phòng ban" trên order_items cần order_items -> products -> categories -> departments, ba bước. Bảng nào được gửi
chi tiết cho mô hình do ask/table_pick.rs chọn khi metadata mọi bảng vượt trần; build_subset dựng nhóm của các bảng đó.

ask/mod.rs và drill/mod.rs gọi load_group trước khi dựng ngữ cảnh: mỗi bảng thành một AllowedTable của guard với bộ
lọc dòng của riêng nó; ask/group_prompt.rs dựng ngữ cảnh của các bảng liên quan cho prompt SQL. Dataset nạp từ file
đứng riêng: nhóm chỉ có đúng bảng đó, như trước.

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

/// Một bảng của nhóm.
#[derive(Clone)]
pub struct GroupTable {
    pub dataset: Dataset,
    pub version: DatasetVersion,
    /// Hồ sơ cột của phiên bản (Store::list_columns).
    pub columns: Vec<ProfileColumn>,
}

#[derive(Clone)]
pub struct TableGroup {
    /// Bảng đang chọn ở đầu, rồi các bảng nối được theo số bước quan hệ (gần trước), rồi bảng không nối được.
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
    let mut group = TableGroup {
        tables: vec![GroupTable {
            dataset: dataset.clone(),
            version: version.clone(),
            columns: columns.to_vec(),
        }],
        allowed: vec![picked_table],
        relations: Vec::new(),
    };
    // File nạp lên không có nguồn database: không có bảng cùng database nào để đọc thêm.
    if dataset.source_id.is_none() {
        return Ok(group);
    }
    let siblings: Vec<Uuid> = state
        .store
        .list_datasets(ws)
        .await?
        .into_iter()
        .filter(|d| d.parent_id == dataset.parent_id && d.id != dataset.id)
        .map(|d| d.id)
        .collect();
    // Bảng chưa có phiên bản, chưa lập hồ sơ xong hay không có file Parquet thì bỏ.
    let mut loaded: Vec<(GroupTable, AllowedTable)> = Vec::new();
    for id in siblings {
        if let Some(found) = load_table(state, member, id).await? {
            loaded.push(found);
        }
    }
    let mut usable: HashSet<Uuid> = loaded.iter().map(|(t, _)| t.dataset.id).collect();
    usable.insert(dataset.id);
    let relations: Vec<DatasetRelation> = state
        .store
        .list_relations(ws, None)
        .await?
        .into_iter()
        .filter(|r| usable.contains(&r.from_dataset_id) && usable.contains(&r.to_dataset_id))
        .collect();
    let names: Vec<(Uuid, String)> = loaded
        .iter()
        .map(|(t, _)| (t.dataset.id, t.dataset.logical_table.clone()))
        .collect();
    let order = build_group_order(dataset.id, &names, &relations);
    for id in order.into_iter().skip(1) {
        if let Some(at) = loaded.iter().position(|(t, _)| t.dataset.id == id) {
            let (table, allowed) = loaded.swap_remove(at);
            group.tables.push(table);
            group.allowed.push(allowed);
        }
    }
    group.relations = relations;
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

/// Thứ tự các bảng của nhóm: start ở đầu, rồi bảng nối được theo quan hệ (hai chiều), ít bước trước, cùng số bước thì
/// theo thứ tự quan hệ; sau cùng là bảng không nối được với start, theo tên bảng. others là (id, tên bảng logic) của
/// mọi bảng khác start. Thứ tự này là thứ tự khối cột trong prompt: bảng gần bảng bắt đầu đứng trước.
pub fn build_group_order(
    start: Uuid,
    others: &[(Uuid, String)],
    relations: &[DatasetRelation],
) -> Vec<Uuid> {
    let known = |id: Uuid| id == start || others.iter().any(|(o, _)| *o == id);
    let mut order = vec![start];
    let mut frontier = vec![start];
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for &id in &frontier {
            for other in build_neighbors(id, relations) {
                if known(other) && !order.contains(&other) {
                    order.push(other);
                    next.push(other);
                }
            }
        }
        frontier = next;
    }
    let mut unlinked: Vec<&(Uuid, String)> = others
        .iter()
        .filter(|(id, _)| !order.contains(id))
        .collect();
    unlinked.sort_by(|a, b| a.1.cmp(&b.1));
    order.extend(unlinked.into_iter().map(|(id, _)| *id));
    order
}

/// Bảng nối trực tiếp với id bằng một quan hệ, cả hai chiều, theo thứ tự quan hệ, mỗi bảng một lần.
pub fn build_neighbors(id: Uuid, relations: &[DatasetRelation]) -> Vec<Uuid> {
    let mut found = Vec::new();
    for r in relations {
        let other = if r.from_dataset_id == id {
            r.to_dataset_id
        } else if r.to_dataset_id == id {
            r.from_dataset_id
        } else {
            continue;
        };
        if other != id && !found.contains(&other) {
            found.push(other);
        }
    }
    found
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
    /// Nhóm chỉ gồm các bảng trong ids, giữ thứ tự của nhóm này; bảng đầu (bảng đang chọn) luôn được giữ. Quan hệ chỉ
    /// giữ những quan hệ có cả hai đầu trong nhóm mới, nên guard chỉ nhận JOIN giữa các bảng đã gửi chi tiết.
    pub fn build_subset(&self, ids: &[Uuid]) -> TableGroup {
        let mut subset = TableGroup {
            tables: Vec::new(),
            allowed: Vec::new(),
            relations: Vec::new(),
        };
        for (i, (table, allowed)) in self.tables.iter().zip(&self.allowed).enumerate() {
            if i == 0 || ids.contains(&table.dataset.id) {
                subset.tables.push(table.clone());
                subset.allowed.push(allowed.clone());
            }
        }
        let kept: Vec<Uuid> = subset.tables.iter().map(|t| t.dataset.id).collect();
        subset.relations = self
            .relations
            .iter()
            .filter(|r| kept.contains(&r.from_dataset_id) && kept.contains(&r.to_dataset_id))
            .cloned()
            .collect();
        subset
    }

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

    /// Tên các cột đánh dấu nhạy cảm của mọi bảng trong nhóm, cho vòng sửa SQL (ask/repair.rs, build_model_error).
    pub fn sensitive_columns(&self) -> Vec<String> {
        self.tables
            .iter()
            .flat_map(|t| t.columns.iter())
            .filter(|c| c.is_sensitive)
            .map(|c| c.name.clone())
            .collect()
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
