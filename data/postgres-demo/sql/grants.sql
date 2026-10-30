-- sql/grants.sql - cấp quyền chỉ đọc cho người dùng analyst (tạo trong init/01_roles.sql) trên hai schema edu và
-- sales. load.sh chạy file này cuối mỗi lần nạp, vì xóa schema thì quyền trên schema đó mất theo.
-- Không cấp gì trên schema staging: ứng dụng kết nối bằng analyst chỉ thấy bảng đã chuẩn hóa.

GRANT USAGE ON SCHEMA edu, sales TO analyst;
GRANT SELECT ON ALL TABLES IN SCHEMA edu, sales TO analyst;

-- Thêm một lớp chặn: mọi giao dịch của analyst mặc định chỉ đọc, kể cả khi sau này ai đó cấp nhầm quyền ghi.
ALTER ROLE analyst SET default_transaction_read_only = on;
