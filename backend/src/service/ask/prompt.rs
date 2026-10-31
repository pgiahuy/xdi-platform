// SPDX-License-Identifier: Apache-2.0
/* service/ask/prompt.rs - prompt SQL cho câu thường (#29), và các phần chung với prompt form câu "tại sao"
(ask/why_prompt.rs): cột, gợi ý, lựa chọn đã có, chỉ số, khái niệm ngành, trần độ dài. Câu trả lời theo
build_sql_reply_schema (ask/reply.rs). Ngữ cảnh (cột, giá trị, gợi ý, cột đã chọn) lấy từ AskContext (ask/context.rs),
chỉ số và câu mẫu đã duyệt đọc từ engine/store (metrics.rs, queries.rs), gói khái niệm ngành từ engine/domains.rs. Endpoint
hỏi (#33) đọc các thứ đó rồi gọi build_sql_prompt. Test ở ask/prompt_tests.rs.

Prompt chỉ có tên bảng logic, tên cột, mô tả, bí danh, giá trị của cột chiều không nhạy cảm và định nghĩa chỉ số; không
có dòng dữ liệu, đường dẫn file hay read_parquet (docs/PRIVACY.md). Phần luật viết tiếng Anh vì là phần máy đọc, như
prompt sửa (ask/repair.rs); câu hỏi, tên cột, mô tả, giá trị giữ nguyên văn. Chữ của phần luật nằm ở
backend/prompts/sql.txt và unanswerable_rule.txt (engine/prompts.rs); nhãn của từng phần dữ liệu ("Columns:",
"Metrics defined by the organization:") và dòng của từng cột, chỉ số dựng ở đây cùng dữ liệu.

Câu hỏi đọc được bảng liên quan (#213, ask/related.rs, ask/group_prompt.rs) thì phần cột thành một khối mỗi bảng, bảng
đang chọn trước, theo sau là danh sách quan hệ và luật nối bảng (backend/prompts/sql_joins.txt); chỉ số ghi kèm tên
bảng. Câu một bảng giữ nguyên chữ như trước.

Thứ tự: luật, luật của guard, cột, gợi ý, lựa chọn đã có, chỉ số, khái niệm ngành, ví dụ, các lượt hỏi trước của hội
thoại, câu hỏi cuối cùng, để mô hình đọc câu hỏi khi đã có đủ ngữ cảnh. Luật của guard sinh từ chính guard
(engine/guard/prompt_rules.rs), để mô hình viết được câu qua guard ngay lần đầu; guard vẫn kiểm lại mọi câu. Chỉ số
chưa xác nhận luôn ghi rõ là bản nháp. */

use std::collections::HashSet;

use crate::engine::domains::DomainPackage;
use crate::engine::guard::build_prompt_rules;
use crate::engine::prompts::{self, fill};
use crate::engine::store::{ColumnAggregation, MetricDefinition, ProfileRole, PromptExample};
use crate::service::ask::choices::COLUMN_CHOICE_PREFIX;
use crate::service::ask::context::{is_detailed, AskContext, ColumnProfile};
use crate::service::ask::contract::ChosenOption;
use crate::service::ask::group_prompt::{GroupPrompt, JoinLine};
use crate::service::ask::phrases::build_phrases;
use crate::service::ask::values::ValueEntry;

/// Trần độ dài prompt, tính theo ký tự. Phần giá trị của DataCo đo ở #28 là 4 052 ký tự; trần này chừa chỗ cho hồ sơ
/// cột, chỉ số, ví dụ. Mô hình chạy trên máy (Ollama) có cửa sổ ngữ cảnh nhỏ, prompt dài quá thì phần đầu (phần luật)
/// bị cắt mà không báo; vượt trần thì bỏ dần phần ít cần nhất (build_within_limit).
pub const PROMPT_MAX_CHARS: usize = 24_000;

/// Số câu mẫu đã duyệt tối đa đưa vào prompt làm ví dụ.
pub const EXAMPLES_MAX: usize = 3;

