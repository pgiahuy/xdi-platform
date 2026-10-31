// SPDX-License-Identifier: Apache-2.0
/* service/explain/run_tests.rs - test trọn đường câu "tại sao" (explain/run.rs): form của mô hình, kiểm form, SQL qua
guard, chạy trên DuckDB thật với file Parquet tạm kiểu DataCo, phân rã, lời diễn giải. Mô hình là mock (câu mẫu) hoặc
mô hình giả trả câu viết sẵn; không cần Postgres.

Dữ liệu dựng để ra đúng các số của mẫu why (ask/examples.rs): quý 2/2017 và quý 3/2017 có mỗi ô Market x Customer
Segment một dòng mang cả số tiền của ô (bảng tính sẵn để cộng theo hàng ra đúng số của từng Market, theo cột ra đúng số
của từng Segment), cộng các dòng 0 đồng để số đơn phân biệt là 5000 và 4400. Một trăm đơn của quý 2 có hai dòng, để
Order Id là cột key lặp lại như DataCo thật. Quý 3/2016 có một dòng để thử cùng kỳ năm trước; quý 1/2017 không có dòng
nào để thử kỳ thủng. */

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use tokio::sync::Semaphore;

use crate::engine::guard::{AllowedTable, RowFilter};
use crate::engine::llm::mock::Mock;
use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::engine::store::{MetricDefinition, ProfileColumn};
use crate::error::AppError;
use crate::service::ask::context::{AmbiguousWord, AskContext};
use crate::service::ask::contract::{
    AskStatus, CellValue, ChartKind, ChosenOption, ColumnRole, DimensionBreakdown, MeasureKind,
    StepName, Understanding, UnderstoodItem, WarningCode,
};
use crate::service::ask::examples::build_why_example;
use crate::service::ask::prompt::PromptInput;
use crate::service::ask::values::ValueIndex;
use crate::service::ask::view::{build_answer_view, AnswerView, RanQuery};
use crate::service::explain::check_tests::{
    dataco_columns, dataco_values, metric, quarter_form, revenue, DATE, TABLE,
};
use crate::service::explain::run::{run_why, WhyInput, WhyOutcome};
use crate::service::explain::{build_number_check, build_template_summary};

const QUARTER_QUESTION: &str = "Tại sao doanh thu quý 3 năm 2017 giảm";

// Ô Market x Segment của hai quý: tổng theo Market và theo Segment đúng bằng các nhóm của mẫu why.
const BASE_CELLS: [(&str, &str, &str, &str); 7] = [
    ("Europe", "Consumer", "5/15/2017 10:00", "2850120.40"),
    ("LATAM", "Consumer", "5/15/2017 10:00", "2030079.75"),
    ("LATAM", "Corporate", "5/15/2017 10:00", "610220.35"),
    ("Pacific Asia", "Corporate", "5/15/2017 10:00", "2105430.25"),
    ("USCA", "Corporate", "5/15/2017 10:00", "134349.50"),
    ("USCA", "Home Office", "5/15/2017 10:00", "1076150.50"),
    // Tháng 4 thay vì tháng 5: để câu theo tháng có tháng trước để so.
    ("Africa", "Home Office", "4/10/2017 09:00", "596149.25"),
];
const COMPARE_CELLS: [(&str, &str, &str, &str); 7] = [
    ("Europe", "Consumer", "8/15/2017 10:00", "2210340.15"),
    ("LATAM", "Consumer", "8/15/2017 10:00", "1939760.35"),
    ("LATAM", "Corporate", "8/15/2017 10:00", "361490.25"),
    ("Pacific Asia", "Corporate", "8/15/2017 10:00", "1960120.00"),
    ("USCA", "Corporate", "8/15/2017 10:00", "158690.00"),
    ("USCA", "Home Office", "8/15/2017 10:00", "931540.00"),
    ("Africa", "Home Office", "8/15/2017 10:00", "557159.25"),
];
const ROWS: i64 = 9_501;

/// Thư mục tạm chứa Parquet của bảng dataco, tự xóa khi ra khỏi phạm vi.
struct Dataco {
    root: PathBuf,
    dir: PathBuf,
    columns: Vec<ProfileColumn>,
    values: ValueIndex,
    metrics: Vec<MetricDefinition>,
    context: AskContext,
    jobs: Arc<Semaphore>,
    rows: i64,
}

impl Dataco {
    fn new() -> Self {
        Self::with_rows(&[], 0)
    }

