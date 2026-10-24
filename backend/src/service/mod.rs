// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api. */

pub mod accounts;
pub mod ask;
// Hợp đồng khai đủ kiểu ô, vai trò cột và loại cảnh báo trước khi luồng hỏi (#33, #35, #36) dựng chúng; mẫu chỉ dùng
// một phần. Tạm cho phép "chưa dùng" ở module này; gỡ khi luồng hỏi chạy để clippy bắt lại.
#[allow(dead_code)]
pub mod ask_contract;
pub mod ask_examples;
#[cfg(test)]
mod ask_examples_tests;
pub mod excel;
// Phần toán của câu "tại sao" có trước SQL và luồng hỏi; nối vào ở #38/#39. Gỡ allow khi đó để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod explain;
#[cfg(test)]
mod explain_tests;
pub mod ingest;
pub mod users;
pub mod workspaces;
