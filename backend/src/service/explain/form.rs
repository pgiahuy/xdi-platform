// SPDX-License-Identifier: Apache-2.0
/* service/explain/form.rs - đầu vào của đường câu "tại sao" (#38): nhận câu "tại sao" bằng từ khóa, đọc form JSON mô
hình điền theo build_why_form_schema (ask/why_prompt.rs, #29), lỗi từng trường và prompt nhờ mô hình sửa form một lần.
explain/check.rs kiểm form với hồ sơ, explain/run.rs ghép các bước. Hàm thuần, test ở explain/check_tests.rs.

Nhận câu "tại sao" bằng từ khóa chứ không nhờ mô hình phân loại: không tốn thêm một lượt gọi, cùng câu luôn đi cùng
đường. Đổi lại câu không có từ khóa nào ("thế nào mà giảm") đi đường câu thường, vẫn có bảng nhưng không có phân rã. */

use serde::Deserialize;

use crate::service::ask::phrases::fold;
use crate::service::ask::reply::{Term, Unanswerable};
use crate::service::explain::period::Grain;

/// Loại câu hỏi, quyết định prompt: câu "tại sao" điền form (ask/why_prompt.rs), câu thường viết SQL (ask/prompt.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    Why,
    Plain,
}

/// Cụm từ của câu "tại sao", đã bỏ dấu. So theo từ trọn vẹn: "lý do" khớp, "lý doanh" thì không. Cụm dài hơn
/// ("tại vì sao", "do đâu mà", "vì lý do gì") đã khớp nhờ cụm ngắn nằm trong nó. Không thêm "sao" đứng một mình:
/// "sao chép", "sao lưu" là câu thường.
const WHY_PHRASES: [&str; 9] = [
    "tai sao",
    "vi sao",
    "sao lai",
    "vi dau",
    "ly do",
    "nguyen nhan",
    "do dau",
    "why",
    "how come",
];

/// Câu hỏi có một cụm của WHY_PHRASES thì là câu "tại sao". Bỏ dấu trước khi so nên "tai sao" gõ không dấu cũng nhận.
pub fn build_question_kind(question: &str) -> QuestionKind {
    // Thêm dấu cách hai đầu để mọi cụm so được theo ranh giới từ bằng một phép contains.
    let words = format!(" {} ", fold(question));
    let found = WHY_PHRASES
        .iter()
        .any(|phrase| words.contains(&format!(" {phrase} ")));
    if found {
        QuestionKind::Why
    } else {
        QuestionKind::Plain
    }
}

/// Form câu "tại sao" như mô hình điền, đúng các trường của build_why_form_schema. Chưa kiểm gì ngoài hình dạng JSON.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WhyForm {
    #[serde(default)]
    pub terms: Vec<Term>,
    /// Tên chỉ số trong metrics; code chèn công thức.
    #[serde(default)]
    pub metric: Option<String>,
    /// Độ đo khi không có chỉ số phù hợp.
    #[serde(default)]
    pub measure: Option<FormMeasure>,
    /// Có khi so hai kỳ; khi so hai nhóm thì là khoảng thời gian của câu hỏi (nếu có compare).
    #[serde(default)]
    pub time: Option<FormTime>,
    /// Kỳ cần giải thích, dạng 2017, 2017-Q3, 2017-05.
    #[serde(default)]
    pub compare: Option<String>,
    /// Kỳ để so; trống là kỳ liền trước.
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub filters: Vec<FormFilter>,
    /// Có khi so hai nhóm.
    #[serde(default)]
    pub groups: Option<FormGroups>,
    /// Dữ liệu không có thông tin để trả lời (#164); có lý do thì không kiểm phần còn lại của form.
    #[serde(default)]
    pub unanswerable: Option<Unanswerable>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormMeasure {
    pub column: String,
    pub agg: FormAgg,
}

/// Hai cách tổng hợp cộng được theo nhóm: tổng nhóm con bằng tổng cả lát cắt, điều kiện để phân rã (#39) đúng.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormAgg {
    Sum,
    Count,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormTime {
    pub column: String,
    pub grain: Grain,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormFilter {
    pub column: String,
    /// Chỉ nhận "="; schema đã giới hạn, kiểm lại vì mô hình không bị ép (mock, Ollama cũ) có thể ghi khác.
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormGroups {
    pub column: String,
    pub compare: String,
    pub base: String,
}

/// Một chỗ sai của form. message viết tiếng Anh: nó được gửi lại cho mô hình trong prompt sửa và ghi vào log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// Đường dẫn tới trường theo tên trong schema, ví dụ "measure.column", "filters[0].value"; "form" khi cả form
    /// không đọc được.
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        FieldError {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Form đã đọc, kèm object JSON gốc: object gốc được lưu cùng câu hỏi (queries.explanation, #33) để #34 truy dòng
/// nguồn đúng theo form đã chạy.
#[derive(Debug, Clone)]
pub struct ReadForm {
    pub form: WhyForm,
    pub json: serde_json::Value,
}

/// Đọc form từ câu trả lời. Như build_answer_from_reply (ask/reply.rs): lấy object JSON đầu tiên kể từ dấu { đầu tiên,
/// bỏ chữ thừa trước sau, vì mô hình không bị ép bằng schema hay bọc object trong khối ```json.
pub fn build_form_from_reply(reply: &str) -> Result<ReadForm, FieldError> {
    let unreadable = |reason: String| FieldError::new("form", reason);
    let Some(start) = reply.find('{') else {
        return Err(unreadable("the reply contains no JSON object".to_string()));
    };
    let mut objects =
        serde_json::Deserializer::from_str(&reply[start..]).into_iter::<serde_json::Value>();
    let json = match objects.next() {
        Some(Ok(json)) => json,
        Some(Err(e)) => return Err(unreadable(format!("the reply is not a JSON object: {e}"))),
        None => return Err(unreadable("the reply contains no JSON object".to_string())),
    };
    let form = WhyForm::deserialize(&json).map_err(|e| {
        unreadable(format!(
            "the reply is not a form of the required shape: {e}"
        ))
    })?;
    Ok(ReadForm { form, json })
}

/// Prompt nhờ mô hình sửa form: prompt form đầy đủ như lần đầu (đủ cột, giá trị, chỉ số), rồi form cũ và lỗi từng
/// trường. Mô hình gửi lại cả form, không chỉ phần sửa, để code đọc lại từ đầu bằng cùng một hàm.
pub fn build_why_repair_prompt(prompt: &str, reply: &str, errors: &[FieldError]) -> String {
    let lines: Vec<String> = errors
        .iter()
        .map(|e| format!("- {}: {}", e.field, e.message))
        .collect();
    format!(
        "{prompt}\n\nYour previous reply:\n{}\n\nThe form has these errors. Reply with the whole form again, in the \
         same JSON format, with them fixed:\n{}",
        reply.trim(),
        lines.join("\n")
    )
}
