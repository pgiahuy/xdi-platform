// SPDX-License-Identifier: Apache-2.0
/* service/ask/database.rs - các bảng của database một câu hỏi được đọc (#249), gom ở một chỗ cho ask/mod.rs:
- full: mọi bảng đã chụp của database (ask/related.rs load_group), cùng quan hệ;
- all: cột ngữ cảnh và chỉ mục giá trị của mọi bảng đó (ask/group_prompt.rs load_question_tables), để bước chọn biết
  cột nào khớp câu hỏi;
- unsnapshotted: bảng của cùng nguồn mới có metadata (#247), chỉ vào danh sách gọn kèm dấu "no data yet".

ask/table_pick.rs chọn bảng gửi chi tiết; build_view dựng nhóm bảng và ngữ cảnh của đúng các bảng đó cho guard, prompt
SQL, câu "tại sao" và phần giải thích, nên các phần sau của luồng hỏi không đổi gì. Bảng không gửi chi tiết vào danh
sách gọn (build_others). */

use uuid::Uuid;

use crate::auth::WorkspaceId;
use crate::engine::store::{Dataset, MetricDefinition};
use crate::service::ask::classify::Classified;
use crate::service::ask::context::AskContext;
use crate::service::ask::group_prompt::{CompactTable, QuestionTables};
use crate::service::ask::missing_tables::{build_unrelated_message, find_unrelated_pair};
use crate::service::ask::phrases::build_phrases;
use crate::service::ask::related::TableGroup;
use crate::service::ask::table_pick::{
    build_choice_detail, build_table_choice, DetailReason, TableChoice, TableFacts,
};
use crate::{error::AppError, state::AppState};

/// Số cột in thay cho mô tả của một bảng không có COMMENT ON TABLE trong danh sách gọn: đủ để mô hình đoán bảng giữ
/// gì, mà một dòng vẫn ngắn.
const COMPACT_COLUMNS: usize = 8;

/// Các bảng của database cho một câu hỏi.
pub struct DatabaseTables {
    pub full: TableGroup,
    /// Ngữ cảnh của mọi bảng của full trừ bảng đang chọn, cùng thứ tự với full.tables[1..].
    pub all: QuestionTables,
    pub unsnapshotted: Vec<CompactTable>,
}

impl DatabaseTables {
    /// Bảng gửi chi tiết. context là ngữ cảnh của bảng đang chọn, metrics là chỉ số của nó; model_tables là tên bảng
    /// lượt phân loại cần (build_model_tables), rỗng trước lượt đó.
    pub fn build_choice(
        &self,
        context: &AskContext,
        metrics: &[MetricDefinition],
        question: &str,
        model_tables: &[String],
    ) -> TableChoice {
        let start = &self.full.tables[0].dataset;
        let mut facts = vec![TableFacts {
            id: start.id,
            table: &start.logical_table,
            description: start.description.as_deref(),
            columns: &context.columns,
            metrics,
        }];
        for (t, related) in self.full.tables.iter().skip(1).zip(&self.all.related) {
            facts.push(TableFacts {
                id: t.dataset.id,
                table: &t.dataset.logical_table,
                description: t.dataset.description.as_deref(),
                columns: &related.columns,
                metrics: &related.metrics,
            });
        }
        let phrases: Vec<String> = build_phrases(question)
            .into_iter()
            .filter(|p| !p.has_stop)
            .map(|p| p.folded)
            .collect();
        build_table_choice(&facts, &self.full.relations, &phrases, model_tables)
    }

    /// Nhóm bảng và ngữ cảnh của các bảng gửi chi tiết.
    pub fn build_view(&self, choice: &TableChoice) -> (TableGroup, QuestionTables) {
        let group = self.full.build_subset(&choice.ids());
        let names: Vec<String> = choice.detailed.iter().map(|t| t.table.clone()).collect();
        (group, self.all.build_subset(&names))
    }

