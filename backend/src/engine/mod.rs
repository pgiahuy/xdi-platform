// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics), guard SQL, nhà cung cấp LLM.
Chỉ service gọi vào đây; csv_encoding (đổi CSV không phải UTF-8 sang UTF-8) chỉ analytics gọi. */

pub mod analytics;
mod csv_encoding;
pub mod excel;
pub mod excel_worker;
// Khung: service/ask gọi guard từ issue endpoint hỏi (#33). Chưa có code gọi nên tạm cho phép "chưa dùng"; gỡ khi đó
// để clippy bắt lại.
#[allow(dead_code)]
pub mod guard;
pub mod llm;
pub mod store;
