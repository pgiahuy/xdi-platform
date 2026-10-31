-- sql/verify_sales.sql - kiểm dữ liệu của sales_demo sau khi nạp, so với bảng staging (bản chép nguyên CSV DataCo).
-- Cần nạp với KEEP_STAGING=1: KEEP_STAGING=1 sh load.sh sales, rồi
--   docker compose exec -T demo-db psql -U demo -d sales_demo -f - < sql/verify_sales.sql
-- Mỗi câu in cột ok; mọi dòng phải là t. Câu 1, 3, 4, 5 so dữ liệu thật; câu 2 đếm khóa ngoại; câu 6 kiểm bước cài
-- nguyên nhân: đổi đúng 187 đơn đã định, đổi đúng như mô tả. Phần OULAD ở sql/verify_edu.sql.

\pset pager off

\echo '1. Số dòng và số khóa khác nhau so với CSV'
SELECT check_name, db, csv, db = csv AS ok FROM (
SELECT 'order_items = dòng CSV' AS check_name, (SELECT count(*) FROM sales.order_items) AS db, (SELECT count(*) FROM staging.dataco) AS csv
UNION ALL SELECT 'orders = Order Id khác nhau', (SELECT count(*) FROM sales.orders), (SELECT count(DISTINCT order_id) FROM staging.dataco)
UNION ALL SELECT 'customers = Customer Id khác nhau', (SELECT count(*) FROM sales.customers), (SELECT count(DISTINCT customer_id) FROM staging.dataco)
UNION ALL SELECT 'products = Product Card Id khác nhau', (SELECT count(*) FROM sales.products), (SELECT count(DISTINCT product_card_id) FROM staging.dataco)
UNION ALL SELECT 'categories = Category Id khác nhau', (SELECT count(*) FROM sales.categories), (SELECT count(DISTINCT category_id) FROM staging.dataco)
UNION ALL SELECT 'departments = Department Id khác nhau', (SELECT count(*) FROM sales.departments), (SELECT count(DISTINCT department_id) FROM staging.dataco)
) counts;

\echo '2. Khóa ngoại: đủ 5 ràng buộc'
\echo '   Không kiểm convalidated: khóa ngoại khai trong CREATE TABLE luôn là true vì được kiểm ở mỗi lần INSERT'
SELECT count(*) AS foreign_keys, count(*) = 5 AS ok
FROM pg_constraint
WHERE contype = 'f' AND connamespace = 'sales'::regnamespace;

\echo '3. Nối bảng sự kiện nhiều-một với mọi bảng chiều: số dòng không đổi'
SELECT fact, joined_rows, fact_rows, joined_rows = fact_rows AS ok FROM (
SELECT 'sales.order_items' AS fact, count(*) AS joined_rows, (SELECT count(*) FROM sales.order_items) AS fact_rows
FROM sales.order_items oi
JOIN sales.orders o ON o.order_id = oi.order_id
JOIN sales.customers c ON c.customer_id = o.customer_id
JOIN sales.products p ON p.product_id = oi.product_id
JOIN sales.categories cat ON cat.category_id = p.category_id
JOIN sales.departments d ON d.department_id = cat.department_id
) joined;

\echo '4. Tổng thành tiền và lợi nhuận theo thị trường: bảng chuẩn hóa so với CSV'
SELECT db.market, db.item_total, csv.item_total AS csv_item_total, db.profit, csv.profit AS csv_profit,
       db.item_total = csv.item_total AND db.profit = csv.profit AS ok
FROM (SELECT o.market, sum(oi.item_total) AS item_total, sum(oi.profit) AS profit
      FROM sales.order_items oi JOIN sales.orders o ON o.order_id = oi.order_id GROUP BY o.market) db
JOIN (SELECT market, sum(order_item_total::numeric) AS item_total, sum(order_profit_per_order::numeric) AS profit
      FROM staging.dataco GROUP BY market) csv ON csv.market = db.market
ORDER BY 1;

\echo '5. Doanh số theo phòng ban (qua sản phẩm, danh mục) so với CSV'
SELECT db.department_name, db.sales, csv.sales AS csv_sales, db.sales = csv.sales AS ok
FROM (SELECT d.department_name, sum(oi.sales) AS sales
      FROM sales.order_items oi JOIN sales.products p ON p.product_id = oi.product_id
      JOIN sales.categories c ON c.category_id = p.category_id
      JOIN sales.departments d ON d.department_id = c.department_id GROUP BY 1) db
JOIN (SELECT btrim(department_name) AS department_name, sum(sales::numeric) AS sales FROM staging.dataco GROUP BY 1) csv
    USING (department_name)
ORDER BY 1;

\echo '6. Nguyên nhân cài sẵn ở sales (sql/plant_demo_cause_sales.sql): đúng 187 đơn khác CSV, là mọi đơn của nhóm đã định'
\echo '   (tháng 11/2016, Pacific Asia, Standard Class, giao sớm hoặc đúng hạn), mỗi đơn đổi đúng như mô tả:'
\echo '   số ngày giao cộng 3 (thành 5 đến 7), ngày giao cộng 3 ngày, Late delivery, late_delivery_risk true.'
\echo '   Nạp với SKIP_DEMO_CAUSE=1 thì changed_orders là 0 và ok là f: câu này chỉ dùng khi có bước cài nguyên nhân.'
WITH csv_orders AS (
    SELECT DISTINCT order_id::integer AS order_id, days_for_shipping_real::integer AS days_real, delivery_status,
           late_delivery_risk = '1' AS late, to_timestamp(shipping_date, 'MM/DD/YYYY HH24:MI')::timestamp AS shipping_date
    FROM staging.dataco
),
compared AS (
    SELECT o.*, c.days_real AS csv_days_real, c.shipping_date AS csv_shipping_date,
           -- Đơn khác CSV ở một trong 4 cột bước cài nguyên nhân được phép đổi.
           (o.days_for_shipping_real, o.delivery_status, o.late_delivery_risk, o.shipping_date)
               IS DISTINCT FROM (c.days_real, c.delivery_status, c.late, c.shipping_date) AS changed,
           -- Đơn thuộc nhóm đã định, xét theo giá trị trong CSV. Ngày đặt, thị trường, cách giao không bị đổi.
           (o.order_date >= '2016-11-01' AND o.order_date < '2016-12-01'
            AND o.market = 'Pacific Asia' AND o.shipping_mode = 'Standard Class'
            AND c.delivery_status IN ('Advance shipping', 'Shipping on time')) AS in_target
    FROM sales.orders o JOIN csv_orders c USING (order_id)
),
summary AS (
    SELECT count(*) FILTER (WHERE changed) AS changed_orders,
           count(*) FILTER (WHERE in_target) AS target_orders,
           -- Đơn bị đổi mà nằm ngoài nhóm, hoặc đổi khác mô tả.
           count(*) FILTER (WHERE changed AND NOT (in_target
                                                   AND days_for_shipping_real = csv_days_real + 3
                                                   AND days_for_shipping_real BETWEEN 5 AND 7
                                                   AND shipping_date = csv_shipping_date + interval '3 days'
                                                   AND delivery_status = 'Late delivery'
                                                   AND late_delivery_risk)) AS wrong_changes,
           -- Đơn trong nhóm mà không bị đổi.
           count(*) FILTER (WHERE in_target AND NOT changed) AS missed_orders
    FROM compared
)
SELECT changed_orders, target_orders, wrong_changes, missed_orders,
       changed_orders = 187 AND target_orders = 187 AND wrong_changes = 0 AND missed_orders = 0 AS ok
FROM summary;
