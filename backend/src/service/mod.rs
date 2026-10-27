// SPDX-License-Identifier: Apache-2.0
/* service/mod.rs - tầng nghiệp vụ: ghép các bước, không biết gì về HTTP.
Gọi engine và auth; không bao giờ gọi ngược lên api.

Bốn luồng nghiệp vụ lớn nằm trong thư mục riêng: ask/ (hỏi đáp), explain/ (toán câu "tại sao",
ask/why.rs gọi run_why của nó), drill/ (truy dòng nguồn), understanding/ (thẻ "Đã hiểu là"),
semantic/ (hồ sơ ngữ nghĩa). File lẻ còn lại là tài khoản, workspace, nạp dữ liệu, hồ sơ cấu trúc. */

pub mod accounts;
pub mod ask;
pub mod drill;
pub mod excel;
pub mod explain;
pub mod ingest;
pub mod profile;
pub mod semantic;
pub mod understanding;
pub mod users;
pub mod workspaces;