    /// Bảng như new() cộng các câu SELECT trong extra (cùng năm cột), thêm extra_rows dòng.
    fn with_rows(extra: &[String], extra_rows: i64) -> Self {
        let root = std::env::temp_dir().join(format!("xdi-why-{}", uuid::Uuid::new_v4()));
        let dir = root.join("ws").join("dataset").join("v1");
        std::fs::create_dir_all(&dir).unwrap();
        let cells: Vec<String> = BASE_CELLS
            .iter()
            .chain(COMPARE_CELLS.iter())
            .enumerate()
            .map(|(i, (market, segment, day, amount))| {
                let id = if i < 7 { i + 1 } else { 10_001 + i - 7 };
                format!("({id}, '{market}', '{segment}', '{day}', CAST({amount} AS DOUBLE))")
            })
            .collect();
        let zeros = |from: i64, until: i64, day: &str| {
            format!(
                "SELECT CAST(i AS INTEGER), 'Europe', 'Consumer', '{day}', CAST(0 AS DOUBLE) FROM range({from}, {until}) t(i)"
            )
        };
        let rows = format!(
            "SELECT * FROM (VALUES {}, (20001, 'Europe', 'Consumer', '8/1/2016 10:00', CAST(1000 AS DOUBLE))) \
             v(\"Order Id\", \"Market\", \"Customer Segment\", \"{DATE}\", \"Order Item Total\") \
             UNION ALL {} UNION ALL {} UNION ALL {}",
            cells.join(", "),
            zeros(8, 5_001, "6/1/2017 08:00"),
            zeros(1, 101, "6/2/2017 08:00"),
            zeros(10_008, 14_401, "9/1/2017 08:00"),
        );
        let rows = std::iter::once(rows)
            .chain(extra.iter().cloned())
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let file = dir.join("part-0001.parquet");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
        let count: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM '{}'", file.display()),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, ROWS + extra_rows);
        Dataco {
            root,
            dir,
            columns: dataco_columns("2016-08-01", "2017-09-01"),
            values: dataco_values(),
            metrics: vec![revenue()],
            context: AskContext {
                columns: Vec::new(),
                hints: Vec::new(),
                ambiguous: Vec::new(),
                picks: Vec::new(),
            },
            jobs: Arc::new(Semaphore::new(2)),
            rows: count,
        }
    }

    fn tables(&self, row_filters: Vec<RowFilter>) -> Vec<AllowedTable> {
        vec![AllowedTable {
            name: TABLE.to_string(),
            columns: self.columns.iter().map(|c| c.name.clone()).collect(),
            parquet_dir: self.dir.clone(),
            row_filters,
        }]
    }

    async fn ask(
        &self,
        llm: &dyn LlmProvider,
        question: &str,
        choices: &[ChosenOption],
    ) -> WhyOutcome {
        self.ask_as(llm, question, choices, Vec::new())
            .await
            .unwrap()
    }

    async fn ask_as(
        &self,
        llm: &dyn LlmProvider,
        question: &str,
        choices: &[ChosenOption],
        row_filters: Vec<RowFilter>,
    ) -> Result<WhyOutcome, AppError> {
        let tables = self.tables(row_filters);
        let input = WhyInput {
            prompt: PromptInput {
                table: TABLE,
                question,
                context: &self.context,
                metrics: &self.metrics,
                examples: &[],
                domain: None,
                choices,
                history: "",
                group: None,
                lessons: &[],
            },
            columns: &self.columns,
            values: &self.values,
            row_count: Some(self.rows),
            tables: &tables,
            max_rows: 100_000,
            timeout: Duration::from_secs(30),
            jobs: &self.jobs,
            narrate_timeout: Duration::from_secs(5),
        };
        run_why(llm, input).await
    }
}

impl Drop for Dataco {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Mô hình giả: trả lần lượt các câu viết sẵn, hết câu thì báo lỗi (bước narrate rơi về câu khuôn), ghi lại request.
struct ScriptedLlm {
    replies: Mutex<VecDeque<String>>,
    requests: Mutex<Vec<(LlmTask, String)>>,
}

impl ScriptedLlm {
    fn new(replies: Vec<serde_json::Value>) -> Self {
        let replies = replies.into_iter().map(|r| r.to_string()).collect();
        ScriptedLlm {
            replies: Mutex::new(replies),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn tasks(&self) -> Vec<LlmTask> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|(t, _)| *t)
            .collect()
    }

