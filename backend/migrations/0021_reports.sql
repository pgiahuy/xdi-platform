-- SPDX-License-Identifier: Apache-2.0
-- 0021_reports.sql - báo cáo: người dùng gom các câu trả lời đã chọn thành một trang có tiêu đề và đoạn mở đầu, hay để
-- hệ thống tạo một báo cáo tự động cho một dataset (service/report/). Mỗi mục là một dòng queries đã trả lời; bảng kết
-- quả không lưu ở đây (như queries): mở báo cáo hay xuất Excel thì backend chạy lại câu SQL đã lưu trên đúng phiên bản
-- câu hỏi đã đọc, qua guard với quyền của người đang xem.
--
-- Như cuộc trò chuyện, mỗi báo cáo thuộc một người tạo và chỉ người đó đọc được, kể cả admin của workspace.
--
-- Số 0021 vì 0019 và 0020 dành cho các nhánh khác đang mở.

CREATE TABLE reports (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    -- Xóa tài khoản thì báo cáo của người đó mất theo, như cuộc trò chuyện của họ.
    created_by   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title        text NOT NULL CHECK (btrim(title) <> ''),
    -- Đoạn mở đầu người tạo tự viết; báo cáo tự động bắt đầu bằng một câu do code dựng.
    intro        text NOT NULL DEFAULT '',
    -- manual: người dùng tự thêm câu trả lời; automatic: hệ thống tạo từ một dataset.
    kind         text NOT NULL CHECK (kind IN ('manual', 'automatic')),
    -- Dataset (dòng sources) của báo cáo tự động; null với báo cáo tự gom.
    source_id    uuid,
    -- Số lần gọi mô hình khi tạo báo cáo tự động; null với báo cáo tự gom.
    model_calls  integer CHECK (model_calls >= 0),
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, id),
    CHECK ((kind = 'automatic') = (source_id IS NOT NULL)),
    -- Dataset chỉ xóa được khi đã xóa hết bảng (datasets ON DELETE RESTRICT), lúc đó các mục đã mất theo câu hỏi;
    -- báo cáo tự động của dataset đó không còn gì để hiện nên mất theo.
    FOREIGN KEY (workspace_id, source_id) REFERENCES sources (workspace_id, id) ON DELETE CASCADE
);
CREATE INDEX reports_owner_idx ON reports (workspace_id, created_by, updated_at DESC);

-- Khóa ngoại từ report_items gồm cả workspace_id như mọi khóa ngoại giữa bảng nghiệp vụ, nên cần khóa này trên queries.
ALTER TABLE queries ADD CONSTRAINT queries_workspace_id_id_key UNIQUE (workspace_id, id);

CREATE TABLE report_items (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    report_id    uuid NOT NULL,
    query_id     uuid NOT NULL,
    -- Thứ tự trong báo cáo, nhỏ trước; đổi thứ tự là ghi lại cả dãy.
    position     integer NOT NULL CHECK (position >= 0),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (report_id, query_id),
    FOREIGN KEY (workspace_id, report_id) REFERENCES reports (workspace_id, id) ON DELETE CASCADE,
    -- Xóa dataset hay phiên bản thì câu hỏi mất theo (0011), mục của nó cũng vậy: báo cáo không giữ số liệu của dữ
    -- liệu đã xóa.
    FOREIGN KEY (workspace_id, query_id) REFERENCES queries (workspace_id, id) ON DELETE CASCADE
);
CREATE INDEX report_items_report_idx ON report_items (report_id, position);
CREATE INDEX report_items_query_idx ON report_items (query_id);
