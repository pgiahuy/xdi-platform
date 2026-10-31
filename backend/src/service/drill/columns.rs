// SPDX-License-Identifier: Apache-2.0
/* service/drill/columns.rs - danh sách cột của trang dòng nguồn (drill/sql.rs gọi). Câu đọc một chỗ: `*`, như trước
khi truy được câu nối bảng, nên dòng nguồn của câu một bảng vẫn có mọi cột của dataset kể cả `_source_file`. Câu nối
bảng: `<bảng sự kiện>.*` (bảng drill/sources.rs chọn), rồi cột thuộc bảng nối khác mà câu dùng làm khóa nhóm hay điều
kiện lọc, để người dùng thấy vì sao dòng thuộc ô (segment của khách, không phải cột của order_items). Sau cùng, mỗi độ
đo SUM trên một biểu thức (`quantity * unit_price`, CASE) một cột giá trị từng dòng (drill/measures.rs); SUM trên một
cột đã có trên trang thì không chép thêm, độ đo trỏ tới cột đó.

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
    pub row: RowColumn,
}

/// Cột giá trị từng dòng của một độ đo trên trang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowColumn {
    /// Độ đo không cộng theo dòng (COUNT, AVG...).
    Absent,
    /// Thêm ở cuối trang, tên theo cột kết quả: biểu thức như `quantity * unit_price` hay CASE.
    Added,
    /// `SUM(cột)` trên một cột trang đã có (cột của bảng sự kiện): không chép thêm, trỏ tới cột đó theo tên.
    Existing(String),
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
    // None khi câu đọc một chỗ: mọi cột của chỗ đó đã có trên trang qua `*`.
    let mut fact = None;
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
        let chosen = choose_fact(sources, &aggregates, tables, links)?;
        fact = Some(chosen);
        // choose_fact chỉ chọn chỗ đọc có tên gọi.
        let Some(qualifier) = &sources[chosen].qualifier else {
            return Err(AppError::Internal(
                "drill: fact source has no name".to_string(),
            ));
        };
        info.source_table = sources[chosen].table.clone();
        let mut items = build_projection(&format!("SELECT {qualifier}.*"))?;
        let joined = build_joined_items(shape, select, sources, chosen, names, tables);
        info.joined_added = joined.len();
        items.extend(joined);
        items
    };
    for measure in measures {
        let column = names[measure.output].clone();
        let row = match &measure.row_expr {
            None => RowColumn::Absent,
            Some(expr) => match find_page_column(expr, sources, fact, tables) {
                Some(existing) => RowColumn::Existing(existing),
                None => {
                    items.push(aliased(expr.clone(), &column));
                    RowColumn::Added
                }
            },
        };
        info.measures.push(MeasureColumn {
            column,
            rule: measure.rule,
            row,
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

/// Tên cột khi biểu thức là đúng một cột trang đã có qua `*` hay `<bảng sự kiện>.*`.
fn find_page_column(
    expr: &Expr,
    sources: &[Source],
    fact: Option<usize>,
    tables: &[AllowedTable],
) -> Option<String> {
    if !matches!(expr, Expr::Identifier(_) | Expr::CompoundIdentifier(_)) {
        return None;
    }
    let [column] = build_column_refs(expr).try_into().ok()?;
    let on_page = match fact {
        None => true,
        Some(fact) => find_owner(sources, tables, &column) == Some(fact),
    };
    on_page.then(|| column.column.value.clone())
}

// Nháy kép để tên có dấu cách hay chữ hoa vẫn đúng như cột kết quả; sqlparser tự thoát dấu nháy khi in.
fn aliased(expr: Expr, name: &str) -> SelectItem {
    SelectItem::ExprWithAlias {
        expr,
        alias: Ident::with_quote('"', name),
    }
}
