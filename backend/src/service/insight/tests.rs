// SPDX-License-Identifier: Apache-2.0
/* service/insight/tests.rs - test phần giải thích theo hình dạng kết quả (#227): câu do code viết cho từng hình dạng và
cho trường hợp không có gì đáng kể (insight/facts.rs), đọc kỳ và câu SQL (insight/periods.rs, insight/plan.rs), bước kiểm
lời của mô hình (insight/narrate.rs), và trọn đường trên DuckDB thật với hai bảng nối nhiều-một (Parquet tạm, mọi câu qua
guard): chuỗi thời gian có phân rã theo cột của bảng liên quan, một con số so với kỳ trước. Không cần Postgres. */

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlparser::ast::Expr;
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use tokio::sync::Semaphore;

use crate::engine::guard::AllowedTable;
use crate::engine::llm::mock::Mock;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::ProfileColumn;
use crate::error::AppError;
use crate::service::ask::contract::{AnswerVerdict, ColumnRole, InsightKind, ResultTable};
use crate::service::ask::fanout::JoinLink;
use crate::service::explain::breakdown::{run_code_query, CodeQuery, QueryEnv, ASK_STOP_HINT};
use crate::service::explain::check_tests::column;
use crate::service::explain::period::{Grain, Period};
use crate::service::explain::significance::{build_change_weight, ChangeWeight};
use crate::service::insight::facts::{
    build_group_parts, build_rows_parts, build_series_parts, build_single_parts, build_top_parts,
    find_largest_step, InsightParts, Point,
};
use crate::service::insight::narrate::{
    build_insight_check, build_insight_prompt, find_causal_word, narrate_insight,
};
use crate::service::insight::periods::{read_cell_period, read_filter_period, read_series_time};
use crate::service::insight::plan::{build_total_sql, read_plain_query, PlainTable};
use crate::service::insight::{build_insight, InsightInput};
use crate::service::prompt_fixtures::check_fixture;

fn point(label: &str, value: i64) -> Point {
    Point {
        label: label.to_string(),
        value: Decimal::from(value),
    }
}

fn texts(parts: &InsightParts) -> Vec<String> {
    std::iter::once(parts.title.clone())
        .chain(
            parts
                .points
                .iter()
                .map(|p| format!("{}: {}", p.lead, p.text)),
        )
        .collect()
}

#[test]
fn change_weight_flags_tiny_changes_and_small_samples() {
    let d = Decimal::from;
    assert_eq!(
        build_change_weight(d(1000), d(1015), None),
        ChangeWeight::Tiny
    );
    assert_eq!(
        build_change_weight(d(1000), d(1030), None),
        ChangeWeight::Notable
    );
    assert_eq!(build_change_weight(d(0), d(0), None), ChangeWeight::Tiny);
    assert_eq!(
        build_change_weight(d(10), d(100), Some((9, 4))),
        ChangeWeight::SmallSample
    );
}

#[test]
fn groups_give_the_top_share_the_count_to_80_percent_and_outliers() {
    let groups = [
        point("Europe", 700),
        point("LATAM", 150),
        point("USCA", 100),
        point("Africa", 50),
    ];
    let parts = build_group_parts(
        "Doanh thu",
        "Thị trường",
        &groups,
        Some(Decimal::from(1000)),
    );
    assert!(parts.notable);
    assert_eq!(
        parts.title,
        "Europe đứng đầu với 700, chiếm 70% tổng doanh thu."
    );
    assert_eq!(
        texts(&parts)[1..],
        [
            "Mức tập trung: 2 trên 4 nhóm chiếm từ 80% tổng trở lên.",
            "Cao hẳn mức trung bình: Europe (700); trung bình mỗi nhóm 250.",
            "Thấp hẳn mức trung bình: Africa (50), USCA (100); trung bình mỗi nhóm 250.",
        ]
    );
}

#[test]
fn even_groups_are_not_notable() {
    let groups = [
        point("A", 100),
        point("B", 110),
        point("C", 95),
        point("D", 105),
    ];
    let parts = build_group_parts("Doanh thu", "Nhóm", &groups, Some(Decimal::from(410)));
    assert!(!parts.notable);
    assert_eq!(
        parts.points.last().unwrap().lead,
        "Không có nhóm nào nổi bật"
    );
}

