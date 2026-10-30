// SPDX-License-Identifier: Apache-2.0
/* engine/guard/width.rs - giới hạn những thứ mà luật số tầng (depth.rs) và số mảnh ghép (pieces.rs) không đếm: độ dài
mỗi chuỗi hằng viết trong câu, số cột của mỗi SELECT (kể cả `*` trên nhiều nguồn), số nguồn trong mỗi FROM, và tổng số
nhánh UNION, INTERSECT, EXCEPT trong cả câu. build_guarded_sql (mod.rs) gọi check_query_width cùng các luật kia.

Vì sao: chép một giá trị lớn ra nhiều cột không cần ghép (`SELECT v, v, v, ...`), mà mỗi bản chép là một chuỗi riêng
DuckDB dựng lúc chuẩn bị câu. Đo qua guard trước khi có luật này: chuỗi hằng 16 KB chép ra 8000 cột cho RSS đỉnh 690 MB
dù không ghép gì; thêm ghép qua các tầng thì nhân lên (600 cột, 49 lần, 2,5 GB). Chuỗi hằng tối đa 1000 byte, 200 cột
mỗi SELECT và 4 nguồn mỗi FROM (trần lúc đo; nay 6, xem MAX_FROM_SOURCES) giữ các ca cho một dòng kết quả đã đo dưới 270
MB (bảng số đo ở docs/GUARD.md, mục Số tầng, mảnh ghép, độ rộng). Chuỗi dài lấy từ dữ liệu thì không bị luật này giới
hạn: đó là phần tính lúc chạy, trong trần của engine/query.rs. Mỗi nhánh UNION ALL cho một dòng chép lại cùng giá trị,
nên nhân như chép cột: 74 nhánh × 200 cột đo được 1,94 GB; tối đa 10 nhánh. Phần nhân theo số dòng của bảng thì để tiến
trình con (#119). */

use std::ops::ControlFlow;

use sqlparser::ast::{
    Query, Select, SelectItem, SelectItemQualifiedWildcardKind, SetExpr, TableFactor,
    TableWithJoins, ValueWithSpan, Visit, Visitor,
};

use super::{find_table, name_key, single_ident, AllowedTable};

/// Độ dài tối đa của một chuỗi hằng, tính bằng byte: đủ cho tên, mã, định dạng ngày, mẫu tìm kiếm.
pub const MAX_LITERAL_BYTES: usize = 1000;

/// Số mục tối đa trong danh sách cột của một SELECT. `*` và `t.*` tính là một mục khi FROM có một nguồn, tính theo số
/// cột thật khi có nhiều nguồn (xem pre_visit_select).
pub const MAX_SELECT_ITEMS: usize = 200;

/// Số nguồn tối đa trong một FROM, tính cả JOIN, bảng tạm và câu con. 6 từ #213: câu hỏi đọc được bảng đang chọn cùng
/// các bảng liên quan theo khóa ngoại, và chuỗi order_items, orders, customers, products cộng một hai bảng tạm đã cần
/// 5 tới 6 nguồn. `*` qua nhiều lần JOIN vẫn tính theo số cột thật (MAX_SELECT_ITEMS), nên số cột không tăng theo trần
/// này; số đo bộ nhớ ở đầu file đo khi trần là 4.
pub const MAX_FROM_SOURCES: usize = 6;

/// Tổng số nhánh tối đa của mọi phép tập hợp (UNION, INTERSECT, EXCEPT) trong cả câu, ở mọi tầng: đủ cho dòng tổng,
/// vài nhóm top và bottom ghép lại.
pub const MAX_SET_BRANCHES: usize = 10;

/// Err là lý do từ chối (tiếng Việt).
pub(super) fn check_query_width(query: &Query, tables: &[AllowedTable]) -> Result<(), String> {
    let mut checker = WidthChecker {
        tables,
        branches: 0,
    };
    match query.visit(&mut checker) {
        ControlFlow::Break(reason) => Err(reason),
        ControlFlow::Continue(()) => Ok(()),
    }
}

struct WidthChecker<'a> {
    tables: &'a [AllowedTable],
    /// Số nhánh phép tập hợp đã gặp, cộng qua mọi câu con.
    branches: usize,
}

impl WidthChecker<'_> {
    /// Số cột của một nguồn khi biết chắc: bảng dữ liệu thì lấy từ hồ sơ. Bảng tạm, câu con thì không tính (None).
    fn source_width(&self, source: &TableFactor) -> Option<usize> {
        let TableFactor::Table {
            name, args: None, ..
        } = source
        else {
            return None;
        };
        let table =
            single_ident(name).and_then(|ident| find_table(self.tables, &name_key(ident)))?;
        Some(table.columns.len())
    }

    /// Số cột `*` hay `t.*` lấy ra khi FROM có nhiều nguồn. None khi một nguồn liên quan không biết số cột.
    fn star_width(&self, item: &SelectItem, sources: &[&TableFactor]) -> Option<usize> {
        match item {
            SelectItem::Wildcard(_) => sources.iter().map(|s| self.source_width(s)).sum(),
            SelectItem::QualifiedWildcard(SelectItemQualifiedWildcardKind::ObjectName(name), _) => {
                let qualifier = single_ident(name).map(name_key)?;
                let source = sources
                    .iter()
                    .find(|s| source_name(s).as_deref() == Some(&qualifier))?;
                self.source_width(source)
            }
            _ => None,
        }
    }
}

