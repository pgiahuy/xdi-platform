// SPDX-License-Identifier: Apache-2.0
/* service/ask_reply.rs - định dạng câu trả lời của mô hình ở bước sinh SQL và bước sửa (#29): JSON Schema gửi kèm
request để Gemini và Ollama bị ép trả đúng dạng (LlmRequest::json_schema), và hàm đọc câu trả lời. ask_run.rs đọc câu
trả lời của cả lần đầu lẫn lần sửa bằng build_answer_from_reply; prompt dặn mô hình điền từng trường nằm ở
service/ask_prompt.rs (PR sau của #29).

Câu trả lời là một object:
- terms: cách mô hình hiểu từng cụm chữ của người hỏi (cột, giá trị, chỉ số, kỳ). #33 kiểm với hồ sơ và column_values.
- assumptions: cách hiểu mặc định mô hình đã chọn khi câu có nhiều cách hiểu. Mô hình chỉ ghi loại, cụm chữ, phương án
  đã chọn và các phương án khác; id của giả định do code dựng từ loại và cụm chữ, vì mô hình tự đặt id thì cùng một câu
  mỗi lần ra một id và lựa chọn người dùng gửi lại không áp được.
- clarify: câu hỏi lại, chỉ khi không có cách hiểu mặc định hợp lý; khi đó sql rỗng.
- sql: một câu SELECT trên tên bảng logic.

Ở đây không sửa hay kiểm SQL: câu đọc ra đi thẳng vào guard (engine/guard/). Sửa ở đây thì guard kiểm câu khác với câu
mô hình viết, và lịch sử (queries.sql_generated) sẽ sai. */

use serde::Deserialize;
use serde_json::json;

/// Loại của một cụm chữ đã hiểu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TermKind {
    /// Cụm chữ chỉ một cột ("thị trường" là "Market").
    Column,
    /// Cụm chữ chỉ một giá trị trong cột ("châu Âu" là Market = Europe).
    Value,
    /// Cụm chữ chỉ một chỉ số đã định nghĩa ("doanh thu").
    Metric,
    /// Cụm chữ chỉ một kỳ trên cột thời gian ("năm 2017").
    Period,
}

/// Cách mô hình hiểu một cụm chữ của câu hỏi.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Term {
    /// Cụm chữ như người hỏi gõ.
    pub text: String,
    pub kind: TermKind,
    /// Cột được nhắc (column), cột chứa giá trị (value), cột thời gian (period). Không có với metric.
    #[serde(default)]
    pub column: Option<String>,
    /// Giá trị trong cột (value), tên chỉ số (metric), kỳ dạng 2017, 2017-Q3, 2017-05 (period).
    #[serde(default)]
    pub value: Option<String>,
}

/// Thứ mà một giả định hay một câu hỏi lại chọn giữa các phương án; quyết định tiền tố id (metric:, column:, time:).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceKind {
    Column,
    Metric,
    Time,
}

/// Cách hiểu mặc định mô hình đã chọn khi câu có nhiều cách hiểu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Assumption {
    pub kind: ChoiceKind,
    /// Cụm chữ có nhiều cách hiểu. Không có khi câu thiếu hẳn một phần, ví dụ không nêu thời gian.
    #[serde(default)]
    pub phrase: Option<String>,
    /// Phương án đã chọn: tên cột, tên chỉ số, hoặc kỳ.
    pub chosen: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Câu tiếng Việt cho người dùng, ví dụ "Doanh thu tính sau giảm giá".
    pub label: String,
}

/// Câu hỏi lại người dùng khi không có cách hiểu mặc định hợp lý.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Clarify {
    pub kind: ChoiceKind,
    #[serde(default)]
    pub phrase: Option<String>,
    /// Câu hỏi lại, tiếng Việt.
    pub question: String,
    /// Các phương án: tên cột, tên chỉ số, hoặc kỳ.
    pub options: Vec<String>,
}

/// Câu trả lời có SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlAnswer {
    pub terms: Vec<Term>,
    pub assumptions: Vec<Assumption>,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAnswer {
    Sql(SqlAnswer),
    /// Mô hình cần hỏi lại người dùng; chưa có SQL.
    Clarify {
        terms: Vec<Term>,
        clarify: Clarify,
    },
}