    fn prompt(&self, i: usize) -> String {
        self.requests.lock().unwrap()[i].1.clone()
    }
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> String {
        "scripted".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        self.requests
            .lock()
            .unwrap()
            .push((request.task, request.prompt.to_string()));
        let reply = self.replies.lock().unwrap().pop_front();
        reply.ok_or_else(|| AppError::Upstream("no more scripted replies".to_string()))
    }
}

fn steps(outcome: &WhyOutcome) -> Vec<(StepName, bool)> {
    outcome.steps.iter().map(|s| (s.name, s.ok)).collect()
}

fn groups(dimension: &DimensionBreakdown) -> Vec<(String, String, String, String)> {
    dimension
        .groups
        .iter()
        .map(|g| {
            let value = match &g.value {
                Some(CellValue::String(text)) => text.clone(),
                other => format!("{other:?}"),
            };
            (
                value,
                g.base_value.clone(),
                g.compare_value.clone(),
                g.contribution.clone(),
            )
        })
        .collect()
}

fn choose(choice_id: &str, option_id: &str) -> Vec<ChosenOption> {
    vec![ChosenOption {
        choice_id: choice_id.to_string(),
        option_id: option_id.to_string(),
    }]
}

/// Trọn chuỗi với mock: số của bảng, phân rã theo hai chiều và tách lượng, giá ra đúng như mẫu why.
#[tokio::test]
async fn sample_question_gives_the_sample_breakdown() {
    let data = Dataco::new();
    let outcome = data.ask(&Mock::new(), QUARTER_QUESTION, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Llm, true),
            (StepName::Guard, true),
            (StepName::Run, true),
            (StepName::Explain, true),
            (StepName::Narrate, true),
        ]
    );
    let result = outcome.result.as_ref().unwrap();
    assert_eq!(result.total_rows, 2);
    assert_eq!(result.columns[0].name, "Quý");
    assert!(outcome
        .sql_guarded
        .as_deref()
        .unwrap()
        .contains("LIMIT 100000"));

    let explanation = outcome.explanation.as_ref().unwrap();
    let example = build_why_example().explanation.unwrap();
    assert_eq!(explanation.measure, example.measure);
    for (got, want) in [
        (&explanation.base, &example.base),
        (&explanation.compare, &example.compare),
    ] {
        assert_eq!(
            (&got.label, &got.filter, &got.value),
            (&want.label, &want.filter, &want.value)
        );
    }
    assert_eq!(explanation.change, example.change);
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert_eq!(columns, ["Market", "Customer Segment"]);
    for (got, want) in explanation.dimensions.iter().zip(&example.dimensions) {
        assert_eq!(groups(got), groups(want), "{}", got.column);
        assert!(got.sql.ends_with("LIMIT 100000"), "{}", got.sql);
    }
    let split = explanation.volume_price.as_ref().unwrap();
    let want = example.volume_price.as_ref().unwrap();
    assert_eq!(
        (
            &split.base_volume,
            &split.compare_volume,
            &split.base_price,
            &split.compare_price,
            &split.volume_effect,
            &split.price_effect,
            &split.if_volume_unchanged
        ),
        (
            &want.base_volume,
            &want.compare_volume,
            &want.base_price,
            &want.compare_price,
            &want.volume_effect,
            &want.price_effect,
            &want.if_volume_unchanged
        )
    );
    assert_eq!(split.volume_label, "Số Order Id");
    // Lời của mock chỉ chép số có trong bảng nên qua bước kiểm số.
    assert!(explanation.summary_by_model, "{}", explanation.summary);

    // Form lưu được cùng câu hỏi; giả định kỳ so sánh đang ở kỳ liền trước.
    assert_eq!(outcome.form.as_ref().unwrap()["compare"], "2017-Q3");
    let base = outcome
        .assumptions
        .iter()
        .find(|c| c.id == "why:base")
        .unwrap();
    assert_eq!(base.selected.as_deref(), Some("previous_quarter"));
    assert!(outcome.warnings.is_empty());
}

/// Bảng tổng theo kỳ bằng đúng tổng các nhóm phân rã của mỗi chiều, cả khi số tiền có phần lẻ mà cộng số thực sẽ ra
/// khác: một nghìn dòng 0.1 (cộng DOUBLE ra 99.9999...), một nghìn dòng 0.004 (làm tròn ở từng dòng thành 0.00, cộng
/// số thực rồi mới làm tròn thì thành 4.00).
#[tokio::test]
async fn period_totals_equal_the_breakdown_sums() {
    let extra = |from: i64, market: &str, segment: &str, day: &str, amount: &str| {
        format!(
            "SELECT CAST(i AS INTEGER), '{market}', '{segment}', '{day}', CAST({amount} AS DOUBLE) \
             FROM range({from}, {}) t(i)",
            from + 1_000
        )
    };
    let data = Dataco::with_rows(
        &[
            extra(30_000, "Europe", "Consumer", "6/3/2017 08:00", "0.1"),
            extra(31_000, "LATAM", "Corporate", "9/2/2017 08:00", "0.004"),
            extra(32_000, "USCA", "Home Office", "9/3/2017 08:00", "0.1"),
        ],
        3_000,
    );
    let outcome = data.ask(&Mock::new(), QUARTER_QUESTION, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let totals: Vec<String> = outcome
        .result
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| match &row[1] {
            Some(CellValue::String(text)) => text.clone(),
            other => panic!("total cell is not a decimal string: {other:?}"),
        })
        .collect();
    assert_eq!(totals, ["9402600.00", "8119200.00"]);
    let explanation = outcome.explanation.as_ref().unwrap();
    assert_eq!(
        [&explanation.base.value, &explanation.compare.value],
        [&totals[0], &totals[1]]
    );
    let sum = |values: Vec<&String>| -> Decimal {
        values
            .into_iter()
            .map(|v| v.parse::<Decimal>().unwrap())
            .sum()
    };
    for dimension in &explanation.dimensions {
        let base = sum(dimension.groups.iter().map(|g| &g.base_value).collect());
        let compare = sum(dimension.groups.iter().map(|g| &g.compare_value).collect());
        assert_eq!(
            [base.to_string(), compare.to_string()],
            [totals[0].clone(), totals[1].clone()],
            "{}",
            dimension.column
        );
    }
}

