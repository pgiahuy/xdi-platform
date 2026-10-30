// SPDX-License-Identifier: Apache-2.0
/* service/ask/row_filters.rs - điều kiện lọc dòng của một thành viên trên một dataset, cho guard (AllowedTable::
row_filters). Luồng hỏi (ask/mod.rs, cả câu "tại sao"), truy dòng nguồn (drill/mod.rs) và tổng quan dataset
(overview/mod.rs) cùng gọi load_row_filters, để mọi SQL chạy trên dữ liệu bị lọc như nhau.

Hai phần:
- Chính sách của chính dataset (rls_policies theo data_role, engine/store/policies.rs): RowFilter::Column.
- Điều kiện lan theo khóa ngoại (#215): dataset trỏ tới một dataset bị lọc qua dataset_relations (from -> to, phía
  nhiều tới phía một) chỉ thấy dòng có khóa ngoại trỏ tới dòng được thấy của bảng kia: RowFilter::Parent. Không có
  phần này thì người bị lọc theo customers.region đọc orders (không có cột vùng, nên không có chính sách) thấy đơn của
  mọi vùng. Lan nhiều bước: order_items -> orders -> customers thì điều kiện của orders nằm trong câu con của
  order_items.

Luật đã chốt:
- Bảng có chính sách riêng giữ chính sách đó và cộng thêm điều kiện lan xuống (AND).
- Bảng có hai đường tới bảng bị lọc (order_items.order_id -> orders -> customers và order_items.gift_customer_id ->
  customers) nhận cả hai điều kiện (AND): dòng phải qua mọi đường.
- Quan hệ một-một lan như nhiều-một: điều kiện chỉ đi theo chiều khóa ngoại, không theo cardinality.
- Khóa ngoại rỗng (NULL) thì dòng bị ẩn, như NULL ở cột chính sách: NULL không khớp IN.
- Vòng quan hệ (bảng trỏ vào chính nó, hay A -> B -> A) dừng ở bảng đã có trên đường đang đi, để điều kiện không lồng
  mãi. Bảng trỏ vào chính nó vì thế không nhận điều kiện lan từ chính nó; chính sách riêng của nó vẫn áp.
- Bảng cha bị lọc mà không đọc được (chưa có phiên bản, chưa có file Parquet): ẩn mọi dòng của bảng con, vì không kiểm
  được dòng nào trỏ tới phần được thấy.

Đi theo mọi quan hệ trong workspace, không giới hạn số bước như nhóm bảng của câu hỏi (ask/related.rs): dừng ở bước
thứ ba thì bảng xa hơn một bước lại thấy hết. Test phần dựng điều kiện ở ask/row_filters_tests.rs, qua route ở
api/row_filter_fk_tests.rs. */

use std::collections::HashMap;

use uuid::Uuid;

use crate::auth::{Member, Role};
use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::store::DatasetRelation;
use crate::{error::AppError, state::AppState};

/// Những gì cần về một dataset để dựng điều kiện của nó và của các bảng trỏ tới nó.
pub struct PolicyTable {
    /// Chính sách riêng (RowFilter::Column), rỗng khi không có.
    pub own: Vec<RowFilter>,
    /// Bảng đọc được của phiên bản đang dùng, row_filters để rỗng; None khi chưa đọc được.
    pub table: Option<AllowedTable>,
}

/// Điều kiện lọc dòng của member trên dataset_id: chính sách riêng cộng điều kiện lan theo khóa ngoại. Admin không bị
/// lọc: họ tự sửa được chính sách, lọc với họ không bảo vệ được gì (#45).
pub async fn load_row_filters(
    state: &AppState,
    member: &Member,
    dataset_id: Uuid,
) -> Result<Vec<RowFilter>, AppError> {
    if member.role == Role::Admin {
        return Ok(Vec::new());
    }
    let ws = &member.workspace;
    let relations = state.store.list_relations(ws, None).await?;
    let ids = build_parent_ids(dataset_id, &relations);
    let mut tables = HashMap::new();
    for &id in &ids {
        let own = state.store.list_row_filters(ws, id, member.user.id).await?;
        tables.insert(id, PolicyTable { own, table: None });
    }
    // Thường không bảng nào trên đường có chính sách (người không có data_role): khỏi đọc phiên bản và hồ sơ cột.
    if ids.iter().skip(1).any(|id| !tables[id].own.is_empty()) {
        for &id in ids.iter().skip(1) {
            let table = load_readable_table(state, member, id).await?;
            if let Some(entry) = tables.get_mut(&id) {
                entry.table = table;
            }
        }
    }
    Ok(build_row_filters(dataset_id, &relations, &tables))
}

