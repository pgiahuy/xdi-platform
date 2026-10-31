// SPDX-License-Identifier: Apache-2.0
/* service/ask/table_pick.rs - bảng nào của database được gửi chi tiết (cột, kiểu, mô tả, giá trị) cho mô hình (#249).
Câu hỏi đọc được mọi bảng đã chụp của database (ask/related.rs). Hai trường hợp:
- Tổng metadata cột của mọi bảng tới WHOLE_DATASET_MAX_CHARS: gửi đủ mọi bảng, không có bước chọn. Database mẫu
  (sales_demo 6 bảng, edu_demo 7 bảng) luôn ở trường hợp này.
- Vượt trần: bảng gửi chi tiết là hợp của các nguồn dưới đây; bảng còn lại chỉ có tên và một dòng mô tả trong danh sách
  gọn của prompt (ask/group_prompt.rs, CompactTable).
  1. Bảng bắt đầu (ask/scope.rs chọn, hay người hỏi ghim).
  2. Bảng khớp chữ: một cụm của câu hỏi có trong tên bảng, mô tả bảng hay tên chỉ số của bảng, hoặc một cột của bảng
     khớp câu hỏi (tên, bí danh, mô tả, giá trị qua chỉ mục giá trị, ask/context.rs đánh dấu matched).
  3. Bảng lượt phân loại chọn từ danh sách gọn (ask/classify.rs trả `tables`), và bảng của cột lượt đó chọn.
  4. Bảng nằm trên đường khóa ngoại ngắn nhất từ bảng bắt đầu tới một bảng ở 2, 3: thiếu bảng giữa thì không viết
     được JOIN.
  5. Bảng nối trực tiếp một bước khóa ngoại với một bảng ở 1 tới 4, bảng có cột thời gian trước (câu "năm 2017" trên
     order_items cần orders.order_date), trong khi tổng metadata còn dưới trần. Bảng ở 1 tới 4 luôn được gửi, kể cả
     khi vượt trần: thiếu bảng thì câu sai, còn prompt dài chỉ rút cột không liên quan về dòng gọn (apply_metadata_cap).
  Chỗ thêm nguồn bảng gần câu hỏi theo embedding chạy tại chỗ (issue #249 nêu, chưa làm): một danh sách bảng nữa ở
  bước 2 của build_seeds, cùng lý do riêng.
Thiếu bảng vẫn sửa được: câu SQL dùng bảng chỉ có trong danh sách gọn hay mô hình báo cần bảng đó thì ask/mod.rs gửi lại
một lần kèm chi tiết bảng (ask/missing_tables.rs, lý do Needed).

Mỗi bảng gửi chi tiết mang một lý do (DetailReason), in ở chi tiết bước context để Minh chứng ghi bảng nào đã gửi và vì
sao. Hàm thuần, test ở ask/table_pick_tests.rs. */

use std::collections::VecDeque;

use uuid::Uuid;

use crate::engine::store::{DatasetRelation, MetricDefinition, ProfileRole};
use crate::service::ask::context::{metadata_chars, ColumnProfile};
use crate::service::ask::phrases::fold;
use crate::service::ask::prompt::PROMPT_MAX_CHARS;
use crate::service::ask::related::build_neighbors;

/// Trần tổng metadata cột (metadata_chars) của các bảng gửi chi tiết: nửa trần prompt, nửa còn lại cho luật, giá trị,
/// chỉ số, ví dụ và lịch sử hội thoại. Dưới trần này thì gửi đủ mọi bảng. sales_demo (6 bảng, 48 cột) chưa tới 5 000 ký
/// tự; trần chỉ chạm khi database có hàng chục bảng.
pub const WHOLE_DATASET_MAX_CHARS: usize = PROMPT_MAX_CHARS / 2;

