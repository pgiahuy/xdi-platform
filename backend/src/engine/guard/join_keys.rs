// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_keys.rs - khóa ngoại của luật nối bảng (guard/joins.rs): các quan hệ guard biết (JoinLink do service
truyền vào, cộng quan hệ suy ra từ điều kiện lan theo khóa ngoại RowFilter::Parent), và ba phép so: bộ phép bằng giữa
hai bảng có đúng bằng cột của một khóa không (key_matches), hai cột có cùng miền giá trị không (same_domain), và lời
từ chối nêu đúng các khóa nối được giữa hai bảng (describe_keys). Tên bảng và cột so theo chữ thường, như guard. */

use super::{find_parent_paths, AllowedTable};

/// Một quan hệ khóa ngoại giữa hai bảng, theo tên bảng logic (tên câu SQL dùng). Service dựng từ dataset_relations
/// (service/ask/fanout.rs, build_join_links); guard dùng để kiểm điều kiện nối, ask/fanout.rs để chặn cộng lặp,
/// explain/source.rs để dựng phần FROM của phân rã.
#[derive(Debug, Clone)]
pub struct JoinLink {
    /// Bảng giữ khóa ngoại (bên nhiều).
    pub many: String,
    pub many_columns: Vec<String>,
    /// Bảng được trỏ tới (bên một).
    pub one: String,
    pub one_columns: Vec<String>,
    /// true với many_to_one: một dòng bên một ứng với nhiều dòng bên nhiều. false với one_to_one.
    pub fans_out: bool,
}

/// Một cột của bảng dữ liệu, tên viết thường.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Origin {
    pub table: String,
    pub column: String,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.table, self.column)
    }
}

/// Một khóa viết theo hai bảng a, b: các cặp (cột của a, cột của b).
type KeyPairs = Vec<(String, String)>;

/// Các quan hệ guard biết, tên đã viết thường.
pub(super) struct Keys {
    links: Vec<JoinLink>,
}

impl Keys {
    /// Quan hệ service truyền vào, cộng quan hệ của mọi RowFilter::Parent trong tables (kể cả bảng cha nhiều bước):
    /// điều kiện lan do chính service dựng từ dataset_relations, nên là khóa thật. Nhờ vậy câu đã qua guard có hộp lọc
    /// (truy dòng nguồn đưa lại sql_guarded qua guard) không bị từ chối vì nửa nối `IN (SELECT ...)` của hộp.
    pub fn build(links: &[JoinLink], tables: &[AllowedTable]) -> Keys {
        let mut all: Vec<JoinLink> = links.iter().map(lowercase_link).collect();
        let mut pending: Vec<&AllowedTable> = tables.iter().collect();
        while let Some(table) = pending.pop() {
            for (columns, parent_columns, parent) in find_parent_paths(&table.row_filters) {
                all.push(lowercase_link(&JoinLink {
                    many: table.name.clone(),
                    many_columns: columns.to_vec(),
                    one: parent.name.clone(),
                    one_columns: parent_columns.to_vec(),
                    fans_out: true,
                }));
                pending.push(parent);
            }
        }
        Keys { links: all }
    }

    /// Mọi khóa giữa bảng a và bảng b, viết theo (cột của a, cột của b). Khóa tự trỏ (a là b) cho cả hai chiều, vì
    /// trong `e JOIN m` bí danh nào là bên giữ khóa ngoại cũng được.
    fn between(&self, a: &str, b: &str) -> Vec<KeyPairs> {
        let mut keys = Vec::new();
        for link in &self.links {
            let forward = || zip_pairs(&link.many_columns, &link.one_columns);
            let backward = || zip_pairs(&link.one_columns, &link.many_columns);
            if link.many == a && link.one == b {
                keys.push(forward());
            }
            if link.many == b && link.one == a {
                keys.push(backward());
            }
        }
        keys
    }

