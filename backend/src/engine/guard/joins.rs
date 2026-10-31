// SPDX-License-Identifier: Apache-2.0
/* engine/guard/joins.rs - luật nối bảng của guard (#256): mọi phép nối giữa hai bảng dữ liệu phải đi đúng khóa ngoại
đã khai trong database (dataset_relations, service truyền vào dạng JoinLink). build_guarded_sql (mod.rs) gọi
check_joins sau các luật kích thước, trước bước lọc dòng: hộp lọc do guard thêm có nửa nối theo khóa ngoại, nhưng nó
thêm sau bước này nên không bị kiểm lại. Nguồn và cột gốc ở guard/join_sources.rs, gom điều kiện ở
guard/join_conditions.rs, so khóa ở guard/join_keys.rs. Test ở guard/join_tests.rs.

Vì sao: điều kiện ON trước đây chỉ được dặn qua prompt (prompts/sql_joins.txt); `JOIN orders o ON
i.order_item_id = o.customer_id` vẫn qua guard, DuckDB chạy và ra số sai mà trông hợp lý.

Luật, xét từng SELECT ở mọi tầng (câu ngoài, WITH, câu con ở FROM, WHERE, danh sách cột, IN, EXISTS):
- Hai nguồn giữ nguyên dòng của bảng dữ liệu (chính bảng, hay câu con, bảng tạm chỉ lọc và chọn cột của một bảng)
  nối với nhau: mọi phép bằng giữa cột của hai nguồn, ở ON, USING hay WHERE, gộp lại phải đúng bằng cột của một khóa
  ngoại giữa hai bảng (khóa nhiều cột thì đủ cả bộ, thứ tự và chiều tùy ý). Bảng nối với chính nó chỉ qua khóa tự trỏ.
- Vế ON đụng hai bảng như vậy chỉ được là phép bằng giữa hai cột; vế lọc một bảng thì được. OR, so sánh lớn nhỏ hay
  biểu thức tính ra giữa hai bảng bị từ chối. Ở WHERE, vế khác phép bằng là điều kiện lọc.
- Mọi bảng giữ nguyên dòng trong FROM phải nối với nhau qua phép bằng: CROSS JOIN, ON true, FROM a, b không điều kiện
  bị từ chối. NATURAL JOIN bị từ chối vì nối theo mọi cột trùng tên.
- Nối có nguồn đã gộp nhóm (bảng tạm theo khách, theo tháng) hay nửa nối (IN, EXISTS, câu con tương quan): mỗi phép
  bằng trên cột biết gốc phải so hai cột cùng miền giá trị (cùng cột, một cặp của khóa, hay hai khóa ngoại trỏ cùng
  cột của bảng cha), và phần khóa ngoại phải đủ cột. Lỏng hơn vì nguồn gộp nhóm không nhân dòng như bảng thật, và
  `WHERE region IN (SELECT region FROM sales ...)` (lọc theo giá trị của chính cột đó) là câu đúng.
- Không có quan hệ (dataset nạp từ file) thì câu một bảng như trước; chỉ câu nối bảng với chính nó bị từ chối.

Lý do từ chối nêu hai bảng và đúng các khóa nối được, hay nói hai bảng không có quan hệ, để vòng sửa (ask/run.rs) viết
lại được. Hạn chế: không lần theo cột qua UNION, và câu con tương quan không có phép bằng nào thì không kiểm. */

use sqlparser::ast::{Expr, JoinConstraint, JoinOperator, Query, SetExpr, TableFactor};

use super::join_conditions::{check_loose, split_and, Conditions, Equality};
use super::join_keys::{Keys, Origin};
use super::join_nested::{find_nested, Nested};
use super::join_sources::{build_select_shape, resolve_column, Found, Resolved, Shape, Source};
use super::{build_parent_tables, name_key, single_ident, AllowedTable, GuardError};

pub use super::join_keys::JoinLink;

pub(super) fn check_joins(
    query: &Query,
    tables: &[AllowedTable],
    links: &[JoinLink],
) -> Result<(), GuardError> {
    let parents = build_parent_tables(tables);
    let keys = Keys::build(links, tables);
    let mut checker = JoinChecker {
        tables: tables.iter().chain(&parents).collect(),
        keys: &keys,
        ctes: Vec::new(),
    };
    checker
        .check_query(query, &[])
        .map(|_| ())
        .map_err(GuardError::Rejected)
}

struct JoinChecker<'a> {
    /// Bảng được phép cộng bảng cha của điều kiện lan: hộp lọc trong câu truy dòng nguồn đọc bảng cha.
    tables: Vec<&'a AllowedTable>,
    keys: &'a Keys,
    /// Bảng tạm đang thấy được, trong cùng ở cuối.
    ctes: Vec<(String, Shape)>,
}

