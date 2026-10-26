// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api. */

pub mod accounts;
pub mod ask;
mod ask_chart;
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
pub mod ask_why_prompt;
// Bước narrate của câu "tại sao" (#41): mô hình viết lời từ bảng phân rã, code kiểm số.
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
mod ask_view;
// Nhánh câu "tại sao" của endpoint hỏi (#38): gọi run_why, ghi form và phân rã vào lịch sử.
mod ask_why;
// Truy dòng nguồn (#34): hợp đồng, dựng SQL từ câu đã chạy, ghép các bước.
pub mod drill;
pub mod drill_contract;
mod drill_sql;
#[cfg(test)]
mod drill_tests;
pub mod excel;
// Đường câu "tại sao" (#38, #39, #40): phần toán, form, kiểm form, SQL, phân rã; run_why ghép tất cả, ask_why.rs gọi.
pub mod explain;
pub mod explain_breakdown;
pub mod explain_check;
#[cfg(test)]
mod explain_check_tests;
pub mod explain_choices;
pub mod explain_form;
pub mod explain_measure;
pub mod explain_number;
pub mod explain_period;
pub mod explain_plain;
pub mod explain_run;
#[cfg(test)]
mod explain_run_tests;
pub mod explain_sql;
#[cfg(test)]
mod explain_tests;
pub mod ingest;
pub mod profile;
pub mod semantic;
pub mod semantic_schema;
#[cfg(test)]
mod semantic_tests;
pub mod understanding;
pub mod understanding_labels;
#[cfg(test)]
mod understanding_tests;
pub mod understanding_why;
pub mod users;
pub mod workspaces;
