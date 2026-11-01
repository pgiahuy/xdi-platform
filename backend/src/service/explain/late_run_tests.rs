// SPDX-License-Identifier: Apache-2.0
/* service/explain/late_run_tests.rs - test trọn đường câu hỏi nguyên nhân (explain/run.rs) trên một bảng nhỏ dựng theo
nguyên nhân cài sẵn của sales_demo (data/postgres-demo/README.md): tỷ lệ giao trễ tháng 11/2016 tăng vì các đơn Standard
Class của Pacific Asia gần như đều trễ. Đã gặp khi chạy thật: lời chỉ nêu Standard Class, không nêu Pacific Asia; tỷ lệ
in "0,1024" thay cho điểm phần trăm; cột tình trạng giao (trễ đúng khi cờ trễ bật) chiếm chỗ thứ hai của phân rã.

Dữ liệu: mỗi ô (cách giao, thị trường) mười đơn mỗi tháng. Tháng 10: Standard Class trễ 4/10, First Class trễ 9/10 ở cả
hai thị trường. Tháng 11: như tháng 10, trừ Standard Class của Pacific Asia trễ 10/10. Tỷ lệ chung 26/40 = 65% lên 32/40
= 80%, tăng 15 điểm phần trăm; cả phần tăng nằm ở Standard Class và ở Pacific Asia. Mô hình giả chỉ trả form, bước viết lời
dùng câu khuôn; không cần Postgres. */

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::engine::guard::AllowedTable;
use crate::engine::store::{ProfileColumn, ValueRow};
use crate::service::ask::context::AskContext;
use crate::service::ask::contract::{AskStatus, Explanation};
use crate::service::ask::prompt::PromptInput;
use crate::service::ask::values::ValueIndex;
use crate::service::explain::build_number_check;
use crate::service::explain::check_tests::column;
use crate::service::explain::claims::{build_claim_check, Claim, ClaimField};
use crate::service::explain::joined::NO_JOINED;
use crate::service::explain::run::{run_why, WhyInput, WhyOutcome};
use crate::service::explain::run_tests::ScriptedLlm;
use crate::service::explain::source::NO_RELATED;

const TABLE: &str = "orders";

struct Late {
    root: PathBuf,
    dir: PathBuf,
    columns: Vec<ProfileColumn>,
    values: ValueIndex,
}

impl Late {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("xdi-late-{}", Uuid::new_v4()));
        let dir = root.join("ws").join("dataset").join("v1");
        std::fs::create_dir_all(&dir).unwrap();
        // Mỗi ô mười dòng; i < số đơn trễ thì trễ. Tình trạng giao đi đúng theo cờ trễ.
        let mut cells = Vec::new();
        for (month, day) in [("10", "2016-10-15"), ("11", "2016-11-15")] {
            for (mode, market, late) in [
                (
                    "Standard Class",
                    "Pacific Asia",
                    if month == "11" { 10 } else { 4 },
                ),
                ("Standard Class", "Europe", 4),
                ("First Class", "Pacific Asia", 9),
                ("First Class", "Europe", 9),
            ] {
                cells.push(format!(
                    "SELECT TIMESTAMP '{day}' AS order_date, '{mode}' AS shipping_mode, '{market}' AS market, \
                     CASE WHEN i < {late} THEN 'Late delivery' ELSE 'Shipping on time' END AS delivery_status, \
                     CASE WHEN i < {late} THEN 'true' ELSE 'false' END AS late_delivery_risk FROM range(10) t(i)"
                ));
            }
        }
        let file = dir.join("part-0001.parquet");
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "COPY ({}) TO '{}' (FORMAT parquet)",
            cells.join(" UNION ALL "),
            file.display()
        ))
        .unwrap();
        let mut date = column(1, "order_date", "TIMESTAMP", "time", 2);
        date.min_value = Some("2016-10-15 00:00:00".to_string());
        date.max_value = Some("2016-11-15 00:00:00".to_string());
        let mut mode = column(2, "shipping_mode", "VARCHAR", "dimension", 2);
        mode.aliases = vec!["cách giao".to_string()];
        let mut market = column(3, "market", "VARCHAR", "dimension", 2);
        market.aliases = vec!["thị trường".to_string()];
        let mut status = column(4, "delivery_status", "VARCHAR", "dimension", 2);
        status.aliases = vec!["tình trạng giao".to_string()];
        let late = column(5, "late_delivery_risk", "VARCHAR", "dimension", 2);
        let rows: Vec<ValueRow> = ["true", "false"]
            .iter()
            .map(|v| ValueRow {
                column_id: Uuid::from_u128(5),
                column_name: "late_delivery_risk".to_string(),
                value: v.to_string(),
                aliases: Vec::new(),
            })
            .collect();
        Late {
            root,
            dir,
            columns: vec![date, mode, market, status, late],
            values: ValueIndex::build(&rows),
        }
    }

    async fn ask(&self, question: &str) -> WhyOutcome {
        let form = json!({
            "terms": [],
            "measure": {"column": "late_delivery_risk", "agg": "share", "value": "true"},
            "time": {"column": "order_date", "grain": "month"},
            "compare": "2016-11",
            "base": "2016-10",
            "filters": []
        });
        let llm = ScriptedLlm::new(vec![form]);
        let context = AskContext {
            columns: Vec::new(),
            hints: Vec::new(),
            picks: Vec::new(),
        };
        let tables = vec![AllowedTable {
            name: TABLE.to_string(),
            columns: self.columns.iter().map(|c| c.name.clone()).collect(),
            parquet_dir: self.dir.clone(),
            row_filters: Vec::new(),
        }];
        let jobs = Arc::new(Semaphore::new(2));
        let input = WhyInput {
            prompt: PromptInput {
                table: TABLE,
                question,
                context: &context,
                metrics: &[],
                examples: &[],
                domain: None,
                choices: &[],
                history: "",
                group: None,
                lessons: &[],
            },
            columns: &self.columns,
            values: &self.values,
            row_count: Some(80),
            tables: &tables,
            links: &[],
            max_rows: 100_000,
            timeout: Duration::from_secs(30),
            jobs: &jobs,
            narrate_timeout: Duration::from_secs(5),
            related: &NO_RELATED,
            joined: &NO_JOINED,
        };
        run_why(&llm, input).await.unwrap()
    }
}