/// Một bảng như bước chọn cần.
pub struct TableFacts<'a> {
    pub id: Uuid,
    /// Tên bảng logic.
    pub table: &'a str,
    /// Mô tả bảng (COMMENT ON TABLE của nguồn, Dataset::description); None khi không có.
    pub description: Option<&'a str>,
    /// Cột của ngữ cảnh bảng đó, đã đánh dấu matched (ask/context.rs build_context).
    pub columns: &'a [ColumnProfile],
    pub metrics: &'a [MetricDefinition],
}

/// Vì sao một bảng được gửi chi tiết.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailReason {
    /// Bảng bắt đầu của câu hỏi.
    Start,
    /// Metadata mọi bảng vừa trần nên gửi đủ.
    AllFit,
    /// Cụm của câu hỏi khớp tên, mô tả, chỉ số của bảng (words) hay cột của bảng (columns).
    Matched {
        words: Vec<String>,
        columns: Vec<String>,
    },
    /// Lượt phân loại chọn bảng này hay một cột của nó.
    Model,
    /// Bảng giữa trên đường khóa ngoại từ bảng bắt đầu tới bảng to.
    Path { to: String },
    /// Nối một bước khóa ngoại với bảng of.
    Neighbor { of: String },
    /// Câu SQL lần đầu cần bảng này mà nó chỉ có trong danh sách gọn; gửi lại kèm chi tiết.
    Needed,
}

/// Một bảng gửi chi tiết.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailTable {
    pub id: Uuid,
    pub table: String,
    pub reason: DetailReason,
}

/// Kết quả bước chọn: bảng gửi chi tiết, bảng bắt đầu ở đầu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableChoice {
    pub detailed: Vec<DetailTable>,
    /// true khi mọi bảng được gửi vì tổng metadata vừa trần.
    pub all_fit: bool,
}

impl TableChoice {
    pub fn ids(&self) -> Vec<Uuid> {
        self.detailed.iter().map(|t| t.id).collect()
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.detailed.iter().any(|t| t.id == id)
    }

    /// Thêm bảng với lý do, nếu chưa có.
    pub fn apply_table(&mut self, id: Uuid, table: &str, reason: DetailReason) {
        if !self.contains(id) {
            self.detailed.push(DetailTable {
                id,
                table: table.to_string(),
                reason,
            });
        }
    }
}

/// Bảng gửi chi tiết. tables[0] là bảng bắt đầu. phrases là các cụm đã bỏ dấu, không có từ dừng, của câu hỏi.
/// model_tables là tên bảng logic lượt phân loại chọn (bảng nó nêu và bảng của cột nó nêu); rỗng trước lượt đó.
pub fn build_table_choice(
    tables: &[TableFacts],
    relations: &[DatasetRelation],
    phrases: &[String],
    model_tables: &[String],
) -> TableChoice {
    let Some(start) = tables.first() else {
        return TableChoice {
            detailed: Vec::new(),
            all_fit: true,
        };
    };
    let total: usize = tables.iter().map(|t| build_table_chars(t)).sum();
    if total <= WHOLE_DATASET_MAX_CHARS {
        let detailed = tables
            .iter()
            .enumerate()
            .map(|(i, t)| DetailTable {
                id: t.id,
                table: t.table.to_string(),
                reason: if i == 0 {
                    DetailReason::Start
                } else {
                    DetailReason::AllFit
                },
            })
            .collect();
        return TableChoice {
            detailed,
            all_fit: true,
        };
    }
    let mut choice = build_seeds(tables, phrases, model_tables);
    let seeds: Vec<Uuid> = choice.ids();
    for &seed in seeds.iter().skip(1) {
        apply_path(&mut choice, tables, relations, start.id, seed);
    }
    apply_neighbors(&mut choice, tables, relations);
    choice
}

