-- SPDX-License-Identifier: Apache-2.0
-- 0016_query_answer.sql - lời trả lời đã hiện cho người hỏi (#201): đoạn trả lời của câu thường hay lời giải thích của
-- câu "tại sao", kèm kết quả hai cổng kiểm (lời do mô hình viết hay câu của code, kết luận của mô hình chấm, các chỗ
-- chưa rõ). Mở lại cuộc trò chuyện, kể cả tải lại trang, thì hiện lại đúng lời này thay vì chỉ có câu hỏi. Không lưu
-- bảng kết quả: số liệu vẫn xem bằng cách hỏi lại trên dữ liệu hiện tại.
--
-- JSON dạng {"text", "by_model", "verdict", "warnings"} (service/conversation/contract.rs, SavedAnswer); null với câu
-- không answered và với các câu ghi trước migration này.
--
-- Số 0016 vì 0015 thuộc một nhánh khác đang mở (#252).

ALTER TABLE queries ADD COLUMN answer jsonb;
