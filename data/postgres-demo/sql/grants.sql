-- sql/grants.sql - cấp quyền chỉ đọc cho người dùng analyst (tạo trong init/01_roles.sql) trên schema dữ liệu của một
-- database: sales trong sales_demo, edu trong edu_demo. load.sh chạy file này cuối mỗi lần nạp, với hai biến psql
-- database và schema (psql -v database=sales_demo -v schema=sales), vì xóa schema thì quyền trên schema đó mất theo.
-- Không cấp gì trên schema staging: ứng dụng kết nối bằng analyst chỉ thấy bảng đã chuẩn hóa.

GRANT CONNECT ON DATABASE :"database" TO analyst;
GRANT USAGE ON SCHEMA :"schema" TO analyst;
GRANT SELECT ON ALL TABLES IN SCHEMA :"schema" TO analyst;

-- Người dùng chỉ đọc không được tạo bảng trong schema public. Postgres 15 trở lên đã mặc định như vậy; ghi lại cho rõ.
REVOKE CREATE ON SCHEMA public FROM PUBLIC;

-- Thêm một lớp chặn: mọi giao dịch của analyst mặc định chỉ đọc, kể cả khi sau này ai đó cấp nhầm quyền ghi. Đặt trên
-- role nên áp cho mọi database của container.
ALTER ROLE analyst SET default_transaction_read_only = on;