    /// Bộ phép bằng `pairs` giữa bảng a và b có đúng bằng cột của một khóa không: đủ mọi cột, không thừa cột nào, thứ
    /// tự tùy ý.
    pub fn key_matches(&self, a: &str, b: &str, pairs: &[(String, String)]) -> bool {
        let wanted = sorted(pairs.to_vec());
        self.between(a, b)
            .into_iter()
            .any(|key| sorted(key) == wanted)
    }

    /// Cặp cột là một cặp của khóa nào đó (một cột của khóa nhiều cột cũng tính).
    pub fn is_key_pair(&self, a: &Origin, b: &Origin) -> bool {
        self.between(&a.table, &b.table)
            .iter()
            .flatten()
            .any(|(x, y)| *x == a.column && *y == b.column)
    }

    /// Hai cột cùng miền giá trị: cùng một cột, một cặp của khóa, hay hai khóa ngoại trỏ cùng một cột của bảng cha
    /// (orders.customer_id và returns.customer_id cùng trỏ customers.customer_id).
    pub fn same_domain(&self, a: &Origin, b: &Origin) -> bool {
        a == b
            || self.is_key_pair(a, b)
            || self
                .parents_of(a)
                .iter()
                .any(|p| self.parents_of(b).contains(p))
    }

    /// Cột của bảng cha mà cột này trỏ tới, qua mọi khóa ngoại có cột đó.
    fn parents_of(&self, origin: &Origin) -> Vec<Origin> {
        let mut parents = Vec::new();
        for link in self.links.iter().filter(|l| l.many == origin.table) {
            for (own, parent) in link.many_columns.iter().zip(&link.one_columns) {
                if *own == origin.column {
                    parents.push(Origin {
                        table: link.one.clone(),
                        column: parent.clone(),
                    });
                }
            }
        }
        parents
    }

    /// Lời cho vòng sửa: các khóa nối được giữa hai bảng, hay câu nói hai bảng không có quan hệ.
    pub fn describe_keys(&self, a: &str, b: &str) -> String {
        let keys = self.between(a, b);
        if keys.is_empty() && a == b {
            return format!(
                "Bảng {a} không có khóa ngoại trỏ vào chính nó trong database, nên không được nối bảng này với chính nó"
            );
        }
        if keys.is_empty() {
            return format!(
                "Bảng {a} và {b} không có quan hệ trong database, nên không được nối hai bảng này, kể cả CROSS JOIN hay \
                 nối qua WHERE; chỉ nối các bảng theo quan hệ đã liệt kê"
            );
        }
        // Viết theo chiều khóa ngoại (bảng giữ khóa trước), như danh sách quan hệ trong prompt.
        let written: Vec<String> = self
            .links
            .iter()
            .filter(|l| (l.many == a && l.one == b) || (l.many == b && l.one == a))
            .map(|l| {
                let pairs = l.many_columns.iter().zip(&l.one_columns);
                let parts: Vec<String> = pairs
                    .map(|(x, y)| format!("{}.{x} = {}.{y}", l.many, l.one))
                    .collect();
                parts.join(" AND ")
            })
            .collect();
        format!(
            "Hai bảng {a} và {b} chỉ nối theo khóa ngoại, đủ mọi cột của khóa: {}",
            written.join("; hoặc ")
        )
    }
}

fn lowercase_link(link: &JoinLink) -> JoinLink {
    let lower = |columns: &[String]| columns.iter().map(|c| c.to_lowercase()).collect();
    JoinLink {
        many: link.many.to_lowercase(),
        many_columns: lower(&link.many_columns),
        one: link.one.to_lowercase(),
        one_columns: lower(&link.one_columns),
        fans_out: link.fans_out,
    }
}

fn zip_pairs(left: &[String], right: &[String]) -> KeyPairs {
    left.iter().cloned().zip(right.iter().cloned()).collect()
}

fn sorted(mut pairs: KeyPairs) -> KeyPairs {
    pairs.sort();
    pairs.dedup();
    pairs
}
