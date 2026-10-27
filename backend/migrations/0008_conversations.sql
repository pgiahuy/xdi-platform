-- SPDX-License-Identifier: Apache-2.0
-- 0008_conversations.sql - cuộc trò chuyện trên một dataset (#164): câu sau hiểu theo các câu trước.
-- Mỗi cuộc thuộc một người tạo; chỉ người đó đọc và hỏi tiếp được (service/conversation/). Các lượt là các dòng queries
-- có conversation_id; queries.understood lưu phần "Đã hiểu là" dạng chữ ngắn để gửi lại mô hình ở lượt sau, không lưu
-- dòng dữ liệu hay giá trị kết quả (docs/PRIVACY.md).
CREATE TABLE conversations (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id   uuid NOT NULL,
    created_by   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title        text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    -- Cuộc trò chuyện chỉ có nghĩa trên dataset của nó: xóa dataset thì xóa cuộc, các câu hỏi vẫn ở lại lịch sử.
    FOREIGN KEY (workspace_id, dataset_id) REFERENCES datasets (workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX conversations_owner_idx ON conversations (workspace_id, dataset_id, created_by, created_at DESC);

-- SET NULL thay cho CASCADE: lịch sử câu hỏi giữ lại khi cuộc trò chuyện bị xóa, như khi dataset bị xóa.
ALTER TABLE queries ADD COLUMN conversation_id uuid REFERENCES conversations (id) ON DELETE SET NULL;
ALTER TABLE queries ADD COLUMN understood text;

CREATE INDEX queries_conversation_idx ON queries (conversation_id, created_at);