/// Các nguồn của FROM, các điều kiện ON và phép bằng của USING.
struct FromParts<'q> {
    sources: Vec<Source>,
    on: Vec<&'q Expr>,
    using: Vec<Equality>,
    level: usize,
}

impl JoinChecker<'_> {
    /// Kiểm một câu và trả hình dạng kết quả của nó. outer là nguồn của các tầng ngoài, cho câu con tương quan.
    fn check_query(&mut self, query: &Query, outer: &[&[Source]]) -> Result<Shape, String> {
        let saved = self.ctes.len();
        let result = self.check_with_body(query, outer);
        // Bảng tạm chỉ thấy được trong câu định nghĩa nó.
        self.ctes.truncate(saved);
        result
    }

    fn check_with_body(&mut self, query: &Query, outer: &[&[Source]]) -> Result<Shape, String> {
        for cte in query.with.iter().flat_map(|with| &with.cte_tables) {
            let mut shape = self.check_query(&cte.query, outer)?;
            shape.apply_alias_columns(Some(&cte.alias));
            self.ctes.push((name_key(&cte.alias.name), shape));
        }
        self.check_set(&query.body, outer)
    }

    fn check_set(&mut self, body: &SetExpr, outer: &[&[Source]]) -> Result<Shape, String> {
        match body {
            SetExpr::Select(select) => self.check_select(select, outer),
            SetExpr::Query(query) => self.check_query(query, outer),
            SetExpr::SetOperation { left, right, .. } => {
                let shape = self.check_set(left, outer)?;
                self.check_set(right, outer)?;
                // Không lần theo cột qua UNION: hai vế có thể đến từ hai bảng khác nhau.
                let columns = shape.columns.into_iter().map(|(n, _)| (n, None)).collect();
                Ok(Shape::derived(columns))
            }
            // Bước 1 đã từ chối mọi dạng khác.
            _ => Ok(Shape::derived(Vec::new())),
        }
    }

    fn check_select(
        &mut self,
        select: &sqlparser::ast::Select,
        outer: &[&[Source]],
    ) -> Result<Shape, String> {
        let mut from = FromParts {
            sources: Vec::new(),
            on: Vec::new(),
            using: Vec::new(),
            level: outer.len(),
        };
        for table in &select.from {
            self.add_factor(&table.relation, outer, &mut from)?;
            for join in &table.joins {
                let left_count = from.sources.len();
                self.add_factor(&join.relation, outer, &mut from)?;
                add_constraint(&join.join_operator, left_count, &mut from)?;
            }
        }
        let mut levels = outer.to_vec();
        levels.push(&from.sources);
        let mut conditions = Conditions {
            equalities: from.using.clone(),
            edges: Vec::new(),
        };
        for on in &from.on {
            for conjunct in split_and(on) {
                conditions.read_on(conjunct, &levels)?;
            }
        }
        for conjunct in select.selection.iter().flat_map(split_and) {
            conditions.read_where(conjunct, &levels);
        }
        conditions.check(&levels, self.keys)?;
        for nested in find_nested(select, &from.on) {
            self.check_nested(nested, &levels)?;
        }
        Ok(build_select_shape(select, &levels))
    }

    fn add_factor<'q>(
        &mut self,
        factor: &'q TableFactor,
        outer: &[&[Source]],
        from: &mut FromParts<'q>,
    ) -> Result<(), String> {
        match factor {
            TableFactor::Table { name, alias, .. } => {
                let Some(ident) = single_ident(name) else {
                    return Ok(());
                };
                let mut shape = self.shape_of_name(&name_key(ident));
                shape.apply_alias_columns(alias.as_ref());
                let key = alias
                    .as_ref()
                    .map_or(name_key(ident), |a| name_key(&a.name));
                from.sources.push(Source { key, shape });
            }
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
                ..
            } => {
                let mut shape = if *lateral {
                    // LATERAL đọc được các nguồn đứng trước trong cùng FROM. Coi như đã gộp: nó chạy một lần cho mỗi
                    // dòng bên trái, không phải nối hai bảng theo cột.
                    let mut levels = outer.to_vec();
                    levels.push(&from.sources);
                    Shape::derived(self.check_query(subquery, &levels)?.columns)
                } else {
                    self.check_query(subquery, outer)?
                };
                shape.apply_alias_columns(alias.as_ref());
                let key = alias.as_ref().map_or(String::new(), |a| name_key(&a.name));
                from.sources.push(Source { key, shape });
            }
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => {
                self.add_factor(&table_with_joins.relation, outer, from)?;
                for join in &table_with_joins.joins {
                    let left_count = from.sources.len();
                    self.add_factor(&join.relation, outer, from)?;
                    add_constraint(&join.join_operator, left_count, from)?;
                }
            }
            // Bước 2 đã từ chối hàm ở chỗ bảng, UNNEST, PIVOT.
            _ => {}
        }
        Ok(())
    }

    fn shape_of_name(&self, key: &str) -> Shape {
        if let Some((_, shape)) = self.ctes.iter().rev().find(|(name, _)| name == key) {
            return shape.clone();
        }
        match self.tables.iter().find(|t| t.name.to_lowercase() == key) {
            Some(table) => Shape::of_table(table),
            None => Shape::derived(Vec::new()),
        }
    }

    /// Câu con trong biểu thức. `x IN (SELECT y ...)` là nửa nối giữa cột x và cột y nên kiểm như nối lỏng.
    fn check_nested(&mut self, nested: Nested, levels: &[&[Source]]) -> Result<(), String> {
        match nested {
            Nested::Query(query) => self.check_query(&query, levels).map(|_| ()),
            Nested::In { left, query } => {
                let shape = self.check_query(&query, levels)?;
                let lefts = match &left {
                    Expr::Tuple(items) => items.iter().collect(),
                    other => vec![other],
                };
                let text = format!("{left} IN (SELECT ...)");
                let mut pairs = Vec::new();
                for (expr, (_, inner)) in lefts.into_iter().zip(&shape.columns) {
                    if let (Some(outer), Some(inner)) = (origin_of(expr, levels), inner) {
                        pairs.push((outer, inner.clone(), text.clone()));
                    }
                }
                check_loose(&pairs, self.keys)
            }
        }
    }
}

