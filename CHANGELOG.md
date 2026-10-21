# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/vi/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Khung dự án: backend Rust với `/api/health` và `/api/version` (mọi API nằm dưới `/api`, đường dẫn API sai trả 404 JSON), Postgres trong compose, Makefile, CI, tài liệu kiến trúc.
- Schema Postgres đầu tiên, tách dữ liệu theo workspace; backend tự chạy migration khi khởi động.
- Khung giao diện React và Vite.
- Backend phục vụ giao diện cùng cổng với API; `docker compose up` rồi mở http://localhost:8000.
- Hướng dẫn cài đặt `docs/INSTALL.md`: Docker, build từ nguồn, đưa lên Internet với Caddy và HTTPS, sao lưu và khôi phục, lỗi hay gặp.
- Tài liệu giấy phép thành phần (`docs/LICENSES.md`, `make licenses`) và quyền riêng tư (`docs/PRIVACY.md`); giao diện kèm giấy phép các gói đã đóng ở `/licenses.md`.
- Quy trình phát hành: binary Linux, tarball mã nguồn, image trên GitHub Container Registry; `make install` và `make uninstall`.
- Tài khoản: lần chạy đầu in mã thiết lập ra log, dùng mã để tạo tài khoản quản trị kèm workspace đầu tiên; đăng nhập, đăng xuất, đổi mật khẩu qua API.
- Nạp dữ liệu: một hoặc nhiều file CSV (UTF-8, latin-1) hoặc Parquet; nhiều file cùng cột gộp thành một bảng, có cột `_source_file` ghi file gốc; mỗi lần nạp là một phiên bản, nạp lại đúng bộ file cũ thì bỏ qua; giới hạn dung lượng `MAX_UPLOAD_MB` (mặc định 200).
- Nạp file Excel (.xlsx): xem danh sách sheet kèm cột, chọn một hoặc nhiều sheet; các sheet cùng cột gộp thành một bảng, cột `_source_file` ghi tên sheet; sheet khác cột nạp riêng thành dataset riêng.
- Workspace: mỗi người dùng tạo được nhiều workspace; API dữ liệu nằm dưới `/api/workspaces/{slug}/`, chỉ thành viên truy cập được.
- Quản lý tài khoản cho quản trị hệ thống: tạo, khóa, mở tài khoản, đặt lại mật khẩu bằng mật khẩu tạm; lệnh `xdi-backend reset-password` và `xdi-backend list-admins` chạy trên máy chủ.
- Vai trong workspace: `admin` nạp dữ liệu, đổi tên, xóa workspace, thêm, đổi vai, xóa thành viên; `member` xem và hỏi. Workspace luôn còn ít nhất một admin. Quản trị hệ thống xem mọi workspace kèm thành viên (không xem dữ liệu) và chỉ định admin cho bất kỳ workspace nào, kể cả chính mình; mỗi lần chỉ định ghi vào log của máy chủ.
- Bố cục chung của giao diện: thanh bên có nút đổi workspace (hiện vai của mình ở từng workspace) và mục điều hướng, thanh trên có tài khoản và đăng xuất; trên điện thoại thanh bên mở dạng ngăn kéo. Trang làm việc nằm dưới `/w/<workspace>`; phiên bản máy chủ và mô hình ngôn ngữ đang dùng hiện ở chân thanh bên.
