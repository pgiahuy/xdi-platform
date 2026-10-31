-- SPDX-License-Identifier: Apache-2.0
-- 0017_dataset_tables.sql - dataset là một database đã kết nối hoặc một file, bảng nằm bên trong dataset (#257).
--
-- Tên trong Postgres giữ như cũ để không phải sửa mọi câu SQL: dòng `sources` là một dataset (nguồn Postgres là một
-- database, nguồn local_file là một file và có đúng một bảng), dòng `datasets` là một bảng của dataset đó, nối qua
-- datasets.source_id. Phiên bản, hồ sơ cột, quan hệ, lịch sử câu hỏi và bằng chứng vẫn theo từng bảng như trước, nên
-- câu hỏi cũ, dòng nguồn và bằng chứng vẫn trỏ đúng phiên bản bảng đã đọc. API mới /catalog trả dataset kèm bảng
-- (service/catalog/); route cũ /datasets/{id}/... nhận id bảng như trước.
--
-- Chuyển dữ liệu cũ: mỗi nguồn database thành một dataset tên theo database, mỗi file thành một dataset tên theo bảng
-- của nó. Không đổi tên bảng logic nào đã có (orders_2 vẫn là orders_2): SQL đã lưu của câu hỏi cũ dùng tên đó, truy
-- dòng nguồn dựng lại câu từ SQL đã lưu.

-- Tên và gói ngành của dataset. Gói ngành trước đây nằm trên từng bảng (datasets.domain); một database dùng một gói, nên
-- lấy gói của bảng tạo sớm nhất có gói.
ALTER TABLE sources ADD COLUMN name text, ADD COLUMN domain text;

UPDATE sources s SET name = COALESCE(
    CASE WHEN s.kind = 'local_file'
        THEN (SELECT d.name FROM datasets d WHERE d.source_id = s.id ORDER BY d.created_at, d.id LIMIT 1)
        ELSE NULLIF(btrim(s.config->>'database'), '')
    END,
    'Dataset');

UPDATE sources s SET domain = (
    SELECT d.domain FROM datasets d
    WHERE d.source_id = s.id AND d.domain IS NOT NULL
    ORDER BY d.created_at, d.id LIMIT 1);

ALTER TABLE sources
    ALTER COLUMN name SET NOT NULL,
    ADD CONSTRAINT sources_name_check CHECK (length(btrim(name)) > 0);

ALTER TABLE datasets DROP COLUMN domain;

-- Tên bảng logic chỉ cần không trùng trong một dataset: hai database cùng có bảng orders thì cả hai giữ tên orders. Mọi
-- câu hỏi đọc bảng của một dataset (service/ask/), nên guard không bao giờ thấy hai bảng cùng tên. Dữ liệu cũ không
-- trùng trong workspace nên càng không trùng trong một dataset.
DROP INDEX datasets_logical_table_key;
CREATE UNIQUE INDEX datasets_logical_table_key ON datasets (source_id, lower(logical_table));

-- Cho khóa ngoại theo cặp (bảng, dataset) của metrics bên dưới.
ALTER TABLE datasets ADD CONSTRAINT datasets_id_source_key UNIQUE (id, source_id);

-- Chỉ số gắn theo dataset: tên không trùng trong một dataset, vì câu hỏi trên dataset thấy chỉ số của mọi bảng nó đọc
-- cùng lúc. dataset_id vẫn là bảng mà công thức đọc cột (prompt in chỉ số cạnh bảng đó). Hai bảng cùng dataset có chỉ
-- số cùng tên (trước đây được phép): bảng tạo trước giữ tên, bảng sau thêm tên bảng logic vào sau tên.
ALTER TABLE metrics ADD COLUMN source_id uuid;
UPDATE metrics m SET source_id = d.source_id FROM datasets d WHERE d.id = m.dataset_id;

UPDATE metrics m SET name = m.name || ' (' || d.logical_table || ')'
FROM datasets d
WHERE d.id = m.dataset_id
  AND EXISTS (
      SELECT 1 FROM metrics o JOIN datasets od ON od.id = o.dataset_id
      WHERE o.source_id = m.source_id AND o.name = m.name AND o.dataset_id <> m.dataset_id
        AND (od.created_at, od.id) < (d.created_at, d.id));

ALTER TABLE metrics
    ALTER COLUMN source_id SET NOT NULL,
    DROP CONSTRAINT metrics_dataset_id_name_version_key,
    ADD CONSTRAINT metrics_source_id_name_version_key UNIQUE (source_id, name, version),
    -- Dataset của chỉ số phải đúng dataset chứa bảng của nó.
    ADD CONSTRAINT metrics_table_source_fk FOREIGN KEY (dataset_id, source_id)
        REFERENCES datasets (id, source_id) ON DELETE CASCADE;
CREATE INDEX metrics_source_idx ON metrics (source_id);

-- Câu mẫu đã duyệt (queries) và bài học (lessons) giữ dataset_id là bảng câu hỏi đã hỏi hay bảng bài học được viết,
-- không cần cột mới: câu hỏi trên một bảng đọc câu mẫu và bài học của mọi bảng cùng dataset, qua datasets.source_id
-- (engine/store/queries.rs, lessons.rs).