/// Bước 1 tới 3: bảng bắt đầu, bảng khớp chữ, bảng mô hình chọn. Một bảng vừa khớp chữ vừa được mô hình chọn giữ lý
/// do khớp chữ (đứng trước, cụ thể hơn).
fn build_seeds(tables: &[TableFacts], phrases: &[String], model_tables: &[String]) -> TableChoice {
    let mut choice = TableChoice {
        detailed: Vec::new(),
        all_fit: false,
    };
    for (i, t) in tables.iter().enumerate() {
        if i == 0 {
            choice.apply_table(t.id, t.table, DetailReason::Start);
            continue;
        }
        let words = build_matched_words(t, phrases);
        let columns: Vec<String> = t
            .columns
            .iter()
            .filter(|c| c.matched)
            .map(|c| c.name.clone())
            .collect();
        if !words.is_empty() || !columns.is_empty() {
            choice.apply_table(t.id, t.table, DetailReason::Matched { words, columns });
        }
    }
    for t in tables {
        if model_tables.iter().any(|m| m == t.table) {
            choice.apply_table(t.id, t.table, DetailReason::Model);
        }
    }
    choice
}

/// Cụm của câu hỏi có trong tên bảng (gạch dưới thành dấu cách), mô tả bảng hay tên chỉ số của bảng, so trọn từ sau khi
/// bỏ dấu, như ask/unsnapshotted.rs.
fn build_matched_words(table: &TableFacts, phrases: &[String]) -> Vec<String> {
    let mut parts = vec![table.table.replace('_', " ")];
    parts.extend(table.description.map(str::to_string));
    parts.extend(table.metrics.iter().map(|m| m.name.clone()));
    let text = format!(" {} ", fold(&parts.join(" | ")));
    phrases
        .iter()
        .filter(|p| text.contains(&format!(" {p} ")))
        .cloned()
        .collect()
}

/// Bước 4: các bảng giữa trên đường khóa ngoại ngắn nhất (hai chiều) từ start tới target, qua mọi bảng của database.
/// Không có đường thì không thêm gì: hai bảng không nối được, guard sẽ từ chối JOIN và ask/missing_tables.rs báo hai
/// bảng không có quan hệ.
fn apply_path(
    choice: &mut TableChoice,
    tables: &[TableFacts],
    relations: &[DatasetRelation],
    start: Uuid,
    target: Uuid,
) {
    let Some(path) = build_shortest_path(start, target, relations) else {
        return;
    };
    let Some(to) = tables.iter().find(|t| t.id == target) else {
        return;
    };
    for id in &path[1..path.len().saturating_sub(1)] {
        if let Some(t) = tables.iter().find(|t| t.id == *id) {
            let reason = DetailReason::Path {
                to: to.table.to_string(),
            };
            choice.apply_table(t.id, t.table, reason);
        }
    }
}

/// Đường ngắn nhất từ start tới target theo quan hệ, hai chiều, kể cả hai đầu; None khi không nối được.
pub fn build_shortest_path(
    start: Uuid,
    target: Uuid,
    relations: &[DatasetRelation],
) -> Option<Vec<Uuid>> {
    // came_from[i] = (bảng, bảng đứng trước nó trên đường); đi theo chiều rộng nên lần đầu gặp là đường ngắn nhất.
    let mut came_from: Vec<(Uuid, Uuid)> = vec![(start, start)];
    let mut queue = VecDeque::from([start]);
    while let Some(id) = queue.pop_front() {
        if id == target {
            let mut path = vec![target];
            let mut current = target;
            while current != start {
                current = came_from.iter().find(|(t, _)| *t == current)?.1;
                path.push(current);
            }
            path.reverse();
            return Some(path);
        }
        for next in build_neighbors(id, relations) {
            if !came_from.iter().any(|(t, _)| *t == next) {
                came_from.push((next, id));
                queue.push_back(next);
            }
        }
    }
    None
}

