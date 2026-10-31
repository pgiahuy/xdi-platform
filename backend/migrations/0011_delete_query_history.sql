-- SPDX-License-Identifier: Apache-2.0
-- 0011_delete_query_history.sql - xóa dataset thì xóa luôn lịch sử câu hỏi của nó (#187).
-- Trước đây ba khóa ngoại của queries tới datasets và dataset_versions là ON DELETE SET NULL: câu hỏi, SQL, lời báo
-- lỗi, phần "Đã hiểu là" và bảng phân rã của câu "tại sao" (queries.explanation, có tên nhóm và con số thật) còn lại
-- trong Postgres và trong bản sao lưu sau khi dataset đã xóa, mà giao diện không còn chỗ nào hiện chúng. Đổi thành
-- CASCADE: xóa dataset (hay một phiên bản) thì các dòng queries của nó mất theo, gồm cả các lượt của cuộc trò chuyện.
--
-- Tên ràng buộc là tên Postgres tự đặt cho ba khóa ngoại khai trong 0001_init.sql (bảng_cột_fkey).

ALTER TABLE queries
    DROP CONSTRAINT queries_workspace_id_dataset_id_fkey,
    DROP CONSTRAINT queries_workspace_id_version_id_fkey,
    DROP CONSTRAINT queries_dataset_id_version_id_fkey,
    ADD CONSTRAINT queries_workspace_id_dataset_id_fkey FOREIGN KEY (workspace_id, dataset_id)
        REFERENCES datasets (workspace_id, id) ON DELETE CASCADE,
    ADD CONSTRAINT queries_workspace_id_version_id_fkey FOREIGN KEY (workspace_id, version_id)
        REFERENCES dataset_versions (workspace_id, id) ON DELETE CASCADE,
    -- Phiên bản phải thuộc đúng dataset của câu hỏi, như trước.
    ADD CONSTRAINT queries_dataset_id_version_id_fkey FOREIGN KEY (dataset_id, version_id)
        REFERENCES dataset_versions (dataset_id, id) ON DELETE CASCADE;

-- Dòng đã mồ côi từ những lần xóa dataset trước migration này. Mọi đường ghi queries đều có dataset
-- (engine/store/queries.rs), nên dataset_id null chỉ có thể là dòng của dataset đã xóa.
DELETE FROM queries WHERE dataset_id IS NULL;
