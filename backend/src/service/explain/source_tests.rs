// SPDX-License-Identifier: Apache-2.0
/* service/explain/source_tests.rs - test nguồn dòng nối bảng liên quan của phân rã (explain/source.rs, #227): đi theo
quan hệ nào, phần FROM in ra sao, và phân rã theo cột của bảng liên quan trên DuckDB thật (Parquet tạm, mọi câu qua
guard, có cả lọc dòng theo quyền của #215). Không cần Postgres. */

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;
use tokio::sync::Semaphore;

use crate::engine::guard::{build_guarded_sql, AllowedTable, GuardError, RowFilter};
use crate::service::ask::contract::CellValue;
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::breakdown::{
    build_breakdown, read_slice_totals, run_code_query, Breakdown, CodeQuery, QueryEnv,
    ASK_STOP_HINT,
};
use crate::service::explain::check::{apply_related, PlanSlices, WhyPlan};
use crate::service::explain::check_tests::column;
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::measure::{Additive, PlanMeasure};
use crate::service::explain::source::{
    build_from_sql, build_related_tables, build_tables_used, SourceTable,
};
use crate::service::explain::sql::{
    build_dimension_sql, build_ident, build_pair_count_sql, build_slices_sql, build_why_sql,
};

fn link(
    many: &str,
    many_columns: &[&str],
    one: &str,
    one_columns: &[&str],
    fans_out: bool,
) -> JoinLink {
    let names = |list: &[&str]| list.iter().map(|c| c.to_string()).collect();
    JoinLink {
        many: many.to_string(),
        many_columns: names(many_columns),
        one: one.to_string(),
        one_columns: names(one_columns),
        fans_out,
    }
}

/// Kiểu OULAD: scores trỏ students (hai cột khóa), students trỏ courses; feedback trỏ ngược vào scores (bên nhiều,
/// không được đi); profiles là một-một với students.
fn oulad_links() -> Vec<JoinLink> {
    vec![
        link(
            "scores",
            &["module", "id_student"],
            "students",
            &["module", "id_student"],
            true,
        ),
        link("students", &["module"], "courses", &["module"], true),
        link("feedback", &["id_score"], "scores", &["id_score"], true),
        link(
            "profiles",
            &["id_student"],
            "students",
            &["id_student"],
            false,
        ),
    ]
}

#[test]
fn only_many_to_one_and_one_to_one_steps_are_followed() {
    let students = vec![
        column(1, "module", "VARCHAR", "dimension", 3),
        column(2, "id_student", "INTEGER", "key", 900),
        column(3, "region", "VARCHAR", "dimension", 13),
        column(4, "studied_credits", "INTEGER", "measure", 20),
    ];
    let courses = vec![
        column(5, "module", "VARCHAR", "dimension", 3),
        column(6, "length", "INTEGER", "dimension", 4),
    ];
    let feedback = vec![column(7, "rating", "VARCHAR", "dimension", 5)];
    let profiles = vec![column(8, "band", "VARCHAR", "dimension", 4)];
    let tables = [
        SourceTable {
            table: "students",
            columns: &students,
        },
        SourceTable {
            table: "courses",
            columns: &courses,
        },
        SourceTable {
            table: "feedback",
            columns: &feedback,
        },
        SourceTable {
            table: "profiles",
            columns: &profiles,
        },
    ];
    let related = build_related_tables("scores", &tables, &oulad_links());
    let joined: Vec<(&str, Option<usize>)> = related
        .source
        .joins
        .iter()
        .map(|j| (j.table.as_str(), j.parent))
        .collect();
    // feedback là bên nhiều của scores: nối vào thì mỗi điểm bị đếm theo số phản hồi.
    assert_eq!(
        joined,
        [
            ("students", None),
            ("courses", Some(0)),
            ("profiles", Some(0))
        ]
    );
    let names: Vec<&str> = related.dimensions.iter().map(|d| d.name.as_str()).collect();
    // Cột khóa nối (students.module, courses.module) trùng khóa ngoại của bảng cha nên không là chiều.
    assert_eq!(
        names,
        ["students.region", "courses.length", "profiles.band"]
    );
}

#[test]
fn from_reads_the_table_alone_when_no_related_column_is_needed() {
    let related = build_related_tables("scores", &[], &oulad_links());
    assert!(related.source.joins.is_empty());
    assert_eq!(
        build_from_sql("scores", &related.source, &["region"]),
        "\"scores\""
    );
}