impl Visitor for WidthChecker<'_> {
    type Break = String;

    // Mỗi câu có thân là phép tập hợp cộng số nhánh của thân đó. Nhánh trong ngoặc mà tự là phép tập hợp được cộng thêm
    // lần nữa khi tới câu của nó: đếm dư một, không đếm thiếu.
    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<String> {
        if matches!(*query.body, SetExpr::SetOperation { .. }) {
            self.branches = self.branches.saturating_add(count_branches(&query.body));
        }
        if self.branches > MAX_SET_BRANCHES {
            return ControlFlow::Break(format!(
                "Câu SQL có {} nhánh UNION, INTERSECT, EXCEPT, tối đa {MAX_SET_BRANCHES}: gộp bớt nhánh, ví dụ dùng \
                 GROUP BY hay CASE thay cho nhiều câu ghép lại",
                self.branches
            ));
        }
        ControlFlow::Continue(())
    }

    // Mọi chuỗi hằng ở mọi chỗ, kể cả `DATE '...'` và chuỗi trong IN (...).
    fn pre_visit_value(&mut self, value: &ValueWithSpan) -> ControlFlow<String> {
        let bytes = value.clone().into_string().map_or(0, |text| text.len());
        if bytes > MAX_LITERAL_BYTES {
            return ControlFlow::Break(format!(
                "Câu SQL có chuỗi hằng dài {bytes} byte, tối đa {MAX_LITERAL_BYTES}: viết lại không cần chuỗi dài như vậy"
            ));
        }
        ControlFlow::Continue(())
    }

    // Một nguồn: `*` chép đúng các cột của nguồn đó, mà nguồn là bảng dữ liệu (số cột của dữ liệu thật) hay một câu đã
    // qua luật này (tối đa 200 cột), nên tính là một mục. Nhiều nguồn: `*` nối cột của mọi nguồn, và
    // `FROM w a1, w a2, ...` nhân số cột theo số lần JOIN; nên tính theo số cột thật, và từ chối khi có bảng tạm, câu
    // con không biết số cột.
    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<String> {
        let mut sources = Vec::new();
        select
            .from
            .iter()
            .for_each(|from| collect_sources(from, &mut sources));
        if sources.len() > MAX_FROM_SOURCES {
            return ControlFlow::Break(format!(
                "Câu SELECT đọc {} nguồn trong một FROM (bảng, bảng tạm, câu con, kể cả JOIN), tối đa \
                 {MAX_FROM_SOURCES}: gộp bớt hoặc tách ra bảng tạm",
                sources.len()
            ));
        }
        let mut items: usize = 0;
        for item in &select.projection {
            let is_star = matches!(
                item,
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..)
            );
            let width = if is_star && sources.len() > 1 {
                match self.star_width(item, &sources) {
                    Some(width) => width,
                    None => {
                        return ControlFlow::Break(
                            "Câu SELECT dùng * trên nhiều nguồn, trong đó có bảng tạm hay câu con: liệt kê các cột cần \
                             lấy thay cho *"
                                .to_string(),
                        )
                    }
                }
            } else {
                1
            };
            items = items.saturating_add(width);
        }
        if items > MAX_SELECT_ITEMS {
            return ControlFlow::Break(format!(
                "Câu SELECT có {items} cột, tối đa {MAX_SELECT_ITEMS}: chọn ít cột hơn"
            ));
        }
        ControlFlow::Continue(())
    }
}

/// Số nhánh của một thân câu: `a UNION b UNION c` là 3.
fn count_branches(body: &SetExpr) -> usize {
    match body {
        SetExpr::SetOperation { left, right, .. } => {
            count_branches(left).saturating_add(count_branches(right))
        }
        _ => 1,
    }
}

/// Mọi nguồn của một mục FROM, kể cả bên trong JOIN trong ngoặc.
fn collect_sources<'a>(from: &'a TableWithJoins, sources: &mut Vec<&'a TableFactor>) {
    let relations =
        std::iter::once(&from.relation).chain(from.joins.iter().map(|join| &join.relation));
    for relation in relations {
        match relation {
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => collect_sources(table_with_joins, sources),
            other => sources.push(other),
        }
    }
}

/// Tên gọi nguồn trong `t.*`: bí danh nếu có, không thì tên bảng.
fn source_name(source: &TableFactor) -> Option<String> {
    match source {
        TableFactor::Table {
            alias: Some(alias), ..
        }
        | TableFactor::Derived {
            alias: Some(alias), ..
        } => Some(name_key(&alias.name)),
        TableFactor::Table { name, .. } => single_ident(name).map(name_key),
        _ => None,
    }
}
