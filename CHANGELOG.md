# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

## [0.1.0] - 2026-10-26

### Added

- Nền tảng: backend Rust (Axum, DuckDB nhúng, Postgres), giao diện React và Vite do backend phục vụ cùng cổng, một image Docker chứa cả hai; hướng dẫn cài đặt, đưa lên Internet, sao lưu và khôi phục ở `docs/INSTALL.md`.
- Tài khoản và workspace: mã thiết lập in ra log lần chạy đầu, vai admin và member trong từng workspace, trang quản trị cho quản trị hệ thống.
- Nạp dữ liệu: CSV (kể cả windows-1252), Parquet, Excel nhiều sheet; mỗi lần nạp một phiên bản, nạp trùng bỏ qua; hồ sơ cột tự động (kiểu, vai trò, đơn vị, cột nhạy cảm) và câu hỏi gợi ý do mô hình đề xuất.
- Hỏi dữ liệu bằng tiếng Việt: mô hình ngôn ngữ (Gemini, Ollama, hoặc mock chạy offline) sinh SQL, guard chỉ cho truy vấn và tiêm phân quyền, tự sửa một vòng khi SQL lỗi, hỏi lại khi câu mơ hồ.
- Giải thích từng con số: thẻ "Đã hiểu là" với giả định đổi được, pane bằng chứng (câu SQL trước và sau guard, các bước backend), câu "tại sao" phân rã theo từng chiều, bấm ô số hoặc dòng phân rã để xem dòng nguồn, cảnh báo khi kết quả đáng ngờ.
- Biểu đồ chọn theo chuẩn IBCS; số và lời giải thích định dạng kiểu Việt Nam.
- Đặc tả OpenAPI sinh từ code (`/api/openapi.json`, Swagger UI ở `/api/docs`); quy trình phát hành gồm binary Linux, tarball mã nguồn và image trên GitHub Container Registry.
