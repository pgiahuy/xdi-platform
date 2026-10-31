-- SPDX-License-Identifier: Apache-2.0
-- 0014_domain_concepts.sql - gói khái niệm ngành theo workspace (#229): admin xem, thêm, sửa, tắt khái niệm trên màn
-- Khái niệm ngành thay vì sửa file JSON rồi build lại. File data/domains/*.json vẫn là nơi đóng góp gói mới qua PR, và
-- là dữ liệu khởi tạo: service/domains/ chép gói, khái niệm còn thiếu theo khóa vào workspace (engine/store/domains.rs),
-- không ghi đè bản admin đã sửa. Prompt sinh SQL và form câu "tại sao" đọc khái niệm đang bật từ đây theo
-- datasets.domain.
--
-- Số 0014 giữ cho issue này; 0013 thuộc một nhánh khác đang mở.

-- key trùng tên file gói (retail.json thì key là retail) và là giá trị của datasets.domain.
CREATE TABLE domain_packs (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    key           text NOT NULL CHECK (key ~ '^[a-z0-9_]+$'),
    name          text NOT NULL CHECK (length(btrim(name)) > 0),
    description   text NOT NULL DEFAULT '',
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, key),
    -- Cho khóa ngoại theo cặp của domain_concepts.
    UNIQUE (workspace_id, id)
);

-- key là id của khái niệm trong file gói; null với khái niệm admin thêm. Nạp khởi tạo so theo (pack_id, key) để biết
-- khái niệm nào của file còn thiếu; UNIQUE cho phép nhiều dòng key null.
-- position giữ thứ tự của file, để prompt của workspace chưa sửa gì giống hệt prompt đọc từ file; khái niệm admin thêm
-- đứng sau.
-- calculations, sanity_checks cùng dạng với file (docs/DOMAINS.md); chưa code nào ngoài màn quản trị đọc hai cột này.
-- updated_at, updated_by null khi khái niệm chưa ai sửa từ lúc nạp khởi tạo.
CREATE TABLE domain_concepts (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id   uuid NOT NULL,
    pack_id        uuid NOT NULL,
    key            text,
    name           text NOT NULL CHECK (length(btrim(name)) > 0),
    description    text NOT NULL DEFAULT '',
    synonyms       text[] NOT NULL DEFAULT '{}',
    calculations   jsonb NOT NULL DEFAULT '[]',
    dimensions     text[] NOT NULL DEFAULT '{}',
    sanity_checks  jsonb NOT NULL DEFAULT '[]',
    -- Tắt thì khái niệm không vào prompt nữa, vẫn còn trong bảng để bật lại.
    enabled        boolean NOT NULL DEFAULT true,
    position       integer NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz,
    updated_by     uuid REFERENCES users (id) ON DELETE SET NULL,
    UNIQUE (pack_id, key),
    FOREIGN KEY (workspace_id, pack_id) REFERENCES domain_packs (workspace_id, id) ON DELETE CASCADE
);
CREATE INDEX domain_concepts_pack_idx ON domain_concepts (pack_id, position);