/// Câu trả lời không dùng được. reason viết tiếng Anh: nó được gửi lại cho mô hình ở vòng sửa (ask_repair.rs) và ghi
/// vào log; người dùng nhận lời tiếng Việt của ask_run.rs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableReply {
    pub reason: String,
}

/// Đúng hình dạng JSON mô hình trả, trước khi kiểm. Trường thiếu thì lấy rỗng: schema đã bắt buộc chúng với Gemini và
/// Ollama, còn mô hình không bị ép mà quên một danh sách rỗng thì câu trả lời vẫn dùng được.
#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    terms: Vec<Term>,
    #[serde(default)]
    assumptions: Vec<Assumption>,
    #[serde(default)]
    clarify: Option<Clarify>,
    #[serde(default)]
    sql: String,
}

/// Đọc câu trả lời của bước sinh SQL hay bước sửa.
pub fn build_answer_from_reply(reply: &str) -> Result<ModelAnswer, UnreadableReply> {
    // Bị ép bằng schema thì câu trả lời là đúng một object. Mô hình không bị ép (mock, mô hình cũ trên Ollama) hay bọc
    // object trong khối ```json kèm lời giải thích; đọc object đầu tiên kể từ dấu { đầu tiên và bỏ qua phần sau nó,
    // nên khối code, nhãn và chữ thừa trước sau đều không cản.
    let Some(start) = reply.find('{') else {
        return Err(unreadable("the reply contains no JSON object"));
    };
    let mut objects = serde_json::Deserializer::from_str(&reply[start..]).into_iter::<Reply>();
    let parsed = match objects.next() {
        Some(Ok(parsed)) => parsed,
        Some(Err(e)) => {
            return Err(unreadable(&format!(
                "the reply is not a JSON object of the required shape: {e}"
            )))
        }
        None => return Err(unreadable("the reply contains no JSON object")),
    };
    let sql = parsed.sql.trim();
    // Có cả SQL lẫn câu hỏi lại thì dùng SQL: người hỏi có ngay kết quả, và cách hiểu đã ghi trong assumptions.
    if !sql.is_empty() {
        return Ok(ModelAnswer::Sql(SqlAnswer {
            terms: parsed.terms,
            assumptions: parsed.assumptions,
            sql: sql.to_string(),
        }));
    }
    match parsed.clarify {
        Some(clarify) if !clarify.question.trim().is_empty() => Ok(ModelAnswer::Clarify {
            terms: parsed.terms,
            clarify,
        }),
        _ => Err(unreadable(
            "\"sql\" is empty and there is no \"clarify\" question",
        )),
    }
}

fn unreadable(reason: &str) -> UnreadableReply {
    UnreadableReply {
        reason: reason.to_string(),
    }
}

/// Schema của danh sách terms, dùng chung cho câu trả lời SQL ở đây và form câu "tại sao" (ask_prompt.rs), để hai
/// đường ghi cách hiểu cùng một dạng và #33 kiểm chung một cách.
pub fn build_terms_schema() -> serde_json::Value {
    json!({
        "type": "array",
        "description": "How each phrase of the question maps to the data",
        "items": {
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The phrase as the user wrote it" },
                "kind": { "type": "string", "enum": ["column", "value", "metric", "period"] },
                "column": { "type": "string", "description": "The column; leave out for a metric" },
                "value": {
                    "type": "string",
                    "description": "The column value, the metric name, or the period such as 2017, 2017-Q3, 2017-05"
                }
            },
            "required": ["text", "kind"]
        }
    })
}

