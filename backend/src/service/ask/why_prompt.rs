// SPDX-License-Identifier: Apache-2.0
/* service/ask/why_prompt.rs - prompt form cho câu "tại sao" (#29) và JSON Schema của form. Mô hình điền form (độ đo,
hai kỳ hoặc hai nhóm, bộ lọc), code của #38 kiểm form với hồ sơ và metrics rồi dựng SQL; đọc và kiểm form là việc của
#38, file này chỉ định nghĩa prompt và hình dạng form. Các phần chung (cột, gợi ý, lựa chọn, chỉ số, khái niệm ngành, trần
độ dài) dùng lại của ask/prompt.rs, nên hai prompt mô tả dataset giống hệt nhau.

Chỉ số chỉ có tên và mô tả, không có công thức: mô hình ghi tên, #38 chèn công thức, nên mô hình không tự viết lại
được công thức của tổ chức. Chữ của phần luật nằm ở backend/prompts/why_form.txt (engine/prompts.rs). */

use serde_json::json;

use crate::engine::prompts::{self, fill};
use crate::service::ask::lessons::build_lessons_section;
use crate::service::ask::prompt::{
    build_choices_section, build_columns_section, build_domain_section,
    build_form_examples_section, build_hints_section, build_metrics_section,
    build_unanswerable_rule, build_within_limit, join_sections, pick_examples, PromptInput,
};
use crate::service::ask::reply::{build_terms_schema, build_unanswerable_schema};
use crate::service::explain::filter::FILTER_OPS;

/// Prompt cho câu "tại sao". Ví dụ là câu mẫu "tại sao" đã duyệt (#212) kèm form của nó, không phải câu SQL.
pub fn build_why_prompt(input: &PromptInput) -> String {
    build_within_limit(|detail| {
        join_sections(vec![
            build_why_rules(input.table),
            build_columns_section(input.context, detail),
            build_hints_section(input.context),
            build_choices_section(input.context, input.choices),
            build_metrics_section(input.metrics, false),
            build_domain_section(input.domain),
            build_form_examples_section(&pick_examples(input, true)),
            build_lessons_section(input.lessons),
            input.history.to_string(),
            format!("Question: {}", input.question),
        ])
    })
}

/// JSON Schema của form câu "tại sao", gửi kèm request (LlmRequest::json_schema); #38 đọc và kiểm form. Trường không
/// có thì để vắng thay cho null, cùng lý do như build_sql_reply_schema: time vắng là so hai nhóm, groups vắng là so hai
/// kỳ, metric và measure dùng một trong hai.
// compare nằm trong required dù so hai nhóm có thể không cần kỳ: khi compare không bắt buộc, Gemini hay bỏ trống nó ở
// lần điền đầu (kỳ chỉ ghi trong terms), kiểm form báo lỗi và tốn một vòng sửa. So hai nhóm không có kỳ thì ghi chuỗi
// rỗng; check_slices bỏ qua compare khi form không có time. Câu so hai kỳ không nêu kỳ cũng ghi chuỗi rỗng, code chọn
// kỳ gần nhất có dữ liệu (explain/latest.rs).
pub fn build_why_form_schema() -> serde_json::Value {
    let text = |description: &str| json!({ "type": "string", "description": description });
    json!({
        "type": "object",
        "properties": {
            "terms": build_terms_schema(),
            "metric": text("Name of a metric from the list; leave out when you fill measure"),
            "measure": {
                "type": "object",
                "description": "The measure when no metric fits",
                "properties": {
                    "column": text(
                        "A column of role measure for sum and avg, any column for count, a column of role key for \
                         count_distinct, a column of role dimension for share"
                    ),
                    "agg": {
                        "type": "string",
                        "enum": ["sum", "avg", "count", "count_distinct", "share"]
                    },
                    "value": text("For share only: the value of the column whose share of rows is explained")
                },
                "required": ["column", "agg"]
            },
            "time": {
                "type": "object",
                "description": "Set to compare two periods; with groups, set only when the question names a period",
                "properties": {
                    "column": text("A column of role time"),
                    "grain": { "type": "string", "enum": ["day", "week", "month", "quarter", "year"] }
                },
                "required": ["column", "grain"]
            },
            "compare": text(
                "The period being explained, such as 2017, 2017-Q3, 2017-05, 2017-W22 or 2017-05-31, matching the \
                 grain; an empty string when the question names no period"
            ),
            "base": text("The period to compare with; leave out to use compare_with"),
            "compare_with": {
                "type": "string",
                "description": "When base is left out: the period just before (the default) or the same period \
                                one year before",
                "enum": ["previous", "same_period_last_year"]
            },
            "filters": {
                "type": "array",
                "description": "Conditions on the rows the question is about",
                "items": {
                    "type": "object",
                    "properties": {
                        "column": text("A column from the list"),
                        "op": { "type": "string", "enum": FILTER_OPS },
                        "value": text(
                            "For =, >, >=, <, <=: a value from the value list of the column written exactly as \
                             listed, a number, or a date such as 2017, 2017-Q3, 2017-05 or 2017-05-31"
                        ),
                        "values": {
                            "type": "array",
                            "description": "For in: two or more values; for between: the low end and the high end, \
                                            both included",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["column", "op"]
                }
            },
            "groups": {
                "type": "object",
                "description": "Set to compare two groups; leave out to compare two periods",
                "properties": {
                    "column": text("A column of role dimension"),
                    "compare": text("The group being explained, a value of the column"),
                    "base": text(
                        "The group to compare with, a value of the column; leave out to compare with all other \
                         groups of the column"
                    )
                },
                "required": ["column", "compare"]
            },
            "find": {
                "type": "object",
                "description": "Set when the question asks which period or which group is unusual; the code \
                                finds it and explains it",
                "properties": {
                    "kind": { "type": "string", "enum": ["period", "group"] },
                    "column": text("For group: a column of role dimension whose groups are searched")
                },
                "required": ["kind"]
            },
            "unanswerable": build_unanswerable_schema()
        },
        "required": ["terms", "compare", "filters"]
    })
}

/// Luật của prompt form, chữ ở backend/prompts/why_form.txt.
fn build_why_rules(table: &str) -> String {
    fill(
        &prompts::WHY_FORM,
        &[
            ("table", table),
            ("unanswerable_rule", &build_unanswerable_rule()),
        ],
    )
}