fn origin_of(expr: &Expr, levels: &[&[Source]]) -> Option<Origin> {
    match resolve_column(expr, levels) {
        Resolved::Found(found) => found.origin,
        _ => None,
    }
}

/// Điều kiện của một JOIN: ON giữ lại để đọc sau, USING đổi thành phép bằng, NATURAL bị từ chối.
fn add_constraint<'q>(
    operator: &'q JoinOperator,
    left_count: usize,
    from: &mut FromParts<'q>,
) -> Result<(), String> {
    let constraint = match operator {
        JoinOperator::Join(c)
        | JoinOperator::Inner(c)
        | JoinOperator::Left(c)
        | JoinOperator::LeftOuter(c)
        | JoinOperator::Right(c)
        | JoinOperator::RightOuter(c)
        | JoinOperator::FullOuter(c)
        | JoinOperator::CrossJoin(c)
        | JoinOperator::Semi(c)
        | JoinOperator::LeftSemi(c)
        | JoinOperator::RightSemi(c)
        | JoinOperator::Anti(c)
        | JoinOperator::LeftAnti(c)
        | JoinOperator::RightAnti(c)
        | JoinOperator::StraightJoin(c) => c,
        other => {
            return Err(format!(
                "Không hỗ trợ kiểu nối {other:?}: chỉ dùng JOIN, LEFT JOIN, RIGHT JOIN, FULL JOIN kèm ON theo khóa ngoại"
            ))
        }
    };
    match constraint {
        JoinConstraint::On(expr) => {
            // Mượn từ cây của câu, sống lâu hơn FromParts.
            from.on.push(expr);
            Ok(())
        }
        JoinConstraint::Using(columns) => {
            for column in columns {
                let name = column.0.last().and_then(|p| p.as_ident()).map(name_key);
                let equality = build_using(name.unwrap_or_default(), left_count, from)?;
                from.using.push(equality);
            }
            Ok(())
        }
        JoinConstraint::Natural => Err(
            "Không cho NATURAL JOIN: nó nối theo mọi cột trùng tên. Viết JOIN ... ON với đúng cột của khóa ngoại"
                .to_string(),
        ),
        JoinConstraint::None => Ok(()),
    }
}

/// `USING (c)`: phép bằng giữa cột c của nguồn bên trái và bên phải, khi mỗi bên có đúng một nguồn có cột đó.
fn build_using(
    column: String,
    left_count: usize,
    from: &FromParts<'_>,
) -> Result<Equality, String> {
    let has = |s: &Source| s.shape.columns.iter().any(|(name, _)| *name == column);
    let left: Vec<usize> = (0..left_count).filter(|&i| has(&from.sources[i])).collect();
    let right: Vec<usize> = (left_count..from.sources.len())
        .filter(|&i| has(&from.sources[i]))
        .collect();
    let ([l], [r]) = (left.as_slice(), right.as_slice()) else {
        return Err(format!(
            "Không biết cột {column} trong USING thuộc bảng nào: viết JOIN ... ON với tên bảng trước từng cột"
        ));
    };
    let found = |index: usize| Found {
        level: from.level,
        index,
        origin: from.sources[index]
            .shape
            .columns
            .iter()
            .find(|(name, _)| *name == column)
            .and_then(|(_, origin)| origin.clone()),
    };
    Ok(Equality {
        a: found(*l),
        b: found(*r),
        text: format!("USING ({column})"),
    })
}
