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
  customers) nhận cả hai điều kiện (AND): dòng phải qua mọi đường có khóa không rỗng.
- Quan hệ một-một lan như nhiều-một: điều kiện chỉ đi theo chiều khóa ngoại, không theo cardinality.
- Khóa ngoại rỗng (NULL): một đường thì dòng bị ẩn. Nhiều đường thì đường có khóa rỗng được bỏ qua (khóa ngoại tùy
  chọn như gift_customer_id thường rỗng), nhưng dòng mà mọi đường đều rỗng thì ẩn. Guard dựng điều kiện này
  (engine/guard/mod.rs, build_parent_conditions).
- Vòng quan hệ (bảng trỏ vào chính nó, hay A -> B -> A) dừng ở bảng đã có trên đường đang đi, để điều kiện không lồng
  mãi. Bảng trỏ vào chính nó vì thế không nhận điều kiện lan từ chính nó; chính sách riêng của nó vẫn áp.
- Bảng cha bị lọc mà không đọc được (chưa có phiên bản, chưa có file Parquet): ẩn mọi dòng của bảng con, vì không kiểm
  được dòng nào trỏ tới phần được thấy.

Đi theo mọi quan hệ trong workspace, không giới hạn số bước: dừng ở bước nào thì bảng xa hơn bước đó lại thấy hết.

Nhiều vai (#45): một thành viên mang được nhiều vai dữ liệu. Điều kiện của từng vai dựng riêng như trên (AND trong một
vai: mọi luật của vai trên bảng và mọi đường lan); giữa các vai là OR (RowFilter::AnyOf), tức người đó thấy hợp phần
được thấy của các vai. Chỉ vai có điều kiện trên bảng đang đọc mới tính: vai không có luật nào trên bảng đó (hay trên
bảng nó trỏ tới) không mở thêm dòng nào của bảng. Không có luật này thì người mang vai "Miền Nam" (luật trên customers)
và vai "Điện tử" (luật trên products) sẽ thấy mọi khách hàng, vì vai "Điện tử" không giới hạn customers. Thành viên
không mang vai nào, hay không vai nào có điều kiện trên bảng, thấy mọi dòng như trước (docs/ARCHITECTURE.md mục 4b).

Test phần dựng điều kiện ở ask/row_filters_tests.rs, qua route ở api/row_filter_fk_tests.rs và api/data_roles_tests.rs. */

use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use crate::auth::{Member, Role};
use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::store::{DatasetRelation, StoredPolicy};
use crate::{error::AppError, state::AppState};

/// Những gì cần về một dataset để dựng điều kiện của nó và của các bảng trỏ tới nó.
pub struct PolicyTable {
    /// Chính sách riêng (RowFilter::Column), rỗng khi không có.
    pub own: Vec<RowFilter>,
    /// Bảng đọc được của phiên bản đang dùng, row_filters để rỗng; None khi chưa đọc được.
    pub table: Option<AllowedTable>,
}

/// Điều kiện lọc dòng của member trên dataset_id: chính sách riêng cộng điều kiện lan theo khóa ngoại, của từng vai, ghép
/// OR giữa các vai. Admin không bị lọc: họ tự sửa được chính sách, lọc với họ không bảo vệ được gì (#45).
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
    let policies = state
        .store
        .list_member_policies(ws, member.user.id, &ids)
        .await?;
    // Thường người hỏi không mang vai nào có luật trên đường: khỏi đọc phiên bản và hồ sơ cột.
    if policies.is_empty() {
        return Ok(Vec::new());
    }
    let mut readable = HashMap::new();
    if policies.iter().any(|p| p.table_id != dataset_id) {
        for &id in ids.iter().skip(1) {
            readable.insert(id, load_readable_table(state, member, id).await?);
        }
    }
    Ok(build_member_filters(
        dataset_id, &relations, &policies, &readable,
    ))
}

/// Điều kiện của dataset_id cho một người từ luật của mọi vai họ mang (policies) và bảng đọc được của các bảng cha
/// (readable, None khi chưa đọc được). Một vai có điều kiện thì trả đúng điều kiện đó, như trước khi có nhiều vai; từ
/// hai vai thì một RowFilter::AnyOf. Không đọc gì thêm nên test được không cần Postgres.
pub fn build_member_filters(
    dataset_id: Uuid,
    relations: &[DatasetRelation],
    policies: &[StoredPolicy],
    readable: &HashMap<Uuid, Option<AllowedTable>>,
) -> Vec<RowFilter> {
    // BTreeMap để thứ tự các vai cố định, câu SQL sau guard giống nhau giữa các lần hỏi.
    let mut by_role: BTreeMap<Uuid, Vec<&StoredPolicy>> = BTreeMap::new();
    for policy in policies {
        by_role.entry(policy.data_role_id).or_default().push(policy);
    }
    let mut groups: Vec<Vec<RowFilter>> = Vec::new();
    for role_policies in by_role.values() {
        let mut tables: HashMap<Uuid, PolicyTable> = readable
            .iter()
            .map(|(id, table)| {
                let entry = PolicyTable {
                    own: Vec::new(),
                    table: table.clone(),
                };
                (*id, entry)
            })
            .collect();
        for policy in role_policies {
            let entry = tables.entry(policy.table_id).or_insert(PolicyTable {
                own: Vec::new(),
                table: None,
            });
            entry.own.push(RowFilter::Column {
                column: policy.column.clone(),
                allowed_values: policy.allowed_values.clone(),
            });
        }
        let filters = build_row_filters(dataset_id, relations, &tables);
        // Vai không có điều kiện trên bảng này không mở thêm dòng nào (chú thích đầu file); hai vai cùng điều kiện
        // trên bảng này thì giữ một.
        if !filters.is_empty() && !groups.contains(&filters) {
            groups.push(filters);
        }
    }
    match groups.len() {
        0 => Vec::new(),
        1 => groups.remove(0),
        _ => vec![RowFilter::AnyOf(groups)],
    }
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
