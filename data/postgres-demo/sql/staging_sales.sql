-- sql/staging_sales.sql - schema "staging" của sales_demo: file DataCo một bảng, mọi cột kiểu text, đúng thứ tự cột
-- trong file. load.sh chép CSV vào đây bằng \copy, rồi sql/sales_load.sql đổi kiểu và tách sang bảng chuẩn hóa.
-- Để mọi cột là text: COPY không dừng giữa chừng vì một giá trị lạ (chuỗi rỗng, ngày dạng M/D/YYYY), việc đổi kiểu
-- nằm trong SQL đọc được. Người dùng analyst không có quyền trên schema này. OULAD có staging riêng trong edu_demo
-- (sql/staging_edu.sql).

DROP SCHEMA IF EXISTS staging CASCADE;
CREATE SCHEMA staging;

CREATE TABLE staging.dataco (
    type text,
    days_for_shipping_real text,
    days_for_shipment_scheduled text,
    benefit_per_order text,
    sales_per_customer text,
    delivery_status text,
    late_delivery_risk text,
    category_id text,
    category_name text,
    customer_city text,
    customer_country text,
    customer_email text,
    customer_fname text,
    customer_id text,
    customer_lname text,
    customer_password text,
    customer_segment text,
    customer_state text,
    customer_street text,
    customer_zipcode text,
    department_id text,
    department_name text,
    latitude text,
    longitude text,
    market text,
    order_city text,
    order_country text,
    order_customer_id text,
    order_date text,
    order_id text,
    order_item_cardprod_id text,
    order_item_discount text,
    order_item_discount_rate text,
    order_item_id text,
    order_item_product_price text,
    order_item_profit_ratio text,
    order_item_quantity text,
    sales text,
    order_item_total text,
    order_profit_per_order text,
    order_region text,
    order_state text,
    order_status text,
    order_zipcode text,
    product_card_id text,
    product_category_id text,
    product_description text,
    product_image text,
    product_name text,
    product_price text,
    product_status text,
    shipping_date text,
    shipping_mode text
);