/// Người hỏi đổi kỳ so sánh sang cùng kỳ năm trước: quý 3/2016.
#[tokio::test]
async fn base_choice_compares_with_last_year() {
    let data = Dataco::new();
    let choices = choose("why:base", "same_quarter_last_year");
    let outcome = data.ask(&Mock::new(), QUARTER_QUESTION, &choices).await;
    let explanation = outcome.explanation.unwrap();
    assert_eq!(explanation.base.label, "Quý 3/2016");
    assert_eq!(explanation.base.value, "1000.00");
    assert_eq!(explanation.change, "8118100.00");
    let base = outcome
        .assumptions
        .iter()
        .find(|c| c.id == "why:base")
        .unwrap();
    assert_eq!(base.selected.as_deref(), Some("same_quarter_last_year"));
}

#[tokio::test]
async fn month_question_compares_with_the_month_before() {
    let data = Dataco::new();
    let outcome = data
        .ask(&Mock::new(), "Vì sao doanh thu tháng 5 năm 2017 tăng", &[])
        .await;
    let explanation = outcome.explanation.unwrap();
    assert_eq!(
        (
            explanation.base.label.as_str(),
            explanation.compare.label.as_str()
        ),
        ("Tháng 4/2017", "Tháng 5/2017")
    );
    assert_eq!(explanation.base.value, "596149.25");
    assert_eq!(explanation.compare.value, "8806350.75");
    // Mock không có lời cho câu này: câu khuôn.
    assert!(!explanation.summary_by_model);
}

/// "Tại sao doanh thu Europe thấp hơn LATAM": hai nhóm của Market trên toàn bộ dữ liệu.
#[tokio::test]
async fn two_groups_are_compared() {
    let data = Dataco::new();
    let outcome = data
        .ask(&Mock::new(), "Tại sao doanh thu Europe thấp hơn LATAM", &[])
        .await;
    assert_eq!(outcome.status, AskStatus::Answered);
    let explanation = outcome.explanation.unwrap();
    assert_eq!(explanation.base.label, "LATAM");
    assert_eq!(explanation.base.value, "4941550.70");
    assert_eq!(explanation.compare.value, "5061460.55");
    let columns: Vec<&str> = explanation
        .dimensions
        .iter()
        .map(|d| d.column.as_str())
        .collect();
    assert_eq!(columns, ["Customer Segment"]);
    assert_eq!(outcome.result.unwrap().total_rows, 2);
    assert!(outcome.assumptions.iter().all(|c| c.id != "why:base"));
}

/// Hết đường câu "tại sao" (#186): Explanation có số dòng hai lát cắt, % thay đổi chung, % của từng nhóm, cột của hai
/// nhóm, và câu chính cùng các ý do code viết; so hai nhóm thì summary chính là lời đó và qua bước kiểm số.
#[tokio::test]
async fn two_groups_get_a_plain_summary_from_the_code() {
    let data = Dataco::new();
    let outcome = data
        .ask(&Mock::new(), "Tại sao doanh thu Europe thấp hơn LATAM", &[])
        .await;
    let explanation = outcome.explanation.unwrap();
    assert_eq!(explanation.slice_column.as_deref(), Some("Market"));
    assert!(explanation.base.rows.parse::<i64>().unwrap() > 0);
    assert!(explanation.compare.rows.parse::<i64>().unwrap() > 0);
    assert!(explanation.percent_change.is_some());
    let segments = &explanation.notes.dimensions[0];
    assert!(segments.groups.iter().all(|g| g.share_of_change.is_some()));
    assert!(
        explanation
            .summary_title
            .starts_with("Doanh thu của Market Europe cao hơn LATAM"),
        "{}",
        explanation.summary_title
    );
    assert!(!explanation.summary_points.is_empty());
    assert_eq!(explanation.summary, build_template_summary(&explanation));
    build_number_check(&explanation.summary, &explanation).unwrap();
}

/// Bộ lọc dòng theo vai dữ liệu của người hỏi nằm trong mọi câu, cả câu phân rã: người chỉ thấy Europe được phân rã
/// trên dòng Europe.
/// Thêm một dòng Europe, Corporate 0 đồng ở mỗi quý: chỉ thấy Europe thì Market còn một nhóm và Customer Segment cũng
/// chỉ còn Consumer, mà chiều ít hơn hai nhóm bị bỏ (explain/dimension_pick.rs), nên cần Segment có hai nhóm để còn câu
/// phân rã mà kiểm. Hai dòng 0 đồng không đổi tổng nào.
#[tokio::test]
async fn data_role_filter_reaches_the_breakdown_sql() {
    let corporate = |id: i64, day: &str| {
        format!("SELECT CAST({id} AS INTEGER), 'Europe', 'Corporate', '{day}', CAST(0 AS DOUBLE)")
    };
    let data = Dataco::with_rows(
        &[
            corporate(30_001, "6/3/2017 08:00"),
            corporate(30_002, "9/3/2017 08:00"),
        ],
        2,
    );
    let filter = RowFilter::Column {
        column: "Market".to_string(),
        allowed_values: vec!["Europe".to_string()],
    };
    let outcome = data
        .ask_as(&Mock::new(), QUARTER_QUESTION, &[], vec![filter])
        .await
        .unwrap();
    assert!(outcome
        .sql_guarded
        .unwrap()
        .contains("\"Market\" IN ('Europe')"));
    let explanation = outcome.explanation.unwrap();
    assert_eq!(explanation.base.value, "2850120.40");
    assert_eq!(explanation.compare.value, "2210340.15");
    assert!(!explanation.dimensions.is_empty());
    for dimension in &explanation.dimensions {
        assert!(
            dimension.sql.contains("\"Market\" IN ('Europe')"),
            "{}",
            dimension.sql
        );
        let values: Vec<String> = groups(dimension).into_iter().map(|g| g.0).collect();
        assert!(!values.contains(&"LATAM".to_string()), "{values:?}");
    }
    assert!(explanation
        .volume_price
        .unwrap()
        .sql
        .contains("\"Market\" IN ('Europe')"));
}

