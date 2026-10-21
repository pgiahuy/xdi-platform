-- 0002_accounts.sql - quản trị hệ thống và phiên đăng nhập.
-- Phiên lưu trong Postgres (không dùng JWT) để đăng xuất, khóa tài khoản, đổi vai có hiệu lực ngay ở request kế tiếp.

-- Quản trị hệ thống: quản lý tài khoản của cả bản cài. Không tự cho quyền đọc dữ liệu workspace; quyền đó đi qua
-- memberships như mọi người khác.
ALTER TABLE users ADD COLUMN is_system_admin boolean NOT NULL DEFAULT false;

-- Chú thích trong 0001 còn nhắc chế độ chạy không đăng nhập; chế độ đó đã bỏ. File migration đã chạy thì không sửa
-- (sqlx kiểm checksum), nên ghi lại ý nghĩa cột ở đây.
COMMENT ON COLUMN users.password_hash IS 'Argon2 hash in PHC format. Null only for accounts that sign in through SSO.';

CREATE TABLE sessions (
    -- SHA-256 của mã phiên nằm trong cookie, không lưu mã gốc: lộ bảng này cũng không dùng được để đăng nhập.
    token_hash    text PRIMARY KEY CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at    timestamptz NOT NULL DEFAULT now(),
    expires_at    timestamptz NOT NULL,
    CHECK (expires_at > created_at)
);
-- Xóa mọi phiên của một người (khóa tài khoản, đặt lại mật khẩu) đi theo user_id.
CREATE INDEX sessions_user_idx ON sessions (user_id);
