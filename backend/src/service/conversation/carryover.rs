// SPDX-License-Identifier: Apache-2.0
/* service/conversation/carryover.rs - đánh dấu mục của thẻ "Đã hiểu là" lấy từ lượt trước (#164). Câu tiếp nối ("còn
năm 2017 thì sao?") giữ nhóm và độ đo của lượt trước; thẻ ghi "từ câu trước" cạnh các mục đó để người hỏi biết phần
nào mình không nói mà hệ thống vẫn áp. conversation/mod.rs gọi sau khi hỏi xong, trước khi lưu phần đã hiểu của lượt.

Một mục là "từ câu trước" khi hai điều cùng đúng: nó giống một mục của lượt trước gần nhất đã trả lời (đoạn SQL của nó
có nguyên văn trong câu SQL lượt đó đã chạy, hay cùng nhãn trong phần đã hiểu lượt đó lưu), và câu hỏi hiện tại không
nhắc tới nó (không chứa tên cột, giá trị hay năm trong đoạn SQL của mục). Không đọc được lượt trước (lượt lỗi, chưa
có phần đã hiểu) thì không đánh dấu gì: thiếu nhãn thì chỉ kém rõ, đánh dấu sai thì làm người hỏi hiểu lầm. Hàm thuần,
test ở cuối file. */

use crate::engine::store::StoredTurn;
use crate::service::ask::contract::{Understanding, UnderstoodItem};
use crate::service::ask::phrases::fold;

/// Trạng thái của lượt đã trả lời trong queries (QueryStatus::as_str).
const ANSWERED: &str = "ok";

/// Lượt trước gần nhất đã trả lời; turns theo thứ tự hỏi (Store::list_turns).
pub fn find_previous(turns: &[StoredTurn]) -> Option<&StoredTurn> {
    turns.iter().rev().find(|turn| turn.status == ANSWERED)
}

/// Đánh dấu from_previous cho độ đo, bộ lọc, nhóm và kỳ. Thứ tự và giới hạn không đánh dấu: chúng hiếm khi được
/// mang sang và nhãn của chúng phụ thuộc cột kết quả của từng câu.
pub fn apply_carryover(understanding: &mut Understanding, question: &str, previous: &StoredTurn) {
    let words = format!(" {} ", fold(question));
    let labels = build_previous_labels(previous.understood.as_deref());
    let sql = previous.sql_guarded.as_deref().unwrap_or("");
    let items = understanding
        .measures
        .iter_mut()
        .chain(understanding.filters.iter_mut())
        .chain(understanding.group_by.iter_mut())
        .chain(understanding.time_range.iter_mut());
    for item in items {
        let same = (!item.sql.is_empty() && sql.contains(&item.sql))
            || labels.iter().any(|label| *label == item.label);
        item.from_previous = same && !is_named(item, &words);
    }
}

/// Nhãn trong phần đã hiểu dạng chữ (build_understood_text): "measures: a, b; filters: c". Nhãn có dấu phẩy thì bị
/// tách sai và không khớp, nghĩa là không đánh dấu, đúng hướng an toàn ở đầu file.
fn build_previous_labels(understood: Option<&str>) -> Vec<&str> {
    let Some(text) = understood else {
        return Vec::new();
    };
    text.split("; ")
        .filter_map(|part| part.split_once(": ").map(|(_, labels)| labels))
        .flat_map(|labels| labels.split(", "))
        .collect()
}

/// Câu hỏi nhắc tới mục khi chứa (đã bỏ dấu, theo từ trọn vẹn) một tên cột trong nháy kép, một giá trị trong nháy đơn
/// hay một số bốn chữ số (năm) của đoạn SQL, hoặc phần đầu nhãn trước dấu hai chấm (tên chỉ số). Chuỗi định dạng ngày ('%m/%d/%Y') không phải tên nên bỏ qua.
fn is_named(item: &UnderstoodItem, words: &str) -> bool {
    let mut names = build_names(&item.sql);
    // Tên chỉ số đứng đầu nhãn độ đo ("Doanh thu: tổng Sales"): người hỏi gọi độ đo bằng tên đó chứ không bằng cột.
    if let Some((head, _)) = item.label.split_once(": ") {
        names.push(head.to_string());
    }
    names.iter().any(|name| {
        let name = fold(name);
        !name.is_empty() && words.contains(&format!(" {name} "))
    })
}

