// SPDX-License-Identifier: Apache-2.0
/* service/explain/source.rs - nguồn dòng của phân rã trên dataset chụp từ database (#227): bảng chính (bảng của độ đo)
nối nhiều-một tới các bảng liên quan ghi trong dataset_relations, để phân rã xét cả cột của bảng liên quan, ví dụ
student_assessments tách theo students.region. Hàm thuần: explain/sql.rs dựng phần FROM từ PlanSource, ask/why.rs và
service/insight/ dựng PlanSource từ nhóm bảng của câu hỏi (ask/related.rs) và quan hệ đã đổi sang tên bảng
(ask/fanout.rs, build_join_links). Test ở explain/source_tests.rs.

Chỉ đi từ bên nhiều lên bên một, và qua quan hệ một-một, như luật chặn cộng lặp của ask/fanout.rs: mỗi dòng của bảng
chính khớp nhiều nhất một dòng của bảng liên quan, nên các nhóm theo cột liên quan vẫn cộng ra đúng tổng hai lát cắt.
Đi ngược (từ bên một xuống bên nhiều) thì một dòng của bảng chính bị đếm nhiều lần, nên không bao giờ đi. Khóa của bên
một không duy nhất (dữ liệu lỗi) thì các nhóm không cộng ra tổng và bước chấm điểm bỏ chiều đó (explain/mod.rs,
TotalsMismatch).

SQL không ghép chữ của mô hình: tên bảng, tên cột và khóa nối lấy từ hồ sơ và quan hệ đã ghi lúc chụp, in qua
build_ident. Khi cần cột liên quan, phần FROM là một câu con
`(SELECT "b".*, "j1"."region" AS "students.region" FROM "student_assessments" AS "b" LEFT JOIN "students" AS "j1" ON ...) AS "joined"`:
câu ngoài vẫn gọi cột của bảng chính bằng tên trơn như khi đọc một bảng, nên điều kiện của chỉ số, của câu hỏi và cột
thời gian không phải viết lại; cột liên quan có tên "bảng.cột" không trùng cột nào của bảng chính. LEFT JOIN giữ dòng
của bảng chính không có dòng cha (khóa rỗng, hay dòng cha bị lọc theo quyền): dòng đó vào nhóm trống, tổng không mất
dòng nào. Guard lọc dòng của từng bảng được đọc (#215), nên bảng liên quan cũng chỉ có dòng người hỏi được thấy.

Câu không cần cột liên quan nào vẫn đọc thẳng bảng chính như trước, để truy dòng nguồn (service/drill/) vẫn chạy; câu
có câu con ở FROM thì truy dòng nguồn chưa hỗ trợ và báo rõ lý do. */

use crate::engine::store::{ProfileColumn, ProfileRole};
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::check::DIMENSION_MAX_VALUES;
use crate::service::explain::sql::build_ident;

/// Số bước tối đa đi từ bảng chính lên các bảng liên quan khi tìm chiều phân rã (order_items, orders, customers là hai
/// bước). Nhóm bảng của câu hỏi không còn giới hạn số bước (#249, ask/related.rs), nhưng mỗi bảng thêm vào là thêm các
/// chiều ứng viên và thêm câu chạy cho phân rã; hai bước giữ đúng phạm vi phân rã của #227.
pub const BREAKDOWN_MAX_HOPS: usize = 2;

/// Bí danh của bảng chính và của câu con trong phần FROM.
const BASE_ALIAS: &str = "b";
const JOINED_ALIAS: &str = "joined";

/// Một bảng nối vào bảng chính theo một quan hệ nhiều-một hay một-một.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanJoin {
    /// Tên bảng logic.
    pub table: String,
    /// Bí danh trong câu: j1, j2...
    pub alias: String,
    /// Bảng nối tới bảng này: None là bảng chính, Some(i) là joins[i]. Bảng cha luôn đứng trước trong danh sách.
    pub parent: Option<usize>,
    /// Cặp (cột của bảng cha, cột của bảng này) của quan hệ.
    pub keys: Vec<(String, String)>,
}

/// Một cột của bảng liên quan, đọc được qua phần FROM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedColumn {
    /// "students.region": tên trong câu SQL, trong bảng phân rã và trong lời tóm tắt.
    pub name: String,
    /// Chỉ số trong PlanSource::joins.
    pub join: usize,
    pub column: String,
}

/// Các bảng nối được và cột của chúng. Rỗng với dataset nạp từ file: mọi câu đọc thẳng bảng chính.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanSource {
    pub joins: Vec<PlanJoin>,
    pub columns: Vec<RelatedColumn>,
    /// Cột liên quan mọi câu đều cần: cột thời gian hay cột lọc của câu hỏi nằm ở bảng liên quan (service/insight/).
    pub always: Vec<String>,
}

