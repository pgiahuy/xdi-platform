// SPDX-License-Identifier: Apache-2.0
/* service/ask/why_table.rs - bảng bắt đầu của câu hỏi nguyên nhân trên cả một database, sau lượt phân loại. Bảng bắt
đầu ban đầu do ask/scope.rs chọn theo chữ của câu hỏi, trước khi có lượt phân loại. Câu hỏi số liệu không sao vì mô hình
viết SQL nối được mọi bảng; còn form của câu hỏi nguyên nhân chỉ đọc bảng bắt đầu và các bảng nối nhiều-một vào nó
(explain/source.rs). Bảng bắt đầu sai thì form không có cột cần: "Vì sao tỷ lệ bỏ học kỳ 2014J cao hơn kỳ 2014B" bắt đầu ở
assessments nên đo trọng số bài thay cho tỷ lệ bỏ học của students; "Vì sao doanh thu năm 2017 khác nhau giữa Europe và
LATAM" bắt đầu ở customers nên form báo dữ liệu không có doanh thu.

Lượt phân loại (ask/classify.rs) trả các cột câu hỏi cần, dạng bảng.cột. Luật chọn, cố ý đơn giản:
1. Bảng ứng viên là bảng của các cột đó mà từ nó đi tới được mọi bảng còn lại của các cột đó (build_reachable: nhiều-một,
   một-một, tối đa BREAKDOWN_MAX_HOPS bước). order_items đi tới orders nên "doanh thu (order_items.item_total) theo
   orders.market" bắt đầu ở order_items; orders không đi xuống order_items được.
2. Bảng đang chọn là ứng viên thì giữ.
3. Không thì ứng viên giữ cột vai trò measure mà lượt phân loại nêu, rồi ứng viên giữ nhiều cột được nêu nhất, rồi tên
   bảng theo thứ tự chữ để kết quả không đổi theo thứ tự đọc.
Không có ứng viên (lượt phân loại không nêu cột, hay các cột nằm ở các bảng không nối được) thì giữ bảng đang chọn.
Hàm thuần, test ở cuối file. */

use uuid::Uuid;

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::source::build_reachable;

/// Một bảng của database như bước chọn cần.
pub struct WhyTable<'a> {
    pub id: Uuid,
    /// Tên bảng logic.
    pub table: &'a str,
    pub columns: &'a [ProfileColumn],
}

/// Bảng bắt đầu mới cho câu hỏi nguyên nhân, hay None khi giữ bảng current. named là các cột lượt phân loại nêu, cặp
/// (bảng logic, tên cột).
pub fn pick_why_table(
    named: &[(String, String)],
    tables: &[WhyTable],
    links: &[JoinLink],
    current: Uuid,
) -> Option<Uuid> {
    let mut named_tables: Vec<&WhyTable> = Vec::new();
    for (table, _) in named {
        if let Some(found) = tables.iter().find(|t| t.table == table) {
            if !named_tables.iter().any(|t| t.id == found.id) {
                named_tables.push(found);
            }
        }
    }
    let candidates: Vec<&WhyTable> = named_tables
        .iter()
        .copied()
        .filter(|t| reaches_all(t, &named_tables, links))
        .collect();
    if candidates.is_empty() || candidates.iter().any(|t| t.id == current) {
        return None;
    }
    let best = candidates.iter().max_by_key(|t| {
        (
            holds_named_measure(t, named),
            named.iter().filter(|(table, _)| table == t.table).count(),
            std::cmp::Reverse(t.table.to_string()),
        )
    })?;
    Some(best.id)
}

/// Từ table đi tới được mọi bảng khác trong others.
fn reaches_all(table: &WhyTable, others: &[&WhyTable], links: &[JoinLink]) -> bool {
    let reachable = build_reachable(table.table, links);
    others
        .iter()
        .filter(|o| o.id != table.id)
        .all(|o| reachable.iter().any(|r| r.eq_ignore_ascii_case(o.table)))
}

