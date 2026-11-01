// SPDX-License-Identifier: Apache-2.0
/* service/explain/follows_slices_tests.rs - test luật "đi theo hai nhóm" của explain/dimension_pick.rs trên DuckDB thật
(Parquet tạm, mọi câu qua guard kèm lọc dòng theo vai), với hai bảng kiểu OULAD của edu_demo: students (mỗi dòng một sinh
viên đăng ký một học phần ở một mã kỳ) trỏ courses (năm, tháng bắt đầu, độ dài của từng học phần ở từng mã kỳ). Câu thật
"tại sao nó lại cao nhất" so 2014J với trung bình các mã kỳ còn lại; phân rã theo năm và tháng bắt đầu của courses chỉ
chia lại các mã kỳ ("năm 2014 cao hơn 336,5%") mà mô hình chấm và người đọc đều tưởng là nguyên nhân. Không cần
Postgres. */

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;
use tokio::sync::Semaphore;

use crate::engine::guard::{AllowedTable, RowFilter};
use crate::service::ask::contract::Explanation;
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::annotate::DropReason;
use crate::service::explain::breakdown::{
    build_breakdown, read_slice_totals, run_code_query, Breakdown, CodeQuery, QueryEnv,
    ASK_STOP_HINT,
};
use crate::service::explain::check::{apply_related, PlanSlices, WhyPlan};
use crate::service::explain::check_tests::column;
use crate::service::explain::dimension_pick::{may_follow_slices, Presence};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::measure::{Additive, PlanMeasure};
use crate::service::explain::source::{build_related_tables, SourceTable};
use crate::service::explain::sql::build_why_sql;

/// Bốn mã kỳ của hai học phần. 2014J có 12 lượt đăng ký, ba mã kỳ còn lại có 3, 5 và 4 (trung bình 4). Năm và tháng bắt
/// đầu chỉ tùy mã kỳ; độ dài học phần ở 2014J (269, 262) không có ở mã kỳ nào khác.
const COURSES: &str = "SELECT * FROM (VALUES \
    ('AAA', '2013B', 2013, 2, 240), ('AAA', '2013J', 2013, 10, 268), ('BBB', '2013J', 2013, 10, 261), \
    ('AAA', '2014B', 2014, 2, 234), ('AAA', '2014J', 2014, 10, 269), ('BBB', '2014J', 2014, 10, 262)) \
    v(code_module, code_presentation, presentation_year, presentation_start_month, module_presentation_length)";
const STUDENTS: &str = "SELECT code_module, code_presentation, CAST(i AS INTEGER) AS id_student, \
    CASE WHEN i % 2 = 0 THEN 'F' ELSE 'M' END AS gender FROM (VALUES \
    ('AAA', '2014J', 8), ('BBB', '2014J', 4), ('AAA', '2013B', 3), ('AAA', '2013J', 3), ('BBB', '2013J', 2), \
    ('AAA', '2014B', 4)) v(code_module, code_presentation, n), range(n) t(i)";
const COURSE_COLUMNS: [&str; 5] = [
    "code_module",
    "code_presentation",
    "presentation_year",
    "presentation_start_month",
    "module_presentation_length",
];
const STUDENT_COLUMNS: [&str; 4] = ["code_module", "code_presentation", "id_student", "gender"];

/// Parquet tạm của hai bảng; xóa khi test xong.
struct Tables {
    root: PathBuf,
}

impl Tables {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("xdi-follows-{}", uuid::Uuid::new_v4()));
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

fn links() -> Vec<JoinLink> {
    let names = |list: &[&str]| list.iter().map(|c| c.to_string()).collect();
    vec![JoinLink {
        many: "students".to_string(),
        many_columns: names(&["code_module", "code_presentation"]),
        one: "courses".to_string(),
        one_columns: names(&["code_module", "code_presentation"]),
        fans_out: true,
    }]
}

/// Số lượt đăng ký của 2014J so với base, theo giới tính và ba cột của courses.
fn plan(base: GroupBase) -> WhyPlan {
    let aggregate = sqlparser::parser::Parser::new(&sqlparser::dialect::DuckDbDialect {})
        .try_with_sql("COUNT(*)")
        .unwrap()
        .parse_expr()
        .unwrap();
    let course_columns = vec![
        column(1, "code_module", "VARCHAR", "key", 2),
        column(2, "code_presentation", "VARCHAR", "key", 4),
        column(3, "presentation_year", "INTEGER", "dimension", 2),
        column(4, "presentation_start_month", "INTEGER", "dimension", 2),
        column(5, "module_presentation_length", "INTEGER", "dimension", 6),
    ];
    let source_tables = [SourceTable {
        table: "courses",
        columns: &course_columns,
    }];
    let related = build_related_tables("students", &source_tables, &links());
    let mut plan = WhyPlan {
        table: "students".to_string(),
        measure: PlanMeasure {
            label: "Số lượt đăng ký".to_string(),
            metric: None,
            aggregate,
            additive: Additive::Count { value: None },
            denominator: None,
        },
        slices: PlanSlices::Groups {
            column: "code_presentation".to_string(),
            base,
            compare: "2014J".to_string(),
            period: None,
        },
        filters: vec![],
        dimensions: vec!["gender".to_string()],
        volume: None,
        labels: HashMap::from([("code_presentation".to_string(), "mã kỳ".to_string())]),
        source: Default::default(),
    };
    apply_related(&mut plan, &related, &[]);
    plan
}

