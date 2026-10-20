# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Khung dự án: backend Rust với `/health` và `/version`, Postgres trong compose, Makefile, CI, tài liệu kiến trúc.
- Schema Postgres đầu tiên, tách dữ liệu theo workspace; backend tự chạy migration khi khởi động.
- Khung giao diện React và Vite; trang chủ hiện phiên bản máy chủ và mô hình ngôn ngữ đang dùng.

### Changed

- Mọi API chuyển sang dưới `/api` (ví dụ `/api/health`, `/api/version`); đường dẫn API không tồn tại trả 404 JSON.
