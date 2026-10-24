-- SPDX-License-Identifier: Apache-2.0
-- 0004_column_time_format.sql - định dạng ngày của cột thời gian lưu dạng chữ.
-- Cột ngày trong CSV không theo dạng ISO (DataCo ghi "1/31/2018 22:56") được DuckDB đọc là VARCHAR. Hồ sơ cột (#22)
-- không đổi kiểu cột trong Parquet, mà ghi lại định dạng strptime đã đọc được mọi giá trị của cột, để câu "tại sao"
-- (#38) dựng strptime("cột", '<định dạng>') thay vì đoán lại. Null khi cột là DATE hoặc TIMESTAMP thật, hoặc không
-- phải cột thời gian.

ALTER TABLE columns ADD COLUMN time_format text;
