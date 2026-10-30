-- sql/row_counts.sql - đếm dòng thật của từng bảng trong edu và sales; load.sh in ra sau khi nạp.
SELECT 'edu.courses' AS table_name, count(*) AS row_count FROM edu.courses
UNION ALL SELECT 'edu.students', count(*) FROM edu.students
UNION ALL SELECT 'edu.registrations', count(*) FROM edu.registrations
UNION ALL SELECT 'edu.assessments', count(*) FROM edu.assessments
UNION ALL SELECT 'edu.student_assessments', count(*) FROM edu.student_assessments
UNION ALL SELECT 'edu.vle_materials', count(*) FROM edu.vle_materials
UNION ALL SELECT 'edu.student_daily_clicks', count(*) FROM edu.student_daily_clicks
UNION ALL SELECT 'sales.departments', count(*) FROM sales.departments
UNION ALL SELECT 'sales.categories', count(*) FROM sales.categories
UNION ALL SELECT 'sales.products', count(*) FROM sales.products
UNION ALL SELECT 'sales.customers', count(*) FROM sales.customers
UNION ALL SELECT 'sales.orders', count(*) FROM sales.orders
UNION ALL SELECT 'sales.order_items', count(*) FROM sales.order_items;