#[test]
fn from_joins_the_path_to_the_column() {
    let students = vec![column(1, "module", "VARCHAR", "dimension", 3)];
    let courses = vec![column(2, "length", "INTEGER", "dimension", 4)];
    let tables = [
        SourceTable {
            table: "students",
            columns: &students,
        },
        SourceTable {
            table: "courses",
            columns: &courses,
        },
    ];
    let related = build_related_tables("scores", &tables, &oulad_links());
    assert_eq!(
        build_from_sql("scores", &related.source, &["courses.length", "grade"]),
        "(SELECT \"b\".*, \"j2\".\"length\" AS \"courses.length\" FROM \"scores\" AS \"b\" \
         LEFT JOIN \"students\" AS \"j1\" ON \"b\".\"module\" = \"j1\".\"module\" AND \"b\".\"id_student\" = \
         \"j1\".\"id_student\" LEFT JOIN \"courses\" AS \"j2\" ON \"j1\".\"module\" = \"j2\".\"module\") AS \"joined\""
    );
    assert_eq!(
        build_tables_used(&related.source, &["courses.length"]),
        ["students", "courses"]
    );
}

/// Parquet tạm của nhiều bảng; xóa khi test xong.
struct Tables {
    root: PathBuf,
}

impl Tables {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("xdi-source-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        Tables { root }
    }

    fn write(&self, name: &str, rows: &str, columns: &[&str]) -> AllowedTable {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let file: &Path = &dir.join("part-0001.parquet");
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
        AllowedTable {
            name: name.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            parquet_dir: dir,
            row_filters: vec![],
        }
    }
}

