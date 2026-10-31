// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_sources.rs - nguồn dòng trong FROM của một SELECT cho luật nối bảng (guard/joins.rs): nguồn nào
giữ nguyên dòng của một bảng dữ liệu (Grain::Rows), cột của câu trỏ về cột nào của bảng dữ liệu (Origin), và hình dạng
kết quả của một câu con hay bảng tạm (Shape), để câu ngoài nối với nó vẫn biết cột khóa đến từ bảng nào.

Vì sao phân biệt Rows với Derived: nối hai nguồn giữ nguyên dòng là nối bảng thật, phải đúng khóa ngoại. Câu con đã gộp
nhóm (một dòng mỗi đơn, mỗi tháng) không còn là dòng của bảng nào; nối nó với bảng khác theo cột cùng miền giá trị
(customer_id của hai bảng tạm gộp theo khách) là cách đúng để tránh cộng lặp, nên luật lỏng hơn (joins.rs). */

use sqlparser::ast::{Expr, Ident, TableAlias};

use super::join_keys::Origin;
use super::{name_key, AllowedTable};

/// Mỗi dòng của nguồn ứng với gì.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Grain {
    /// Một dòng của bảng dữ liệu (tên viết thường): chính bảng đó, hay câu con, bảng tạm chỉ lọc và chọn cột của nó.
    Rows(String),
    /// Kết quả gộp nhóm, DISTINCT, nối nhiều nguồn, UNION: dòng không còn là dòng của một bảng.
    Derived,
}

/// Kết quả của một câu hay một bảng: mỗi cột (tên viết thường) kèm cột gốc nếu cột chỉ chép từ một cột của bảng dữ
/// liệu, theo thứ tự cột của câu.
#[derive(Debug, Clone)]
pub(super) struct Shape {
    pub grain: Grain,
    pub columns: Vec<(String, Option<Origin>)>,
}

impl Shape {
    pub fn derived(columns: Vec<(String, Option<Origin>)>) -> Shape {
        Shape {
            grain: Grain::Derived,
            columns,
        }
    }

    pub fn of_table(table: &AllowedTable) -> Shape {
        let name = table.name.to_lowercase();
        let columns = table
            .columns
            .iter()
            .map(|c| {
                let column = c.to_lowercase();
                let origin = Origin {
                    table: name.clone(),
                    column: column.clone(),
                };
                (column, Some(origin))
            })
            .collect();
        Shape {
            grain: Grain::Rows(name),
            columns,
        }
    }

    /// `AS t(a, b)`, `WITH t(a, b)`: đổi tên cột theo vị trí.
    pub fn apply_alias_columns(&mut self, alias: Option<&TableAlias>) {
        let Some(alias) = alias else { return };
        for (slot, column) in self.columns.iter_mut().zip(&alias.columns) {
            slot.0 = name_key(&column.name);
        }
    }

    fn find(&self, column: &str) -> Option<&Option<Origin>> {
        self.columns
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, origin)| origin)
    }
}

/// Một nguồn trong FROM.
#[derive(Debug, Clone)]
pub(super) struct Source {
    /// Tên đứng trước dấu chấm: bí danh, hay tên bảng khi không đặt bí danh. Viết thường.
    pub key: String,
    pub shape: Shape,
}

impl Source {
    pub fn rows_table(&self) -> Option<&str> {
        match &self.shape.grain {
            Grain::Rows(table) => Some(table),
            Grain::Derived => None,
        }
    }
}

/// Chỗ một cột của câu trỏ tới: tầng câu (0 là câu ngoài cùng), nguồn thứ mấy của tầng đó, và cột gốc nếu biết.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Found {
    pub level: usize,
    pub index: usize,
    pub origin: Option<Origin>,
}

pub(super) enum Resolved {
    Found(Found),
    /// Cột không ghi bảng mà có ở hai nguồn của cùng tầng.
    Ambiguous(String),
    /// Không phải cột, hay không tìm thấy (tên câu tự đặt, cột của tầng không biết cột).
    Missing,
}

/// Tìm nguồn của một cột, từ tầng trong cùng ra ngoài như SQL. levels[i] là các nguồn của tầng i.
pub(super) fn resolve_column(expr: &Expr, levels: &[&[Source]]) -> Resolved {
    match expr {
        Expr::Nested(inner) => resolve_column(inner, levels),
        Expr::Identifier(column) => resolve_plain(&name_key(column), levels),
        Expr::CompoundIdentifier(parts) => match parts.as_slice() {
            [qualifier, column] => resolve_qualified(qualifier, column, levels),
            _ => Resolved::Missing,
        },
        _ => Resolved::Missing,
    }
}

fn resolve_plain(column: &str, levels: &[&[Source]]) -> Resolved {
    for (level, sources) in levels.iter().enumerate().rev() {
        let owners: Vec<usize> = (0..sources.len())
            .filter(|&i| sources[i].shape.find(column).is_some())
            .collect();
        match owners.as_slice() {
            [] => continue,
            [index] => {
                let origin = sources[*index].shape.find(column).cloned().flatten();
                return Resolved::Found(Found {
                    level,
                    index: *index,
                    origin,
                });
            }
            _ => return Resolved::Ambiguous(column.to_string()),
        }
    }
    Resolved::Missing
}

fn resolve_qualified(qualifier: &Ident, column: &Ident, levels: &[&[Source]]) -> Resolved {
    let key = name_key(qualifier);
    for (level, sources) in levels.iter().enumerate().rev() {
        if let Some(index) = sources.iter().position(|s| s.key == key) {
            let origin = sources[index]
                .shape
                .find(&name_key(column))
                .cloned()
                .flatten();
            return Resolved::Found(Found {
                level,
                index,
                origin,
            });
        }
    }
    Resolved::Missing
}
