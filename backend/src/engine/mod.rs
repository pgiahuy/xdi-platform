// SPDX-License-Identifier: Apache-2.0
/* engine/mod.rs - tầng hạ tầng: Postgres (store), DuckDB (analytics), guard SQL, nhà cung cấp LLM.
Chỉ service gọi vào đây. */

pub mod analytics;
pub mod excel;
pub mod excel_worker;
pub mod guard;
pub mod llm;
pub mod store;
