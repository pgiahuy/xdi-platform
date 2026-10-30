-- SPDX-License-Identifier: Apache-2.0
-- 0009_dataset_relations.sql - quan hệ khóa ngoại giữa các dataset chụp từ cùng một database (#165). Mỗi bảng nguồn
-- chụp thành một dataset riêng; quan hệ ghi lại khóa ngoại của database nguồn để sau này mô hình viết JOIN giữa các
-- dataset (#213). service/sources/ ghi lúc chụp, engine/store/relations.rs đọc và ghi.
--
-- Số 0009 dùng lại cho file mới: bản 0009_source_model.sql trước đây chưa từng vào main. Database phát triển đã chạy
-- bản cũ thì sqlx báo checksum lệch: tạo lại database đó (docs/INSTALL.md).

-- from_* là phía nhiều, bảng giữ khóa ngoại (order_items); to_* là phía một, cột khóa chính hay UNIQUE (orders).
-- Quan hệ gắn với dataset, không gắn với phiên bản: chụp lại cả nhóm thì thay toàn bộ quan hệ của nhóm.
CREATE TABLE dataset_relations (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id     uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    from_dataset_id  uuid NOT NULL,
    from_columns     text[] NOT NULL CHECK (cardinality(from_columns) > 0),
    to_dataset_id    uuid NOT NULL,
    to_columns       text[] NOT NULL CHECK (cardinality(to_columns) = cardinality(from_columns)),
    -- Tên ràng buộc khóa ngoại ở database nguồn; Postgres không cho hai ràng buộc cùng tên trên một bảng.
    constraint_name  text NOT NULL,
    -- one_to_one khi cột khóa ngoại cũng là khóa chính hay UNIQUE của bảng giữ nó (mỗi dòng bên một có tối đa một dòng
    -- bên kia); còn lại many_to_one.
    cardinality      text NOT NULL DEFAULT 'many_to_one' CHECK (cardinality IN ('many_to_one', 'one_to_one')),
    created_at       timestamptz NOT NULL DEFAULT now(),
    UNIQUE (from_dataset_id, constraint_name),
    -- Khóa theo cặp (workspace, dataset): không quan hệ nào trỏ sang dataset của workspace khác. Xóa một dataset thì
    -- quan hệ của nó mất theo.
    FOREIGN KEY (workspace_id, from_dataset_id) REFERENCES datasets (workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, to_dataset_id) REFERENCES datasets (workspace_id, id) ON DELETE CASCADE
);
CREATE INDEX dataset_relations_to_idx ON dataset_relations (to_dataset_id);

-- Các dataset và phiên bản một câu hỏi đã đọc, dạng [{"dataset_id": ..., "version_id": ...}]. Bây giờ chỉ có dataset
-- đang hỏi; khi hỏi được qua nhiều bảng (#213) thì có thêm các bảng liên quan. Null với câu ghi trước migration này.
ALTER TABLE queries ADD COLUMN read_versions jsonb;
