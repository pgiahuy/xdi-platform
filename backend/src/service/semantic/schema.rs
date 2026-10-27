// SPDX-License-Identifier: Apache-2.0
/* service/semantic/schema.rs - JSON Schema câu trả lời của task Profile (hồ sơ lớp ngữ nghĩa, #23), gửi kèm request
qua LlmRequest::json_schema để Gemini (responseJsonSchema) và Ollama (format) bị ép trả đúng khuôn mà
service/semantic/mod.rs (build_profile_answer) đọc. Không có schema, khuôn chỉ nằm trong lời prompt; chạy thử với
gemini-3.5-flash-lite thì không mục nào của câu trả lời qua được build_profile_answer và cả hồ sơ bị bỏ. Schema không
thay bước kiểm: mock và mô hình không theo schema vẫn đi qua build_profile_answer như cũ. */

use serde_json::json;

use crate::service::semantic::{
    ALIASES_MAX, ALIAS_MAX_CHARS, DESCRIPTION_MAX_CHARS, QUESTIONS_MAX, QUESTION_MAX_CHARS,
};

/// JSON Schema của câu trả lời hồ sơ ngữ nghĩa. Như build_sql_reply_schema (service/ask/reply.rs), chỉ dùng các từ khóa
/// cả Gemini lẫn Ollama nhận: type, properties, required, items, enum, description. Giới hạn độ dài và số mục ghi
/// trong description chứ không dùng maxLength, maxItems: Ollama không chắc theo các từ khóa đó, và build_profile_answer
/// đã áp giới hạn. Mô tả không bắt buộc thay vì cho kiểu null, cùng lý do với build_sql_reply_schema.
/// role_changes và sensitive_columns không bắt buộc vì prompt dặn bỏ hẳn khi không có gì.
pub fn build_profile_schema() -> serde_json::Value {
    let aliases = |of: &str| {
        json!({
            "type": "array",
            "description": format!(
                "Up to {ALIASES_MAX} Vietnamese names users may use for this {of}, each 1 to {ALIAS_MAX_CHARS} characters"
            ),
            "items": { "type": "string" }
        })
    };
    json!({
        "type": "object",
        "properties": {
            "columns": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Column name exactly as listed" },
                        "description": {
                            "type": "string",
                            "description": format!(
                                "One Vietnamese sentence, 1 to {DESCRIPTION_MAX_CHARS} characters; leave out when nothing useful to say"
                            )
                        },
                        "aliases": aliases("column"),
                        "unit": {
                            "type": "string",
                            "enum": ["money", "quantity", "percent", "duration", "other"],
                            "description": "For a measure column only, what one value counts; leave out when unsure"
                        }
                    },
                    "required": ["name", "aliases"]
                }
            },
            "values": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "column": { "type": "string", "description": "Column name exactly as listed" },
                        "value": { "type": "string", "description": "Value exactly as listed for that column" },
                        "aliases": aliases("value")
                    },
                    "required": ["column", "value", "aliases"]
                }
            },
            "role_changes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Column name exactly as listed" },
                        "role": { "type": "string", "enum": ["key", "dimension", "text"] }
                    },
                    "required": ["name", "role"]
                }
            },
            "sensitive_columns": {
                "type": "array",
                "description": "Names of columns that look like personal data",
                "items": { "type": "string" }
            },
            "suggested_questions": {
                "type": "array",
                "description": format!(
                    "Up to {QUESTIONS_MAX} short Vietnamese questions, each at most {QUESTION_MAX_CHARS} characters"
                ),
                "items": { "type": "string" }
            }
        },
        "required": ["columns", "values", "suggested_questions"]
    })
}
