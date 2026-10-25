// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api. */

pub mod accounts;
pub mod ask;
// build_metric_mismatches chờ #36 chọn cách báo chỉ số dùng sai công thức; phần còn lại endpoint hỏi đã gọi. Gỡ allow
// khi #36 gọi hàm đó, để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod ask_choices;
pub mod ask_context;
#[cfg(test)]
mod ask_context_tests;
pub mod ask_contract;
pub mod ask_examples;
#[cfg(test)]
mod ask_examples_tests;
pub mod ask_match;
pub mod ask_phrases;
pub mod ask_prompt;
#[cfg(test)]
mod ask_prompt_tests;
mod ask_response_choices;
// Prompt form của câu "tại sao" (#38); chỉ run_why gọi, nên gỡ allow cùng các module explain_* bên dưới.
#[allow(dead_code)]
pub mod ask_why_prompt;
// Bước narrate của câu "tại sao" (#41): mô hình viết lời từ bảng phân rã, code kiểm số. Gỡ allow khi endpoint hỏi gọi
// run_why, để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod ask_narrate;
#[cfg(test)]
mod ask_narrate_tests;
pub mod ask_other_text;
pub mod ask_repair;
pub mod ask_reply;
pub mod ask_result;
pub mod ask_run;
#[cfg(test)]
mod ask_run_tests;
pub mod ask_values;
// Truy dòng nguồn (#34): hợp đồng, dựng SQL từ câu đã chạy, ghép các bước.
pub mod drill;
pub mod drill_contract;
mod drill_sql;
#[cfg(test)]
mod drill_tests;
pub mod excel;
// Đường câu "tại sao" (#38, #39, #40): phần toán, form, kiểm form, SQL, phân rã; run_why ghép tất cả. Chỉ endpoint hỏi
// gọi run_why; luồng hỏi của #33 mới có câu thường, nên gỡ allow khi service/ask.rs nối câu "tại sao" vào.
#[allow(dead_code)]
pub mod explain;
#[allow(dead_code)]
pub mod explain_breakdown;
#[allow(dead_code)]
pub mod explain_check;
#[cfg(test)]
mod explain_check_tests;
#[allow(dead_code)]
pub mod explain_choices;
#[allow(dead_code)]
pub mod explain_form;
#[allow(dead_code)]
pub mod explain_measure;
#[allow(dead_code)]
pub mod explain_period;
#[allow(dead_code)]
pub mod explain_plain;
#[allow(dead_code)]
pub mod explain_run;
#[cfg(test)]
mod explain_run_tests;
#[allow(dead_code)]
pub mod explain_sql;
#[cfg(test)]
mod explain_tests;
pub mod ingest;
pub mod profile;
pub mod semantic;
pub mod semantic_schema;
#[cfg(test)]
mod semantic_tests;
// Thẻ "Đã hiểu là" (#35): endpoint hỏi (service/ask.rs) sẽ gọi build_understanding và build_column_roles; luồng hỏi của
// #33 chưa gọi. Gỡ allow khi đó, cùng allow trên GuardedSql::row_filters.
#[allow(dead_code)]
pub mod understanding;
#[allow(dead_code)]
pub mod understanding_labels;
#[cfg(test)]
mod understanding_tests;
pub mod users;
pub mod workspaces;
