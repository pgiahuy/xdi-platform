-- sql/plant_demo_cause_sales.sql - cố ý cài MỘT nguyên nhân biết trước vào dữ liệu bán hàng, để thử câu hỏi tìm nguyên
-- nhân với đáp án đã biết. Đây là dữ liệu sửa tay, không phải dữ liệu DataCo gốc.
--
-- load.sh chạy file này sau khi đã đổ dữ liệu thật (sql/sales_load.sql), trong cùng giao dịch. Bỏ qua bằng
-- SKIP_DEMO_CAUSE=1 sh load.sh: khi đó sales giữ đúng như CSV.
--
-- Nguyên nhân cài vào: tháng 11/2016, mọi đơn của thị trường Pacific Asia giao bằng Standard Class, chưa bị hủy và
-- đang giao sớm hoặc đúng hạn, thành giao trễ 3 ngày. Đổi 4 cột của sales.orders trên các đơn đó:
--   days_for_shipping_real   cộng 3 (2 thành 5, 3 thành 6, 4 thành 7; lịch của Standard Class là 4 ngày)
--   shipping_date            cộng 3 ngày, để vẫn bằng ngày đặt cộng số ngày giao như dữ liệu gốc
--   delivery_status          thành 'Late delivery'
--   late_delivery_risk       thành true
-- Không đổi bảng nào khác, không đổi tiền hay số lượng. Chọn tháng 11/2016 vì từ 8/2016 đến 1/2017 mỗi tháng có
-- từ bốn thị trường trở lên (các tháng khác DataCo chỉ có một hoặc hai thị trường), nên câu "vì sao tỷ lệ giao trễ tháng
-- 11/2016 tăng so với tháng 10/2016" có hai tháng cùng cơ cấu thị trường để so. Đáp án mong đợi: Pacific Asia và
-- Standard Class. README.md (mục "Nguyên nhân cài sẵn") ghi số đơn và tỷ lệ trước, sau.
--
-- Cố ý không ghi gì về việc sửa này vào COMMENT của bảng hay cột: ứng dụng có thể đưa mô tả cột cho mô hình ngôn
-- ngữ, ghi vào đó là lộ đáp án.

CREATE TEMP TABLE demo_cause_orders ON COMMIT DROP AS
SELECT order_id
FROM sales.orders
WHERE order_date >= '2016-11-01' AND order_date < '2016-12-01'
  AND market = 'Pacific Asia'
  AND shipping_mode = 'Standard Class'
  AND delivery_status IN ('Advance shipping', 'Shipping on time');

-- In tỷ lệ trước khi đổi, để người chạy load.sh thấy ngay mức thay đổi.
SELECT 'before' AS stage,
       count(*) FILTER (WHERE market = 'Pacific Asia' AND shipping_mode = 'Standard Class') AS slice_orders,
       round(100.0 * avg(late_delivery_risk::integer) FILTER (WHERE market = 'Pacific Asia' AND shipping_mode = 'Standard Class'), 1) AS slice_late_pct,
       count(*) AS month_orders,
       round(100.0 * avg(late_delivery_risk::integer), 1) AS month_late_pct
FROM sales.orders
WHERE order_date >= '2016-11-01' AND order_date < '2016-12-01';

UPDATE sales.orders o
SET days_for_shipping_real = o.days_for_shipping_real + 3,
    shipping_date = o.shipping_date + interval '3 days',
    delivery_status = 'Late delivery',
    late_delivery_risk = true
FROM demo_cause_orders d
WHERE o.order_id = d.order_id;

SELECT 'after' AS stage,
       count(*) FILTER (WHERE market = 'Pacific Asia' AND shipping_mode = 'Standard Class') AS slice_orders,
       round(100.0 * avg(late_delivery_risk::integer) FILTER (WHERE market = 'Pacific Asia' AND shipping_mode = 'Standard Class'), 1) AS slice_late_pct,
       count(*) AS month_orders,
       round(100.0 * avg(late_delivery_risk::integer), 1) AS month_late_pct,
       (SELECT count(*) FROM demo_cause_orders) AS changed_orders
FROM sales.orders
WHERE order_date >= '2016-11-01' AND order_date < '2016-12-01';