/// JSON Schema của câu trả lời, gửi kèm request ở bước sinh SQL và bước sửa. Chỉ dùng các từ khóa cả Gemini
/// (responseJsonSchema) lẫn Ollama (format) nhận: type, properties, required, items, enum, description. Trường không bắt
/// buộc thì để ngoài required thay vì cho kiểu null, vì kiểu ["string", "null"] chưa chắc mọi mô hình trên Ollama theo
/// được. description là lời dặn cho mô hình nên viết tiếng Anh như phần luật của prompt.
pub fn build_sql_reply_schema() -> serde_json::Value {
    let choice_kind = json!({
        "type": "string",
        "enum": ["column", "metric", "time"],
        "description": "What the choice is about: a column, a metric, or the time range"
    });
    let phrase = json!({
        "type": "string",
        "description": "The words of the question that the choice is about; leave out when the question says nothing about it"
    });
    json!({
        "type": "object",
        "properties": {
            "terms": build_terms_schema(),
            "assumptions": {
                "type": "array",
                "description": "Defaults you chose where the question can be read in more than one way",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": choice_kind,
                        "phrase": phrase,
                        "chosen": { "type": "string", "description": "The column name, metric name or period you used" },
                        "alternatives": { "type": "array", "items": { "type": "string" } },
                        "label": { "type": "string", "description": "One sentence in Vietnamese telling the user what you assumed" }
                    },
                    "required": ["kind", "chosen", "alternatives", "label"]
                }
            },
            "clarify": {
                "type": "object",
                "description": "Only when there is no sensible default; then sql is an empty string",
                "properties": {
                    "kind": choice_kind,
                    "phrase": phrase,
                    "question": { "type": "string", "description": "The question to ask the user, in Vietnamese" },
                    "options": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["kind", "question", "options"]
            },
            "sql": { "type": "string", "description": "One DuckDB SELECT statement, or an empty string with clarify" }
        },
        "required": ["terms", "assumptions", "sql"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQL: &str =
        "SELECT \"Market\", SUM(\"Order Item Total\") FROM dataco GROUP BY \"Market\"";

    fn reply_with_sql(sql: &str) -> String {
        json!({ "terms": [], "assumptions": [], "sql": sql }).to_string()
    }

    fn sql_of(reply: &str) -> String {
        match build_answer_from_reply(reply) {
            Ok(ModelAnswer::Sql(answer)) => answer.sql,
            other => panic!("expected an SQL answer for {reply:?}, got {other:?}"),
        }
    }

    fn reason_of(reply: &str) -> String {
        match build_answer_from_reply(reply) {
            Err(e) => e.reason,
            other => panic!("expected an unreadable reply for {reply:?}, got {other:?}"),
        }
    }

    #[test]
    fn reads_terms_assumptions_and_sql() {
        let reply = r#"{
            "terms": [
                {"text": "châu Âu", "kind": "value", "column": "Market", "value": "Europe"},
                {"text": "doanh thu", "kind": "metric", "value": "doanh thu"}
            ],
            "assumptions": [
                {"kind": "time", "chosen": "all", "alternatives": [], "label": "Tính trên toàn bộ dữ liệu"}
            ],
            "sql": "  SELECT 1  "
        }"#;
        let Ok(ModelAnswer::Sql(answer)) = build_answer_from_reply(reply) else {
            panic!("expected an SQL answer");
        };
        assert_eq!(answer.sql, "SELECT 1");
        assert_eq!(
            answer.terms,
            [
                Term {
                    text: "châu Âu".to_string(),
                    kind: TermKind::Value,
                    column: Some("Market".to_string()),
                    value: Some("Europe".to_string()),
                },
                Term {
                    text: "doanh thu".to_string(),
                    kind: TermKind::Metric,
                    column: None,
                    value: Some("doanh thu".to_string()),
                },
            ]
        );
        assert_eq!(answer.assumptions.len(), 1);
        assert_eq!(answer.assumptions[0].kind, ChoiceKind::Time);
        assert_eq!(answer.assumptions[0].phrase, None);
    }

    /// Mô hình không bị ép bằng schema hay bọc JSON trong khối code, kèm lời trước và sau.
    #[test]
    fn reads_the_object_inside_code_blocks_and_text() {
        let object = reply_with_sql(SQL);
        for reply in [
            object.clone(),
            format!("```json\n{object}\n```"),
            format!("```\n{object}\n```"),
            format!("Đây là câu trả lời:\n```json\n{object}\n```\nCâu này nhóm theo thị trường {{Market}}."),
            format!("{object} và vài chữ thừa"),
        ] {
            assert_eq!(sql_of(&reply), SQL, "{reply:?}");
        }
    }

    #[test]
    fn quotes_inside_sql_survive() {
        let sql = "SELECT * FROM dataco WHERE \"Customer Lname\" = 'O''Brien'";
        assert_eq!(sql_of(&reply_with_sql(sql)), sql);
    }

    #[test]
    fn missing_lists_are_empty() {
        let Ok(ModelAnswer::Sql(answer)) = build_answer_from_reply(r#"{"sql": "SELECT 1"}"#) else {
            panic!("expected an SQL answer");
        };
        assert!(answer.terms.is_empty() && answer.assumptions.is_empty());
    }

    #[test]
    fn clarify_without_sql_asks_the_user() {
        let reply = r#"{"terms": [], "assumptions": [], "sql": "",
            "clarify": {"kind": "metric", "question": "Bạn muốn xem giảm của chỉ số nào?",
                        "options": ["doanh thu", "lợi nhuận"]}}"#;
        let Ok(ModelAnswer::Clarify { clarify, .. }) = build_answer_from_reply(reply) else {
            panic!("expected a clarify answer");
        };
        assert_eq!(clarify.kind, ChoiceKind::Metric);
        assert_eq!(clarify.options, ["doanh thu", "lợi nhuận"]);
    }

    #[test]
    fn sql_wins_over_clarify() {
        let reply = r#"{"sql": "SELECT 1",
            "clarify": {"kind": "column", "question": "Cột nào?", "options": ["a", "b"]}}"#;
        assert_eq!(sql_of(reply), "SELECT 1");
    }

    #[test]
    fn unusable_replies_give_a_reason_for_the_model() {
        assert!(reason_of("").contains("no JSON object"));
        assert!(reason_of("SELECT 1").contains("no JSON object"));
        assert!(reason_of("```sql\nSELECT 1\n```").contains("no JSON object"));
        assert!(reason_of(r#"{"sql": "SELECT 1""#).contains("required shape"));
        assert!(reason_of(r#"{"sql": 42}"#).contains("required shape"));
        assert!(
            reason_of(r#"{"terms": [{"text": "x", "kind": "table"}], "sql": "SELECT 1"}"#)
                .contains("required shape")
        );
        assert!(
            reason_of(r#"{"terms": [], "assumptions": [], "sql": "  "}"#)
                .contains("no \"clarify\"")
        );
        assert!(reason_of(
            r#"{"sql": "", "clarify": {"kind": "metric", "question": " ", "options": []}}"#
        )
        .contains("no \"clarify\""));
    }

    /// Mọi câu trả lời task sql và repair của mock đọc được, để luồng hỏi chạy với mock như với mô hình thật.
    #[test]
    fn mock_answers_are_readable() {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../engine/llm/mock_answers.json")).unwrap();
        let replies: Vec<String> = entries
            .iter()
            .filter(|e| e["task"] == "sql" || e["task"] == "repair")
            .map(|e| e["answer"].to_string())
            .collect();
        assert_eq!(replies.len(), 16);
        for reply in &replies {
            assert!(!sql_of(reply).is_empty(), "{reply}");
        }
    }

    /// Schema bắt buộc đúng các trường mà build_answer_from_reply cần, và chỉ dùng từ khóa cả Gemini lẫn Ollama nhận.
    #[test]
    fn schema_requires_the_fields_and_uses_common_keywords() {
        let schema = build_sql_reply_schema();
        assert_eq!(schema["required"], json!(["terms", "assumptions", "sql"]));
        let allowed = [
            "type",
            "properties",
            "required",
            "items",
            "enum",
            "description",
        ];
        let mut stack = vec![&schema];
        while let Some(node) = stack.pop() {
            let object = node.as_object().unwrap();
            for (key, value) in object {
                assert!(allowed.contains(&key.as_str()), "keyword {key} in {node}");
                match key.as_str() {
                    "properties" => stack.extend(value.as_object().unwrap().values()),
                    "items" => stack.push(value),
                    _ => {}
                }
            }
        }
    }
}
