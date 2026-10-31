// SPDX-License-Identifier: Apache-2.0
/* service/explain/dimension_pick_tests.rs - test chọn chiều của phân rã (explain/dimension_pick.rs) và việc giữ
mọi chiều còn lại sau khi chấm điểm (explain/breakdown.rs), trên DuckDB thật với file Parquet tạm, mọi câu qua guard
như khi người dùng hỏi. Hai lát cắt là hai nhóm của cột segment: A (gốc) và B (so sánh). Không cần Postgres. */

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use tokio::sync::Semaphore;

use crate::engine::guard::AllowedTable;
use crate::service::ask::contract::ValueKind;
use crate::service::explain::annotate::DropReason;
use crate::service::explain::breakdown::{
    build_breakdown, run_candidates, Breakdown, DimensionRows, QueryEnv, SliceTotals, ASK_STOP_HINT,
};
use crate::service::explain::check::{PlanSlices, WhyPlan};
use crate::service::explain::dimension_pick::{
    build_useless_reason, has_same_numbers, pick_dimensions, prefers, Presence,
};
use crate::service::explain::group_base::GroupBase;
use crate::service::explain::measure::{Additive, PlanMeasure};
use crate::service::explain::sql::build_ident;

const TABLE: &str = "sales";
const ADDITIVE: Presence = Presence {
    base: 0,
    compare: 1,
};

/// File Parquet tạm của một bảng; xóa khi test xong.
struct Table {
    root: PathBuf,
    allowed: Vec<AllowedTable>,
}

