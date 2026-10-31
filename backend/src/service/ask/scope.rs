// SPDX-License-Identifier: Apache-2.0
/* service/ask/scope.rs - câu hỏi trên cả một dataset (#257) bắt đầu từ bảng nào. Phạm vi của câu là mọi bảng của
dataset; luồng hỏi (ask/mod.rs) vẫn đọc một bảng bắt đầu cùng các bảng nối với nó qua quan hệ, trong giới hạn
MAX_HOPS bước và MAX_GROUP_TABLES bảng (ask/related.rs). Hàm pick_start_table ở đây là chỗ duy nhất chọn bảng bắt đầu
khi người hỏi không ghim bảng; #249 thay nó bằng cách chọn các bảng theo câu hỏi.

Luật hiện tại, cố ý đơn giản để giải thích được: bảng có nhiều cụm từ của câu hỏi trùng tên bảng, tên cột hay bí
danh cột nhất; bằng nhau thì bảng của lượt trước trong cùng cuộc trò chuyện (câu nối tiếp như "còn năm 2017 thì sao"
không nhắc tên cột nào); rồi bảng giữ nhiều khóa ngoại nhất (bảng sự kiện như order_items, đi tới được nhiều bảng chiều
nhất trong hai bước); rồi bảng nhiều dòng hơn. Chỉ xét bảng hỏi được (đã có hồ sơ cấu trúc). */

use uuid::Uuid;

use crate::engine::store::{CatalogColumnRow, Dataset, DatasetRelation, ProfileStatus};
use crate::service::ask::phrases::{build_phrases, fold};

/// Bảng bắt đầu của câu hỏi trên dataset có các bảng `tables` (cùng một dataset). None khi dataset không có bảng nào.
/// Không bảng nào hỏi được thì trả bảng đầu, để luồng hỏi báo đúng lý do (chưa có dữ liệu, hồ sơ đang lập).
pub fn pick_start_table(
    question: &str,
    tables: &[Dataset],
    columns: &[CatalogColumnRow],
    relations: &[DatasetRelation],
    previous: Option<Uuid>,
) -> Option<Uuid> {
    let phrases: Vec<String> = build_phrases(question)
        .into_iter()
        .filter(|p| !p.has_stop)
        .map(|p| p.folded)
        .collect();
    let ready = tables.iter().filter(|t| is_ready(t));
    let best = ready.max_by_key(|t| {
        let text = build_table_text(t, columns);
        let score = phrases.iter().filter(|p| contains_words(&text, p)).count();
        let held_keys = relations
            .iter()
            .filter(|r| r.from_dataset_id == t.id)
            .count();
        // Bộ so sánh của tuple đi từ trái sang phải; tên bảng đảo chiều (Reverse) để cùng điểm thì bảng tên xếp
        // trước thắng, kết quả không đổi theo thứ tự đọc từ Postgres.
        (
            score,
            previous == Some(t.id),
            held_keys,
            t.row_count.unwrap_or(0),
            std::cmp::Reverse(t.logical_table.clone()),
        )
    });
    best.or(tables.first()).map(|t| t.id)
}

/// Bảng có phiên bản hiện hành đã lập hồ sơ cấu trúc (như check_profile_ready của ask/context.rs).
fn is_ready(table: &Dataset) -> bool {
    let status = table.current_profile_status.as_deref();
    status == Some(ProfileStatus::Structure.as_str())
        || status == Some(ProfileStatus::Semantic.as_str())
}

/// Chữ của một bảng để so với câu hỏi, đã bỏ dấu, mỗi phần cách nhau bằng " | " để một cụm không khớp vắt qua hai cột.
fn build_table_text(table: &Dataset, columns: &[CatalogColumnRow]) -> String {
    let mut parts = vec![fold(&table.name), fold(&table.logical_table)];
    for c in columns.iter().filter(|c| c.table_id == table.id) {
        parts.push(fold(&c.name));
        parts.extend(c.aliases.iter().map(|a| fold(a)));
    }
    format!(" {} ", parts.join(" | "))
}

