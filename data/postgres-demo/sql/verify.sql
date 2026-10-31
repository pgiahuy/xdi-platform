-- sql/verify.sql - kiểm dữ liệu sau khi nạp, so với bảng staging (bản chép nguyên CSV).
-- Cần nạp với KEEP_STAGING=1: KEEP_STAGING=1 sh load.sh, rồi
--   docker compose exec -T demo-db psql -U demo -d company_demo -f - < sql/verify.sql
-- Mỗi câu in cột ok; mọi dòng phải là t. Câu 1 và 3 tới 7 so dữ liệu thật; câu 2 đếm khóa ngoại; câu 8 và 9 kiểm
-- hai bước cài nguyên nhân: đổi đúng 187 đơn và 381 sinh viên đã định, đổi đúng như mô tả.

\pset pager off

\echo '1. Số dòng và số khóa khác nhau so với CSV'
SELECT check_name, db, csv, db = csv AS ok FROM (
SELECT 'order_items = dòng CSV' AS check_name, (SELECT count(*) FROM sales.order_items) AS db, (SELECT count(*) FROM staging.dataco) AS csv
UNION ALL SELECT 'orders = Order Id khác nhau', (SELECT count(*) FROM sales.orders), (SELECT count(DISTINCT order_id) FROM staging.dataco)
UNION ALL SELECT 'customers = Customer Id khác nhau', (SELECT count(*) FROM sales.customers), (SELECT count(DISTINCT customer_id) FROM staging.dataco)
UNION ALL SELECT 'products = Product Card Id khác nhau', (SELECT count(*) FROM sales.products), (SELECT count(DISTINCT product_card_id) FROM staging.dataco)
UNION ALL SELECT 'categories = Category Id khác nhau', (SELECT count(*) FROM sales.categories), (SELECT count(DISTINCT category_id) FROM staging.dataco)
UNION ALL SELECT 'departments = Department Id khác nhau', (SELECT count(*) FROM sales.departments), (SELECT count(DISTINCT department_id) FROM staging.dataco)
UNION ALL SELECT 'courses = dòng CSV', (SELECT count(*) FROM edu.courses), (SELECT count(*) FROM staging.oulad_courses)
UNION ALL SELECT 'students = dòng CSV', (SELECT count(*) FROM edu.students), (SELECT count(*) FROM staging.oulad_student_info)
UNION ALL SELECT 'registrations = dòng CSV', (SELECT count(*) FROM edu.registrations), (SELECT count(*) FROM staging.oulad_student_registration)
UNION ALL SELECT 'assessments = dòng CSV', (SELECT count(*) FROM edu.assessments), (SELECT count(*) FROM staging.oulad_assessments)
UNION ALL SELECT 'student_assessments = dòng CSV', (SELECT count(*) FROM edu.student_assessments), (SELECT count(*) FROM staging.oulad_student_assessment)
UNION ALL SELECT 'vle_materials = dòng CSV', (SELECT count(*) FROM edu.vle_materials), (SELECT count(*) FROM staging.oulad_vle)
UNION ALL SELECT 'tổng sum_click = CSV', (SELECT sum(sum_click) FROM edu.student_daily_clicks), (SELECT sum(sum_click::bigint) FROM staging.oulad_student_vle)
UNION ALL SELECT 'tổng num_sites = (sinh viên, tài liệu, ngày) khác nhau', (SELECT sum(num_sites) FROM edu.student_daily_clicks),
    (SELECT count(*) FROM (SELECT DISTINCT code_module, code_presentation, id_student, id_site, date FROM staging.oulad_student_vle) s)
) counts;

\echo '2. Khóa ngoại: đủ 12 ràng buộc (edu 7, sales 5)'
\echo '   Không kiểm convalidated: khóa ngoại khai trong CREATE TABLE luôn là true vì được kiểm ở mỗi lần INSERT'
SELECT count(*) AS foreign_keys, count(*) = 12 AS ok
FROM pg_constraint
WHERE contype = 'f' AND connamespace IN ('edu'::regnamespace, 'sales'::regnamespace);

\echo '3. Nối bảng sự kiện nhiều-một với mọi bảng chiều: số dòng không đổi'
SELECT fact, joined_rows, fact_rows, joined_rows = fact_rows AS ok FROM (
SELECT 'sales.order_items' AS fact, count(*) AS joined_rows, (SELECT count(*) FROM sales.order_items) AS fact_rows
FROM sales.order_items oi
JOIN sales.orders o ON o.order_id = oi.order_id
JOIN sales.customers c ON c.customer_id = o.customer_id
JOIN sales.products p ON p.product_id = oi.product_id
JOIN sales.categories cat ON cat.category_id = p.category_id
JOIN sales.departments d ON d.department_id = cat.department_id
UNION ALL
SELECT 'edu.student_assessments', count(*), (SELECT count(*) FROM edu.student_assessments)
FROM edu.student_assessments sa
JOIN edu.assessments a ON a.id_assessment = sa.id_assessment
JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (sa.code_module, sa.code_presentation, sa.id_student)
JOIN edu.courses co ON (co.code_module, co.code_presentation) = (s.code_module, s.code_presentation)
JOIN edu.registrations r ON (r.code_module, r.code_presentation, r.id_student) = (s.code_module, s.code_presentation, s.id_student)
UNION ALL
SELECT 'edu.student_daily_clicks', count(*), (SELECT count(*) FROM edu.student_daily_clicks)
FROM edu.student_daily_clicks dc
JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (dc.code_module, dc.code_presentation, dc.id_student)
JOIN edu.courses co ON (co.code_module, co.code_presentation) = (s.code_module, s.code_presentation)
) joined;

