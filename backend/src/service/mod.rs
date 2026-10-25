// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api. */

pub mod accounts;
pub mod ask;
// Ngữ cảnh câu hỏi có trước luồng hỏi; endpoint hỏi (#33) và prompt (#29) gọi tới. Gỡ allow khi đó để clippy bắt lại
// mã thừa.
#[allow(dead_code)]
pub mod ask_context;
#[cfg(test)]
mod ask_context_tests;
// Hợp đồng khai đủ kiểu ô, vai trò cột và loại cảnh báo trước khi luồng hỏi (#33, #35, #36) dựng chúng; mẫu chỉ dùng
// một phần. Tạm cho phép "chưa dùng" ở module này; gỡ khi luồng hỏi chạy để clippy bắt lại.
#[allow(dead_code)]
pub mod ask_contract;
pub mod ask_examples;
#[cfg(test)]
mod ask_examples_tests;
// Các phần của ngữ cảnh câu hỏi (ask_context), cùng lý do cho phép "chưa dùng".
#[allow(dead_code)]
pub mod ask_match;
#[allow(dead_code)]
pub mod ask_phrases;
// Bước narrate của câu "tại sao" (#41): mô hình viết lời từ bảng phân rã, code kiểm số. Gỡ allow khi #33/#38 gọi,
// như ở ask_contract.
#[allow(dead_code)]
pub mod ask_narrate;
#[cfg(test)]
mod ask_narrate_tests;
// Kết quả và vòng sửa của câu hỏi; endpoint hỏi (#33) gọi run_with_repair. Gỡ allow khi đó, như ở ask_contract.
#[allow(dead_code)]
pub mod ask_other_text;
// Định dạng câu trả lời của mô hình (#29); ask_run.rs đọc nó, endpoint hỏi (#33) gửi schema. Lý do allow như trên.
#[allow(dead_code)]
pub mod ask_repair;
#[allow(dead_code)]
pub mod ask_reply;
#[allow(dead_code)]
pub mod ask_result;
#[allow(dead_code)]
pub mod ask_run;
#[cfg(test)]
mod ask_run_tests;
#[allow(dead_code)]
pub mod ask_values;
pub mod excel;
// Phần toán của câu "tại sao" có trước SQL và luồng hỏi; nối vào ở #38/#39. Gỡ allow khi đó để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod explain;
#[cfg(test)]
mod explain_tests;
pub mod ingest;
pub mod profile;
pub mod semantic;
pub mod users;
pub mod workspaces;
