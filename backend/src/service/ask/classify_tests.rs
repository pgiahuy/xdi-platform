// SPDX-License-Identifier: Apache-2.0
/* service/ask/classify_tests.rs - test bước phân loại câu hỏi (ask/classify.rs): prompt so với file mẫu
service/testdata/classify_prompt.txt, prompt không có giá trị của cột, câu trả lời của mô hình quyết định loại câu,
và mọi cách mô hình hỏng (lỗi, quá giờ, JSON không đọc được) đều rơi về luật từ khóa với bước ok = false. Mô hình giả,
không cần Postgres. */

use std::time::Duration;

use async_trait::async_trait;
use uuid::Uuid;

use crate::engine::llm::{LlmProvider, LlmRequest, LlmTask};
use crate::error::AppError;
use crate::service::ask::classify::{
    build_classify_prompt, build_classify_timeout, classify, ClassifyInput, MAX_REASON_CHARS,
};
use crate::service::ask::context::ColumnProfile;
use crate::service::ask::contract::StepName;
use crate::service::ask::prompt::PROMPT_MAX_CHARS;
use crate::service::ask::values::ValueEntry;
use crate::service::explain::form::QuestionKind;
use crate::service::prompt_fixtures::check_fixture;

const TIMEOUT: Duration = Duration::from_secs(5);

fn column(name: &str, aliases: &[&str], description: Option<&str>) -> ColumnProfile {
    ColumnProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: "dimension".to_string(),
        time_format: None,
        description: description.map(str::to_string),
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: false,
        unit: None,
        aggregation: crate::engine::store::ColumnAggregation::Sum,
        join_key: false,
    }
}

/// Ba cột, trong đó một cột tên "lý do hủy": mô hình phải thấy nó là tên cột. Cột chiều mang giá trị, để test kiểm giá
/// trị không lọt vào prompt.
fn columns() -> Vec<ColumnProfile> {
    let mut market = column(
        "Market",
        &["thị trường"],
        Some("Thị trường nơi giao đơn hàng"),
    );
    market.values = vec![ValueEntry {
        value: "Europe".to_string(),
        aliases: vec!["châu Âu".to_string()],
    }];
    vec![
        market,
        column("Cancel Reason", &["lý do hủy"], Some("Lý do khách hủy đơn")),
        column("Order Item Total", &["doanh thu"], None),
    ]
}

/// Mô hình giả: trả một câu cố định hay một lỗi, và kiểm loại việc cùng schema của request.
struct StubLlm(Result<&'static str, &'static str>);

#[async_trait]
impl LlmProvider for StubLlm {
    fn name(&self) -> String {
        "stub".to_string()
    }

    async fn complete(&self, request: &LlmRequest<'_>) -> Result<String, AppError> {
        assert_eq!(request.task, LlmTask::Classify);
        assert!(request.json_schema.is_some());
        match self.0 {
            Ok(text) => Ok(text.to_string()),
            Err(text) => Err(AppError::Upstream(text.to_string())),
        }
    }
}

/// Mô hình không bao giờ trả lời xong, để thử nhánh quá giờ.
struct SlowLlm;

#[async_trait]
impl LlmProvider for SlowLlm {
    fn name(&self) -> String {
        "slow".to_string()
    }

    async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok("{}".to_string())
    }
}

async fn run(llm: &dyn LlmProvider, question: &str) -> (QuestionKind, bool, bool, String) {
    let columns = columns();
    let input = ClassifyInput {
        table: "dataco",
        question,
        columns: &columns,
        earlier: &[],
    };
    let classified = classify(llm, &input, TIMEOUT).await;
    assert_eq!(classified.step.name, StepName::Classify);
    let detail = classified.step.detail.unwrap_or_default();
    (
        classified.kind,
        classified.by_model,
        classified.step.ok,
        detail,
    )
}

#[test]
fn classify_prompt_matches_the_fixture() {
    let columns = columns();
    let earlier = vec!["Vì sao doanh thu tháng 5 năm 2017 giảm?".to_string()];
    let input = ClassifyInput {
        table: "dataco",
        question: "Còn tháng 6 thì sao?",
        columns: &columns,
        earlier: &earlier,
    };
    check_fixture("classify_prompt.txt", &build_classify_prompt(&input));
}

#[test]
fn prompt_has_column_names_and_aliases_but_no_values() {
    let columns = columns();
    let input = ClassifyInput {
        table: "dataco",
        question: "Số đơn theo lý do hủy",
        columns: &columns,
        earlier: &[],
    };
    let prompt = build_classify_prompt(&input);
    assert!(
        prompt.contains("\"Cancel Reason\" (aliases: lý do hủy)"),
        "{prompt}"
    );
    assert!(!prompt.contains("Europe"), "{prompt}");
    assert!(!prompt.contains("châu Âu"), "{prompt}");
    assert!(prompt.contains("Earlier questions:\n(none)"), "{prompt}");
}