/// Form sai một lần: lỗi từng trường gửi lại, form sửa chạy bình thường.
#[tokio::test]
async fn wrong_form_is_fixed_once() {
    let data = Dataco::new();
    let mut wrong = quarter_form();
    wrong["filters"] = json!([{"column": "Market", "op": "=", "value": "Europe' OR 1=1 --"}]);
    wrong["time"]["column"] = json!("Order Date");
    let llm = ScriptedLlm::new(vec![wrong, quarter_form()]);
    let outcome = data.ask(&llm, QUARTER_QUESTION, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered);
    assert!(outcome.explanation.is_some());
    assert_eq!(
        steps(&outcome)[..3],
        [
            (StepName::Llm, true),
            (StepName::Repair, true),
            (StepName::Guard, true)
        ]
    );
    let repair = llm.prompt(1);
    assert!(
        repair.contains("- time.column: \"Order Date\" is not a column"),
        "{repair}"
    );
    assert!(
        repair.contains("- filters[0].value: \"Europe' OR 1=1 --\" is not a value"),
        "{repair}"
    );
    // Hai lần điền form, rồi narrate (hết câu, rơi về câu khuôn).
    assert_eq!(llm.tasks(), [LlmTask::Why, LlmTask::Why, LlmTask::Narrate]);
}

/// Kỳ liền trước nằm trong khoảng dữ liệu mà không có dòng nào: lỗi của form, mô hình chọn lại.
#[tokio::test]
async fn slice_without_rows_is_sent_back() {
    let data = Dataco::new();
    let mut empty = quarter_form();
    empty["compare"] = json!("2017-Q2");
    let llm = ScriptedLlm::new(vec![empty, quarter_form()]);
    let outcome = data.ask(&llm, QUARTER_QUESTION, &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered);
    assert_eq!(
        steps(&outcome)[..5],
        [
            (StepName::Llm, true),
            (StepName::Guard, true),
            (StepName::Run, true),
            (StepName::Explain, false),
            (StepName::Repair, true),
        ]
    );
    assert!(
        llm.prompt(1).contains("- base: no rows match 2017-Q1"),
        "{}",
        llm.prompt(1)
    );
    assert_eq!(outcome.explanation.unwrap().compare.label, "Quý 3/2017");
}

/// Form sai cả hai lần: chạy như câu thường, vẫn answered, kèm cảnh báo explanation_unavailable.
#[tokio::test]
async fn wrong_form_twice_runs_as_a_plain_question() {
    let data = Dataco::new();
    let mut wrong = quarter_form();
    wrong["metric"] = json!("lợi nhuận");
    let sql =
        "SELECT \"Market\", SUM(\"Order Item Total\") AS revenue FROM dataco GROUP BY \"Market\"";
    let llm = ScriptedLlm::new(vec![
        wrong.clone(),
        wrong,
        json!({"terms": [], "assumptions": [], "sql": sql}),
    ]);
    let outcome = data.ask(&llm, "tai sao doanh thu giam", &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered);
    assert_eq!(outcome.sql_generated.as_deref(), Some(sql));
    assert!(outcome.explanation.is_none() && outcome.form.is_none());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(
        outcome.warnings[0].code,
        WarningCode::ExplanationUnavailable
    );
    assert_eq!(
        steps(&outcome),
        [
            (StepName::Llm, true),
            (StepName::Repair, true),
            (StepName::Llm, true),
            (StepName::Guard, true),
            (StepName::Run, true),
        ]
    );
    assert_eq!(llm.tasks(), [LlmTask::Why, LlmTask::Why, LlmTask::Sql]);
    assert_eq!(outcome.result.unwrap().total_rows, 5);
}

