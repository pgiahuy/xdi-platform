-- SPDX-License-Identifier: Apache-2.0
-- 0012_answer_feedback.sql - đánh giá câu trả lời và câu mẫu đã duyệt (#212).
-- queries.feedback (0001) giữ nguyên hai giá trị: 'correct' là nút "Tốt", 'incorrect' là nút "Chưa đạt". Thêm lý do và
-- ghi chú của "Chưa đạt", ai đánh giá và lúc nào, ai duyệt câu "Tốt" làm câu mẫu và lúc nào. Chỉ câu đã duyệt vào
-- prompt làm ví dụ (Store::list_approved_examples): câu người dùng tự đánh dấu "Tốt" chưa đủ, vì một câu SQL sai lọt
-- vào ví dụ thì mô hình chép cái sai sang mọi câu hỏi sau có từ chung.
ALTER TABLE queries
    ADD COLUMN feedback_reason text
        CHECK (feedback_reason IN ('misunderstood', 'wrong_numbers', 'unclear_explanation', 'other')),
    ADD COLUMN feedback_note text,
    -- SET NULL: xóa tài khoản thì đánh giá và lần duyệt vẫn còn, chỉ mất tên người.
    ADD COLUMN feedback_by uuid REFERENCES users (id) ON DELETE SET NULL,
    ADD COLUMN feedback_at timestamptz,
    ADD COLUMN example_approved_by uuid REFERENCES users (id) ON DELETE SET NULL,
    ADD COLUMN example_approved_at timestamptz,
    -- Lý do chỉ có nghĩa với "Chưa đạt"; câu mẫu chỉ là câu "Tốt". Đổi đánh giá từ "Tốt" sang khác thì ứng dụng xóa lần
    -- duyệt trong cùng câu lệnh; ràng buộc này chặn mọi đường ghi khác quên làm việc đó.
    ADD CONSTRAINT queries_feedback_reason_only_incorrect CHECK (feedback_reason IS NULL OR feedback = 'incorrect'),
    ADD CONSTRAINT queries_example_only_correct CHECK (example_approved_at IS NULL OR feedback = 'correct');

-- Danh sách câu "Tốt" cho admin duyệt đọc theo workspace; chỉ mục một phần vì đa số câu hỏi không được đánh giá.
CREATE INDEX queries_feedback_idx ON queries (workspace_id, feedback_at DESC) WHERE feedback IS NOT NULL;