#[test]
fn top_n_gives_the_share_of_the_total_and_the_gap() {
    let shown = [point("Europe", 600), point("LATAM", 400)];
    let parts = build_top_parts("Doanh thu", &shown, Some((Decimal::from(2000), 5)));
    assert_eq!(
        parts.title,
        "2 nhóm đầu chiếm 50% tổng doanh thu (2.000 trên 5 nhóm)."
    );
    assert_eq!(parts.points[0].text, "Europe cao hơn LATAM 200 (50%).");
    let close = [point("Europe", 1000), point("LATAM", 990)];
    assert!(!build_top_parts("Doanh thu", &close, None).notable);
}

#[test]
fn series_names_the_largest_step_or_says_nothing_notable() {
    let series = [
        point("Tháng 9/2017", 400),
        point("Tháng 10/2017", 380),
        point("Tháng 11/2017", 800),
    ];
    assert_eq!(find_largest_step(&series), Some(2));
    let parts = build_series_parts("Doanh thu", &series, None, Some((40, 40)));
    assert!(parts.notable);
    assert_eq!(
        parts.title,
        "Tháng 11/2017 đổi mạnh nhất so với kỳ trước: tăng 420 (110,5%) so với Tháng 10/2017."
    );
    assert_eq!(
        parts.points[0].text,
        "Từ Tháng 9/2017 tới Tháng 11/2017, doanh thu tăng 400 (100%)."
    );
    let flat = [
        point("Tháng 9/2017", 1000),
        point("Tháng 10/2017", 1010),
        point("Tháng 11/2017", 1005),
    ];
    let parts = build_series_parts("Doanh thu", &flat, None, None);
    assert!(!parts.notable);
    assert_eq!(parts.title, "Không có thay đổi đáng kể giữa các kỳ.");
}

#[test]
fn single_value_with_a_small_sample_is_not_notable() {
    let current = point("tháng 11/2017", 900);
    let previous = point("tháng 10/2017", 300);
    let parts = build_single_parts("Doanh thu", &current, Some((&previous, Some((9, 4)))), None);
    assert!(!parts.notable);
    assert_eq!(parts.title, "Doanh thu tháng 11/2017 là 900.");
    assert_eq!(
        parts.points[0].text,
        "tháng 10/2017 có 9 dòng, tháng 11/2017 có 4 dòng: quá ít dòng để kết luận."
    );
    let parts = build_single_parts(
        "Doanh thu",
        &current,
        Some((&previous, Some((90, 40)))),
        None,
    );
    assert!(parts.notable);
    assert_eq!(
        parts.title,
        "Doanh thu tháng 11/2017 là 900, tăng 600 (200%) so với tháng 10/2017."
    );
}

#[test]
fn rows_give_only_the_count_and_the_filters() {
    let parts = build_rows_parts(42, false, &["Thị trường là Europe".to_string()]);
    assert!(!parts.notable);
    assert_eq!(
        texts(&parts),
        [
            "Kết quả có 42 dòng.",
            "Điều kiện lọc: Thị trường là Europe."
        ]
    );
}

fn parse(sql: &str) -> Expr {
    Parser::new(&DuckDbDialect {})
        .try_with_sql(sql)
        .unwrap()
        .parse_expr()
        .unwrap()
}

fn time_column(name: &str) -> ProfileColumn {
    column(90, name, "TIMESTAMP", "time", 900)
}

#[test]
fn filter_periods_are_read_from_common_shapes() {
    let finder = |name: &str| (name == "orders.order_date").then(|| time_column(name));
    let cases = [
        (
            vec![
                "year(\"orders.order_date\") = 2017",
                "month(\"orders.order_date\") = 11",
            ],
            "Tháng 11/2017",
        ),
        (
            vec![
                "\"orders.order_date\" >= TIMESTAMP '2017-10-01'",
                "\"orders.order_date\" < TIMESTAMP '2018-01-01'",
            ],
            "Quý 4/2017",
        ),
        (
            vec!["strftime(\"orders.order_date\", '%Y') = '2016'"],
            "Năm 2016",
        ),
        (
            vec!["\"orders.order_date\" BETWEEN '2017-11-01' AND '2017-11-30'"],
            "Tháng 11/2017",
        ),
    ];
    for (filters, label) in cases {
        let mut exprs: Vec<Expr> = filters.iter().map(|f| parse(f)).collect();
        exprs.push(parse("\"orders.market\" = 'Europe'"));
        let (time, period, used) = read_filter_period(&exprs, &finder).unwrap();
        assert_eq!(time.column, "orders.order_date");
        assert_eq!(period.label(), label);
        assert_eq!(
            used.len(),
            filters.len(),
            "the market filter is not a time filter"
        );
    }
    // Hai tháng không phải một kỳ: không đoán.
    let two_months = [
        parse("\"orders.order_date\" >= '2017-10-01'"),
        parse("\"orders.order_date\" < '2017-12-01'"),
    ];
    assert!(read_filter_period(&two_months, &finder).is_none());
}