/// Một bảng của nhóm và hồ sơ cột của nó.
pub struct SourceTable<'a> {
    pub table: &'a str,
    pub columns: &'a [ProfileColumn],
}

/// Một cột liên quan dùng được làm chiều phân rã, kèm tên tiếng Việt từ hồ sơ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedDimension {
    pub name: String,
    pub label: Option<String>,
}

/// Nguồn dòng và chiều ứng viên từ các bảng liên quan của bảng chính base.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelatedTables {
    pub source: PlanSource,
    pub dimensions: Vec<RelatedDimension>,
}

/// Không có bảng liên quan, cho test của đường câu "tại sao" trên một bảng.
#[cfg(test)]
pub static NO_RELATED: RelatedTables = RelatedTables {
    source: PlanSource {
        joins: Vec::new(),
        columns: Vec::new(),
        always: Vec::new(),
    },
    dimensions: Vec::new(),
};

/// Đi từ base theo quan hệ, tối đa BREAKDOWN_MAX_HOPS bước, mỗi bảng một lần, gần trước xa sau. Chỉ
/// bảng có trong tables (đã đọc được hồ sơ) mới được nối.
pub fn build_related_tables(
    base: &str,
    tables: &[SourceTable],
    links: &[JoinLink],
) -> RelatedTables {
    let mut source = PlanSource::default();
    let mut dimensions = Vec::new();
    // frontier: (tên bảng, chỉ số trong joins; None là bảng chính).
    let mut frontier: Vec<(String, Option<usize>)> = vec![(base.to_string(), None)];
    let mut reached = vec![base.to_lowercase()];
    for _ in 0..BREAKDOWN_MAX_HOPS {
        let mut next = Vec::new();
        for (current, parent) in &frontier {
            for (table, keys) in build_steps(current, links) {
                let Some(found) = tables.iter().find(|t| t.table.eq_ignore_ascii_case(&table))
                else {
                    continue;
                };
                if reached.contains(&table.to_lowercase()) {
                    continue;
                }
                reached.push(table.to_lowercase());
                let index = source.joins.len();
                source.joins.push(PlanJoin {
                    table: found.table.to_string(),
                    alias: format!("j{}", index + 1),
                    parent: *parent,
                    keys: keys.clone(),
                });
                let key_columns: Vec<&str> = keys.iter().map(|(_, own)| own.as_str()).collect();
                for column in found.columns {
                    let name = format!("{}.{}", found.table, column.name);
                    if is_dimension(column) && !key_columns.contains(&column.name.as_str()) {
                        dimensions.push(RelatedDimension {
                            name: name.clone(),
                            label: build_label(column),
                        });
                    }
                    source.columns.push(RelatedColumn {
                        name,
                        join: index,
                        column: column.name.clone(),
                    });
                }
                next.push((found.table.to_string(), Some(index)));
            }
        }
        frontier = next;
    }
    RelatedTables { source, dimensions }
}

/// Tên các bảng đi tới được từ base theo đúng luật của build_related_tables (nhiều-một, một-một, tối đa
/// BREAKDOWN_MAX_HOPS bước), không kể base. ask/why_table.rs dùng để biết bảng nào làm bảng bắt đầu được cho câu hỏi
/// nguyên nhân: mọi cột câu hỏi cần phải nằm ở bảng đó hay ở một bảng đi tới được từ nó.
pub fn build_reachable(base: &str, links: &[JoinLink]) -> Vec<String> {
    let mut reached = vec![base.to_string()];
    let mut frontier = vec![base.to_string()];
    for _ in 0..BREAKDOWN_MAX_HOPS {
        let mut next = Vec::new();
        for current in &frontier {
            for (table, _) in build_steps(current, links) {
                if !reached.iter().any(|t| t.eq_ignore_ascii_case(&table)) {
                    reached.push(table.clone());
                    next.push(table);
                }
            }
        }
        frontier = next;
    }
    reached.remove(0);
    reached
}

/// Các bảng đi tới được từ current mà không nhân dòng: lên bên một của quan hệ current giữ khóa ngoại, hay sang bên
/// kia của quan hệ một-một. Kèm cặp khóa (cột của current, cột của bảng tới). Bỏ quan hệ của một bảng với chính nó.
fn build_steps(current: &str, links: &[JoinLink]) -> Vec<(String, Vec<(String, String)>)> {
    let mut steps = Vec::new();
    for link in links {
        if link.many.eq_ignore_ascii_case(&link.one) {
            continue;
        }
        let pairs = |from: &[String], to: &[String]| -> Vec<(String, String)> {
            from.iter().cloned().zip(to.iter().cloned()).collect()
        };
        if link.many.eq_ignore_ascii_case(current) {
            steps.push((
                link.one.clone(),
                pairs(&link.many_columns, &link.one_columns),
            ));
        } else if !link.fans_out && link.one.eq_ignore_ascii_case(current) {
            steps.push((
                link.many.clone(),
                pairs(&link.one_columns, &link.many_columns),
            ));
        }
    }
    steps
}

