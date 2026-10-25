// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics nạp file, profile hồ sơ cột, query chạy câu
hỏi), guard SQL, nhà cung cấp LLM.
Chỉ service gọi vào đây; csv_encoding (đổi CSV không phải UTF-8 sang UTF-8) chỉ analytics gọi. */

pub mod analytics;
mod csv_encoding;
// Chưa có code gọi domains tới khi chọn gói cho dataset lúc nạp và đưa gói vào prompt (#29).
#[allow(dead_code)]
pub mod domains;
pub mod excel;
pub mod excel_worker;
// Chưa có code gọi guard cho tới endpoint hỏi (#33). Gỡ dòng allow ở #33 để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod guard;
pub mod llm;
pub mod profile;
// Chưa có code gọi run_query cho tới endpoint hỏi (#33); gỡ cùng dòng allow của guard.
#[allow(dead_code)]
pub mod query;
// Hình dạng câu đã chạy, cho thẻ "Đã hiểu là" (#35), truy dòng nguồn (#34), kiểm hợp lý (#36). Gỡ allow khi endpoint
// hỏi (#33) gọi tới, như guard.
#[allow(dead_code)]
pub mod sql_shape;
#[cfg(test)]
mod sql_shape_tests;
pub mod store;
