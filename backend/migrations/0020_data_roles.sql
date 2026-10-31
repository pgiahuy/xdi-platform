-- SPDX-License-Identifier: Apache-2.0
-- 0020_data_roles.sql - vai dữ liệu có định nghĩa riêng trong workspace, một thành viên mang được nhiều vai (#45, #167).
--
-- Trước đây vai dữ liệu chỉ là chữ tự do: memberships.data_role (mỗi người tối đa một vai) và rls_policies.data_role.
-- Gõ sai tên thì không có gì từ chối, và không trả lời được "vai X gồm những ai". Nay:
-- - data_roles: mỗi vai một dòng (tên, mô tả); tên không trùng trong workspace, không phân biệt hoa thường.
-- - member_data_roles: vai của từng thành viên, mỗi người nhiều vai.
-- - rls_policies trỏ tới vai bằng data_role_id; xóa vai thì luật của nó bị xóa theo.
-- Cách các vai ghép với nhau (AND trong một vai, OR giữa các vai) ở service/ask/row_filters.rs và docs/ARCHITECTURE.md
-- mục 4b.
--
-- Chuyển dữ liệu: mỗi tên vai khác nhau đang có trong memberships hay rls_policies của một workspace thành một dòng
-- data_roles; người đang mang vai nào thì vẫn mang vai đó, luật giữ nguyên, nên không ai đổi phạm vi dữ liệu. Sau đó
-- bỏ cột memberships.data_role và rls_policies.data_role để chỉ còn một chỗ lưu.

CREATE TABLE data_roles (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    name          text NOT NULL CHECK (length(btrim(name)) > 0),
    description   text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, id)
);
CREATE UNIQUE INDEX data_roles_name_key ON data_roles (workspace_id, lower(name));

INSERT INTO data_roles (workspace_id, name)
SELECT DISTINCT workspace_id, data_role FROM (
    SELECT workspace_id, data_role FROM memberships WHERE data_role IS NOT NULL AND btrim(data_role) <> ''
    UNION
    SELECT workspace_id, data_role FROM rls_policies
) AS old_roles
-- Hai tên chỉ khác hoa thường thì giữ một dòng (chỉ mục tên không phân biệt hoa thường); luật và người mang vai của cả
-- hai tên nhập vào dòng đó ở dưới.
ON CONFLICT DO NOTHING;

CREATE TABLE member_data_roles (
    workspace_id  uuid NOT NULL,
    user_id       uuid NOT NULL,
    data_role_id  uuid NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id, data_role_id),
    -- Rời workspace thì mất vai; xóa vai thì gỡ khỏi mọi người (service chặn xóa vai đang gán, xem service/data_roles.rs).
    FOREIGN KEY (workspace_id, user_id) REFERENCES memberships (workspace_id, user_id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, data_role_id) REFERENCES data_roles (workspace_id, id) ON DELETE CASCADE
);
CREATE INDEX member_data_roles_role_idx ON member_data_roles (data_role_id);

INSERT INTO member_data_roles (workspace_id, user_id, data_role_id)
SELECT m.workspace_id, m.user_id, r.id
FROM memberships m
JOIN data_roles r ON r.workspace_id = m.workspace_id AND lower(r.name) = lower(m.data_role);

ALTER TABLE rls_policies ADD COLUMN data_role_id uuid;
UPDATE rls_policies p SET data_role_id = r.id
FROM data_roles r WHERE r.workspace_id = p.workspace_id AND lower(r.name) = lower(p.data_role);
-- Hai tên cũ chỉ khác hoa thường có thể cùng có luật trên một cột: gộp giá trị của chúng vào một dòng, để ràng buộc
-- UNIQUE dưới đây đứng được và người mang một trong hai tên vẫn thấy đúng những dòng như trước.
UPDATE rls_policies keep SET allowed_values = ARRAY(
    SELECT DISTINCT v FROM rls_policies p, unnest(p.allowed_values) AS v
    WHERE p.data_role_id = keep.data_role_id AND p.dataset_id = keep.dataset_id AND p.column_name = keep.column_name
    ORDER BY v
)
WHERE EXISTS (SELECT 1 FROM rls_policies other
              WHERE other.id <> keep.id AND other.data_role_id = keep.data_role_id
                AND other.dataset_id = keep.dataset_id AND other.column_name = keep.column_name);
DELETE FROM rls_policies p USING rls_policies other
WHERE p.data_role_id = other.data_role_id AND p.dataset_id = other.dataset_id AND p.column_name = other.column_name
  AND p.id > other.id;

ALTER TABLE rls_policies ALTER COLUMN data_role_id SET NOT NULL;
ALTER TABLE rls_policies
    ADD FOREIGN KEY (workspace_id, data_role_id) REFERENCES data_roles (workspace_id, id) ON DELETE CASCADE;
ALTER TABLE rls_policies DROP CONSTRAINT rls_policies_dataset_id_data_role_column_name_key;
ALTER TABLE rls_policies ADD UNIQUE (dataset_id, data_role_id, column_name);
ALTER TABLE rls_policies DROP COLUMN data_role;
CREATE INDEX rls_policies_role_idx ON rls_policies (data_role_id);

ALTER TABLE memberships DROP COLUMN data_role;
