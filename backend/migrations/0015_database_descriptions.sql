-- SPDX-License-Identifier: Apache-2.0
-- 0015_database_descriptions.sql - mô tả bảng và cột mà tổ chức đã ghi trong database nguồn (COMMENT ON, #245).
-- Lúc chụp bảng Postgres (service/sources/), comment của từng cột thành mô tả cột ngay ở hồ sơ cấu trúc, ghi nguồn
-- 'database'; mô tả do lớp ngữ nghĩa viết ghi nguồn 'model'. Lớp ngữ nghĩa không ghi đè mô tả nguồn 'database'
-- (engine/store/columns.rs, apply_semantic_profile), và giao diện ghi rõ mô tả từ đâu ra. Mô tả trước migration này
-- đều do mô hình viết. Null là không rõ nguồn (chưa có mô tả, hay dòng ghi thẳng bằng SQL): giao diện không ghi nguồn.
ALTER TABLE columns ADD COLUMN description_source text CHECK (description_source IN ('database', 'model'));
UPDATE columns SET description_source = 'model' WHERE description IS NOT NULL;

-- Comment của bảng (COMMENT ON TABLE) lúc chụp. Nằm trên phiên bản chứ không trên dataset: mỗi lần chụp là một ảnh của
-- database tại một thời điểm, comment đổi thì phiên bản sau mang comment mới. Null với file nạp tay hay bảng không có
-- comment.
ALTER TABLE dataset_versions ADD COLUMN description text;