// Cùng luật chọn cột chiều của explain/check.rs (build_dimensions): vai trò dimension, không nhạy cảm, 2 tới
// DIMENSION_MAX_VALUES giá trị. Cột khóa nối bị bỏ ở bên gọi: nó trùng cột khóa ngoại của bảng chính.
fn is_dimension(column: &ProfileColumn) -> bool {
    column.role == ProfileRole::Dimension.as_str()
        && !column.is_sensitive
        && column
            .distinct_count
            .is_some_and(|n| (2..=DIMENSION_MAX_VALUES).contains(&n))
}

// Bí danh đầu tiên của hồ sơ ngữ nghĩa, như build_labels của explain/check.rs.
fn build_label(column: &ProfileColumn) -> Option<String> {
    let alias = column
        .aliases
        .iter()
        .map(|a| a.trim())
        .find(|a| !a.is_empty())?;
    (alias.to_lowercase() != column.name.to_lowercase()).then(|| alias.to_string())
}

/// Phần FROM của một câu: tên bảng chính khi câu không cần cột liên quan nào, hay câu con nối các bảng cần cho các cột
/// trong names cộng PlanSource::always. Tên trong names không phải cột liên quan (cột của bảng chính) thì bỏ qua.
pub fn build_from_sql(table: &str, source: &PlanSource, names: &[&str]) -> String {
    let (wanted, needed) = build_needed(source, names);
    if wanted.is_empty() {
        return build_ident(table).to_string();
    }
    let mut items = vec![format!("{}.*", build_ident(BASE_ALIAS))];
    for column in &wanted {
        let alias = &source.joins[column.join].alias;
        items.push(format!(
            "{}.{} AS {}",
            build_ident(alias),
            build_ident(&column.column),
            build_ident(&column.name)
        ));
    }
    let mut from = format!("{} AS {}", build_ident(table), build_ident(BASE_ALIAS));
    // Bảng cha đứng trước bảng con trong joins, nên nối theo thứ tự này luôn có bí danh của bảng cha.
    for (index, join) in source.joins.iter().enumerate() {
        if !needed[index] {
            continue;
        }
        let parent = join
            .parent
            .map_or(BASE_ALIAS, |p| source.joins[p].alias.as_str());
        let on: Vec<String> = join
            .keys
            .iter()
            .map(|(parent_column, own)| {
                format!(
                    "{}.{} = {}.{}",
                    build_ident(parent),
                    build_ident(parent_column),
                    build_ident(&join.alias),
                    build_ident(own)
                )
            })
            .collect();
        from.push_str(&format!(
            " LEFT JOIN {} AS {} ON {}",
            build_ident(&join.table),
            build_ident(&join.alias),
            on.join(" AND ")
        ));
    }
    format!(
        "(SELECT {} FROM {from}) AS {}",
        items.join(", "),
        build_ident(JOINED_ALIAS)
    )
}

/// Bảng liên quan mà các cột trong names cần (kể cả bảng cha trên đường nối), theo thứ tự của joins: để ghi đúng phiên
/// bản mọi bảng câu đã đọc.
pub fn build_tables_used(source: &PlanSource, names: &[&str]) -> Vec<String> {
    let (_, needed) = build_needed(source, names);
    source
        .joins
        .iter()
        .zip(needed)
        .filter(|(_, needed)| *needed)
        .map(|(join, _)| join.table.clone())
        .collect()
}

/// Cột liên quan cần (trong names hay PlanSource::always) và bảng cần nối (theo chỉ số của joins): bảng của mỗi cột
/// cần và mọi bảng cha của nó.
fn build_needed<'a>(source: &'a PlanSource, names: &[&str]) -> (Vec<&'a RelatedColumn>, Vec<bool>) {
    let wanted: Vec<&RelatedColumn> = source
        .columns
        .iter()
        .filter(|c| names.contains(&c.name.as_str()) || source.always.contains(&c.name))
        .collect();
    let mut needed = vec![false; source.joins.len()];
    for column in &wanted {
        let mut next = Some(column.join);
        while let Some(index) = next {
            if needed[index] {
                break;
            }
            needed[index] = true;
            next = source.joins[index].parent;
        }
    }
    (wanted, needed)
}
