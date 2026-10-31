// SPDX-License-Identifier: Apache-2.0
/* service/drill/columns.rs - danh sách cột của trang dòng nguồn (drill/sql.rs gọi). Câu đọc một chỗ: `*`, như trước
khi truy được câu nối bảng, nên dòng nguồn của câu một bảng vẫn có mọi cột của dataset kể cả `_source_file`. Câu nối
bảng: `<bảng sự kiện>.*` (bảng drill/sources.rs chọn), rồi cột thuộc bảng nối khác mà câu dùng làm khóa nhóm hay điều
kiện lọc, để người dùng thấy vì sao dòng thuộc ô (segment của khách, không phải cột của order_items). Sau cùng, mỗi độ
đo SUM một cột giá trị từng dòng (drill/measures.rs).

Không lấy mọi cột của mọi bảng nối: trang hẹp và cột của bảng một (orders, customers) lặp lại trên mỗi dòng sự kiện;
cột cần để kiểm con số là cột khóa, cột lọc và cột của độ đo. */

use sqlparser::ast::{Expr, Ident, Select, SelectItem};

use crate::engine::guard::{AllowedTable, JoinLink};
use crate::engine::sql_shape::QueryShape;
use crate::error::AppError;
use crate::service::drill::contract::MeasureRule;
use crate::service::drill::measures::PlanMeasure;
use crate::service::drill::sources::{build_column_refs, choose_fact, find_owner, Source};
use crate::service::drill::sql::build_projection;

/// Cột của trang và cách đọc chúng sau khi chạy. Thứ tự trên trang: cột của nguồn (`*` hay `<bảng>.*`), `joined_added`
/// cột của bảng nối, rồi cột từng dòng của các độ đo có `row_column`.
#[derive(Debug, Clone, Default)]
pub struct PageColumns {
    pub source_table: Option<String>,
    /// Số cột của bảng nối thêm sau cột của nguồn.
    pub joined_added: usize,
    /// Tên cột trong `*` đến từ bảng nối (cột câu con của phân rã #227 thêm cạnh bảng chính).
    pub joined_named: Vec<String>,
    pub measures: Vec<MeasureColumn>,
}

#[derive(Debug, Clone)]
pub struct MeasureColumn {
    /// Tên cột kết quả của câu đã chạy, duy nhất như ResultTable.columns.
    pub column: String,
    pub rule: MeasureRule,
    /// Trang có cột giá trị từng dòng của độ đo này.
    pub has_row_column: bool,
}

pub struct BuiltColumns {
    pub items: Vec<SelectItem>,
    pub info: PageColumns,
}

/// names: tên cột kết quả duy nhất của câu đã chạy. catalog: hồ sơ bảng và quan hệ của nhóm.
pub fn build_page_columns(
    shape: &QueryShape,
    select: &Select,
    sources: &[Source],
    names: &[String],
    measures: &[&PlanMeasure],
    catalog: (&[AllowedTable], &[JoinLink]),
) -> Result<BuiltColumns, AppError> {
    let (tables, links) = catalog;
    let mut info = PageColumns::default();
    let mut items = if let [only] = sources {
        info.source_table = only.table.clone().or_else(|| only.base_table.clone());
        info.joined_named = only.added_columns.clone();
        build_projection("SELECT *")?
    } else {
        let aggregates: Vec<&Expr> = shape
            .outputs
            .iter()
            .filter(|o| o.is_aggregate)
            .filter_map(|o| o.tree.as_ref())
            .collect();
        let fact = choose_fact(sources, &aggregates, tables, links)?;
        // choose_fact chỉ chọn chỗ đọc có tên gọi.
        let Some(qualifier) = &sources[fact].qualifier else {
            return Err(AppError::Internal(
                "drill: fact source has no name".to_string(),
            ));
        };
        info.source_table = sources[fact].table.clone();
        let mut items = build_projection(&format!("SELECT {qualifier}.*"))?;
        let joined = build_joined_items(shape, select, sources, fact, names, tables);
        info.joined_added = joined.len();
        items.extend(joined);
        items
    };
    for measure in measures {
        let column = names[measure.output].clone();
        if let Some(row) = &measure.row_expr {
            items.push(aliased(row.clone(), &column));
        }
        info.measures.push(MeasureColumn {
            column,
            rule: measure.rule,
            has_row_column: measure.row_expr.is_some(),
        });
    }
    Ok(BuiltColumns { items, info })
}

/// Cột khóa nhóm không thuộc hẳn bảng sự kiện (tên theo cột kết quả), rồi cột của bảng nối khác mà WHERE dùng (tên
/// "bảng.cột").
fn build_joined_items(
    shape: &QueryShape,
    select: &Select,
    sources: &[Source],
    fact: usize,
    names: &[String],
    tables: &[AllowedTable],
) -> Vec<SelectItem> {
    let mut items = Vec::new();
    let mut shown: Vec<String> = Vec::new();
    for key in &shape.group_by {
        let Some(output) = key.output else { continue };
        let Some(tree) = &shape.outputs[output].tree else {
            continue;
        };
        let refs = build_column_refs(tree);
        let on_fact = refs
            .iter()
            .all(|r| find_owner(sources, tables, r) == Some(fact));
        if refs.is_empty() || on_fact {
            continue;
        }
        shown.push(tree.to_string());
        items.push(aliased(tree.clone(), &names[output]));
    }
    let Some(selection) = &select.selection else {
        return items;
    };
    let mut named: Vec<String> = Vec::new();
    for column in build_column_refs(selection) {
        let Some(owner) = find_owner(sources, tables, &column).filter(|o| *o != fact) else {
            continue;
        };
        let Some(qualifier) = &sources[owner].qualifier else {
            continue;
        };
        let expr = Expr::CompoundIdentifier(vec![qualifier.clone(), column.column.clone()]);
        let table = sources[owner]
            .table
            .clone()
            .unwrap_or_else(|| qualifier.value.clone());
        let name = format!("{table}.{}", column.column.value);
        if shown.contains(&expr.to_string()) || named.contains(&name) {
            continue;
        }
        named.push(name.clone());
        items.push(aliased(expr, &name));
    }
    items
}

// Nháy kép để tên có dấu cách hay chữ hoa vẫn đúng như cột kết quả; sqlparser tự thoát dấu nháy khi in.
fn aliased(expr: Expr, name: &str) -> SelectItem {
    SelectItem::ExprWithAlias {
        expr,
        alias: Ident::with_quote('"', name),
    }
}
