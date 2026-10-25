-- SPDX-License-Identifier: Apache-2.0
-- 0006_query_explanation.sql - chỗ lưu phần giải thích của câu "tại sao" cùng dòng queries (#33).
-- Câu "tại sao" (run_why, #38) ghi vào đây form mô hình điền và phân rã đã tính, gồm điều kiện SQL của hai lát cắt. Truy dòng
-- nguồn (#34) đọc lại điều kiện từ đúng dòng của câu hỏi, nên client chỉ gửi query_id và tên lát cắt, không gửi SQL.
-- Null với câu hỏi thường và câu hỏi ghi trước migration này.
ALTER TABLE queries ADD COLUMN explanation jsonb;