/// Form không nêu độ đo: hỏi lại chỉ số nào, không chạy gì.
#[tokio::test]
async fn missing_measure_asks_which_metric() {
    let data = Dataco::new();
    let mut form = quarter_form();
    form["metric"] = serde_json::Value::Null;
    let llm = ScriptedLlm::new(vec![form]);
    let outcome = data.ask(&llm, "vì sao giảm", &[]).await;
    assert_eq!(outcome.status, AskStatus::NeedsChoice);
    let choice = &outcome.clarifications[0];
    assert_eq!(choice.id, "metric:default");
    assert_eq!(choice.options[0].id, "doanh thu");
    assert!(outcome.result.is_none());
    assert_eq!(steps(&outcome), [(StepName::Llm, true)]);
}

/// Cụm khớp hai cột gần nhau mà người hỏi chưa chọn: hỏi lại trước khi gọi mô hình, cùng id như #28.
#[tokio::test]
async fn ambiguous_word_asks_before_the_model() {
    let mut data = Dataco::new();
    data.context.ambiguous.push(AmbiguousWord {
        choice_id: "column:khu vuc".to_string(),
        phrase: "khu vực".to_string(),
        columns: vec!["Market".to_string(), "Customer Segment".to_string()],
    });
    let llm = ScriptedLlm::new(vec![]);
    let outcome = data.ask(&llm, "Tại sao doanh thu khu vực giảm", &[]).await;
    assert_eq!(outcome.status, AskStatus::NeedsChoice);
    assert_eq!(outcome.clarifications[0].id, "column:khu vuc");
    assert!(llm.tasks().is_empty());
}

/// Mô hình lỗi ngay lần đầu: lỗi của request (502), như câu thường.
#[tokio::test]
async fn model_error_before_any_form_is_an_error() {
    let data = Dataco::new();
    let llm = ScriptedLlm::new(vec![]);
    let error = data
        .ask_as(&llm, QUARTER_QUESTION, &[], Vec::new())
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Upstream(_)), "{error:?}");
}

/// Kỳ so sánh do người hỏi chọn mà không có dòng nào (quý 1/2017 thủng): báo ngay bằng cảnh báo, không gọi lại mô hình,
/// vì lựa chọn đè lên base của form và mô hình sửa form cũng không đổi được kỳ đó.
#[tokio::test]
async fn chosen_base_without_rows_is_reported_at_once() {
    let data = Dataco::new();
    let llm = ScriptedLlm::new(vec![quarter_form()]);
    let choices = choose("why:base", "2017-Q1");
    let outcome = data.ask(&llm, QUARTER_QUESTION, &choices).await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    assert!(outcome.explanation.is_none());
    assert!(outcome.result.is_some() && outcome.form.is_some());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(
        outcome.warnings[0].code,
        WarningCode::ExplanationUnavailable
    );
    assert!(
        outcome.warnings[0].message.starts_with("Quý 1/2017"),
        "{}",
        outcome.warnings[0].message
    );
    assert_eq!(llm.tasks(), [LlmTask::Why]);
    assert_eq!(steps(&outcome).last(), Some(&(StepName::Explain, false)));
    // Giả định kỳ so sánh vẫn trên thẻ, đang ở kỳ đã chọn, để người hỏi đổi.
    let base = outcome
        .assumptions
        .iter()
        .find(|c| c.id == "why:base")
        .unwrap();
    assert_eq!(base.selected.as_deref(), Some("2017-Q1"));
}

fn view(outcome: &WhyOutcome, metrics: &[MetricDefinition]) -> AnswerView {
    let ran = RanQuery {
        status: outcome.status,
        sql_guarded: outcome.sql_guarded.as_deref(),
        result: outcome.result.as_ref(),
        limit_kept: outcome.limit_kept,
        row_filters: &outcome.row_filters,
        plan: outcome.plan.as_ref(),
    };
    build_answer_view(&ran, &[], metrics, &outcome.assumptions).unwrap()
}

fn items(understanding: &Understanding) -> Vec<&UnderstoodItem> {
    let lists = [
        &understanding.measures,
        &understanding.filters,
        &understanding.group_by,
        &understanding.order_by,
    ];
    lists
        .into_iter()
        .flatten()
        .chain(understanding.time_range.as_ref())
        .collect()
}

/// Thẻ "Đã hiểu là" dựng từ form: hai kỳ là khoảng thời gian, mỗi đoạn SQL là đoạn nguyên văn của sql_guarded; bảng tổng
/// theo quý vẽ đường.
#[tokio::test]
async fn why_card_names_the_two_periods() {
    let data = Dataco::new();
    let outcome = data.ask(&Mock::new(), QUARTER_QUESTION, &[]).await;
    let view = view(&outcome, &data.metrics);
    let sql = outcome.sql_guarded.as_deref().unwrap();
    for item in items(&view.understanding) {
        assert!(sql.contains(&item.sql), "{} not in {sql}", item.sql);
    }
    let card = &view.understanding;
    let period = card.time_range.as_ref().unwrap();
    assert_eq!(
        period.label,
        format!("Quý 3/2017 so với quý 2/2017, theo {DATE}")
    );
    assert!(period.sql.starts_with('(') && period.sql.contains(" OR "));
    // Chỉ số không có mô tả: dòng "Đo" ghi cách tính bằng lời (#185).
    assert_eq!(
        card.measures[0].label,
        "doanh thu: tổng cột Order Item Total"
    );
    assert_eq!(
        card.measures[0].sql,
        "SUM(CAST(\"Order Item Total\" AS DECIMAL(18,2)))"
    );
    assert!(card.filters.is_empty() && card.limit.is_none());
    assert_eq!(card.group_by.len(), 1);
    assert_eq!(card.assumptions[0].id, "why:base");
    assert_eq!(view.roles, [ColumnRole::Time, ColumnRole::Measure]);
    let chart = view.chart.unwrap();
    // Hai quý, tới 10 kỳ là cột đứng (ask/chart.rs, luật 2).
    assert_eq!((chart.kind, chart.x.as_str()), (ChartKind::Column, "Quý"));
}

