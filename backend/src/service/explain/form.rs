// SPDX-License-Identifier: Apache-2.0
/* service/explain/form.rs - đầu vào của đường câu "tại sao" (#38): luật từ khóa nhận câu "tại sao", đọc form JSON mô
hình điền theo build_why_form_schema (ask/why_prompt.rs, #29), lỗi từng trường và prompt nhờ mô hình sửa form một lần.
explain/check.rs kiểm form với hồ sơ, explain/run.rs ghép các bước. Hàm thuần, test ở explain/check_tests.rs.

Luật từ khóa chỉ còn là đường dự phòng của bước classify (ask/classify.rs, #197): mô hình phân loại câu hỏi, lỗi hay
quá giờ thì dùng luật này. Luật bỏ sót câu không có cụm nào ("Doanh thu giảm do gì?") và bắt nhầm câu đếm theo một cột
tên "lý do"; bộ câu eval/classify.json đo cả hai cách. */

use serde::Deserialize;

use crate::engine::prompts::{self, fill};
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
    /// Kỳ để so; trống thì theo compare_with.
    #[serde(default)]
    pub base: Option<String>,
    /// Kỳ so khi base trống (#244): kỳ liền trước (mặc định) hay cùng kỳ năm trước.
    #[serde(default)]
    pub compare_with: Option<CompareWith>,
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
    /// Giá trị của cột khi agg là share: tỷ lệ dòng có giá trị này.
    #[serde(default)]
    pub value: Option<String>,
}

/// Cách tổng hợp một cột; mỗi cách một dạng phân rã, xem explain/measure_column.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormAgg {
    Sum,
    Avg,
    Count,
    CountDistinct,
    Share,
}

/// Kỳ gốc khi form không ghi base: "tuần này so với tuần trước" hay "quý 3/2017 so với cùng kỳ năm trước".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareWith {
    Previous,
    SamePeriodLastYear,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormTime {
    pub column: String,
    pub grain: Grain,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormFilter {
    pub column: String,
    /// Một phép của FILTER_OPS (explain/filter.rs); schema đã giới hạn, kiểm lại vì mô hình không bị ép (mock, Ollama
    /// cũ) có thể ghi khác.
    pub op: String,
    /// Giá trị của "=", ">", ">=", "<", "<=".
    #[serde(default)]
    pub value: String,
    /// Các giá trị của "in", hay hai đầu của "between" (#244).
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormGroups {
    pub column: String,
    pub compare: String,
    /// Nhóm để so; vắng hay chuỗi rỗng là mọi nhóm còn lại của cột (#244): "vì sao Scotland rút học nhiều" so Scotland
    /// với các vùng khác gộp lại.
    #[serde(default)]
    pub base: Option<String>,
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
/// trường. Mô hình gửi lại cả form, không chỉ phần sửa, để code đọc lại từ đầu bằng cùng một hàm. Chữ ở
/// backend/prompts/why_form_repair.txt.
pub fn build_why_repair_prompt(prompt: &str, reply: &str, errors: &[FieldError]) -> String {
    let lines: Vec<String> = errors
        .iter()
        .map(|e| format!("- {}: {}", e.field, e.message))
        .collect();
    fill(
        &prompts::WHY_FORM_REPAIR,
        &[
            ("prompt", prompt),
            ("reply", reply.trim()),
            ("errors", &lines.join("\n")),
        ],
    )
}