\echo '4. Môn và kỳ chép vào student_assessments khớp bài đánh giá'
SELECT count(*) AS mismatched, count(*) = 0 AS ok
FROM edu.student_assessments sa JOIN edu.assessments a ON a.id_assessment = sa.id_assessment
WHERE (sa.code_module, sa.code_presentation) <> (a.code_module, a.code_presentation);

\echo '5. Tổng thành tiền và lợi nhuận theo thị trường: bảng chuẩn hóa so với CSV'
SELECT db.market, db.item_total, csv.item_total AS csv_item_total, db.profit, csv.profit AS csv_profit,
       db.item_total = csv.item_total AND db.profit = csv.profit AS ok
FROM (SELECT o.market, sum(oi.item_total) AS item_total, sum(oi.profit) AS profit
      FROM sales.order_items oi JOIN sales.orders o ON o.order_id = oi.order_id GROUP BY o.market) db
JOIN (SELECT market, sum(order_item_total::numeric) AS item_total, sum(order_profit_per_order::numeric) AS profit
      FROM staging.dataco GROUP BY market) csv ON csv.market = db.market
ORDER BY 1;

\echo '6. Doanh số theo phòng ban (qua sản phẩm, danh mục) so với CSV'
SELECT db.department_name, db.sales, csv.sales AS csv_sales, db.sales = csv.sales AS ok
FROM (SELECT d.department_name, sum(oi.sales) AS sales
      FROM sales.order_items oi JOIN sales.products p ON p.product_id = oi.product_id
      JOIN sales.categories c ON c.category_id = p.category_id
      JOIN sales.departments d ON d.department_id = c.department_id GROUP BY 1) db
JOIN (SELECT btrim(department_name) AS department_name, sum(sales::numeric) AS sales FROM staging.dataco GROUP BY 1) csv
    USING (department_name)
ORDER BY 1;

\echo '7. Điểm trung bình theo kết quả cuối khóa so với CSV'
\echo '   Bỏ nhóm kỳ 2014J, Scotland, 0-35 ở cả hai phía: bước cài nguyên nhân ở edu đổi kết quả của nhóm này (câu 9),'
\echo '   nên câu này qua cả khi có và khi không có bước cài.'
SELECT db.final_result, db.avg_score, csv.avg_score AS csv_avg_score, db.avg_score = csv.avg_score AS ok
FROM (SELECT s.final_result, round(avg(sa.score), 4) AS avg_score
      FROM edu.student_assessments sa
      JOIN edu.students s ON (s.code_module, s.code_presentation, s.id_student) = (sa.code_module, sa.code_presentation, sa.id_student)
      WHERE NOT (s.code_presentation = '2014J' AND s.region = 'Scotland' AND s.age_band = '0-35')
      GROUP BY 1) db
JOIN (SELECT i.final_result, round(avg(nullif(sa.score, '?')::numeric), 4) AS avg_score
      FROM staging.oulad_student_assessment sa
      JOIN staging.oulad_assessments a ON a.id_assessment = sa.id_assessment
      JOIN staging.oulad_student_info i ON (i.code_module, i.code_presentation, i.id_student) = (a.code_module, a.code_presentation, sa.id_student)
      WHERE NOT (i.code_presentation = '2014J' AND i.region = 'Scotland' AND i.age_band = '0-35')
      GROUP BY 1) csv USING (final_result)
ORDER BY 1;

\echo '8. Nguyên nhân cài sẵn ở sales (sql/plant_demo_cause_sales.sql): đúng 187 đơn khác CSV, là mọi đơn của nhóm đã định'
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

\echo '9. Nguyên nhân cài sẵn ở edu (sql/plant_demo_cause_edu.sql): đúng 381 sinh viên khác CSV, là mọi sinh viên'
\echo '   của nhóm đã định (kỳ 2014J, Scotland, nhóm tuổi 0-35, CSV ghi Pass), mỗi dòng chỉ đổi final_result thành'
\echo '   Withdrawn. Nạp với SKIP_DEMO_CAUSE=1 thì changed_students là 0 và ok là f.'
WITH compared AS (
    SELECT s.final_result, i.final_result AS csv_final_result,
           -- So mọi cột mô tả với CSV: bước cài chỉ được đổi final_result.
           (s.gender, s.region, s.highest_education, s.imd_band, s.age_band, s.num_of_prev_attempts,
            s.studied_credits)
               IS DISTINCT FROM (i.gender, i.region, i.highest_education,
                                 CASE WHEN i.imd_band IN ('?', '') THEN NULL
                                      WHEN i.imd_band = '10-20' THEN '10-20%' ELSE i.imd_band END,
                                 i.age_band, i.num_of_prev_attempts::integer, i.studied_credits::integer)
               AS other_columns_changed,
           s.final_result IS DISTINCT FROM i.final_result AS changed,
           (s.code_presentation = '2014J' AND s.region = 'Scotland' AND s.age_band = '0-35'
            AND i.final_result = 'Pass') AS in_target
    FROM edu.students s
    JOIN staging.oulad_student_info i
        ON (i.code_module, i.code_presentation, i.id_student::integer) = (s.code_module, s.code_presentation, s.id_student)
),
summary AS (
    SELECT count(*) FILTER (WHERE changed) AS changed_students,
           count(*) FILTER (WHERE in_target) AS target_students,
           count(*) FILTER (WHERE changed AND NOT (in_target AND final_result = 'Withdrawn')) AS wrong_changes,
           count(*) FILTER (WHERE in_target AND NOT changed) AS missed_students,
           count(*) FILTER (WHERE other_columns_changed) AS other_columns_changed
    FROM compared
)
SELECT changed_students, target_students, wrong_changes, missed_students, other_columns_changed,
       changed_students = 381 AND target_students = 381 AND wrong_changes = 0 AND missed_students = 0
           AND other_columns_changed = 0 AS ok
FROM summary;
