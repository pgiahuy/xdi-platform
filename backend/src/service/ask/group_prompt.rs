// SPDX-License-Identifier: Apache-2.0
/* service/ask/group_prompt.rs - các bảng một câu hỏi đọc (#213): với mỗi bảng của nhóm (ask/related.rs) ngoài bảng
đang chọn, dựng cột gửi mô hình bằng chính build_context của ask/context.rs (cột khớp câu hỏi), đánh dấu cột khóa nối
bảng và cột của chỉ số. ask/mod.rs gọi theo thứ tự:
1. load_question_tables: cột, chỉ số và chỉ mục giá trị của mọi bảng, trước lượt phân loại (ask/classify.rs đọc metadata
   cột của mọi bảng).
2. apply_question_columns: sau lượt phân loại, đánh dấu cột mô hình chọn, rồi trần metadata và trần giá trị tính chung
   cho mọi bảng, kể cả bảng đang chọn. Gọi được hai lần (trước lượt phân loại, chưa có cột chọn), mỗi lần tính lại.
3. build_group_prompt: GroupPrompt cho prompt SQL khi nhóm có hơn một bảng; ask/prompt.rs in một khối cột mỗi bảng và
   danh sách quan hệ. */

use std::sync::Arc;

use crate::auth::Member;
use crate::engine::store::MetricDefinition;
use crate::service::ask::context::{
    apply_group_values, apply_join_keys, apply_metadata_cap, apply_metric_columns,
    apply_picked_columns, build_context, build_profiles, AskContext, ColumnProfile,
};
use crate::service::ask::phrases::build_phrases;
use crate::service::ask::related::TableGroup;
use crate::service::ask::values::{load_value_index, ValueIndex};
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

/// Các bảng câu hỏi đọc: bảng liên quan (rỗng khi câu chỉ đọc bảng đang chọn) và chỉ mục giá trị của mọi bảng.
pub struct QuestionTables {
    pub related: Vec<TableContext>,
    /// Chỉ mục giá trị (ask/values.rs): phần tử đầu là của bảng đang chọn, sau đó theo thứ tự của related.
    indexes: Vec<Arc<ValueIndex>>,
}

/// Bước 1: cột, chỉ số và chỉ mục giá trị của mọi bảng của nhóm, chưa gắn danh sách giá trị.
pub async fn load_question_tables(
    state: &AppState,
    member: &Member,
    group: &TableGroup,
    question: &str,
) -> Result<QuestionTables, AppError> {
    let ws = &member.workspace;
    let phrases = build_phrases(question);
    let mut related = Vec::new();
    let mut indexes = vec![load_value_index(state, ws, &group.tables[0].version).await?];
    for t in &group.tables[1..] {
        let index = load_value_index(state, ws, &t.version).await?;
        let metrics = state.store.list_metrics(ws, t.dataset.id).await?;
        let mut profiles = build_profiles(t.columns.clone());
        apply_join_keys(&mut profiles, &group.join_keys(t.dataset.id));
        apply_metric_columns(&mut profiles, &metrics);
        // Không truyền lựa chọn của người hỏi: hỏi lại và giả định cột chỉ xét bảng đang chọn, vì tên cột ở đó không
        // kèm tên bảng.
        let table_context = build_context(&phrases, profiles, &index, &[]);
        related.push(TableContext {
            table: t.dataset.logical_table.clone(),
            columns: table_context.columns,
            metrics,
        });
        indexes.push(index);
    }
    Ok(QuestionTables { related, indexes })
}

/// Bước 2: cột lượt phân loại chọn (picked, cặp bảng logic và cột), trần metadata (apply_metadata_cap) và danh sách
/// giá trị (apply_group_values) chung cho bảng đang chọn và mọi bảng liên quan.
pub fn apply_question_columns(
    context: &mut AskContext,
    table: &str,
    tables: &mut QuestionTables,
    picked: &[(String, String)],
) {
    apply_picked_columns(table, &mut context.columns, picked);
    for t in tables.related.iter_mut() {
        apply_picked_columns(&t.table, &mut t.columns, picked);
    }
    let mut lists: Vec<&mut Vec<ColumnProfile>> = std::iter::once(&mut context.columns)
        .chain(tables.related.iter_mut().map(|t| &mut t.columns))
        .collect();
    apply_metadata_cap(&mut lists);
    let mut with_values: Vec<_> = lists
        .into_iter()
        .zip(tables.indexes.iter().map(|index| index.as_ref()))
        .collect();
    apply_group_values(&mut with_values);
}

/// Bước 3: phần của prompt SQL về các bảng liên quan; None khi nhóm chỉ có bảng đang chọn.
pub fn build_group_prompt(group: &TableGroup, tables: QuestionTables) -> Option<GroupPrompt> {
    if tables.related.is_empty() {
        return None;
    }
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
    Some(GroupPrompt {
        tables: tables.related,
        joins,
    })
}