/// So hai nhóm: hai nhóm là một mục lọc có nhãn riêng; điều kiện phân quyền ở cuối; bảng theo Market vẽ thanh ngang.
#[tokio::test]
async fn why_card_names_the_two_groups() {
    let data = Dataco::new();
    let filter = RowFilter::Column {
        column: "Customer Segment".to_string(),
        allowed_values: vec!["Consumer".to_string(), "Corporate".to_string()],
    };
    let question = "Tại sao doanh thu Europe thấp hơn LATAM";
    let outcome = data
        .ask_as(&Mock::new(), question, &[], vec![filter])
        .await
        .unwrap();
    let view = view(&outcome, &data.metrics);
    let sql = outcome.sql_guarded.as_deref().unwrap();
    for item in items(&view.understanding) {
        assert!(sql.contains(&item.sql), "{} not in {sql}", item.sql);
    }
    let labels: Vec<&str> = view
        .understanding
        .filters
        .iter()
        .map(|f| f.label.as_str())
        .collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert_eq!(labels[0], "Market: Europe so với LATAM");
    assert!(labels[1].starts_with("Phân quyền"), "{labels:?}");
    assert!(view.understanding.time_range.is_none());
    assert_eq!(view.roles, [ColumnRole::Dimension, ColumnRole::Measure]);
    assert_eq!(view.chart.unwrap().kind, ChartKind::Bar);
}

/// Độ đo tỷ lệ đi trọn đường: tử và mẫu theo từng lát cắt và từng chiều, ba số lớn là tỷ lệ bốn chữ số, mỗi chiều tách
/// cơ cấu và hiệu suất cộng ra đúng chênh lệch, câu khuôn qua được bước kiểm số. Quý 2/2017 có 7 dòng có tiền trên 5100
/// dòng (0.00137... làm tròn 0.0014), quý 3/2017 có 7 trên 4400 (0.00159... làm tròn 0.0016).
#[tokio::test]
async fn ratio_metric_splits_into_mix_and_rate() {
    let mut data = Dataco::new();
    let name = "tỷ lệ dòng có tiền";
    data.metrics.push(metric(
        name,
        "COUNT(CASE WHEN \"Order Item Total\" > 0 THEN 1 END) / COUNT(*)",
        None,
    ));
    let mut form = quarter_form();
    form["metric"] = json!(name);
    form["terms"] = json!([{"text": name, "kind": "metric", "value": name}]);
    let llm = ScriptedLlm::new(vec![form]);
    let outcome = data
        .ask(&llm, "Tại sao tỷ lệ dòng có tiền quý 3 năm 2017 tăng", &[])
        .await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let explanation = outcome.explanation.as_ref().expect("explanation");
    assert_eq!(explanation.measure_kind, MeasureKind::Ratio);
    assert_eq!(
        (
            explanation.base.value.as_str(),
            explanation.compare.value.as_str(),
            explanation.change.as_str()
        ),
        ("0.0014", "0.0016", "0.0002")
    );
    assert!(explanation.volume_price.is_none());
    assert!(!explanation.dimensions.is_empty());
    let change = Decimal::from_str_exact(&explanation.change).unwrap();
    for dimension in &explanation.dimensions {
        assert!(
            dimension.sql.contains("compare_denominator"),
            "{}",
            dimension.sql
        );
        let parse =
            |text: &Option<String>| Decimal::from_str_exact(text.as_deref().unwrap()).unwrap();
        assert_eq!(
            parse(&dimension.mix_effect) + parse(&dimension.rate_effect),
            change,
            "{}",
            dimension.column
        );
        let sum: Decimal = dimension
            .groups
            .iter()
            .map(|g| Decimal::from_str_exact(&g.contribution).unwrap())
            .sum();
        assert_eq!(sum, change, "{}", dimension.column);
    }
    // Bảng tổng theo kỳ làm tròn như phân rã.
    let result = outcome.result.as_ref().unwrap();
    assert_eq!(result.rows.len(), 2);
    // Mô hình giả hết câu ở bước narrate nên lời là câu khuôn; câu khuôn qua bước kiểm số.
    assert!(!explanation.summary_by_model);
    assert!(
        explanation.summary.contains("phần do cơ cấu"),
        "{}",
        explanation.summary
    );
    assert_eq!(
        build_number_check(&explanation.summary, explanation),
        Ok(()),
        "{}",
        explanation.summary
    );
    assert!(llm.prompt(1).contains("\"measure_kind\": \"ratio\""));
}