fn build_names(sql: &str) -> Vec<String> {
    let mut names = Vec::new();
    for quote in ['"', '\''] {
        // Phần nằm giữa hai dấu nháy là các phần lẻ khi tách theo dấu nháy.
        for name in sql.split(quote).skip(1).step_by(2) {
            if name.contains('%') {
                continue;
            }
            names.push(name.to_string());
        }
    }
    names.extend(build_years(sql));
    names
}

// Số bốn chữ số của đoạn SQL, như năm trong year(...) = 2017 hay trong giá trị ngày '2017-07-01': câu hỏi nêu năm chứ
// không nêu cả ngày.
fn build_years(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|part| part.len() == 4)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, sql: &str) -> UnderstoodItem {
        UnderstoodItem {
            label: label.to_string(),
            sql: sql.to_string(),
            from_previous: false,
        }
    }

    fn turn(status: &str, sql: Option<&str>, understood: Option<&str>) -> StoredTurn {
        StoredTurn {
            query_id: uuid::Uuid::nil(),
            question: "Doanh thu theo thị trường".to_string(),
            status: status.to_string(),
            sql_generated: sql.map(str::to_string),
            sql_guarded: sql.map(str::to_string),
            understood: understood.map(str::to_string),
            created_at: chrono::Utc::now(),
        }
    }

    // Thẻ của "còn năm 2017 thì sao?" sau "Doanh thu theo thị trường".
    fn follow_up_card() -> Understanding {
        Understanding {
            measures: vec![item("Doanh thu: tổng Sales", r#"SUM("Sales")"#)],
            filters: vec![item("Năm 2017", r#"year("Order Date") = 2017"#)],
            group_by: vec![item("Theo Market", r#""Market""#)],
            order_by: Vec::new(),
            limit: None,
            time_range: None,
            assumptions: Vec::new(),
        }
    }

    const PREVIOUS_SQL: &str =
        r#"SELECT "Market", SUM("Sales") AS revenue FROM "dataco" GROUP BY "Market""#;

    fn marks(card: &Understanding) -> Vec<(&str, bool)> {
        card.measures
            .iter()
            .chain(card.filters.iter())
            .chain(card.group_by.iter())
            .map(|i| (i.label.as_str(), i.from_previous))
            .collect()
    }

    #[test]
    fn items_of_the_previous_turn_are_marked() {
        let mut card = follow_up_card();
        let previous = turn("ok", Some(PREVIOUS_SQL), None);
        apply_carryover(&mut card, "còn năm 2017 thì sao?", &previous);
        assert_eq!(
            marks(&card),
            [
                ("Doanh thu: tổng Sales", true),
                ("Năm 2017", false),
                ("Theo Market", true)
            ]
        );
    }

    // Câu hỏi nhắc lại cột nhóm: mục đó không phải lấy từ lượt trước.
    #[test]
    fn items_the_question_names_are_not_marked() {
        let mut card = follow_up_card();
        let previous = turn("ok", Some(PREVIOUS_SQL), None);
        apply_carryover(&mut card, "Doanh thu theo market năm 2017", &previous);
        assert!(!card.group_by[0].from_previous);
        assert!(!card.measures[0].from_previous);
    }

    #[test]
    fn same_label_is_enough() {
        let mut card = follow_up_card();
        let understood = "measures: Doanh thu: tổng Sales; grouped by: Theo Market";
        let previous = turn("ok", Some("SELECT 1"), Some(understood));
        apply_carryover(&mut card, "còn năm 2017 thì sao?", &previous);
        assert!(card.group_by[0].from_previous);
    }

    #[test]
    fn unreadable_previous_turn_marks_nothing() {
        let mut card = follow_up_card();
        apply_carryover(&mut card, "còn năm 2017?", &turn("ok", None, None));
        assert!(marks(&card).iter().all(|(_, marked)| !marked));
    }

    #[test]
    fn previous_is_the_last_answered_turn() {
        let turns = vec![
            turn("ok", Some("SELECT 1"), None),
            turn("error", Some("SELECT 2"), None),
        ];
        let previous = find_previous(&turns).unwrap();
        assert_eq!(previous.sql_guarded.as_deref(), Some("SELECT 1"));
        assert!(find_previous(&[]).is_none());
    }

    #[test]
    fn years_in_dates_count_as_named() {
        let range = item("Quý 3/2017", "\"Order Date\" >= TIMESTAMP '2017-07-01'");
        assert!(is_named(&range, " vi sao quy 3 nam 2017 giam "));
        assert!(!is_named(&range, " con europe thi sao "));
    }
}
