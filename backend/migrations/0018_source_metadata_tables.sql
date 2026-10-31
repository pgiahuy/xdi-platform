-- SPDX-License-Identifier: Apache-2.0
-- 0018_source_metadata_tables.sql - bảng của nguồn database mới có metadata, chưa có dữ liệu (#247).
--
-- Kết nối database thì mặc định chụp mọi bảng tài khoản đọc được (service/sources/). Bảng quá SOURCE_MAX_ROWS, hay
-- không còn chỗ trong trần dung lượng của một lần chụp (SOURCE_SNAPSHOT_MAX_MB), không chép dòng nào mà chỉ ghi cấu
-- trúc ở đây: cột, kiểu, khóa chính, mô tả (COMMENT ON) của bảng và cột. Câu hỏi cần bảng đó thì luồng hỏi nêu đúng tên
-- bảng và nói nó chưa được chụp (service/ask/unsnapshotted.rs), thay vì "không có thông tin". Admin chụp bảng đó sau
-- bằng POST /sources/{id}/snapshot với đúng một bảng; chụp được thì dòng ở đây bị xóa và bảng thành dataset như mọi bảng.
--
-- Không tạo dòng datasets cho các bảng này: dataset luôn có phiên bản Parquet để hỏi, đọc bằng chứng và truy dòng nguồn;
-- một dataset rỗng sẽ phải được mọi đường đọc dataset bỏ qua. Bảng đã chụp ghi ở sources.config.tables như trước.

CREATE TABLE source_metadata_tables (
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    source_id     uuid NOT NULL,
    -- schema.bảng ở database nguồn, như sources.config.tables.
    table_name    text NOT NULL CHECK (length(table_name) > 0),
    -- row_limit: quá SOURCE_MAX_ROWS; size_limit: cộng vào thì quá SOURCE_SNAPSHOT_MAX_MB.
    cause         text NOT NULL CHECK (cause IN ('row_limit', 'size_limit')),
    -- Lời giải thích cho admin, tiếng Việt, ghi luôn số dòng hay dung lượng và tên biến cấu hình.
    reason        text NOT NULL,
    -- Số dòng đếm được (hay ước lượng của Postgres khi bảng quá lớn để đếm) và dung lượng Postgres báo, lúc ghi.
    row_count     bigint,
    size_bytes    bigint,
    -- COMMENT ON TABLE đã làm sạch (service/sources/comments.rs).
    description   text,
    -- [{"name", "data_type", "nullable", "primary_key", "description"}], theo thứ tự cột của bảng.
    columns       jsonb NOT NULL DEFAULT '[]',
    updated_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source_id, table_name),
    -- Xóa nguồn thì các dòng này mất theo; không như datasets, chúng không giữ dữ liệu nào cần xóa tay trước.
    FOREIGN KEY (workspace_id, source_id) REFERENCES sources (workspace_id, id) ON DELETE CASCADE
);