impl Table {
    /// rows: câu SELECT sinh các dòng; columns: tên các cột của câu đó.
    fn new(rows: &str, columns: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!("xdi-pick-{}", uuid::Uuid::new_v4()));
        let dir = root.join("ws").join("dataset").join("v1");
        std::fs::create_dir_all(&dir).unwrap();
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let file = dir.join("part-0001.parquet");
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
        let allowed = vec![AllowedTable {
            name: TABLE.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            parquet_dir: dir,
            row_filters: vec![],
        }];
        Table { root, allowed }
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn env<'a>(table: &'a Table, jobs: &'a Arc<Semaphore>) -> QueryEnv<'a> {
    QueryEnv {
        tables: &table.allowed,
        max_rows: 100_000,
        timeout: Duration::from_secs(10),
        jobs,
        stop_hint: ASK_STOP_HINT,
    }
}

/// Tổng amount, so nhóm A với nhóm B của segment.
fn plan(dimensions: &[&str]) -> WhyPlan {
    let aggregate = Parser::new(&DuckDbDialect {})
        .try_with_sql("SUM(amount)")
        .unwrap()
        .parse_expr()
        .unwrap();
    WhyPlan {
        table: TABLE.to_string(),
        measure: PlanMeasure {
            label: "Tổng amount".to_string(),
            metric: None,
            aggregate,
            additive: Additive::Sum {
                value: build_ident("amount"),
                decimal: false,
            },
            denominator: None,
        },
        slices: PlanSlices::Groups {
            column: "segment".to_string(),
            base: GroupBase::Value("A".to_string()),
            compare: "B".to_string(),
            period: None,
        },
        filters: vec![],
        dimensions: dimensions.iter().map(|d| d.to_string()).collect(),
        volume: None,
        labels: HashMap::new(),
        source: Default::default(),
    }
}

/// Bốn dòng nhóm A, hai dòng nhóm B, amount đều là 1:
/// - region và region_code chia y hệt nhau (North là 1, South là 2): trùng, giữ region (cột chữ);
/// - x và y cho cùng bộ số (mỗi nhóm 2 ở A, 1 ở B) nhưng chia khác nhau (bốn cặp x, y): không trùng, giữ cả hai;
/// - seg_code chia y hệt segment nên mỗi nhóm chỉ có ở một lát cắt: bỏ;
/// - single chỉ một giá trị: bỏ, và không có trong danh sách bỏ cho lời tóm tắt.
const PICK_ROWS: &str = "SELECT * FROM (VALUES \
    ('A', 'North', 1, 'a', 'p', 1, 'k', 1), \
    ('A', 'North', 1, 'a', 'q', 1, 'k', 1), \
    ('A', 'North', 1, 'b', 'p', 1, 'k', 1), \
    ('A', 'South', 2, 'b', 'q', 1, 'k', 1), \
    ('B', 'North', 1, 'a', 'p', 2, 'k', 1), \
    ('B', 'South', 2, 'b', 'q', 2, 'k', 1)) \
    v(segment, region, region_code, x, y, seg_code, single, amount)";
const PICK_COLUMNS: [&str; 8] = [
    "segment",
    "region",
    "region_code",
    "x",
    "y",
    "seg_code",
    "single",
    "amount",
];

#[tokio::test]
async fn duplicate_one_sided_and_single_value_dimensions_are_dropped() {
    let table = Table::new(PICK_ROWS, &PICK_COLUMNS);
    let jobs = Arc::new(Semaphore::new(2));
    let env = env(&table, &jobs);
    // region_code đứng trước region: cột chữ đứng sau vẫn thay được cột số đứng trước.
    let plan = plan(&["region_code", "region", "x", "y", "seg_code", "single"]);
    let candidates = run_candidates(&env, &plan, 2).await.unwrap();
    assert_eq!(candidates.len(), 6, "every candidate query ran");
    let picked = pick_dimensions(&env, &plan, candidates, ADDITIVE)
        .await
        .unwrap();
    let columns: Vec<&str> = picked.kept.iter().map(|c| c.column.as_str()).collect();
    assert_eq!(columns, ["region", "x", "y"]);
    let dropped: Vec<(&str, DropReason, Option<&str>)> = picked
        .dropped
        .iter()
        .map(|d| (d.column.as_str(), d.reason, d.duplicate_of.as_deref()))
        .collect();
    assert_eq!(
        dropped,
        [
            ("region_code", DropReason::Duplicates, Some("region")),
            ("seg_code", DropReason::SeparatesSlices, None),
        ]
    );
    let separated = &picked.dropped[1];
    assert_eq!(separated.base_only, ["1"]);
    assert_eq!(separated.compare_only, ["2"]);
}

/// Chín chiều d chia khác nhau (số nhóm 2 tới 11, bỏ 7), cộng cặp c_code, c_name chia y hệt nhau theo i % 7: bỏ c_code,
/// mười chiều còn lại đều có bảng, không cắt theo số bảng (trước đây chỉ giữ 8).
#[tokio::test]
async fn every_dimension_left_is_kept() {
    let moduli = [2, 3, 4, 5, 6, 8, 9, 10, 11];
    let dims: Vec<String> = moduli.iter().map(|m| format!("d{m}")).collect();
    let items: Vec<String> = moduli
        .iter()
        .map(|m| format!("'v' || CAST(i % {m} AS VARCHAR) AS d{m}"))
        .collect();
    let rows = format!(
        "SELECT CASE WHEN i < 40 THEN 'A' ELSE 'B' END AS segment, {}, CAST(i % 7 AS INTEGER) AS c_code, \
         'n' || CAST(i % 7 AS VARCHAR) AS c_name, CAST(i + 1 AS INTEGER) AS amount FROM range(60) t(i)",
        items.join(", ")
    );
    let mut columns: Vec<&str> = vec!["segment", "c_code", "c_name", "amount"];
    columns.extend(dims.iter().map(String::as_str));
    let table = Table::new(&rows, &columns);
    let jobs = Arc::new(Semaphore::new(2));
    let env = env(&table, &jobs);
    let mut candidates: Vec<&str> = vec!["c_code", "c_name"];
    candidates.extend(dims.iter().map(String::as_str));
    let plan = plan(&candidates);
    // Tổng amount: A là 1 + ... + 40 = 820, B là 41 + ... + 60 = 1010.
    let totals = SliceTotals {
        sql: String::new(),
        base: Decimal::from(820),
        compare: Decimal::from(1010),
        base_rows: Decimal::from(40),
        compare_rows: Decimal::from(20),
        volume: None,
        denominators: None,
    };
    let Breakdown::Ready(explanation) =
        build_breakdown(&env, &plan, &totals, (String::new(), String::new()))
            .await
            .unwrap()
    else {
        panic!("breakdown should be ready");
    };
    let shown: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    let mut sorted = shown.clone();
    sorted.sort_unstable();
    let mut expected = vec!["c_name"];
    expected.extend(dims.iter().map(String::as_str));
    expected.sort_unstable();
    assert_eq!(sorted, expected);
    // c_code không có bảng nhưng lời tóm tắt vẫn có ý "Bỏ qua" cho nó (explain/summary.rs đọc notes.dropped).
    let skipped: Vec<(&str, Option<&str>)> = explanation
        .notes
        .dropped
        .iter()
        .map(|d| (d.column.as_str(), d.duplicate_of.as_deref()))
        .collect();
    assert_eq!(skipped, [("c_code", Some("c_name"))]);
}

// Các hàm thuần

fn rows(groups: &[(&str, [i64; 2])]) -> DimensionRows {
    DimensionRows {
        shown: String::new(),
        kind: ValueKind::Text,
        groups: groups
            .iter()
            .map(|(value, numbers)| {
                (
                    Some(crate::service::ask::contract::CellValue::String(
                        value.to_string(),
                    )),
                    numbers.iter().map(|n| Decimal::from(*n)).collect(),
                )
            })
            .collect(),
    }
}

#[test]
fn useless_dimensions_have_a_reason() {
    let one_group = rows(&[("a", [3, 1])]);
    let one_sided = rows(&[("a", [3, 0]), ("b", [0, 2])]);
    let both_sides = rows(&[("a", [3, 0]), ("b", [1, 2])]);
    assert!(build_useless_reason(&one_group, ADDITIVE, true).is_some());
    assert!(build_useless_reason(&one_group, ADDITIVE, false).is_some());
    assert!(build_useless_reason(&one_sided, ADDITIVE, true).is_some());
    assert!(build_useless_reason(&both_sides, ADDITIVE, true).is_none());
}

/// So hai kỳ thì chiều một phía vẫn giữ: Consumer có ở quý trước rồi mất, Corporate mới có ở quý sau là thay đổi thật.
#[test]
fn one_sided_dimension_is_kept_between_periods() {
    let one_sided = rows(&[("Consumer", [200, 0]), ("Corporate", [0, 120])]);
    assert!(build_useless_reason(&one_sided, ADDITIVE, false).is_none());
}

#[test]
fn same_numbers_ignore_group_names_and_order() {
    let first = rows(&[("a", [2, 1]), ("b", [5, 0])]);
    assert!(has_same_numbers(
        &first,
        &rows(&[("q", [5, 0]), ("p", [2, 1])])
    ));
    assert!(!has_same_numbers(
        &first,
        &rows(&[("q", [5, 1]), ("p", [2, 1])])
    ));
    assert!(!has_same_numbers(&first, &rows(&[("p", [2, 1])])));
}

#[test]
fn text_column_is_preferred_over_a_code() {
    assert!(prefers(ValueKind::Text, ValueKind::Number));
    assert!(!prefers(ValueKind::Number, ValueKind::Text));
    assert!(!prefers(ValueKind::Text, ValueKind::Text));
}