/// Bước 5: bảng nối một bước với bảng đã chọn, bảng có cột thời gian trước, rồi theo thứ tự bảng của nhóm (gần bảng bắt
/// đầu trước); dừng thêm khi tổng metadata sẽ vượt trần.
fn apply_neighbors(choice: &mut TableChoice, tables: &[TableFacts], relations: &[DatasetRelation]) {
    let mut used: usize = tables
        .iter()
        .filter(|t| choice.contains(t.id))
        .map(|t| build_table_chars(t))
        .sum();
    let chosen: Vec<DetailTable> = choice.detailed.clone();
    let mut candidates: Vec<(&TableFacts, &str)> = Vec::new();
    for picked in &chosen {
        for id in build_neighbors(picked.id, relations) {
            let Some(t) = tables.iter().find(|t| t.id == id) else {
                continue;
            };
            if !choice.contains(id) && !candidates.iter().any(|(c, _)| c.id == id) {
                candidates.push((t, picked.table.as_str()));
            }
        }
    }
    // sort_by_key giữ thứ tự cũ khi bằng nhau, nên trong cùng nhóm vẫn là thứ tự gặp ở trên.
    candidates.sort_by_key(|(t, _)| !has_time_column(t));
    for (t, of) in candidates {
        let chars = build_table_chars(t);
        if used + chars > WHOLE_DATASET_MAX_CHARS {
            continue;
        }
        used += chars;
        let reason = DetailReason::Neighbor { of: of.to_string() };
        choice.apply_table(t.id, t.table, reason);
    }
}

fn has_time_column(table: &TableFacts) -> bool {
    table
        .columns
        .iter()
        .any(|c| c.role == ProfileRole::Time.as_str())
}

/// Độ dài metadata của một bảng: tổng metadata_chars của mọi cột (ask/context.rs), cùng cách đếm với trần của #238.
fn build_table_chars(table: &TableFacts) -> usize {
    table.columns.iter().map(metadata_chars).sum()
}

/// Chi tiết của bước context: các bảng đã gửi chi tiết và vì sao, rồi các bảng chỉ có tên. compact là tên các bảng
/// chỉ có trong danh sách gọn, unsnapshotted là tên bảng chưa chụp (chỉ có metadata, #247).
pub fn build_choice_detail(
    choice: &TableChoice,
    compact: &[String],
    unsnapshotted: &[String],
) -> String {
    let mut text = if choice.all_fit {
        format!(
            "Gửi chi tiết mọi bảng của database ({} bảng): metadata vừa trần prompt.",
            choice.detailed.len()
        )
    } else {
        let parts: Vec<String> = choice
            .detailed
            .iter()
            .map(|t| format!("{} ({})", t.table, build_reason_text(&t.reason)))
            .collect();
        format!("Bảng gửi chi tiết: {}.", parts.join("; "))
    };
    if !compact.is_empty() {
        text.push_str(&format!(" Chỉ gửi tên: {}.", compact.join(", ")));
    }
    if !unsnapshotted.is_empty() {
        text.push_str(&format!(
            " Chưa chụp, chỉ có metadata: {}.",
            unsnapshotted.join(", ")
        ));
    }
    text
}

/// Lý do bằng lời cho người đọc Minh chứng.
pub fn build_reason_text(reason: &DetailReason) -> String {
    match reason {
        DetailReason::Start => "bảng bắt đầu".to_string(),
        DetailReason::AllFit => "đủ mọi bảng".to_string(),
        DetailReason::Matched { words, columns } => {
            let mut parts = Vec::new();
            if !words.is_empty() {
                parts.push(format!("khớp chữ \"{}\"", words.join("\", \"")));
            }
            if !columns.is_empty() {
                parts.push(format!("cột khớp câu hỏi: {}", columns.join(", ")));
            }
            parts.join(", ")
        }
        DetailReason::Model => "mô hình chọn".to_string(),
        DetailReason::Path { to } => format!("nằm trên đường khóa ngoại tới {to}"),
        DetailReason::Neighbor { of } => format!("nối khóa ngoại với {of}"),
        DetailReason::Needed => "câu SQL cần bảng này, gửi lại kèm chi tiết".to_string(),
    }
}