    /// Danh sách gọn: bảng đã chụp không gửi chi tiết, theo thứ tự của nhóm, rồi bảng chưa chụp.
    pub fn build_others(&self, choice: &TableChoice) -> Vec<CompactTable> {
        let mut others: Vec<CompactTable> = self
            .full
            .tables
            .iter()
            .filter(|t| !choice.contains(t.dataset.id))
            .map(|t| CompactTable {
                table: t.dataset.logical_table.clone(),
                description: build_compact_description(
                    t.dataset.description.as_deref(),
                    t.columns.iter().map(|c| c.name.as_str()),
                ),
                unsnapshotted: false,
            })
            .collect();
        others.extend(self.unsnapshotted.iter().cloned());
        others
    }

    /// Chi tiết của bước context: bảng đã gửi chi tiết và vì sao, bảng chỉ gửi tên, bảng chưa chụp.
    pub fn build_detail(&self, choice: &TableChoice) -> String {
        let others = self.build_others(choice);
        let names = |unsnapshotted: bool| -> Vec<String> {
            others
                .iter()
                .filter(|t| t.unsnapshotted == unsnapshotted)
                .map(|t| t.table.clone())
                .collect()
        };
        build_choice_detail(choice, &names(false), &names(true))
    }

    /// Thêm các bảng names (bảng logic của full) vào choice với lý do Needed.
    pub fn apply_needed(&self, choice: &mut TableChoice, names: &[String]) {
        for t in &self.full.tables {
            if names.contains(&t.dataset.logical_table) {
                choice.apply_table(t.dataset.id, &t.dataset.logical_table, DetailReason::Needed);
            }
        }
    }

    /// Lời "hai bảng không có quan hệ" khi câu SQL đọc hai bảng không nối được qua chuỗi khóa ngoại nào.
    pub fn build_unrelated_message(&self, sql: &str) -> Option<String> {
        let tables: Vec<(Uuid, String)> = self
            .full
            .tables
            .iter()
            .map(|t| (t.dataset.id, t.dataset.logical_table.clone()))
            .collect();
        let (a, b) = find_unrelated_pair(sql, &tables, &self.full.relations)?;
        Some(build_unrelated_message(&a, &b))
    }
}

/// Tên bảng lượt phân loại cần: bảng nó nêu từ danh sách gọn và bảng của các cột nó nêu.
pub fn build_model_tables(classified: &Classified) -> Vec<String> {
    let mut tables = classified.tables.clone();
    for (table, _) in &classified.columns {
        if !tables.contains(table) {
            tables.push(table.clone());
        }
    }
    tables
}

/// Bảng của cùng nguồn database mới có metadata (#247), dạng một dòng của danh sách gọn. Dataset nạp từ file không có
/// nguồn database nên không có bảng nào.
pub async fn load_unsnapshotted(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    full: &TableGroup,
) -> Result<Vec<CompactTable>, AppError> {
    let Some(source_id) = dataset.source_id else {
        return Ok(Vec::new());
    };
    let rows = state.store.list_metadata_tables(ws, source_id).await?;
    let mut tables = Vec::new();
    for row in rows {
        let name = row
            .table_name
            .rsplit_once('.')
            .map(|(_, n)| n)
            .unwrap_or(&row.table_name)
            .to_string();
        // Bảng đã chụp lại sau đó thì đã có trong nhóm; không liệt kê hai lần.
        if full.tables.iter().any(|t| t.dataset.logical_table == name) {
            continue;
        }
        let columns: Vec<String> = row.build_columns().into_iter().map(|c| c.name).collect();
        tables.push(CompactTable {
            table: name,
            description: build_compact_description(
                row.description.as_deref(),
                columns.iter().map(String::as_str),
            ),
            unsnapshotted: true,
        });
    }
    Ok(tables)
}

/// Một dòng mô tả: COMMENT ON TABLE nếu có (dòng đầu), không thì tên vài cột đầu.
fn build_compact_description<'a>(
    description: Option<&str>,
    columns: impl Iterator<Item = &'a str>,
) -> String {
    if let Some(line) = description
        .and_then(|d| d.lines().next())
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        return line.to_string();
    }
    let names: Vec<&str> = columns.take(COMPACT_COLUMNS).collect();
    if names.is_empty() {
        return String::new();
    }
    format!("columns {}", names.join(", "))
}