#[test]
fn series_keys_give_the_grain_and_the_periods() {
    let finder = |name: &str| (name == "day").then(|| time_column(name));
    let time = read_series_time(&parse("date_trunc('month', \"day\")"), &finder).unwrap();
    assert_eq!(time.grain, Grain::Month);
    let time = read_series_time(&parse("strftime(\"day\", '%Y')"), &finder).unwrap();
    assert_eq!(time.grain, Grain::Year);
    let cell = crate::service::ask::contract::CellValue::String("2017-11-01 00:00:00".to_string());
    assert_eq!(
        read_cell_period(Grain::Month, Some(&cell)),
        Period::parse(Grain::Month, "2017-11").ok()
    );
}

fn order_tables() -> (Vec<ProfileColumn>, Vec<ProfileColumn>) {
    let items = vec![
        column(1, "order_item_id", "INTEGER", "key", 160),
        column(2, "order_id", "INTEGER", "key", 160),
        column(3, "sales", "DECIMAL(18,2)", "measure", 3),
    ];
    let orders = vec![
        column(4, "order_id", "INTEGER", "key", 160),
        time_column("order_date"),
        column(6, "market", "VARCHAR", "dimension", 2),
        column(7, "shipping_mode", "VARCHAR", "dimension", 2),
        column(8, "late", "INTEGER", "measure", 2),
    ];
    (items, orders)
}

fn order_links() -> Vec<JoinLink> {
    vec![JoinLink {
        many: "order_items".to_string(),
        many_columns: vec!["order_id".to_string()],
        one: "orders".to_string(),
        one_columns: vec!["order_id".to_string()],
        fans_out: true,
    }]
}

const SERIES_SQL: &str = "SELECT date_trunc('month', o.order_date) AS month, SUM(oi.sales) AS revenue \
    FROM order_items oi JOIN orders o ON oi.order_id = o.order_id WHERE o.market = 'Europe' GROUP BY 1 ORDER BY 1";

#[test]
fn plain_sql_is_read_on_the_fact_table_with_related_columns() {
    let (items, orders) = order_tables();
    let tables = [
        PlainTable {
            table: "order_items",
            columns: &items,
            row_count: Some(160),
        },
        PlainTable {
            table: "orders",
            columns: &orders,
            row_count: Some(160),
        },
    ];
    let query = read_plain_query(SERIES_SQL, 1, "Doanh thu", &tables, &order_links()).unwrap();
    assert_eq!(query.table, "order_items");
    assert_eq!(query.filters[0].to_string(), "\"orders.market\" = 'Europe'");
    assert_eq!(
        query.outputs[0].as_ref().unwrap().to_string(),
        "date_trunc('month', \"orders.order_date\")"
    );
    // Cộng cột của bên một sau khi nối bên nhiều là cộng lặp: không có bảng chính nào hợp lệ.
    let fan_out = "SELECT o.market, SUM(o.late) FROM orders o JOIN order_items oi ON oi.order_id = o.order_id GROUP BY 1";
    assert!(read_plain_query(fan_out, 1, "Số đơn trễ", &tables, &order_links()).is_none());
}

#[test]
fn top_n_total_drops_the_limit_and_sums_every_group() {
    let sql =
        "SELECT market, SUM(sales) AS revenue FROM t GROUP BY market ORDER BY revenue DESC LIMIT 3";
    assert_eq!(
        build_total_sql(sql, 1).unwrap(),
        "SELECT SUM(insight_measure) AS total_value, COUNT(*) AS group_count FROM (SELECT market, SUM(sales) AS \
         insight_measure FROM t GROUP BY market) AS all_groups"
    );
    assert!(build_total_sql(
        "SELECT market, AVG(sales) FROM t GROUP BY market LIMIT 3",
        1
    )
    .is_none());
}

