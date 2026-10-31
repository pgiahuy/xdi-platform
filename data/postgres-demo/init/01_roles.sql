-- init/01_roles.sql - chạy một lần khi container demo-db khởi tạo volume trống.
-- Tạo người dùng chỉ đọc "analyst" mà ứng dụng dùng để kết nối. Hai database sales_demo và edu_demo do load.sh tạo
-- (chạy được cả trên volume đã có từ trước), quyền CONNECT và SELECT cấp trong sql/grants.sql mỗi lần nạp, vì load.sh
-- xóa và tạo lại schema, quyền cũ mất theo schema.
-- Mật khẩu "analyst" chỉ dùng cho bản chạy trên máy, ghi trong README.md.

CREATE ROLE analyst LOGIN PASSWORD 'analyst';
