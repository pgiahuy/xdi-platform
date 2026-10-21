// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics), guard SQL, nhà cung cấp LLM.
Chỉ service gọi vào đây. */

// Khung: service nạp dữ liệu (phần API của #20) gọi analytics. Gỡ allow khi có code gọi.
#[allow(dead_code)]
pub mod analytics;
pub mod guard;
pub mod llm;
pub mod store;
