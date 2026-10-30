-- init/01_roles.sql - chạy một lần khi container demo-db khởi tạo volume trống.
-- Tạo người dùng chỉ đọc "analyst" mà ứng dụng dùng để kết nối. Quyền SELECT trên từng schema được cấp trong
-- sql/grants.sql, vì load.sh xóa và tạo lại schema mỗi lần nạp, quyền cũ mất theo schema.
-- Mật khẩu "analyst" chỉ dùng cho bản chạy trên máy, ghi trong README.md.

CREATE ROLE analyst LOGIN PASSWORD 'analyst';

-- Người dùng chỉ đọc không được tạo bảng trong schema public.
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT CONNECT ON DATABASE company_demo TO analyst;
