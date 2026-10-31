-- SPDX-License-Identifier: Apache-2.0
-- 0013_error_log_lessons.sql - nhật ký lỗi và bài học (#228): hệ thống ghi lại chỗ mô hình sai, admin viết bài học ngắn
-- từ đó, bài học đã duyệt và đang bật vào prompt của các bước dùng mô hình (service/lessons/).
--
-- error_log ghi tự động, không ai sửa: guard từ chối, SQL lỗi khi chạy, câu cộng lặp bị chặn, form câu "tại sao" sai,
-- bộ kiểm số hay kiểm chiều chặn lời mô hình viết, người hỏi bấm "Chưa đạt". Có câu hỏi và câu mô hình trả nên chỉ admin
-- của workspace đọc được (docs/PRIVACY.md).
CREATE TABLE error_log (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id   uuid,
    -- Câu bị ghi lịch sử; null khi câu chưa được ghi (còn chờ chọn cách hiểu) hay dòng lịch sử đã bị xóa.
    query_id     uuid REFERENCES queries (id) ON DELETE SET NULL,
    question     text NOT NULL,
    step         text NOT NULL CHECK (step IN ('guard', 'run', 'fanout', 'why_form', 'narrate', 'feedback')),
    error        text NOT NULL,
    -- Câu mô hình trả ở bước đó (SQL, form JSON, lời viết); null khi lỗi không có câu của mô hình.
    model_output text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, id),
    -- Xóa dataset thì giữ nhật ký như giữ lịch sử câu hỏi, chỉ đặt null dataset_id.
    FOREIGN KEY (workspace_id, dataset_id) REFERENCES datasets (workspace_id, id) ON DELETE SET NULL (dataset_id)
);
CREATE INDEX error_log_workspace_idx ON error_log (workspace_id, created_at DESC);

-- Bài học: một câu ngắn admin viết (tay hay từ một dòng nhật ký). Mới tạo là bản nháp; chỉ bài đã duyệt và đang bật vào
-- prompt. Mỗi lần bật ghi lại người duyệt và lúc duyệt.
CREATE TABLE lessons (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    -- Null là bài học của cả workspace; có giá trị thì chỉ câu hỏi trên dataset đó nhận.
    dataset_id   uuid,
    text         text NOT NULL CHECK (btrim(text) <> ''),
    enabled      boolean NOT NULL DEFAULT false,
    error_id     uuid,
    created_by   uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    approved_by  uuid REFERENCES users (id) ON DELETE SET NULL,
    approved_at  timestamptz,
    CHECK (NOT enabled OR approved_at IS NOT NULL),
    -- Bài học của một dataset mất nghĩa khi dataset bị xóa.
    FOREIGN KEY (workspace_id, dataset_id) REFERENCES datasets (workspace_id, id) ON DELETE CASCADE,
    -- Dòng nhật ký phải cùng workspace; xóa dòng đó thì bài học vẫn còn.
    FOREIGN KEY (workspace_id, error_id) REFERENCES error_log (workspace_id, id) ON DELETE SET NULL (error_id)
);
CREATE INDEX lessons_workspace_idx ON lessons (workspace_id, created_at DESC);
