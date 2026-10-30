// SPDX-License-Identifier: Apache-2.0
/* service/sources/join_sql.rs - dựng câu SELECT chụp bảng chính kèm các bảng nối nhiều-một (relations.rs), cùng tên cột
của dataset. Hàm thuần, không kết nối database: test ở tests.rs kiểm tên cột, chỗ trùng tên và cách đặt ngoặc.

Tên cột:
- Cột của bảng chính giữ nguyên tên.
- Cột của bảng nối có dạng "<tiền tố>.<cột>", tiền tố là tên bảng không kèm schema: orders.order_date,
  customers.segment. Bảng cùng tên được nối lần hai (hai khóa ngoại cùng trỏ một bảng, hay hai schema có bảng cùng tên)
  thì tiền tố thêm _2, _3 theo thứ tự nối: courses, courses_2.
- Tên vẫn trùng một cột đã có (không phân biệt hoa thường, vì DuckDB và guard không phân biệt) thì thêm _2, _3 vào sau.
- Không chép cột đích của khóa (orders.order_id): giá trị của nó đúng bằng cột khóa ngoại của bảng trước đã có trong
  dataset.

An toàn: mọi tên bảng, tên cột trong câu SQL lấy từ pg_catalog (relations.rs), không lấy từ chữ người dùng gửi; request
chỉ chọn id quan hệ trong danh sách đã tìm. Tên nào cũng đặt trong ngoặc kép, ngoặc kép bên trong nhân đôi. Bí danh bảng
trong câu (t0, t1...) do code đặt. */

use std::collections::{HashMap, HashSet};

use super::postgres::{quote_ident, SourceTable};
use super::relations::Relation;
use crate::engine::store::ModelJoin;

/// Câu SELECT của một lần chụp và mô hình các bảng nối.
#[derive(Debug, Clone)]
pub struct JoinPlan {
    /// SELECT không có dấu chấm phẩy, dùng trong COPY (...) và trong đếm dòng.
    pub select: String,
    pub joins: Vec<ModelJoin>,
}

pub fn qualified(table: &SourceTable) -> String {
    format!(
        "{}.{}",
        quote_ident(&table.schema),
        quote_ident(&table.name)
    )
}

/// Câu SELECT cho bảng chính và các quan hệ đã chọn, theo thứ tự relations (bước 1 trước bước 2 của nó). Không chọn
/// quan hệ nào thì là SELECT * như khi chưa có tính năng nối, để bản chụp một bảng ra đúng nội dung như trước.
pub fn build_join_plan(
    fact: &SourceTable,
    fact_columns: &[String],
    relations: &[&Relation],
) -> JoinPlan {
    if relations.is_empty() {
        return JoinPlan {
            select: format!("SELECT * FROM {}", qualified(fact)),
            joins: Vec::new(),
        };
    }
    let mut used_names: HashSet<String> = fact_columns.iter().map(|c| c.to_lowercase()).collect();
    let mut used_prefixes: HashSet<String> = HashSet::new();
    let mut select: Vec<String> = fact_columns
        .iter()
        .map(|c| format!("t0.{}", quote_ident(c)))
        .collect();
    let mut from = format!("{} AS t0", qualified(fact));
    // Bí danh trong câu SQL của bảng đích mỗi quan hệ, theo id: bước 2 nối từ bí danh của bước 1.
    let mut aliases: HashMap<&str, String> = HashMap::new();
    // Theo id quan hệ: tên trong dataset của từng cột bảng đích, kể cả cột khóa (bằng cột khóa ngoại phía trước).
    let mut dataset_names: HashMap<&str, HashMap<String, String>> = HashMap::new();
    let mut joins = Vec::new();
    for (i, relation) in relations.iter().enumerate() {
        let alias = format!("t{}", i + 1);
        let from_alias = match &relation.info.parent {
            Some(parent) => aliases
                .get(parent.as_str())
                .cloned()
                .unwrap_or_else(|| "t0".into()),
            None => "t0".to_string(),
        };
        let key = &relation.key;
        let on: Vec<String> = key
            .from_columns
            .iter()
            .zip(&key.to_columns)
            .map(|(f, t)| {
                format!(
                    "{from_alias}.{} = {alias}.{}",
                    quote_ident(f),
                    quote_ident(t)
                )
            })
            .collect();
        from.push_str(&format!(
            " LEFT JOIN {} AS {alias} ON {}",
            qualified(&key.to),
            on.join(" AND ")
        ));
        // Cột trong dataset mang giá trị cột khóa ngoại của bảng phía trước: bảng chính thì đúng tên cột; bảng nối ở
        // bước 1 thì tên đã đặt cho cột đó ở bước 1.
        let parent_names = relation
            .info
            .parent
            .as_deref()
            .and_then(|p| dataset_names.get(p));
        let key_columns: Vec<String> = key
            .from_columns
            .iter()
            .map(|c| {
                parent_names
                    .and_then(|names| names.get(c).cloned())
                    .unwrap_or_else(|| c.clone())
            })
            .collect();
        let mut names: HashMap<String, String> = key
            .to_columns
            .iter()
            .cloned()
            .zip(key_columns.iter().cloned())
            .collect();
        let prefix = build_free_name(&key.to.name, &mut used_prefixes);
        let mut columns = Vec::new();
        for column in relation
            .target_columns
            .iter()
            .filter(|c| !key.to_columns.contains(c))
        {
            let name = build_free_name(&format!("{prefix}.{column}"), &mut used_names);
            select.push(format!(
                "{alias}.{} AS {}",
                quote_ident(column),
                quote_ident(&name)
            ));
            names.insert(column.clone(), name.clone());
            columns.push(name);
        }
        aliases.insert(relation.info.id.as_str(), alias);
        dataset_names.insert(relation.info.id.as_str(), names);
        joins.push(ModelJoin {
            relation: relation.info.id.clone(),
            constraint: key.constraint.clone(),
            from_table: key.from.full_name(),
            from_columns: key.from_columns.clone(),
            to_table: key.to.full_name(),
            to_columns: key.to_columns.clone(),
            prefix,
            columns,
            key_columns,
        });
    }
    JoinPlan {
        select: format!("SELECT {} FROM {from}", select.join(", ")),
        joins,
    }
}

/// Tên chưa dùng (so không phân biệt hoa thường): giữ nguyên nếu được, không thì thêm _2, _3. Ghi tên chọn vào used.
fn build_free_name(wanted: &str, used: &mut HashSet<String>) -> String {
    let mut name = wanted.to_string();
    let mut n = 2;
    while used.contains(&name.to_lowercase()) {
        name = format!("{wanted}_{n}");
        n += 1;
    }
    used.insert(name.to_lowercase());
    name
}
