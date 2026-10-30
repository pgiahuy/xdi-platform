-- sql/sales_load.sql - tách bảng phẳng staging.dataco (một dòng mỗi dòng hàng, mọi cột text) sang schema sales.
-- load.sh chạy file này sau sql/sales_schema.sql, trong cùng một giao dịch.
--
-- Mỗi bảng chiều lấy bằng SELECT DISTINCT trên khóa và các cột của nó. Nếu một khóa có hai giá trị khác nhau ở một
-- cột (phụ thuộc hàm bị vi phạm), DISTINCT ra hai dòng cùng khóa và khóa chính chặn lại, cả lần nạp dừng. Vì vậy nạp
-- xong là bằng chứng mỗi khóa xác định đúng một bộ giá trị. btrim bỏ khoảng trắng thừa ở cuối tên (nguồn có, ví dụ
-- "Smart watch "), không làm đổi số khóa khác nhau.

INSERT INTO sales.departments (department_id, department_name)
SELECT DISTINCT department_id::integer, btrim(department_name)
FROM staging.dataco;

INSERT INTO sales.categories (category_id, category_name, department_id)
SELECT DISTINCT category_id::integer, btrim(category_name), department_id::integer
FROM staging.dataco;

INSERT INTO sales.products (product_id, product_name, category_id, price)
SELECT DISTINCT product_card_id::integer, btrim(product_name), category_id::integer, product_price::numeric
FROM staging.dataco;

INSERT INTO sales.customers (customer_id, first_name, last_name, segment, street, city, state, country, zipcode,
                             latitude, longitude)
SELECT DISTINCT customer_id::integer,
       btrim(customer_fname),
       nullif(btrim(customer_lname), ''),
       customer_segment,
       CASE WHEN shifted THEN NULL ELSE btrim(customer_street) END,
       CASE WHEN shifted THEN btrim(customer_street) ELSE btrim(customer_city) END,
       CASE WHEN shifted THEN btrim(customer_city) ELSE btrim(customer_state) END,
       btrim(customer_country),
       CASE WHEN shifted THEN btrim(customer_state) ELSE nullif(btrim(customer_zipcode), '') END,
       latitude::numeric,
       longitude::numeric
FROM (
    -- 3 khách (ở Elk Grove và El Monte, bang CA) bị lệch cột trong nguồn: mã bưu chính nằm ở cột bang, mã bang ở
    -- cột thành phố, tên thành phố ở cột đường, cột mã bưu chính trống. Dời về đúng cột; địa chỉ đường đã mất nên
    -- để trống. Không dời thì nhóm theo bang ra các "bang" 95758, 91732.
    SELECT *, customer_state ~ '^[0-9]+$' AS shifted FROM staging.dataco
) d;

-- Ngày trong nguồn dạng tháng/ngày/năm giờ:phút, ví dụ 1/31/2018 22:56.
INSERT INTO sales.orders (order_id, customer_id, order_date, shipping_date, payment_type, order_status,
                          delivery_status, late_delivery_risk, days_for_shipping_real, days_for_shipment_scheduled,
                          shipping_mode, market, order_region, order_country, order_state, order_city, order_zipcode)
SELECT DISTINCT order_id::integer,
       order_customer_id::integer,
       to_timestamp(order_date, 'MM/DD/YYYY HH24:MI')::timestamp,
       to_timestamp(shipping_date, 'MM/DD/YYYY HH24:MI')::timestamp,
       type,
       order_status,
       delivery_status,
       late_delivery_risk = '1',
       days_for_shipping_real::integer,
       days_for_shipment_scheduled::integer,
       shipping_mode,
       market,
       btrim(order_region),
       btrim(order_country),
       btrim(order_state),
       btrim(order_city),
       nullif(btrim(order_zipcode), '')
FROM staging.dataco;

-- Bỏ các cột trùng giá trị với cột khác ở mọi dòng (Sales per customer = Order Item Total, Benefit per order =
-- Order Profit Per Order, Order Item Cardprod Id = Product Card Id), các cột chỉ có một giá trị (Customer Email,
-- Customer Password, Product Status) và các cột không dùng cho phân tích (Product Description trống ở mọi dòng,
-- Product Image là đường dẫn ảnh).
INSERT INTO sales.order_items (order_item_id, order_id, product_id, quantity, unit_price, sales, discount,
                               discount_rate, item_total, profit, profit_ratio)
SELECT order_item_id::integer,
       order_id::integer,
       product_card_id::integer,
       order_item_quantity::integer,
       order_item_product_price::numeric,
       sales::numeric,
       order_item_discount::numeric,
       order_item_discount_rate::numeric,
       order_item_total::numeric,
       order_profit_per_order::numeric,
       order_item_profit_ratio::numeric
FROM staging.dataco;
