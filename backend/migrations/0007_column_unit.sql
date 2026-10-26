-- SPDX-License-Identifier: Apache-2.0
-- 0007_column_unit.sql - đơn vị của cột số đo, do lớp ngữ nghĩa (#23, service/semantic.rs) đề xuất.
-- Luật biểu đồ (ARCHITECTURE mục 6) cần biết hai số đo cùng hay khác đơn vị: cùng đơn vị thì vẽ
-- cạnh nhau trên một thang đo, khác đơn vị thì mỗi số đo một biểu đồ riêng. Danh sách cố định để so được bằng dấu
-- bằng: chữ tự do ("ngày", "days") cùng nghĩa mà khác chữ. Null là chưa biết; biểu đồ coi như khác đơn vị.
ALTER TABLE columns ADD COLUMN unit text CHECK (unit IN ('money', 'quantity', 'percent', 'duration', 'other'));
