// SPDX-License-Identifier: Apache-2.0
/* service/ask/group_prompt.rs - ngữ cảnh của các bảng liên quan cho prompt SQL (#213): với mỗi bảng của nhóm
(ask/related.rs) ngoài bảng đang chọn, dựng cột gửi mô hình bằng chính build_context của ask/context.rs (cột khớp câu
hỏi, danh sách giá trị), đánh dấu cột khóa nối bảng, đọc chỉ số; rồi áp trần 60 cột và 400 giá trị chung cho mọi bảng,
kể cả bảng đang chọn. ask/prompt.rs in GroupPrompt thành một khối cột mỗi bảng và danh sách quan hệ. */

use crate::auth::Member;
use crate::engine::store::MetricDefinition;
use crate::service::ask::context::{
    apply_group_column_cap, apply_group_values, apply_join_keys, build_context, build_profiles,
    AskContext, ColumnProfile,
};
use crate::service::ask::phrases::build_phrases;
use crate::service::ask::related::TableGroup;
use crate::service::ask::values::load_value_index;
use crate::{error::AppError, state::AppState};

/// Một bảng liên quan như prompt SQL cần.
pub struct TableContext {
    /// Tên bảng logic (datasets.logical_table).
    pub table: String,
    pub columns: Vec<ColumnProfile>,
    /// Chỉ số của dataset đó (#208).
    pub metrics: Vec<MetricDefinition>,
}

/// Một quan hệ như prompt SQL in ra, tên bảng là tên bảng logic.
pub struct JoinLine {
    /// Bảng giữ khóa ngoại (phía nhiều).
    pub from_table: String,
    pub from_columns: Vec<String>,
    /// Bảng có khóa chính hay UNIQUE (phía một).
    pub to_table: String,
    pub to_columns: Vec<String>,
    pub one_to_one: bool,
}

/// Phần của prompt SQL về các bảng liên quan; ask/mod.rs chỉ dựng khi nhóm có hơn một bảng.
pub struct GroupPrompt {
    pub tables: Vec<TableContext>,
    pub joins: Vec<JoinLine>,
}

/// Ngữ cảnh của các bảng liên quan cho prompt SQL; None khi nhóm chỉ có bảng đang chọn. Sửa cả context của bảng đang
/// chọn: trần cột (60) và trần giá trị (400) tính chung cho mọi bảng.
pub async fn build_group_prompt(
    state: &AppState,
    member: &Member,
    group: &TableGroup,
    question: &str,
    context: &mut AskContext,
) -> Result<Option<GroupPrompt>, AppError> {
    if group.tables.len() < 2 {
        return Ok(None);
    }
    let ws = &member.workspace;
    let phrases = build_phrases(question);
    let mut tables = Vec::new();
    let mut indexes = vec![load_value_index(state, ws, &group.tables[0].version).await?];
    for t in &group.tables[1..] {
        let index = load_value_index(state, ws, &t.version).await?;
        let mut profiles = build_profiles(t.columns.clone());
        apply_join_keys(&mut profiles, &group.join_keys(t.dataset.id));
        // Không truyền lựa chọn của người hỏi: hỏi lại và giả định cột chỉ xét bảng đang chọn, vì tên cột ở đó không
        // kèm tên bảng.
        let table_context = build_context(&phrases, profiles, &index, &[]);
        tables.push(TableContext {
            table: t.dataset.logical_table.clone(),
            columns: table_context.columns,
            metrics: state.store.list_metrics(ws, t.dataset.id).await?,
        });
        indexes.push(index);
    }
    let mut lists: Vec<&mut Vec<ColumnProfile>> = std::iter::once(&mut context.columns)
        .chain(tables.iter_mut().map(|t| &mut t.columns))
        .collect();
    apply_group_column_cap(&mut lists);
    let mut with_values: Vec<_> = lists
        .into_iter()
        .zip(indexes.iter().map(|index| index.as_ref()))
        .collect();
    apply_group_values(&mut with_values);
    let joins = group
        .relations
        .iter()
        .filter_map(|r| {
            Some(JoinLine {
                from_table: group.logical_table(r.from_dataset_id)?.to_string(),
                from_columns: r.from_columns.clone(),
                to_table: group.logical_table(r.to_dataset_id)?.to_string(),
                to_columns: r.to_columns.clone(),
                one_to_one: r.cardinality == "one_to_one",
            })
        })
        .collect();
    Ok(Some(GroupPrompt { tables, joins }))
}
