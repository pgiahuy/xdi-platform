# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Khung dự án: backend Rust với `/api/health` và `/api/version` (mọi API nằm dưới `/api`, đường dẫn API sai trả 404 JSON), Postgres trong compose, Makefile, CI, tài liệu kiến trúc.
- Schema Postgres đầu tiên, tách dữ liệu theo workspace; backend tự chạy migration khi khởi động.
- Khung giao diện React và Vite.
- Backend phục vụ giao diện cùng cổng với API; `docker compose up` rồi mở http://localhost:8000.
- Hướng dẫn cài đặt `docs/INSTALL.md`: Docker, build từ nguồn, đưa lên Internet với Caddy và HTTPS, sao lưu và khôi phục, lỗi hay gặp; RAM và dung lượng đĩa cần khi build image (đo trên máy thật), cách build giới hạn số nhân CPU cho máy ít RAM.
- Tài liệu giấy phép thành phần (`docs/LICENSES.md`, `make licenses`) và quyền riêng tư (`docs/PRIVACY.md`); giao diện kèm giấy phép các gói đã đóng ở `/licenses.md`.
- Quy trình phát hành: binary Linux, tarball mã nguồn, image trên GitHub Container Registry; `make install` và `make uninstall`.
- Tài khoản: lần chạy đầu in mã thiết lập ra log, dùng mã để tạo tài khoản quản trị kèm workspace đầu tiên; đăng nhập, đăng xuất, đổi mật khẩu qua API.
- Nạp dữ liệu: một hoặc nhiều file CSV (UTF-8, hoặc windows-1252 và latin-1 như file xuất từ phần mềm cũ; file UTF-8 có byte hỏng thì báo dòng lỗi) hoặc Parquet; nhiều file cùng cột gộp thành một bảng, có cột `_source_file` ghi file gốc; mỗi lần nạp là một phiên bản, nạp lại đúng bộ file cũ thì bỏ qua; giới hạn dung lượng `MAX_UPLOAD_MB` (mặc định 200).
- Nạp file Excel (.xlsx): xem danh sách sheet kèm cột, chọn một hoặc nhiều sheet; các sheet cùng cột gộp thành một bảng, cột `_source_file` ghi tên sheet; sheet khác cột nạp riêng thành dataset riêng.
- Workspace: mỗi người dùng tạo được nhiều workspace; API dữ liệu nằm dưới `/api/workspaces/{slug}/`, chỉ thành viên truy cập được.
- Quản lý tài khoản cho quản trị hệ thống: tạo, khóa, mở tài khoản, đặt lại mật khẩu bằng mật khẩu tạm; lệnh `xdi-backend reset-password` và `xdi-backend list-admins` chạy trên máy chủ.
- Vai trong workspace: `admin` nạp dữ liệu, đổi tên, xóa workspace, thêm, đổi vai, xóa thành viên; `member` xem và hỏi. Workspace luôn còn ít nhất một admin. Quản trị hệ thống xem mọi workspace kèm thành viên (không xem dữ liệu) và chỉ định admin cho bất kỳ workspace nào, kể cả chính mình; mỗi lần chỉ định ghi vào log của máy chủ.
- Bố cục chung của giao diện: thanh bên có nút đổi workspace (hiện vai của mình ở từng workspace) và mục điều hướng, thanh trên có tài khoản và đăng xuất; trên điện thoại thanh bên mở dạng ngăn kéo. Trang làm việc nằm dưới `/w/<workspace>`; phiên bản máy chủ và mô hình ngôn ngữ đang dùng hiện ở chân thanh bên.
- Viền focus đậm hơn, tương phản từ 3:1 với nền ở nền sáng (WCAG 2.2 mục 1.4.11); mục menu chọn bằng bàn phím có viền riêng.
- Giao diện đăng nhập: màn thiết lập lần đầu (mã thiết lập, tài khoản quản trị, workspace đầu tiên), màn đăng nhập (đăng nhập xong quay lại trang đang mở), đổi mật khẩu ở trang Tài khoản. Chưa đăng nhập hoặc phiên hết hạn thì chuyển tới màn đăng nhập.
- Trang Dữ liệu: danh sách dataset của workspace; admin nạp file CSV, Parquet hoặc Excel ngay trên trang, xem tiến trình gửi, chọn sheet của file Excel (sheet khác cột thành dataset riêng), nạp phiên bản mới cho dataset có sẵn. Nạp trùng báo là đã có; lỗi hiện đúng lời máy chủ trả.
- Trang Thành viên trong mỗi workspace: ai cũng xem danh sách và tự rời được; admin thêm thành viên theo email, đổi vai, xóa thành viên, đổi tên và xóa workspace. Rời, xóa, tự hạ vai đều hỏi xác nhận; lời từ chối của backend (ví dụ hạ vai admin cuối cùng) hiện nguyên văn.
- Trang quản trị cho quản trị hệ thống, mục riêng trên thanh bên: Quản lý tài khoản (tạo, khóa, mở, đặt lại mật khẩu, mật khẩu tạm hiện một lần) và Mọi workspace (xem thành viên, chỉ định admin).
- Đặc tả OpenAPI sinh từ code ở `/api/openapi.json`, Swagger UI ở `/api/docs`; `openapi.json` và kiểu TypeScript của giao diện sinh từ đặc tả (`make openapi`), test và CI báo lỗi khi lệch code.
- Cấu hình chạy thật: `docker-compose.prod.yml` (bật bằng `COMPOSE_FILE` trong `.env`) không mở cổng Postgres, backend chỉ nghe `127.0.0.1`, bắt buộc đặt `POSTGRES_PASSWORD` và `PUBLIC_URL`; cookie đăng nhập có cờ `Secure` khi `PUBLIC_URL` bắt đầu bằng `https://`; backend cảnh báo lúc khởi động khi còn dùng mật khẩu Postgres mẫu; `DB_ACQUIRE_TIMEOUT_SECS` đặt thời gian chờ kết nối Postgres.
- Backend gửi `Content-Security-Policy`, `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy` cho giao diện, API và Swagger UI.
- `make backup` và `make restore` sao lưu, khôi phục database và file dữ liệu; `docs/INSTALL.md` có thêm mẫu nginx cạnh Caddy.