/// Đầu vào chung của prompt SQL và prompt form (ask/why_prompt.rs).
pub struct PromptInput<'a> {
    /// Tên bảng logic (datasets.logical_table), tên bảng duy nhất mô hình được dùng.
    pub table: &'a str,
    pub question: &'a str,
    pub context: &'a AskContext,
    /// Chỉ số của dataset (Store::list_metrics).
    pub metrics: &'a [MetricDefinition],
    /// Câu mẫu admin đã duyệt, mới nhất trước (Store::list_approved_examples, #212). Prompt SQL lấy câu thường, prompt
    /// form lấy câu "tại sao" (có form).
    pub examples: &'a [PromptExample],
    /// Gói khái niệm ngành của dataset; None khi dataset chưa gắn gói.
    pub domain: Option<&'a DomainPackage>,
    /// Lựa chọn người hỏi gửi kèm câu hỏi. Lựa chọn cột đã nằm trong context.picks (#28); ở đây chỉ thêm lựa chọn chỉ
    /// số và khoảng thời gian.
    pub choices: &'a [ChosenOption],
    /// Khối "Hội thoại trước" (service/conversation/history.rs); chuỗi rỗng khi câu mở đầu cuộc trò chuyện.
    pub history: &'a str,
    /// Các bảng liên quan và quan hệ (ask/related.rs); None khi câu chỉ đọc bảng đang chọn, luôn None với prompt form.
    pub group: Option<&'a GroupPrompt>,
}

/// Mức chi tiết của danh sách cột; prompt quá dài thì hạ dần theo thứ tự khai báo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Detail {
    /// Mọi cột, mọi giá trị ngữ cảnh có.
    Full,
    /// Bỏ danh sách giá trị chỉ là mẫu (values_complete = false); cột ít giá trị vẫn đủ danh sách để ánh xạ chữ.
    CompleteValuesOnly,
    /// Như trên, và chỉ cột tầng hai (is_detailed: khớp câu hỏi, thời gian, khóa nối bảng, chỉ số) còn mô tả và giá
    /// trị; cột khác vẫn có mặt ở dạng gọn (#230).
    MatchedColumns,
}

/// Prompt sinh SQL cho câu thường.
pub fn build_sql_prompt(input: &PromptInput) -> String {
    let mut tables = vec![input.table];
    if let Some(group) = input.group {
        tables.extend(group.tables.iter().map(|t| t.table.as_str()));
    }
    build_within_limit(|detail| {
        join_sections(vec![
            build_sql_rules(&tables),
            build_prompt_rules(&tables),
            build_group_columns(input, detail),
            build_hints_section(input.context),
            build_choices_section(input.context, input.choices),
            build_group_metrics(input),
            build_domain_section(input.domain),
            build_examples_section(&pick_examples(input, false)),
            input.history.to_string(),
            format!("Question: {}", input.question),
        ])
    })
}

/// Dựng prompt ở mức chi tiết cao nhất còn nằm trong PROMPT_MAX_CHARS. Hạ hết mức vẫn dài thì gửi bản ngắn nhất và ghi
/// log: cắt tiếp là bỏ luật hay câu hỏi, còn tệ hơn để mô hình báo lỗi.
pub(super) fn build_within_limit(build: impl Fn(Detail) -> String) -> String {
    let mut prompt = String::new();
    for detail in [
        Detail::Full,
        Detail::CompleteValuesOnly,
        Detail::MatchedColumns,
    ] {
        prompt = build(detail);
        if prompt.chars().count() <= PROMPT_MAX_CHARS {
            return prompt;
        }
    }
    tracing::warn!(
        chars = prompt.chars().count(),
        "prompt is over the limit even with unmatched columns in compact form"
    );
    prompt
}

