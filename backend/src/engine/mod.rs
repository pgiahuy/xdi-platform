// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics nạp file, profile hồ sơ cột, query chạy câu
hỏi), guard SQL, nhà cung cấp LLM, chữ cố định của các prompt (prompts, đọc từ backend/prompts/).
Chỉ service gọi vào đây; csv_encoding (đổi CSV không phải UTF-8 sang UTF-8) chỉ analytics gọi. */

pub mod analytics;
mod csv_encoding;
pub mod domains;
pub mod excel;
pub mod excel_worker;
pub mod guard;
pub mod llm;
pub mod profile;
pub mod prompts;
pub mod query;
// Hình dạng câu đã chạy, cho thẻ "Cách tính" (#35), truy dòng nguồn (#34), kiểm hợp lý (#36).
pub mod sql_shape;
#[cfg(test)]
mod sql_shape_tests;
pub mod store;
