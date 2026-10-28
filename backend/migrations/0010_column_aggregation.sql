-- SPDX-License-Identifier: Apache-2.0
-- 0010_column_aggregation.sql - cách gộp mặc định của cột số đo (#185): cộng (sum) hay lấy trung bình (avg).
-- Cột mô tả từng dòng (số tín chỉ, tuổi, điểm, số ngày giao) cộng lại thì vô nghĩa; prompt sinh SQL đọc cột này để
-- viết AVG, câu "tại sao" phân rã trung bình như một tỷ lệ (tử SUM, mẫu số dòng có giá trị). Lớp ngữ nghĩa đề xuất,
-- admin đổi ở màn Chỉ số. Mặc định sum là cách mọi bước đã làm trước migration này; với cột không phải measure, giá
-- trị không được đọc.
ALTER TABLE columns ADD COLUMN aggregation text NOT NULL DEFAULT 'sum' CHECK (aggregation IN ('sum', 'avg'));
