// SPDX-License-Identifier: Apache-2.0
/* service/ask/missing_tables.rs - hai việc của câu hỏi trên cả database (#249) sau lần mô hình viết SQL đầu:
- find_needed_tables: câu trả lời cần một bảng chỉ có trong danh sách gọn (ask/table_pick.rs không gửi chi tiết): mô
  hình ghi tên bảng vào needed_tables (ask/reply.rs), hay câu SQL đọc bảng đó sau FROM hay JOIN. ask/mod.rs gửi lại một
  lần kèm chi tiết các bảng đó, như gửi lại kèm đủ giá trị ở ask/value_retry.rs, rồi mới cho câu qua guard: không gửi
  lại thì guard từ chối vì bảng không có trong danh sách, vòng sửa cũng không biết cột của bảng nên chỉ đoán. Bảng chưa
  chụp (#247) không có chi tiết để gửi, nên không tính; câu đó nhận lời "chưa chụp" của ask/unsnapshotted.rs.
- find_unrelated_pair: câu không trả lời được mà SQL đọc hai bảng không nối được qua chuỗi khóa ngoại nào của database:
  lời cho người hỏi nói rõ hai bảng không có quan hệ, thay cho lời chung của guard. Guard đã từ chối JOIN không theo
  khóa ngoại đã khai (#256); ở đây chỉ dựng lời.

Đọc tên bảng trong SQL bằng chữ, không phân tích SQL: từ đứng ngay sau FROM hay JOIN. Nhận nhầm một bảng thì chỉ gửi
lại thừa một lần hay lời báo kém chính xác, không chạy câu nào sai. Hàm thuần, test ở ask/table_pick_tests.rs. */

use uuid::Uuid;

use crate::engine::store::DatasetRelation;
use crate::service::ask::group_prompt::CompactTable;
use crate::service::ask::table_pick::build_shortest_path;

/// Tên các bảng (đã chụp) của others mà câu trả lời thô reply cần, theo thứ tự của others.
pub fn find_needed_tables(reply: &str, others: &[CompactTable]) -> Vec<String> {
    let Some(object) = build_first_object(reply) else {
        return Vec::new();
    };
    let asked: Vec<String> = object
        .get("needed_tables")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str())
                .map(|name| bare_name(name.trim().trim_matches('"')).to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let sql = object.get("sql").and_then(|v| v.as_str()).unwrap_or("");
    let read = build_sql_tables(sql);
    others
        .iter()
        .filter(|t| !t.unsnapshotted)
        .filter(|t| {
            let name = t.table.to_lowercase();
            asked.contains(&name) || read.contains(&name)
        })
        .map(|t| t.table.clone())
        .collect()
}

/// Hai bảng câu SQL đọc mà không chuỗi quan hệ nào của database nối được. tables là (id, tên bảng logic) của mọi bảng
/// đã chụp của database, relations là mọi quan hệ giữa chúng. None khi SQL đọc một bảng hay mọi bảng nối được nhau.
pub fn find_unrelated_pair(
    sql: &str,
    tables: &[(Uuid, String)],
    relations: &[DatasetRelation],
) -> Option<(String, String)> {
    let read = build_sql_tables(sql);
    let found: Vec<&(Uuid, String)> = tables
        .iter()
        .filter(|(_, name)| read.contains(&name.to_lowercase()))
        .collect();
    for (i, a) in found.iter().enumerate() {
        for b in &found[i + 1..] {
            if build_shortest_path(a.0, b.0, relations).is_none() {
                return Some((a.1.clone(), b.1.clone()));
            }
        }
    }
    None
}

/// Lời cho người hỏi khi câu cần nối hai bảng không có quan hệ.
pub fn build_unrelated_message(a: &str, b: &str) -> String {
    format!(
        "Bảng {a} và bảng {b} không có quan hệ khóa ngoại nào trong database, kể cả qua bảng khác, nên hệ thống không \
         nối hai bảng này (không đoán cách nối). Hỏi riêng trên từng bảng."
    )
}

/// Tên bảng (chữ thường, không kèm schema) đứng ngay sau FROM hay JOIN trong câu SQL.
fn build_sql_tables(sql: &str) -> Vec<String> {
    let lower = sql.to_lowercase();
    // Dấu chấm giữ lại để "sales.orders" thành một từ rồi bỏ schema; nháy kép, dấu phẩy, ngoặc tách từ.
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .filter(|w| !w.is_empty())
        .collect();
    let mut tables = Vec::new();
    for pair in words.windows(2) {
        if pair[0] == "from" || pair[0] == "join" {
            let name = bare_name(pair[1]).to_string();
            if !tables.contains(&name) {
                tables.push(name);
            }
        }
    }
    tables
}

fn bare_name(full: &str) -> &str {
    full.rsplit_once('.').map(|(_, name)| name).unwrap_or(full)
}

/// Object JSON đầu tiên của câu trả lời (mô hình không bị ép schema có thể viết thêm chữ trước sau).
fn build_first_object(reply: &str) -> Option<serde_json::Value> {
    let start = reply.find('{')?;
    let mut objects =
        serde_json::Deserializer::from_str(&reply[start..]).into_iter::<serde_json::Value>();
    objects.next()?.ok().filter(|v| v.is_object())
}