/// Phân rã như bước explain: tổng hai lát cắt rồi build_breakdown, mọi câu qua guard với lọc dòng của students.
async fn breakdown(plan: &WhyPlan) -> Explanation {
    let files = Tables::new();
    let mut students = files.write("students", STUDENTS, &STUDENT_COLUMNS);
    // Vai dữ liệu chỉ cho thấy hai học phần: câu đếm của luật mới cũng phải qua guard có lọc dòng.
    students.row_filters = vec![RowFilter::Column {
        column: "code_module".to_string(),
        allowed_values: vec!["AAA".to_string(), "BBB".to_string()],
    }];
    let courses = files.write("courses", COURSES, &COURSE_COLUMNS);
    let tables = vec![students, courses];
    let links = links();
    let jobs = Arc::new(Semaphore::new(2));
    let env = QueryEnv {
        tables: &tables,
        links: &links,
        max_rows: 1000,
        timeout: Duration::from_secs(10),
        jobs: &jobs,
        stop_hint: ASK_STOP_HINT,
    };
    let totals = match run_code_query(
        &env,
        &crate::service::explain::sql::build_slices_sql(plan),
        None,
    )
    .await
    .unwrap()
    {
        CodeQuery::Ran { shown, result, .. } => read_slice_totals(shown, &result).unwrap(),
        _ => panic!("slice totals query did not run"),
    };
    let sql = build_why_sql(plan);
    match build_breakdown(&env, plan, &totals, (sql.base_filter, sql.compare_filter))
        .await
        .unwrap()
    {
        Breakdown::Ready(explanation) => *explanation,
        Breakdown::Unavailable(reason) => panic!("no breakdown: {reason}"),
    }
}

fn dropped(e: &Explanation) -> Vec<(&str, DropReason, Option<&str>)> {
    e.notes
        .dropped
        .iter()
        .map(|d| (d.column.as_str(), d.reason, d.duplicate_of.as_deref()))
        .collect()
}

/// 2014J so với trung bình các mã kỳ còn lại: năm và tháng bắt đầu có nhóm ở cả hai bên (năm 2014 gồm 2014B) nhưng chỉ
/// đi theo mã kỳ, nên bỏ kèm lý do; độ dài học phần vẫn bỏ như trước vì không nhóm nào có ở hai bên; giới tính giữ.
#[tokio::test]
async fn columns_that_follow_the_compared_column_are_dropped_against_the_rest() {
    let plan = plan(GroupBase::Rest);
    assert!(plan
        .dimensions
        .contains(&"courses.presentation_year".to_string()));
    let e = breakdown(&plan).await;
    assert_eq!(e.base.value, "4");
    assert_eq!(e.compare.value, "12");
    let shown: Vec<&str> = e.dimensions.iter().map(|d| d.column.as_str()).collect();
    assert_eq!(shown, ["gender"]);
    let mut list = dropped(&e);
    list.sort_by_key(|d| d.0);
    assert_eq!(
        list,
        [
            (
                "courses.module_presentation_length",
                DropReason::SeparatesSlices,
                None
            ),
            (
                "courses.presentation_start_month",
                DropReason::FollowsSlices,
                Some("code_presentation")
            ),
            (
                "courses.presentation_year",
                DropReason::FollowsSlices,
                Some("code_presentation")
            ),
        ]
    );
    // Câu khuôn nói vì sao bỏ, gọi cột đang so bằng nghĩa tiếng Việt.
    let summary = crate::service::explain::build_template_summary(&e);
    assert!(
        summary.contains(
            "Mỗi mã kỳ chỉ có một courses.presentation_year và một courses.presentation_start_month, nên các cột \
             này chỉ chia lại các mã kỳ đang so."
        ),
        "{summary}"
    );
}

/// So hai mã kỳ một với một: các luật cũ vẫn bắt trước (năm và độ dài chỉ có ở một bên, tháng bắt đầu chỉ một nhóm),
/// luật mới không đổi lý do bỏ.
#[tokio::test]
async fn two_group_comparison_keeps_the_old_reasons() {
    let plan = plan(GroupBase::Value("2013J".to_string()));
    let e = breakdown(&plan).await;
    let shown: Vec<&str> = e.dimensions.iter().map(|d| d.column.as_str()).collect();
    assert_eq!(shown, ["gender"]);
    let mut list = dropped(&e);
    list.sort_by_key(|d| d.0);
    assert_eq!(
        list,
        [
            (
                "courses.module_presentation_length",
                DropReason::SeparatesSlices,
                None
            ),
            (
                "courses.presentation_year",
                DropReason::SeparatesSlices,
                None
            ),
        ]
    );
}

/// Câu đếm chỉ chạy khi bên được so có số ở không quá một nhóm.
#[test]
fn only_a_single_compared_group_can_follow_the_slices() {
    use crate::service::explain::breakdown::DimensionRows;
    let rows = |groups: &[[i64; 2]]| DimensionRows {
        shown: String::new(),
        kind: crate::service::ask::contract::ValueKind::Number,
        groups: groups
            .iter()
            .map(|n| (None, n.iter().map(|v| Decimal::from(*v)).collect()))
            .collect(),
    };
    let presence = Presence {
        base: 0,
        compare: 1,
    };
    assert!(may_follow_slices(&rows(&[[4, 12], [8, 0]]), presence));
    assert!(!may_follow_slices(&rows(&[[4, 8], [8, 4]]), presence));
}