pub(super) fn join_sections(sections: Vec<String>) -> String {
    sections
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Luật của prompt SQL, chữ ở backend/prompts/sql.txt; tables là bảng đang chọn rồi các bảng liên quan.
fn build_sql_rules(tables: &[&str]) -> String {
    let names = match tables {
        [table] => format!("the table {table}"),
        _ => format!("the tables {}", tables.join(", ")),
    };
    fill(
        &prompts::SQL,
        &[
            ("tables", &names),
            ("unanswerable_rule", &build_unanswerable_rule()),
        ],
    )
}

/// Phần cột: một khối "Columns:" khi chỉ một bảng; nhiều bảng thì một khối mỗi bảng rồi quan hệ và luật nối bảng.
fn build_group_columns(input: &PromptInput, detail: Detail) -> String {
    let Some(group) = input.group else {
        return build_columns_section(input.context, detail);
    };
    let mut sections = vec![build_table_block(
        &format!("Columns of table {}:", input.table),
        &input.context.columns,
        detail,
    )];
    for table in &group.tables {
        let title = format!("Columns of table {}:", table.table);
        sections.push(build_table_block(&title, &table.columns, detail));
    }
    let joins: Vec<String> = group.joins.iter().map(build_join_line).collect();
    sections.push(fill(
        &prompts::SQL_JOINS,
        &[("relationships", &joins.join("\n"))],
    ));
    join_sections(sections)
}

/// `- order_items.order_id = orders.order_id (many order_items rows per orders row)`; khóa nhiều cột nối bằng AND.
fn build_join_line(join: &JoinLine) -> String {
    let pairs: Vec<String> = join
        .from_columns
        .iter()
        .zip(&join.to_columns)
        .map(|(from, to)| {
            format!(
                "{}.{} = {}.{}",
                join.from_table,
                build_join_column(from),
                join.to_table,
                build_join_column(to)
            )
        })
        .collect();
    let (from, to) = (&join.from_table, &join.to_table);
    let kind = if join.one_to_one {
        format!("at most one {from} row per {to} row")
    } else {
        format!("many {from} rows per {to} row")
    };
    format!("- {} ({kind})", pairs.join(" AND "))
}

/// Tên cột trơn (chữ thường, số, gạch dưới) viết nguyên cho dễ đọc; tên khác (dấu cách, chữ hoa) đặt trong nháy kép
/// để chép vào SQL vẫn đúng.
fn build_join_column(name: &str) -> String {
    let plain = name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if plain {
        name.to_string()
    } else {
        quote_ident(name)
    }
}

/// Chỉ số: như build_metrics_section khi một bảng; nhiều bảng thì chỉ số của mọi bảng, mỗi dòng ghi tên bảng của nó.
fn build_group_metrics(input: &PromptInput) -> String {
    let Some(group) = input.group else {
        return build_metrics_section(input.metrics, true);
    };
    let mut lines: Vec<String> = input
        .metrics
        .iter()
        .map(|m| build_metric_line(m, Some(input.table), true))
        .collect();
    for table in &group.tables {
        let table_lines = table
            .metrics
            .iter()
            .map(|m| build_metric_line(m, Some(&table.table), true));
        lines.extend(table_lines);
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("Metrics defined by the organization:\n{}", lines.join("\n"))
}

/// Luật của trường unanswerable (backend/prompts/unanswerable_rule.txt), chung cho prompt SQL và prompt form câu "tại
/// sao" (ask/why_prompt.rs). Chữ nhấn "không dùng khi chỉ mơ hồ": câu mơ hồ có câu hỏi lại hay giả định, còn lối ra này
/// trả rejected và người hỏi không có số nào.
pub(super) fn build_unanswerable_rule() -> String {
    fill(&prompts::UNANSWERABLE_RULE, &[])
}

pub(super) fn build_columns_section(context: &AskContext, detail: Detail) -> String {
    build_table_block("Columns:", &context.columns, detail)
}

/// Khối cột của một bảng: mọi cột, không bỏ cột nào (#230). Cột gọn (brief, đặt ở ask/context.rs khi câu hỏi đọc
/// nhiều cột) và, ở mức MatchedColumns, mọi cột ngoài tầng hai chỉ in dạng gọn.
fn build_table_block(title: &str, columns: &[ColumnProfile], detail: Detail) -> String {
    let lines: Vec<String> = columns
        .iter()
        .map(|c| {
            let compact = c.brief || (detail >= Detail::MatchedColumns && !is_detailed(c));
            if compact {
                build_compact_line(c)
            } else {
                build_column_line(c, detail)
            }
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("{title}\n{}", lines.join("\n"))
}

/// Cột ở dạng gọn, một dòng: tên, kiểu, vai trò, định dạng ngày nếu có, bí danh. Không mô tả, không cách gộp, không giá
/// trị nào (kể cả với cột nhạy cảm): đủ để mô hình biết cột có thật và gọi đúng tên.
fn build_compact_line(column: &ColumnProfile) -> String {
    let mut line = format!(
        "- {} ({}, {}",
        quote_ident(&column.name),
        column.data_type,
        column.role
    );
    if let Some(format) = &column.time_format {
        line.push_str(&format!(", date format {format}"));
    }
    line.push(')');
    if !column.aliases.is_empty() {
        line.push_str(&format!(". Also called: {}", column.aliases.join(", ")));
    }
    line
}

/// Một cột đủ chi tiết: tên, kiểu, vai trò, định dạng ngày nếu là ngày dạng chữ, cách gộp nếu là avg, mô tả, bí danh,
/// rồi giá trị ở dòng sau.
fn build_column_line(column: &ColumnProfile, detail: Detail) -> String {
    let mut line = format!(
        "- {} ({}, {}",
        quote_ident(&column.name),
        column.data_type,
        column.role
    );
    if let Some(format) = &column.time_format {
        line.push_str(&format!(", date format {format}"));
    }
    // Chỉ cột avg có thêm chữ: sum là cách mô hình làm sẵn, ghi cho mọi cột chỉ làm prompt dài thêm. Cột mô tả từng
    // dòng (tín chỉ, tuổi, số ngày) cộng lại ra số vô nghĩa (#185).
    if column.role == ProfileRole::Measure.as_str() && column.aggregation == ColumnAggregation::Avg
    {
        line.push_str(", one value per row: aggregate it with AVG, never SUM");
    }
    line.push(')');
    if let Some(description) = column.description.as_deref().map(str::trim) {
        if !description.is_empty() {
            line.push_str(": ");
            line.push_str(description);
        }
    }
    if !column.aliases.is_empty() {
        line.push_str(&format!(". Also called: {}", column.aliases.join(", ")));
    }
    // Cột nhạy cảm không kèm giá trị nào. #28 đã lọc; kiểm lại ở đây vì đây là chỗ cuối trước khi chữ rời máy chủ.
    let show_values = !column.is_sensitive
        && !column.values.is_empty()
        && (column.values_complete || detail == Detail::Full);
    if show_values {
        let kind = if column.values_complete {
            "All values"
        } else {
            "Some values"
        };
        let values: Vec<String> = column.values.iter().map(build_value_text).collect();
        line.push_str(&format!("\n  {kind}: {}", values.join(", ")));
    }
    line
}

/// Giá trị viết như chuỗi SQL ('O''Brien'): mô hình chép nguyên vào SQL được, và dấu phẩy trong giá trị không lẫn với
/// dấu phẩy ngăn các giá trị.
fn build_value_text(entry: &ValueEntry) -> String {
    let literal = quote_literal(&entry.value);
    if entry.aliases.is_empty() {
        literal
    } else {
        format!("{literal} (also: {})", entry.aliases.join(", "))
    }
}

pub(super) fn build_hints_section(context: &AskContext) -> String {
    if context.hints.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = context
        .hints
        .iter()
        .map(|h| {
            format!(
                "- \"{}\" may be {} = {}",
                h.phrase,
                quote_ident(&h.column),
                quote_literal(&h.value)
            )
        })
        .collect();
    format!(
        "Phrases of the question that equal a value (you decide whether they mean it):\n{}",
        lines.join("\n")
    )
}

/// Cột #28 đã chọn cho từng cụm, và lựa chọn chỉ số, thời gian người hỏi gửi lại.
pub(super) fn build_choices_section(context: &AskContext, choices: &[ChosenOption]) -> String {
    let mut lines: Vec<String> = context
        .picks
        .iter()
        .map(|pick| {
            let who = if pick.chosen_by_user {
                "chosen by the user"
            } else {
                "best match"
            };
            let mut line = format!(
                "- \"{}\" means the column {} ({who})",
                pick.phrase,
                quote_ident(&pick.column)
            );
            if !pick.others.is_empty() {
                let others: Vec<String> = pick.others.iter().map(|o| quote_ident(o)).collect();
                line.push_str(&format!(", not {}", others.join(" or ")));
            }
            line
        })
        .collect();
    lines.extend(
        choices
            .iter()
            .filter(|c| !c.choice_id.starts_with(COLUMN_CHOICE_PREFIX))
            .map(|c| {
                format!(
                    "- for the choice {} the user chose \"{}\"",
                    c.choice_id, c.option_id
                )
            }),
    );
    if lines.is_empty() {
        return String::new();
    }
    format!("Choices already made, follow them:\n{}", lines.join("\n"))
}

/// Định nghĩa chỉ số. with_formula false (prompt form) thì chỉ có tên và mô tả.
pub(super) fn build_metrics_section(metrics: &[MetricDefinition], with_formula: bool) -> String {
    if metrics.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = metrics
        .iter()
        .map(|m| build_metric_line(m, None, with_formula))
        .collect();
    format!("Metrics defined by the organization:\n{}", lines.join("\n"))
}

/// table: tên bảng của chỉ số khi prompt có nhiều bảng.
fn build_metric_line(metric: &MetricDefinition, table: Option<&str>, with_formula: bool) -> String {
    let status = if metric.confirmed {
        ""
    } else {
        " [UNCONFIRMED draft]"
    };
    let mut line = format!("- \"{}\"{status}", metric.name);
    if let Some(table) = table {
        line.push_str(&format!(" (table {table})"));
    }
    if with_formula {
        line.push_str(&format!(": {}", metric.expression));
        if let Some(filter) = &metric.filter {
            line.push_str(&format!(", only rows where {filter}"));
        }
        if let Some(column) = &metric.time_column {
            line.push_str(&format!(", time column {}", quote_ident(column)));
        }
        if let Some(unit) = &metric.unit {
            line.push_str(&format!(", unit {unit}"));
        }
    }
    let description = metric.description.trim();
    if !description.is_empty() {
        line.push_str(&format!(". {description}"));
    }
    line
}

/// Khái niệm của gói ngành: giúp mô hình hiểu thuật ngữ ("doanh số", "AOV"), không phải công thức trên cột thật.
pub(super) fn build_domain_section(domain: Option<&DomainPackage>) -> String {
    let Some(package) = domain else {
        return String::new();
    };
    let lines: Vec<String> = package
        .concepts
        .iter()
        .map(|c| {
            format!(
                "- {} ({}): {}",
                c.name,
                c.synonyms.join(", "),
                c.description
            )
        })
        .collect();
    format!(
        "Business terms of {}; they explain words only, the formulas on the columns are the metrics above:\n{}",
        package.name,
        lines.join("\n")
    )
}

/// Câu mẫu đưa vào prompt: tối đa EXAMPLES_MAX câu đã duyệt có chung ít nhất một từ không phải từ dừng với câu hỏi,
/// mới nhất trước. Câu không liên quan chỉ làm prompt dài và kéo mô hình theo cách viết của câu khác. why chọn câu
/// "tại sao" (có form) cho prompt form; false chọn câu thường cho prompt SQL: SQL của câu "tại sao" do code dựng, không
/// phải cách mô hình viết.
pub(crate) fn pick_examples<'a>(input: &PromptInput<'a>, why: bool) -> Vec<&'a PromptExample> {
    let words = build_content_words(input.question);
    input
        .examples
        .iter()
        .filter(|e| e.form.is_some() == why)
        .filter(|e| !build_content_words(&e.question).is_disjoint(&words))
        .take(EXAMPLES_MAX)
        .collect()
}

/// Chi tiết của bước llm khi prompt có câu mẫu: người xem tab Các bước biết câu trả lời đã dựa vào câu mẫu đã duyệt
/// nào. None khi không có câu nào.
pub(crate) fn build_examples_detail(picked: &[&PromptExample]) -> Option<String> {
    if picked.is_empty() {
        return None;
    }
    let questions: Vec<String> = picked
        .iter()
        .map(|e| format!("\"{}\"", e.question))
        .collect();
    Some(format!(
        "Prompt có {} câu mẫu đã duyệt: {}",
        picked.len(),
        questions.join("; ")
    ))
}

fn build_examples_section(picked: &[&PromptExample]) -> String {
    if picked.is_empty() {
        return String::new();
    }
    // Nhãn khác dòng "Question:" cuối prompt, để mô hình không lẫn câu ví dụ với câu cần trả lời.
    let lines: Vec<String> = picked
        .iter()
        .map(|e| format!("Example question: {}\nExample SQL: {}", e.question, e.sql))
        .collect();
    format!(
        "Earlier questions answered correctly:\n{}",
        lines.join("\n")
    )
}

/// Câu mẫu cho prompt form: câu hỏi và form đã chạy, viết JSON một dòng như câu trả lời mô hình cần viết.
pub(super) fn build_form_examples_section(picked: &[&PromptExample]) -> String {
    let lines: Vec<String> = picked
        .iter()
        .filter_map(|e| {
            let form = e.form.as_ref()?;
            Some(format!(
                "Example question: {}\nExample form: {form}",
                e.question
            ))
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "Earlier questions answered correctly:\n{}",
        lines.join("\n")
    )
}

/// Các từ đơn không phải từ dừng hay số, dạng so được (dấu thanh đặt một chỗ).
fn build_content_words(text: &str) -> HashSet<String> {
    build_phrases(text)
        .into_iter()
        .filter(|p| p.len == 1 && !p.has_stop)
        .map(|p| p.key)
        .collect()
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
