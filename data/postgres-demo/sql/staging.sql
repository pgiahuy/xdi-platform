-- sql/staging.sql - schema "staging": mỗi file CSV một bảng, mọi cột kiểu text, đúng thứ tự cột trong file.
-- load.sh chép CSV vào đây bằng \copy, rồi sql/edu.sql và sql/sales.sql đổi kiểu và tách sang bảng chuẩn hóa.
-- Để mọi cột là text: COPY không dừng giữa chừng vì một giá trị lạ ("?", chuỗi rỗng, ngày dạng M/D/YYYY), việc đổi
-- kiểu nằm trong SQL đọc được. Người dùng analyst không có quyền trên schema này.

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

CREATE TABLE staging.oulad_courses (
    code_module text,
    code_presentation text,
    module_presentation_length text
);

CREATE TABLE staging.oulad_assessments (
    code_module text,
    code_presentation text,
    id_assessment text,
    assessment_type text,
    date text,
    weight text
);

CREATE TABLE staging.oulad_student_info (
    code_module text,
    code_presentation text,
    id_student text,
    gender text,
    region text,
    highest_education text,
    imd_band text,
    age_band text,
    num_of_prev_attempts text,
    studied_credits text,
    disability text,
    final_result text
);

CREATE TABLE staging.oulad_student_assessment (
    id_assessment text,
    id_student text,
    date_submitted text,
    is_banked text,
    score text
);

CREATE TABLE staging.oulad_student_registration (
    code_module text,
    code_presentation text,
    id_student text,
    date_registration text,
    date_unregistration text
);

CREATE TABLE staging.oulad_vle (
    id_site text,
    code_module text,
    code_presentation text,
    activity_type text,
    week_from text,
    week_to text
);

CREATE TABLE staging.oulad_student_vle (
    code_module text,
    code_presentation text,
    id_student text,
    id_site text,
    date text,
    sum_click text
);
