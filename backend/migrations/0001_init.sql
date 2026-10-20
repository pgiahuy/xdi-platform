-- SPDX-License-Identifier: Apache-2.0
-- 0001_init.sql - schema đầu tiên của dữ liệu ứng dụng.
-- Mọi bảng nghiệp vụ có workspace_id: dữ liệu của một workspace không bao giờ lẫn sang workspace khác.
-- users là bảng chung vì một người có thể thuộc nhiều workspace (qua memberships).
-- Dữ liệu phân tích không nằm ở đây: nó là file Parquet, DuckDB đọc trực tiếp. Xem docs/ARCHITECTURE.md mục 4.

CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS vector;

-- Workspace: một phòng ban, một workspace cá nhân, hoặc một workspace tạm của phiên demo.
CREATE TABLE workspaces (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name        text NOT NULL,
    slug        text NOT NULL UNIQUE,
    kind        text NOT NULL DEFAULT 'team' CHECK (kind IN ('team', 'personal', 'demo_session')),
    -- Chỉ workspace tạm có hạn; tiến trình dọn dẹp xóa khi quá hạn.
    expires_at  timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email          text NOT NULL,
    display_name   text NOT NULL DEFAULT '',
    -- Null khi chạy AUTH_MODE=single hoặc đăng nhập qua SSO.
    password_hash  text,
    is_active      boolean NOT NULL DEFAULT true,
    created_at     timestamptz NOT NULL DEFAULT now()
);
-- Email so không phân biệt hoa thường.
CREATE UNIQUE INDEX users_email_key ON users (lower(email));

-- Vai trong ứng dụng (role) quyết định được làm gì; data_role quyết định thấy dòng nào (xem rls_policies).
CREATE TABLE memberships (
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role          text NOT NULL CHECK (role IN ('owner', 'admin', 'analyst', 'viewer')),
    data_role     text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);
CREATE INDEX memberships_user_idx ON memberships (user_id);

-- Nơi dữ liệu đến từ đâu. Bản đầu chỉ có local_file; các kind khác dành cho nguồn database.
CREATE TABLE sources (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id      uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    kind              text NOT NULL CHECK (kind IN ('local_file', 'postgres', 'mysql', 'sqlite')),
    config            jsonb NOT NULL DEFAULT '{}',
    -- Chuỗi kết nối có mật khẩu, mã hóa trước khi lưu. Không bao giờ trả về qua API.
    secret_encrypted  bytea,
    created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sources_workspace_idx ON sources (workspace_id);

CREATE TABLE datasets (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id        uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    source_id           uuid NOT NULL REFERENCES sources (id) ON DELETE RESTRICT,
    name                text NOT NULL,
    -- Tên bảng logic mà mô hình ngôn ngữ dùng khi viết SQL; engine đổi sang nguồn thật.
    logical_table       text NOT NULL,
    -- Gói domain áp dụng, ví dụ 'retail'. Null nếu không có gói phù hợp.
    domain              text,
    -- Phiên bản đang dùng để trả lời câu hỏi. Khóa ngoại thêm sau khi có bảng dataset_versions.
    current_version_id  uuid,
    created_by          uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at          timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, logical_table)
);
CREATE INDEX datasets_workspace_idx ON datasets (workspace_id);

-- Mỗi lần nạp file là một phiên bản, không ghi đè. Với nguồn database, phiên bản là bản chụp hồ sơ tại một thời điểm.
CREATE TABLE dataset_versions (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id      uuid NOT NULL REFERENCES datasets (id) ON DELETE CASCADE,
    version         integer NOT NULL CHECK (version > 0),
    source_kind     text NOT NULL CHECK (source_kind IN ('csv', 'parquet', 'xlsx', 'postgres', 'mysql', 'sqlite')),
    file_hash       text,
    parquet_path    text,
    row_count       bigint,
    profile_status  text NOT NULL DEFAULT 'pending' CHECK (profile_status IN ('pending', 'structure', 'semantic', 'failed')),
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (dataset_id, version)
);
CREATE INDEX dataset_versions_workspace_idx ON dataset_versions (workspace_id);
-- Cùng nội dung file thì không tạo phiên bản mới.
CREATE UNIQUE INDEX dataset_versions_hash_key ON dataset_versions (dataset_id, file_hash) WHERE file_hash IS NOT NULL;