fn checked_parts() -> InsightParts {
    let series = [point("Tháng 10/2017", 400), point("Tháng 11/2017", 800)];
    build_series_parts("Doanh thu", &series, None, Some((40, 40)))
}

#[test]
fn model_text_must_reuse_the_numbers_and_directions_and_avoid_causes() {
    let parts = checked_parts();
    let good = "Doanh thu Tháng 11/2017 tăng 400 (100%) so với Tháng 10/2017.";
    assert_eq!(build_insight_check(good, &parts), Ok(()));
    assert!(build_insight_check("Doanh thu tăng 450 so với Tháng 10/2017.", &parts).is_err());
    assert!(
        build_insight_check("Doanh thu Tháng 11/2017 giảm 400 so với kỳ trước.", &parts).is_err()
    );
    assert!(build_insight_check("Doanh thu tăng 400 do khuyến mãi.", &parts).is_err());
    assert!(build_insight_check("Doanh thu tăng 400 (50%).", &parts).is_err());
    assert_eq!(find_causal_word("Phần tăng nằm ở Europe."), None);
    assert_eq!(
        find_causal_word("Tăng chủ yếu dẫn đến lỗ."),
        Some("dẫn đến")
    );
}

#[test]
fn insight_prompt_matches_the_fixture() {
    check_fixture(
        "insight_prompt.txt",
        &build_insight_prompt("Doanh thu theo tháng năm 2017", &checked_parts()),
    );
}

/// Mô hình giả trả một câu viết sẵn cho task insight, và một kết luận viết sẵn cho mô hình chấm (task judge).
struct Writer {
    text: String,
    verdict: &'static str,
}

const MATCH: &str = r#"{"verdict": "match", "problems": []}"#;
const UNCLEAR: &str = r#"{"verdict": "warning", "problems": ["Chữ \"gấp đôi\" chưa rõ"]}"#;
const WRONG: &str = r#"{"verdict": "mismatch", "problems": ["Lời nói ngược chiều"]}"#;

#[async_trait]
impl LlmProvider for Writer {
    fn name(&self) -> String {
        "writer".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        match request.task {
            LlmTask::Insight => Ok(self.text.clone()),
            LlmTask::Judge => Ok(self.verdict.to_string()),
            other => panic!("unexpected task {other:?}"),
        }
    }
}

#[tokio::test]
async fn narration_falls_back_to_code_text_when_the_model_fails_or_is_off() {
    let parts = checked_parts();
    // Mock không có lời viết sẵn cho task insight: dùng câu do code viết.
    let mock = Mock::new();
    let narrated = narrate_insight(
        &mock,
        "Doanh thu theo tháng",
        &parts,
        true,
        Duration::from_secs(5),
        &[],
    )
    .await;
    assert!(!narrated.by_model);
    assert!(narrated.summary.starts_with(&parts.title));
    let wrong = Writer {
        text: "Doanh thu tăng 999.".to_string(),
        verdict: MATCH,
    };
    assert!(
        !narrate_insight(&wrong, "q", &parts, true, Duration::from_secs(5), &[])
            .await
            .by_model
    );
    let right = Writer {
        text: "Tháng 11/2017 tăng 400 (100%) so với Tháng 10/2017.".to_string(),
        verdict: MATCH,
    };
    let narrated = narrate_insight(&right, "q", &parts, true, Duration::from_secs(5), &[]).await;
    assert!(narrated.by_model);
    assert_eq!(narrated.verdict, Some(AnswerVerdict::Match));
    assert!(
        !narrate_insight(&right, "q", &parts, false, Duration::from_secs(5), &[])
            .await
            .by_model
    );
}

/// Parquet tạm của order_items và orders: tháng 9 và tháng 10/2017 mỗi tháng 40 đơn First Class 10 đồng, tháng 11 có
/// 20 đơn Standard Class 30 đồng và 20 đơn First Class 10 đồng; mọi đơn ở Europe, cộng 40 đơn LATAM tháng 11 (bị lọc).
struct Orders {
    root: PathBuf,
    allowed: Vec<AllowedTable>,
}

