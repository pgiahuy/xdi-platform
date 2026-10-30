-- SPDX-License-Identifier: Apache-2.0
-- 0009_source_model.sql - mô hình dữ liệu của một phiên bản chụp từ database (#165): bảng chính (bảng sự kiện), các
-- bảng nối nhiều-một theo khóa ngoại và số dòng. service/sources/ ghi lúc chụp; giao diện đọc để ghi "Dựng từ
-- order_items, nối nhiều-một sang orders (order_id)"; "Chụp lại" đọc lại để nối đúng các bảng của lần trước.
-- jsonb trên dataset_versions vì mô hình đi theo phiên bản (lần chụp sau có thể nối khác nếu database đổi khóa ngoại),
-- và chỉ đọc, ghi cả khối một lần, không truy vấn theo từng phần. Null với phiên bản nạp từ file.
ALTER TABLE dataset_versions ADD COLUMN source_model jsonb;

-- Bảng gốc (schema.bảng) của cột chép từ bảng nối nhiều-một sang. Giá trị của cột như vậy lặp lại trên mọi dòng của
-- bảng chính trỏ tới cùng một dòng của bảng kia, nên cộng dồn nó là đếm lặp: prompt SQL ghi rõ nguồn của cột và cấm
-- SUM. Null với cột của bảng chính và cột nạp từ file.
ALTER TABLE columns ADD COLUMN origin_table text;
