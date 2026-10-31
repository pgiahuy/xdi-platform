// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api.

Bốn luồng nghiệp vụ lớn nằm trong thư mục riêng: ask/ (hỏi đáp), explain/ (toán câu "tại sao",
ask/why.rs gọi run_why của nó), drill/ (truy dòng nguồn), understanding/ (thẻ "Đã hiểu là"),
semantic/ (hồ sơ ngữ nghĩa). File lẻ còn lại là tài khoản, workspace, nạp dữ liệu, dataset mẫu, hồ sơ cấu trúc. */

pub mod accounts;
pub mod ask;
pub mod catalog;
pub mod conversation;
pub mod domains;
pub mod drill;
pub mod eval;
pub mod eval_classify;
#[cfg(test)]
mod eval_classify_tests;
pub mod eval_judge;
#[cfg(test)]
mod eval_judge_tests;
#[cfg(test)]
mod eval_tests;
pub mod excel;
pub mod explain;
pub mod feedback;
pub mod ingest;
pub mod insight;
pub mod lessons;
pub mod llm_stats;
pub mod metrics;
pub mod overview;
pub mod profile;
#[cfg(test)]
pub(crate) mod prompt_fixtures;
pub mod samples;
pub mod samples_db;
#[cfg(test)]
mod samples_tests;
pub mod semantic;
pub mod sources;
pub mod understanding;
pub mod users;
pub mod workspaces;
