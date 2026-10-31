#!/bin/sh
# data/demo-db/init/02_load.sh - nạp database mẫu nhỏ "company_demo" của docker-compose.yml (dịch vụ demo-db), chạy một
# lần khi volume còn trống. Image Postgres chạy file này sau 01_roles.sql (tạo người dùng analyst), xem
# data/demo-db/README.md.
#
# Dùng lại đúng các file SQL của data/postgres-demo/sql (gắn vào /demo-sql), chỉ khác nguồn: bán hàng lấy từ
# data/samples/dataco.csv (1500 dòng), học tập lấy từ data/samples/oulad/ (bản nhỏ của OULAD, dựng bằng
# build_oulad_subset.py). Cả hai gắn vào /samples. Lượt bấm theo ngày đã gộp sẵn trong studentDailyClicks.csv nên chép
# thẳng vào edu.student_daily_clicks; bảng staging của studentVle để trống, câu gộp trong edu_load.sql ra 0 dòng.
# Không chạy plant_demo_cause_sales.sql, plant_demo_cause_edu.sql: dữ liệu giữ đúng như CSV.
#
# Mọi bước trong một giao dịch: lỗi ở đâu thì không có schema nào được tạo, và khởi tạo container báo lỗi.

set -e

copy_oulad() {
    printf '%s\n' "\\copy staging.$1 from '/samples/oulad/$2' with (format csv, header true)"
}

{
    echo "SET client_min_messages = warning;"
    echo "SET TIME ZONE 'UTC';"
    cat /demo-sql/staging.sql
    # dataco.csv lưu bằng windows-1252, như bản đầy đủ.
    printf '%s\n' "\\copy staging.dataco from '/samples/dataco.csv' with (format csv, header true, encoding 'WIN1252')"
    copy_oulad oulad_courses courses.csv
    copy_oulad oulad_assessments assessments.csv
    copy_oulad oulad_student_info studentInfo.csv
    copy_oulad oulad_student_assessment studentAssessment.csv
    copy_oulad oulad_student_registration studentRegistration.csv
    copy_oulad oulad_vle vle.csv
    cat /demo-sql/edu_schema.sql /demo-sql/edu_load.sql
    printf '%s\n' "\\copy edu.student_daily_clicks from '/samples/oulad/studentDailyClicks.csv' with (format csv, header true)"
    cat /demo-sql/sales_schema.sql /demo-sql/sales_load.sql /demo-sql/grants.sql
    echo "DROP SCHEMA staging CASCADE;"
    echo "ANALYZE;"
} | psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" --single-transaction -q -f -