impl Orders {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("xdi-insight-{}", uuid::Uuid::new_v4()));
        let conn = duckdb::Connection::open_in_memory().unwrap();
        let write = |name: &str, rows: &str| -> PathBuf {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("part-0001.parquet");
            conn.execute_batch(&format!(
                "COPY ({rows}) TO '{}' (FORMAT parquet)",
                file.display()
            ))
            .unwrap();
            dir
        };
        let orders = "SELECT CAST(i AS INTEGER) AS order_id, \
            CASE WHEN i < 40 THEN TIMESTAMP '2017-09-05' WHEN i < 80 THEN TIMESTAMP '2017-10-05' \
                 ELSE TIMESTAMP '2017-11-05' END AS order_date, \
            CASE WHEN i < 120 THEN 'Europe' ELSE 'LATAM' END AS market, \
            CASE WHEN i >= 80 AND i < 100 THEN 'Standard Class' ELSE 'First Class' END AS shipping_mode, \
            CAST(0 AS INTEGER) AS late FROM range(160) t(i)";
        let items = "SELECT CAST(i AS INTEGER) AS order_item_id, CAST(i AS INTEGER) AS order_id, \
            CAST(CASE WHEN i >= 80 AND i < 100 THEN 30 ELSE 10 END AS DECIMAL(18, 2)) AS sales FROM range(160) t(i)";
        let orders_dir = write("orders", orders);
        let items_dir = write("order_items", items);
        let (item_columns, order_columns) = order_tables();
        let allowed = vec![
            AllowedTable {
                name: "order_items".to_string(),
                columns: item_columns.iter().map(|c| c.name.clone()).collect(),
                parquet_dir: items_dir,
                row_filters: vec![],
            },
            AllowedTable {
                name: "orders".to_string(),
                columns: order_columns.iter().map(|c| c.name.clone()).collect(),
                parquet_dir: orders_dir,
                row_filters: vec![],
            },
        ];
        Orders { root, allowed }
    }

    fn env<'a>(&'a self, jobs: &'a Arc<Semaphore>) -> QueryEnv<'a> {
        QueryEnv {
            tables: &self.allowed,
            links: &[],
            max_rows: 1000,
            timeout: Duration::from_secs(10),
            jobs,
            stop_hint: ASK_STOP_HINT,
        }
    }

    /// Chạy câu của "mô hình" qua guard như luồng hỏi, rồi gắn vai trò cột như ask/view.rs.
    async fn run(
        &self,
        jobs: &Arc<Semaphore>,
        sql: &str,
        roles: &[ColumnRole],
    ) -> (ResultTable, String) {
        let CodeQuery::Ran {
            shown, mut result, ..
        } = run_code_query(&self.env(jobs), sql, None).await.unwrap()
        else {
            panic!("query did not run: {sql}");
        };
        for (column, role) in result.columns.iter_mut().zip(roles) {
            column.role = *role;
        }
        (result, shown)
    }
}

impl Drop for Orders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn run_insight(sql: &str, roles: &[ColumnRole]) -> crate::service::insight::InsightOutcome {
    let orders = Orders::new();
    let jobs = Arc::new(Semaphore::new(2));
    let (result, shown) = orders.run(&jobs, sql, roles).await;
    let (items, order_columns) = order_tables();
    let tables = [
        PlainTable {
            table: "order_items",
            columns: &items,
            row_count: Some(160),
        },
        PlainTable {
            table: "orders",
            columns: &order_columns,
            row_count: Some(160),
        },
    ];
    let links = order_links();
    let input = InsightInput {
        question: "Doanh thu theo tháng của Europe",
        result: &result,
        sql_generated: sql,
        sql_guarded: &shown,
        limit_kept: false,
        understanding: None,
        tables: &tables,
        links: &links,
        env: orders.env(&jobs),
        narrate: true,
        narrate_timeout: Duration::from_secs(5),
        lessons: &[],
    };
    build_insight(&Mock::new(), &input).await.unwrap().unwrap()
}

