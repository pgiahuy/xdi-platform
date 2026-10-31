// SPDX-License-Identifier: Apache-2.0
/* engine/guard/join_from.rs - các nguồn và điều kiện JOIN của một FROM cho luật nối bảng (guard/joins.rs): ON giữ lại
để guard/join_conditions.rs đọc, USING đổi thành phép bằng, NATURAL và các kiểu nối lạ bị từ chối. */

use sqlparser::ast::{Expr, JoinConstraint, JoinOperator};

use super::join_conditions::Equality;
use super::join_sources::{Found, Source};
use super::name_key;

/// Các nguồn của FROM, các điều kiện ON và phép bằng của USING.
pub(super) struct FromParts<'q> {
    pub sources: Vec<Source>,
    pub on: Vec<&'q Expr>,
    pub using: Vec<Equality>,
    pub level: usize,
}

/// Điều kiện của một JOIN: ON giữ lại để đọc sau, USING đổi thành phép bằng, NATURAL bị từ chối.
pub(super) fn add_constraint<'q>(
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