impl Drop for Late {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn columns(e: &Explanation) -> Vec<&str> {
    e.dimensions.iter().map(|d| d.column.as_str()).collect()
}

/// Tình trạng giao chỉ nói lại cờ trễ nên bị bỏ; câu khuôn nêu cả Standard Class lẫn Pacific Asia, viết tỷ lệ bằng phần
/// trăm và chênh lệch bằng điểm phần trăm, và qua bước kiểm số.
#[tokio::test]
async fn both_planted_groups_are_named_in_percentage_points() {
    let data = Late::new();
    let outcome = data
        .ask("Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?")
        .await;
    assert_eq!(outcome.status, AskStatus::Answered, "{outcome:?}");
    let e = outcome.explanation.as_ref().expect("a breakdown");
    assert_eq!(
        (e.base.value.as_str(), e.compare.value.as_str()),
        ("0.6500", "0.8000")
    );
    assert!(!columns(e).contains(&"delivery_status"), "{:?}", columns(e));
    assert!(e.notes.fraction);
    let summary = &e.summary;
    assert!(summary.contains("Standard Class"), "{summary}");
    assert!(summary.contains("Pacific Asia"), "{summary}");
    assert!(
        summary.contains("tăng 15,00 điểm phần trăm") && summary.contains("từ 65,00% lên 80,00%"),
        "{summary}"
    );
    assert!(!summary.contains("0,15"), "{summary}");
    build_number_check(summary, e).unwrap();
}

/// Cổng 1: lời của mô hình nêu Standard Class mà bỏ Pacific Asia thì trượt, lý do nêu đúng nhóm còn thiếu.
#[tokio::test]
async fn a_reply_that_skips_a_strong_group_fails_the_claim_check() {
    let data = Late::new();
    let outcome = data
        .ask("Vì sao tỷ lệ giao trễ tháng 11/2016 tăng so với tháng 10/2016?")
        .await;
    let e = outcome.explanation.as_ref().expect("a breakdown");
    let cause = |column: &str, group: &str| Claim {
        field: ClaimField::Cause,
        column: column.to_string(),
        group: group.to_string(),
        value: String::new(),
    };
    let only_mode = [cause("shipping_mode", "Standard Class")];
    let problems = build_claim_check(&only_mode, e).unwrap_err();
    assert!(
        problems.iter().any(|p| p.contains("Pacific Asia")),
        "{problems:?}"
    );
    let both = [
        cause("shipping_mode", "Standard Class"),
        cause("market", "Pacific Asia"),
    ];
    assert_eq!(build_claim_check(&both, e), Ok(2));
}