/// Câu bắt nhầm của luật từ khóa: mô hình nói plain thì đi đường câu thường.
#[tokio::test]
async fn model_answer_decides_the_kind() {
    let reply = r#"{"kind": "plain", "reason": "Câu hỏi đếm đơn theo cột lý do hủy."}"#;
    let (kind, by_model, ok, detail) = run(&StubLlm(Ok(reply)), "Số đơn theo lý do hủy").await;
    assert_eq!(kind, QuestionKind::Plain);
    assert!(by_model && ok);
    assert_eq!(
        detail,
        "Câu hỏi thường: Câu hỏi đếm đơn theo cột lý do hủy."
    );

    // Câu bỏ sót của luật từ khóa; câu trả lời bọc trong khối ```json vẫn đọc được.
    let reply =
        "```json\n{\"kind\": \"cause\", \"reason\": \"Hỏi điều gì làm doanh thu giảm.\"}\n```";
    let (kind, by_model, ok, _) = run(&StubLlm(Ok(reply)), "Doanh thu giảm do gì?").await;
    assert_eq!(kind, QuestionKind::Why);
    assert!(by_model && ok);
}

#[tokio::test]
async fn long_reason_is_cut() {
    let long = format!(r#"{{"kind": "plain", "reason": "{}"}}"#, "a".repeat(500));
    let reply: &'static str = Box::leak(long.into_boxed_str());
    let (_, _, _, detail) = run(&StubLlm(Ok(reply)), "Doanh thu theo tháng").await;
    let reason = detail.strip_prefix("Câu hỏi thường: ").unwrap();
    assert_eq!(reason.chars().count(), MAX_REASON_CHARS);
}

#[tokio::test]
async fn model_error_falls_back_to_keywords() {
    let (kind, by_model, ok, detail) = run(
        &StubLlm(Err("Gemini đang quá tải")),
        "Tại sao doanh thu giảm",
    )
    .await;
    assert_eq!(kind, QuestionKind::Why);
    assert!(!by_model && !ok);
    assert!(
        detail.starts_with("Dùng luật từ khóa vì mô hình lỗi (Gemini đang quá tải)"),
        "{detail}"
    );
    assert!(
        detail.ends_with("Kết quả: câu hỏi tìm nguyên nhân."),
        "{detail}"
    );
}

#[tokio::test]
async fn unreadable_reply_falls_back_to_keywords() {
    for reply in [
        "không phải JSON",
        r#"{"kind": "maybe", "reason": "?"}"#,
        r#"{"reason": "x"}"#,
    ] {
        let (kind, by_model, ok, detail) = run(&StubLlm(Ok(reply)), "Doanh thu theo tháng").await;
        assert_eq!(kind, QuestionKind::Plain, "{reply}");
        assert!(!by_model && !ok, "{reply}");
        assert!(detail.contains("không đọc được"), "{detail}");
    }
}

#[tokio::test]
async fn timeout_falls_back_to_keywords() {
    let columns = columns();
    let input = ClassifyInput {
        table: "dataco",
        question: "Vì sao lợi nhuận giảm",
        columns: &columns,
        earlier: &[],
    };
    let classified = classify(&SlowLlm, &input, Duration::from_millis(10)).await;
    assert_eq!(classified.kind, QuestionKind::Why);
    assert!(!classified.by_model && !classified.step.ok);
    let detail = classified.step.detail.unwrap_or_default();
    assert!(
        detail.contains("không trả lời trong 10 mili giây"),
        "{detail}"
    );
}

/// Trần của bước phân loại là CLASSIFY_TIMEOUT_SECS, không phải timeout của nhà cung cấp, và không bao giờ dài hơn
/// timeout đó.
#[test]
fn classify_timeout_is_its_own_short_limit() {
    let mut config = crate::config::Config::from_env();
    config.llm_provider = "ollama".to_string();
    config.ollama_timeout = Duration::from_secs(300);
    config.classify_timeout = Duration::from_secs(15);
    assert_eq!(build_classify_timeout(&config), Duration::from_secs(15));

    config.llm_provider = "gemini".to_string();
    config.gemini_timeout = Duration::from_secs(10);
    assert_eq!(build_classify_timeout(&config), Duration::from_secs(10));
}

/// Bảng rất nhiều cột: prompt vượt PROMPT_MAX_CHARS thì chỉ còn cột khớp câu hỏi và cột thời gian, như prompt SQL.
#[test]
fn long_prompt_keeps_only_matched_and_time_columns() {
    let mut columns: Vec<ColumnProfile> = (0..400)
        .map(|i| {
            column(
                &format!("Column {i}"),
                &[],
                Some("Mô tả khá dài để danh sách cột vượt trần ký tự của prompt"),
            )
        })
        .collect();
    columns[7].matched = true;
    columns[9].role = "time".to_string();
    let input = ClassifyInput {
        table: "wide",
        question: "Doanh thu theo Column 7",
        columns: &columns,
        earlier: &[],
    };
    let prompt = build_classify_prompt(&input);
    assert!(prompt.chars().count() <= PROMPT_MAX_CHARS);
    assert!(prompt.contains("\"Column 7\""), "{prompt}");
    assert!(prompt.contains("\"Column 9\""), "{prompt}");
    assert!(!prompt.contains("\"Column 8\""), "{prompt}");
}
