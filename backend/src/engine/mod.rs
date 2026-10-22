// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics), guard SQL, nhà cung cấp LLM.
Chỉ service gọi vào đây; csv_encoding (đổi CSV không phải UTF-8 sang UTF-8) chỉ analytics gọi. */

pub mod analytics;
mod csv_encoding;
pub mod excel;
pub mod excel_worker;
// Chưa có code gọi guard cho tới endpoint hỏi (#33). Gỡ dòng allow ở #33 để clippy bắt lại mã thừa.
#[allow(dead_code)]
pub mod guard;
pub mod llm;
pub mod store;