/// Form của câu "Tại sao doanh thu giảm": so hai kỳ theo quý, không nêu kỳ.
fn no_period_form() -> serde_json::Value {
    json!({
        "terms": [{"text": "doanh thu", "kind": "metric", "value": "doanh thu"}],
        "metric": "doanh thu",
        "time": {"column": DATE, "grain": "quarter"},
        "compare": "",
        "filters": []
    })
}

fn compare_choice(outcome: &WhyOutcome) -> &crate::service::ask::contract::Choice {
    outcome
        .assumptions
        .iter()
        .find(|c| c.id == "why:compare")
        .unwrap()
}

fn compared(outcome: &WhyOutcome) -> (String, String) {
    let explanation = outcome.explanation.as_ref().unwrap();
    (
        explanation.compare.label.clone(),
        explanation.base.label.clone(),
    )
}

/// Câu không nêu kỳ: code chọn quý gần nhất có dữ liệu (quý 3/2017) và so với quý liền trước, vẫn có phân rã.
#[tokio::test]
async fn question_without_period_explains_the_latest_period() {
    let data = Dataco::new();
    let llm = ScriptedLlm::new(vec![no_period_form()]);
    let outcome = data.ask(&llm, "Tại sao doanh thu giảm", &[]).await;
    assert_eq!(outcome.status, AskStatus::Answered);
    assert_eq!(
        compared(&outcome),
        ("Quý 3/2017".to_string(), "Quý 2/2017".to_string())
    );
    let choice = compare_choice(&outcome);
    assert_eq!(
        choice.label,
        "Kỳ gần nhất có dữ liệu: Quý 3/2017 so với Quý 2/2017"
    );
    assert_eq!(choice.selected.as_deref(), Some("2017-Q3"));
    let options: Vec<&str> = choice.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(options, ["2017-Q3", "2017-Q2", "2016-Q3"]);
    // Một lượt gọi mô hình cho form, không vòng sửa.
    assert_eq!(llm.tasks()[0], LlmTask::Why);
    assert!(!llm.tasks().contains(&LlmTask::Repair));
    let form = outcome.form.unwrap();
    assert_eq!(form["compare"], "2017-Q3");
}

/// Mười dòng ở quý 4/2017, ít hơn hẳn 4400 dòng của quý 3: dữ liệu dừng giữa quý, code lùi về quý 3.
#[tokio::test]
async fn partial_last_period_is_skipped() {
    let extra = vec!["SELECT CAST(i AS INTEGER), 'Europe', 'Consumer', '10/2/2017 08:00', CAST(5 AS DOUBLE) FROM range(30001, 30011) t(i)".to_string()];
    let mut data = Dataco::with_rows(&extra, 10);
    data.columns = dataco_columns("2016-08-01", "2017-10-02");
    let llm = ScriptedLlm::new(vec![no_period_form()]);
    let outcome = data.ask(&llm, "Tại sao doanh thu giảm", &[]).await;
    assert_eq!(
        compared(&outcome),
        ("Quý 3/2017".to_string(), "Quý 2/2017".to_string())
    );
    let options: Vec<String> = compare_choice(&outcome)
        .options
        .iter()
        .map(|o| o.id.clone())
        .collect();
    assert_eq!(options[0], "2017-Q4");
}

/// Người hỏi chọn chip quý 4/2017: code dùng kỳ đó, nhãn đổi sang "Kỳ đã chọn".
#[tokio::test]
async fn chosen_compare_period_is_used() {
    let extra = vec!["SELECT CAST(i AS INTEGER), 'Europe', 'Consumer', '10/2/2017 08:00', CAST(5 AS DOUBLE) FROM range(30001, 30011) t(i)".to_string()];
    let mut data = Dataco::with_rows(&extra, 10);
    data.columns = dataco_columns("2016-08-01", "2017-10-02");
    let llm = ScriptedLlm::new(vec![no_period_form()]);
    let choices = choose("why:compare", "2017-Q4");
    let outcome = data.ask(&llm, "Tại sao doanh thu giảm", &choices).await;
    assert_eq!(
        compared(&outcome),
        ("Quý 4/2017".to_string(), "Quý 3/2017".to_string())
    );
    let choice = compare_choice(&outcome);
    assert_eq!(choice.label, "Kỳ đã chọn: Quý 4/2017 so với Quý 3/2017");
    assert_eq!(choice.selected.as_deref(), Some("2017-Q4"));
}

/// Câu nêu kỳ giữ hành vi cũ: không có lựa chọn why:compare.
#[tokio::test]
async fn question_with_period_has_no_compare_choice() {
    let data = Dataco::new();
    let outcome = data.ask(&Mock::new(), QUARTER_QUESTION, &[]).await;
    assert_eq!(
        compared(&outcome),
        ("Quý 3/2017".to_string(), "Quý 2/2017".to_string())
    );
    assert!(outcome.assumptions.iter().all(|c| c.id != "why:compare"));
}
