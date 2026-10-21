# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Khung dự án: backend Rust với `/api/health` và `/api/version` (mọi API nằm dưới `/api`, đường dẫn API sai trả 404 JSON), Postgres trong compose, Makefile, CI, tài liệu kiến trúc.
- Schema Postgres đầu tiên, tách dữ liệu theo workspace; backend tự chạy migration khi khởi động.
- Khung giao diện React và Vite; trang chủ hiện phiên bản máy chủ và mô hình ngôn ngữ đang dùng.
- Backend phục vụ giao diện cùng cổng với API; `docker compose up` rồi mở http://localhost:8000.
- Quy trình phát hành: binary Linux, tarball mã nguồn, image trên GitHub Container Registry; `make install` và `make uninstall`.