ALTER TABLE datasets
    ADD CONSTRAINT datasets_current_version_fk
    FOREIGN KEY (current_version_id) REFERENCES dataset_versions (id) ON DELETE SET NULL;

-- Hồ sơ cột của một phiên bản. Lớp cấu trúc do code điền; mô tả, bí danh do mô hình ngôn ngữ đề xuất và người dùng sửa.
CREATE TABLE columns (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    version_id      uuid NOT NULL REFERENCES dataset_versions (id) ON DELETE CASCADE,
    position        integer NOT NULL,
    name            text NOT NULL,
    data_type       text NOT NULL,
    role            text NOT NULL DEFAULT 'ignore' CHECK (role IN ('dimension', 'measure', 'time', 'key', 'text', 'ignore')),
    description     text,
    aliases         text[] NOT NULL DEFAULT '{}',
    null_count      bigint,
    distinct_count  bigint,
    min_value       text,
    max_value       text,
    sample_values   jsonb,
    -- Cột nhạy cảm: không đưa giá trị mẫu vào prompt.
    is_sensitive    boolean NOT NULL DEFAULT false,
    -- Để dành cho tìm theo nghĩa; chưa dùng.
    embedding       vector,
    UNIQUE (version_id, name)
);
CREATE INDEX columns_workspace_idx ON columns (workspace_id);

-- Giá trị của cột phân loại, để ánh xạ "miền Nam" sang "South".
CREATE TABLE column_values (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    column_id     uuid NOT NULL REFERENCES columns (id) ON DELETE CASCADE,
    value         text NOT NULL,
    aliases       text[] NOT NULL DEFAULT '{}',
    frequency     bigint,
    embedding     vector,
    UNIQUE (column_id, value)
);
CREATE INDEX column_values_workspace_idx ON column_values (workspace_id);
CREATE INDEX column_values_trgm_idx ON column_values USING gin (lower(value) gin_trgm_ops);

-- Định nghĩa chỉ số do tổ chức cung cấp. Sửa định nghĩa là thêm phiên bản mới; câu hỏi cũ vẫn tái lập được.
CREATE TABLE metrics (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id    uuid NOT NULL REFERENCES datasets (id) ON DELETE CASCADE,
    name          text NOT NULL,
    description   text NOT NULL DEFAULT '',
    -- Một biểu thức tổng hợp trên cột của dataset, ví dụ SUM("Order Item Total").
    expression    text NOT NULL,
    filter        text,
    time_column   text,
    unit          text,
    status        text NOT NULL DEFAULT 'unconfirmed' CHECK (status IN ('confirmed', 'unconfirmed')),
    origin        text NOT NULL CHECK (origin IN ('formula_table', 'ui', 'llm_suggested', 'sample')),
    version       integer NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by    uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (dataset_id, name, version)
);
CREATE INDEX metrics_workspace_idx ON metrics (workspace_id);

-- Lọc dòng theo data_role của thành viên: guard thêm điều kiện column_name IN allowed_values.
CREATE TABLE rls_policies (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id      uuid NOT NULL REFERENCES datasets (id) ON DELETE CASCADE,
    data_role       text NOT NULL,
    column_name     text NOT NULL,
    allowed_values  text[] NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (dataset_id, data_role, column_name)
);
CREATE INDEX rls_policies_workspace_idx ON rls_policies (workspace_id);

-- Lịch sử câu hỏi; câu đánh dấu đúng còn dùng làm ví dụ cho câu sau và cho bộ đánh giá.
CREATE TABLE queries (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id   uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    dataset_id     uuid REFERENCES datasets (id) ON DELETE SET NULL,
    version_id     uuid REFERENCES dataset_versions (id) ON DELETE SET NULL,
    user_id        uuid REFERENCES users (id) ON DELETE SET NULL,
    question       text NOT NULL,
    sql_generated  text,
    sql_guarded    text,
    status         text NOT NULL CHECK (status IN ('ok', 'rejected', 'error')),
    error          text,
    timings        jsonb NOT NULL DEFAULT '{}',
    row_count      bigint,
    feedback       text CHECK (feedback IN ('correct', 'incorrect')),
    embedding      vector,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX queries_workspace_idx ON queries (workspace_id, created_at DESC);

CREATE TABLE settings (
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    key           text NOT NULL,
    value         jsonb NOT NULL,
    updated_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, key)
);