impl Drop for Tables {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Điểm của hai học phần A (gốc) và B; sinh viên 1, 2 ở North, 3 ở South. Sinh viên 4 không có trong students (khóa
/// không trỏ tới đâu): điểm của họ vào nhóm trống, tổng không mất dòng.
const SCORES: &str =
    "SELECT * FROM (VALUES ('A', 1, 10), ('A', 2, 20), ('A', 3, 30), ('B', 1, 15), ('B', 2, 25), \
    ('B', 3, 60), ('B', 4, 5)) v(module, id_student, score)";
const STUDENTS: &str =
    "SELECT * FROM (VALUES (1, 'North'), (2, 'North'), (3, 'South')) v(id_student, region)";

fn score_plan() -> WhyPlan {
    let aggregate = sqlparser::parser::Parser::new(&sqlparser::dialect::DuckDbDialect {})
        .try_with_sql("SUM(\"score\")")
        .unwrap()
        .parse_expr()
        .unwrap();
    WhyPlan {
        table: "scores".to_string(),
        measure: PlanMeasure {
            label: "Tổng score".to_string(),
            metric: None,
            aggregate,
            additive: Additive::Sum {
                value: build_ident("score"),
                decimal: false,
            },
            denominator: None,
        },
        slices: PlanSlices::Groups {
            column: "module".to_string(),
            base: GroupBase::Value("A".to_string()),
            compare: "B".to_string(),
            period: None,
        },
        filters: vec![],
        dimensions: vec![],
        volume: None,
        labels: HashMap::new(),
        source: Default::default(),
    }
}

/// Phân rã chênh lệch B so với A theo students.region: tổng các nhóm bằng đúng chênh lệch, mọi câu qua guard. Trả các
/// cặp (nhóm, đóng góp).
async fn region_breakdown(
    scores: AllowedTable,
    students: AllowedTable,
) -> Vec<(Option<String>, String)> {
    let tables = vec![scores, students];
    let student_columns = vec![
        column(1, "id_student", "INTEGER", "key", 3),
        column(2, "region", "VARCHAR", "dimension", 2),
    ];
    let source_tables = [SourceTable {
        table: "students",
        columns: &student_columns,
    }];
    let links = [link(
        "scores",
        &["id_student"],
        "students",
        &["id_student"],
        true,
    )];
    let related = build_related_tables("scores", &source_tables, &links);
    let mut plan = score_plan();
    apply_related(&mut plan, &related, &[]);
    assert_eq!(plan.dimensions, ["students.region"]);
    let jobs = Arc::new(Semaphore::new(2));
    // Cùng quan hệ như phần FROM dựng ra: câu do code dựng phải qua luật nối bảng của guard (#256).
    let env = QueryEnv {
        tables: &tables,
        links: &links,
        max_rows: 1000,
        timeout: Duration::from_secs(10),
        jobs: &jobs,
        stop_hint: ASK_STOP_HINT,
    };
    let totals = match run_code_query(&env, &build_slices_sql(&plan), None)
        .await
        .unwrap()
    {
        CodeQuery::Ran { shown, result, .. } => read_slice_totals(shown, &result).unwrap(),
        _ => panic!("slice totals query did not run"),
    };
    let sql = build_why_sql(&plan);
    let explanation =
        match build_breakdown(&env, &plan, &totals, (sql.base_filter, sql.compare_filter))
            .await
            .unwrap()
        {
            Breakdown::Ready(explanation) => explanation,
            Breakdown::Unavailable(reason) => panic!("no breakdown: {reason}"),
        };
    let dimension = &explanation.dimensions[0];
    assert_eq!(dimension.column, "students.region");
    let sum: Decimal = dimension
        .groups
        .iter()
        .map(|g| g.contribution.parse::<Decimal>().unwrap())
        .sum();
    assert_eq!(sum, explanation.change.parse::<Decimal>().unwrap());
    dimension
        .groups
        .iter()
        .map(|g| {
            let value = g.value.as_ref().map(|v| match v {
                CellValue::String(text) => text.clone(),
                other => format!("{other:?}"),
            });
            (value, g.contribution.clone())
        })
        .collect()
}

#[tokio::test]
async fn breakdown_reads_a_column_of_the_related_table() {
    let files = Tables::new();
    let scores = files.write("scores", SCORES, &["module", "id_student", "score"]);
    let students = files.write("students", STUDENTS, &["id_student", "region"]);
    let groups = region_breakdown(scores, students).await;
    // B - A: South 60 - 30 = 30, North 40 - 30 = 10, sinh viên 4 không có trong students: nhóm trống 5.
    assert_eq!(
        groups,
        [
            (Some("South".to_string()), "30".to_string()),
            (Some("North".to_string()), "10".to_string()),
            (None, "5".to_string()),
        ]
    );
}

/// Người có vai dữ liệu: students lọc theo region, scores mang điều kiện lan theo khóa ngoại (#215). Điểm của sinh viên 4
/// (không trỏ tới sinh viên nào được thấy) bị ẩn ở bảng chính, nên nhóm trống không còn và tổng hai lát cắt cũng bớt
/// theo: mọi câu của phân rã đọc dữ liệu đã lọc như nhau.
#[tokio::test]
async fn breakdown_over_related_tables_keeps_row_filters() {
    let files = Tables::new();
    let mut scores = files.write("scores", SCORES, &["module", "id_student", "score"]);
    let mut students = files.write("students", STUDENTS, &["id_student", "region"]);
    students.row_filters = vec![RowFilter::Column {
        column: "region".to_string(),
        allowed_values: vec!["North".to_string(), "South".to_string()],
    }];
    scores.row_filters = vec![RowFilter::Parent {
        columns: vec!["id_student".to_string()],
        parent_columns: vec!["id_student".to_string()],
        parent: students.clone(),
    }];
    let groups = region_breakdown(scores, students).await;
    assert_eq!(
        groups,
        [
            (Some("South".to_string()), "30".to_string()),
            (Some("North".to_string()), "10".to_string()),
        ]
    );
}

/// Mọi câu do code dựng của đường câu "tại sao" (cả các khuôn của #244: một nhóm so với phần còn lại, bộ lọc IN và
/// BETWEEN, đếm cặp giá trị, đếm dòng theo kỳ) trên nguồn nối bảng liên quan theo khóa hai cột qua luật nối bảng của
/// guard (#256) khi có quan hệ, và bị từ chối khi không có quan hệ nào: phần FROM do code dựng thật sự đi qua luật.
#[test]
fn code_built_frame_sql_passes_the_join_rule() {
    let student_columns = vec![
        column(1, "module", "VARCHAR", "key", 2),
        column(2, "id_student", "INTEGER", "key", 3),
        column(3, "region", "VARCHAR", "dimension", 2),
    ];
    let source_tables = [SourceTable {
        table: "students",
        columns: &student_columns,
    }];
    let links = [link(
        "scores",
        &["module", "id_student"],
        "students",
        &["module", "id_student"],
        true,
    )];
    let related = build_related_tables("scores", &source_tables, &links);
    let mut plan = score_plan();
    plan.slices = PlanSlices::Groups {
        column: "module".to_string(),
        base: GroupBase::Rest,
        compare: "B".to_string(),
        period: None,
    };
    let parse = |sql: &str| {
        sqlparser::parser::Parser::new(&sqlparser::dialect::DuckDbDialect {})
            .try_with_sql(sql)
            .unwrap()
            .parse_expr()
            .unwrap()
    };
    plan.filters = vec![
        parse("\"students.region\" IN ('North', 'South')"),
        parse("\"score\" BETWEEN 0 AND 100"),
    ];
    apply_related(&mut plan, &related, &["students.region".to_string()]);
    plan.source.always = vec!["students.region".to_string()];
    let from = build_from_sql("scores", &plan.source, &[]);
    let sqls = vec![
        build_why_sql(&plan).total_sql,
        build_slices_sql(&plan),
        build_dimension_sql(&plan, "module"),
        build_pair_count_sql(&plan, "module", "score"),
        format!(
            "SELECT \"module\", COUNT(*) AS row_count FROM {from} WHERE \"students.region\" IN ('North') GROUP BY 1"
        ),
    ];
    let tables = [
        table_at("scores", &["module", "id_student", "score"]),
        table_at("students", &["module", "id_student", "region"]),
    ];
    for sql in &sqls {
        assert!(sql.contains("LEFT JOIN"), "{sql}");
        if let Err(e) = build_guarded_sql(sql, &tables, &links, 1000) {
            panic!("{sql}: {e:?}");
        }
        assert!(
            matches!(
                build_guarded_sql(sql, &tables, &[], 1000),
                Err(GuardError::Rejected(_))
            ),
            "{sql}"
        );
    }
}

fn table_at(name: &str, columns: &[&str]) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir: PathBuf::from(format!("/data/{name}")),
        row_filters: vec![],
    }
}
