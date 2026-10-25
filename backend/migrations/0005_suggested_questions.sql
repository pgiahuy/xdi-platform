-- SPDX-License-Identifier: Apache-2.0
-- 0005_suggested_questions.sql - câu hỏi gợi ý và dấu đã thử lập hồ sơ ngữ nghĩa của một phiên bản dataset (#23).
-- Gợi ý đi theo phiên bản vì cột và giá trị đổi theo phiên bản; trang hỏi (#50) đọc qua phiên bản hiện hành của
-- dataset. jsonb thay text[] để sau này thêm được thông tin cho từng câu (ví dụ câu nào đã chạy thử) mà không cần
-- migration; hiện tại là một mảng chuỗi, tối đa 5 câu (service/semantic.rs kiểm trước khi ghi).
-- semantic_attempted_with: tên provider của lần thử lớp ngữ nghĩa trả lời mà không dùng được (JSON hỏng, mô hình từ
-- chối nội dung, mock không có bảng). Việc nền lúc khởi động chỉ chạy cho phiên bản structure chưa thử với provider
-- đang cấu hình: lỗi như vậy không tự hết khi gọi lại y nguyên, nhưng đổi LLM_PROVIDER (ví dụ từ mock sang gemini)
-- thì thử lại. Lỗi tạm (Upstream: 429, timeout, mạng; Unavailable: thiếu cấu hình) không ghi cột này, lần khởi động
-- sau thử lại. Nạp lại cùng bộ file là đường thử lại tường minh, bỏ qua cột này.

ALTER TABLE dataset_versions ADD COLUMN suggested_questions jsonb;
ALTER TABLE dataset_versions ADD COLUMN semantic_attempted_with text;