#[tokio::test]
async fn time_series_breaks_down_the_largest_step_by_a_related_column() {
    let outcome = run_insight(SERIES_SQL, &[ColumnRole::Time, ColumnRole::Measure]).await;
    let insight = &outcome.insight;
    assert_eq!(insight.kind, InsightKind::TimeSeries);
    assert!(insight.notable);
    assert_eq!(
        insight.title,
        "Tháng 11/2017 đổi mạnh nhất so với kỳ trước: tăng 400 (100%) so với Tháng 10/2017."
    );
    let explanation = outcome
        .explanation
        .expect("the largest step is broken down");
    assert_eq!(explanation.base.value, "400.00");
    assert_eq!(explanation.compare.value, "800.00");
    let dimension = &explanation.dimensions[0];
    assert_eq!(dimension.column, "orders.shipping_mode");
    let sum: Decimal = dimension
        .groups
        .iter()
        .map(|g| g.contribution.parse::<Decimal>().unwrap())
        .sum();
    assert_eq!(sum, Decimal::from(400));
    assert_eq!(
        insight.points.last().unwrap().text,
        "Standard Class theo orders.shipping_mode: tăng 600, trong khi chênh lệch chung tăng 400."
    );
    assert_eq!(outcome.tables_read, ["order_items", "orders"]);
}

#[tokio::test]
async fn single_value_is_compared_with_the_period_before() {
    let sql = "SELECT SUM(oi.sales) AS revenue FROM order_items oi JOIN orders o ON oi.order_id = o.order_id \
        WHERE o.market = 'Europe' AND year(o.order_date) = 2017 AND month(o.order_date) = 11";
    let outcome = run_insight(sql, &[ColumnRole::Measure]).await;
    let insight = &outcome.insight;
    assert_eq!(insight.kind, InsightKind::SingleValue);
    assert_eq!(
        insight.title,
        "revenue tháng 11/2017 là 800, tăng 400 (100%) so với tháng 10/2017."
    );
    assert!(outcome.explanation.is_some());
}

/// Khóa là tháng viết thành chữ: vai trò cột là dimension (ask/view.rs), nhưng phần giải thích vẫn đọc như chuỗi thời
/// gian và phân rã tháng đổi mạnh nhất, như câu thật Gemini viết trên database mẫu.
#[tokio::test]
async fn text_month_keys_are_read_as_a_time_series() {
    let sql = "SELECT strftime(o.order_date, '%Y-%m') AS month, SUM(oi.sales) AS revenue FROM order_items oi \
        JOIN orders o ON oi.order_id = o.order_id WHERE o.market = 'Europe' AND strftime(o.order_date, '%Y') = '2017' \
        GROUP BY 1 ORDER BY 1";
    let outcome = run_insight(sql, &[ColumnRole::Dimension, ColumnRole::Measure]).await;
    assert_eq!(outcome.insight.kind, InsightKind::TimeSeries);
    assert!(outcome
        .insight
        .title
        .starts_with("Tháng 11/2017 đổi mạnh nhất"));
    assert!(outcome.explanation.is_some());
}

/// Cổng 2 (#201): mô hình chấm nói sai ở mọi lần viết thì dùng câu của code, kết luận mismatch, ghi nhật ký lỗi; chỉ
/// thấy chỗ chưa rõ thì giữ lời của mô hình kèm các chỗ đó, kết luận warning.
#[tokio::test]
async fn judge_verdict_decides_whether_the_model_text_is_used() {
    let parts = checked_parts();
    let text = "Tháng 11/2017 tăng 400 (100%) so với Tháng 10/2017.".to_string();
    let wrong = Writer {
        text: text.clone(),
        verdict: WRONG,
    };
    let narrated = narrate_insight(&wrong, "q", &parts, true, Duration::from_secs(5), &[]).await;
    assert!(!narrated.by_model);
    assert_eq!(narrated.verdict, Some(AnswerVerdict::Mismatch));
    assert!(narrated.failure.is_some());
    assert!(narrated
        .step
        .detail
        .unwrap()
        .contains("Lời nói ngược chiều"));

    let unclear = Writer {
        text,
        verdict: UNCLEAR,
    };
    let narrated = narrate_insight(&unclear, "q", &parts, true, Duration::from_secs(5), &[]).await;
    assert!(narrated.by_model);
    assert_eq!(narrated.verdict, Some(AnswerVerdict::Warning));
    assert_eq!(narrated.warnings, ["Chữ \"gấp đôi\" chưa rõ"]);
}
