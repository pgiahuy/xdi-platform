// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api. */

pub mod accounts;
pub mod ask;
pub mod explain;
pub mod ingest;
