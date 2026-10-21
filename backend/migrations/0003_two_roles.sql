-- 0003_two_roles.sql - vai trong workspace còn hai: admin (quản lý workspace, dữ liệu, thành viên) và member (xem, hỏi).
-- Bốn vai cũ (owner, admin, analyst, viewer) gộp lại: owner thành admin, analyst và viewer thành member. Tách vai lại
-- sau này chỉ cần thêm giá trị vào ràng buộc dưới đây. Chú thích về cột role trong 0001_init.sql còn ghi bốn vai cũ;
-- không sửa được file đó vì migration đã áp không được đổi.
--
-- Workspace chỉ có analyst hoặc viewer sẽ không còn admin. Bản trước chưa phát hành và luôn đặt người tạo workspace là
-- admin, nên trường hợp này chỉ có ở dữ liệu thử; nếu gặp, quản trị hệ thống chỉ định admin qua
-- POST /api/admin/workspaces/{id}/admins.

-- Bỏ ràng buộc cũ trước khi đổi dữ liệu: ràng buộc cũ không cho giá trị 'member', UPDATE sẽ hỏng.
ALTER TABLE memberships DROP CONSTRAINT memberships_role_check;

UPDATE memberships SET role = 'admin' WHERE role = 'owner';
UPDATE memberships SET role = 'member' WHERE role IN ('analyst', 'viewer');

ALTER TABLE memberships ADD CONSTRAINT memberships_role_check CHECK (role IN ('admin', 'member'));
