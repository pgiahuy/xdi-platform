-- SPDX-License-Identifier: Apache-2.0
-- 0005_suggested_questions.sql - câu hỏi gợi ý của một phiên bản dataset, do hồ sơ lớp ngữ nghĩa (#23) ghi.
-- Gợi ý đi theo phiên bản vì cột và giá trị đổi theo phiên bản; trang hỏi (#50) đọc qua phiên bản hiện hành của
-- dataset. jsonb thay text[] để sau này thêm được thông tin cho từng câu (ví dụ câu nào đã chạy thử) mà không cần
-- migration; hiện tại là một mảng chuỗi, tối đa 5 câu (service/semantic.rs kiểm trước khi ghi).

ALTER TABLE dataset_versions ADD COLUMN suggested_questions jsonb;