/// Bảng giữ một cột vai trò measure trong các cột được nêu: bảng của con số được cộng là bảng sự kiện.
fn holds_named_measure(table: &WhyTable, named: &[(String, String)]) -> bool {
    named
        .iter()
        .filter(|(t, _)| t == table.table)
        .any(|(_, column)| {
            table
                .columns
                .iter()
                .any(|c| &c.name == column && c.role == ProfileRole::Measure.as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, role: &str) -> ProfileColumn {
        ProfileColumn {
            id: Uuid::new_v4(),
            position: 0,
            name: name.to_string(),
            data_type: "VARCHAR".to_string(),
            role: role.to_string(),
            description: None,
            description_source: None,
            aliases: Vec::new(),
            is_sensitive: false,
            null_count: None,
            distinct_count: None,
            min_value: None,
            max_value: None,
            time_format: None,
            unit: None,
            aggregation: "sum".to_string(),
        }
    }

    fn link(many: &str, one: &str) -> JoinLink {
        JoinLink {
            many: many.to_string(),
            many_columns: vec!["k".to_string()],
            one: one.to_string(),
            one_columns: vec!["k".to_string()],
            fans_out: true,
        }
    }

    fn named(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(t, c)| (t.to_string(), c.to_string()))
            .collect()
    }

    /// Bán hàng: order_items -> orders -> customers. "Doanh thu năm 2017 giữa Europe và LATAM" bắt đầu ở customers (chọn
    /// theo chữ), lượt phân loại nêu order_items.item_total cùng orders.market, orders.order_date: đổi sang order_items.
    #[test]
    fn the_child_table_of_the_measure_wins() {
        let items = [column("item_total", "measure")];
        let orders = [column("market", "dimension"), column("order_date", "time")];
        let customers = [column("segment", "dimension")];
        let tables = [
            WhyTable {
                id: Uuid::from_u128(1),
                table: "order_items",
                columns: &items,
            },
            WhyTable {
                id: Uuid::from_u128(2),
                table: "orders",
                columns: &orders,
            },
            WhyTable {
                id: Uuid::from_u128(3),
                table: "customers",
                columns: &customers,
            },
        ];
        let links = [link("order_items", "orders"), link("orders", "customers")];
        let columns = named(&[
            ("orders", "market"),
            ("order_items", "item_total"),
            ("orders", "order_date"),
        ]);
        let pick =
            |current: u128| pick_why_table(&columns, &tables, &links, Uuid::from_u128(current));
        assert_eq!(pick(3), Some(Uuid::from_u128(1)));
        // Đã ở đúng bảng thì giữ.
        assert_eq!(pick(1), None);
    }

    /// Học tập: students và assessments cùng trỏ tới courses, không nối nhau. Lượt phân loại nêu hai cột của students:
    /// câu bắt đầu ở assessments đổi sang students.
    #[test]
    fn the_table_of_the_named_columns_wins() {
        let students = [
            column("code_presentation", "dimension"),
            column("final_result", "dimension"),
        ];
        let assessments = [column("weight", "measure")];
        let tables = [
            WhyTable {
                id: Uuid::from_u128(1),
                table: "assessments",
                columns: &assessments,
            },
            WhyTable {
                id: Uuid::from_u128(2),
                table: "students",
                columns: &students,
            },
        ];
        let links = [link("students", "courses"), link("assessments", "courses")];
        let columns = named(&[
            ("students", "code_presentation"),
            ("students", "final_result"),
        ]);
        assert_eq!(
            pick_why_table(&columns, &tables, &links, Uuid::from_u128(1)),
            Some(Uuid::from_u128(2))
        );
    }

    /// Không cột nào được nêu, hay các cột nằm ở hai bảng không nối được: giữ bảng đang chọn.
    #[test]
    fn no_candidate_keeps_the_current_table() {
        let a = [column("x", "measure")];
        let b = [column("y", "measure")];
        let tables = [
            WhyTable {
                id: Uuid::from_u128(1),
                table: "a",
                columns: &a,
            },
            WhyTable {
                id: Uuid::from_u128(2),
                table: "b",
                columns: &b,
            },
        ];
        let current = Uuid::from_u128(1);
        assert_eq!(pick_why_table(&[], &tables, &[], current), None);
        let columns = named(&[("a", "x"), ("b", "y")]);
        assert_eq!(pick_why_table(&columns, &tables, &[], current), None);
    }
}