/// dataset_id ở đầu, rồi mọi dataset đi được từ nó theo chiều from -> to, mỗi dataset một lần.
pub fn build_parent_ids(dataset_id: Uuid, relations: &[DatasetRelation]) -> Vec<Uuid> {
    let mut ids = vec![dataset_id];
    let mut next = 0;
    while next < ids.len() {
        let id = ids[next];
        next += 1;
        for r in relations.iter().filter(|r| r.from_dataset_id == id) {
            if !ids.contains(&r.to_dataset_id) {
                ids.push(r.to_dataset_id);
            }
        }
    }
    ids
}

/// Bảng của phiên bản đang dùng, như guard đọc khi hỏi trên chính dataset đó; None khi chưa có phiên bản hay file.
async fn load_readable_table(
    state: &AppState,
    member: &Member,
    id: Uuid,
) -> Result<Option<AllowedTable>, AppError> {
    let ws = &member.workspace;
    let Some(dataset) = state.store.get_dataset(ws, id).await? else {
        return Ok(None);
    };
    let Some(version) = state.store.get_current_version(ws, id).await? else {
        return Ok(None);
    };
    let Some(relative) = &version.parquet_path else {
        return Ok(None);
    };
    let columns = state.store.list_columns(ws, version.id).await?;
    Ok(Some(AllowedTable {
        name: dataset.logical_table,
        columns: columns.into_iter().map(|c| c.name).collect(),
        parquet_dir: state.config.data_dir.join(relative),
        row_filters: Vec::new(),
    }))
}

/// Điều kiện của dataset_id từ thông tin đã đọc. Không đọc gì thêm nên test được không cần Postgres.
pub fn build_row_filters(
    dataset_id: Uuid,
    relations: &[DatasetRelation],
    tables: &HashMap<Uuid, PolicyTable>,
) -> Vec<RowFilter> {
    let mut path = vec![dataset_id];
    build_filters_on_path(&mut path, relations, tables)
}

/// Điều kiện của bảng cuối path. path là các bảng đang đi qua, để dừng khi gặp vòng.
fn build_filters_on_path(
    path: &mut Vec<Uuid>,
    relations: &[DatasetRelation],
    tables: &HashMap<Uuid, PolicyTable>,
) -> Vec<RowFilter> {
    let Some(&id) = path.last() else {
        return Vec::new();
    };
    let mut filters = tables.get(&id).map(|t| t.own.clone()).unwrap_or_default();
    for relation in relations.iter().filter(|r| r.from_dataset_id == id) {
        let parent_id = relation.to_dataset_id;
        if path.contains(&parent_id) {
            continue;
        }
        path.push(parent_id);
        let parent_filters = build_filters_on_path(path, relations, tables);
        path.pop();
        // Bảng cha không bị lọc thì bảng con không cần điều kiện gì từ nó.
        if parent_filters.is_empty() {
            continue;
        }
        let readable = tables.get(&parent_id).and_then(|t| t.table.as_ref());
        let filter = match readable {
            Some(parent) => RowFilter::Parent {
                columns: relation.from_columns.clone(),
                parent_columns: relation.to_columns.clone(),
                parent: AllowedTable {
                    row_filters: parent_filters,
                    ..parent.clone()
                },
            },
            None => build_hide_all(relation),
        };
        filters.push(filter);
    }
    filters
}

/// Ẩn mọi dòng: danh sách giá trị rỗng thành `WHERE false` ở guard. Gắn vào cột khóa ngoại vì guard kiểm cột của chính
/// sách có trong bảng.
fn build_hide_all(relation: &DatasetRelation) -> RowFilter {
    RowFilter::Column {
        column: relation.from_columns.first().cloned().unwrap_or_default(),
        allowed_values: Vec::new(),
    }
}