/// text chứa đúng các từ của phrase liền nhau (không khớp nửa từ: "tai" không khớp "retail").
fn contains_words(text: &str, phrase: &str) -> bool {
    text.contains(&format!(" {phrase} "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn table(id: u128, logical: &str, rows: i64) -> Dataset {
        Dataset {
            id: Uuid::from_u128(id),
            name: logical.to_string(),
            logical_table: logical.to_string(),
            domain: None,
            current_version_id: Some(Uuid::from_u128(id + 100)),
            parent_id: Uuid::nil(),
            row_count: Some(rows),
            description: None,
            current_profile_status: Some("semantic".to_string()),
            suggested_questions: Vec::new(),
            semantic_unavailable: false,
            semantic_attempted_with: None,
            source_kind: Some("postgres".to_string()),
            source_id: Some(Uuid::nil()),
            source_table: None,
            current_version_at: None,
            relations: Vec::new(),
            created_at: Utc::now(),
        }
    }

    fn column(table: u128, name: &str, aliases: &[&str]) -> CatalogColumnRow {
        CatalogColumnRow {
            table_id: Uuid::from_u128(table),
            name: name.to_string(),
            data_type: "VARCHAR".to_string(),
            role: "dimension".to_string(),
            description: None,
            description_source: None,
            is_sensitive: false,
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        }
    }

    fn relation(from: u128, to: u128) -> DatasetRelation {
        DatasetRelation {
            from_dataset_id: Uuid::from_u128(from),
            from_dataset_name: String::new(),
            from_columns: vec!["k".to_string()],
            to_dataset_id: Uuid::from_u128(to),
            to_dataset_name: String::new(),
            to_columns: vec!["k".to_string()],
            constraint_name: format!("fk_{from}_{to}"),
            cardinality: "many_to_one".to_string(),
        }
    }

    /// Database có hai nhóm bảng không nối nhau (bán hàng, học tập): câu hỏi nhắc cột của nhóm nào thì bắt đầu ở nhóm
    /// đó.
    #[test]
    fn question_words_pick_the_group() {
        let tables = vec![
            table(1, "order_items", 900),
            table(2, "orders", 300),
            table(3, "students", 500),
        ];
        let columns = vec![
            column(1, "item_total", &["doanh thu"]),
            column(3, "final_result", &["kết quả học tập"]),
        ];
        let relations = vec![relation(1, 2)];
        let pick = |q: &str| pick_start_table(q, &tables, &columns, &relations, None);
        assert_eq!(pick("Doanh thu theo tháng"), Some(Uuid::from_u128(1)));
        assert_eq!(
            pick("Kết quả học tập theo giới tính"),
            Some(Uuid::from_u128(3))
        );
        // Không cụm nào khớp: bảng giữ nhiều khóa ngoại nhất.
        assert_eq!(pick("Tổng quan"), Some(Uuid::from_u128(1)));
    }

    /// Câu nối tiếp không nhắc cột nào ở lại bảng của lượt trước.
    #[test]
    fn follow_up_keeps_the_previous_table() {
        let tables = vec![table(1, "order_items", 900), table(3, "students", 500)];
        let previous = Some(Uuid::from_u128(3));
        let picked = pick_start_table("còn năm 2017 thì sao", &tables, &[], &[], previous);
        assert_eq!(picked, previous);
    }

    /// Bảng chưa lập hồ sơ không được chọn khi còn bảng khác hỏi được; dataset chỉ có bảng như vậy thì trả bảng đầu để
    /// luồng hỏi báo lý do.
    #[test]
    fn tables_not_ready_are_skipped() {
        let mut pending = table(1, "order_items", 900);
        pending.current_profile_status = Some("pending".to_string());
        let tables = vec![pending.clone(), table(2, "orders", 10)];
        assert_eq!(
            pick_start_table("x", &tables, &[], &[], None),
            Some(Uuid::from_u128(2))
        );
        assert_eq!(
            pick_start_table("x", &[pending], &[], &[], None),
            Some(Uuid::from_u128(1))
        );
        assert_eq!(pick_start_table("x", &[], &[], &[], None), None);
    }
}
