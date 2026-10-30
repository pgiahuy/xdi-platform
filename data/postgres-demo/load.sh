#!/bin/sh
# data/postgres-demo/load.sh - nạp DataCo và OULAD từ CSV vào cơ sở dữ liệu company_demo (container demo-db).
#
# Cách chạy, từ bất kỳ thư mục nào: sh data/postgres-demo/load.sh
# Chạy lại bao nhiêu lần cũng ra cùng kết quả: mỗi lần xóa và tạo lại schema staging, edu, sales.
# Mọi bước chạy trong một psql, một giao dịch (--single-transaction, đọc lệnh từ stdin bằng -f -): lỗi giữa chừng
# thì dữ liệu cũ còn nguyên.
#
# psql chạy trong container (docker compose exec), CSV đọc từ /downloads, là thư mục DEMO_DATA_DIR (mặc định
# data/downloads) gắn chỉ đọc. Vì vậy máy chủ chỉ cần Docker, không cần cài psql hay iconv.
#
# Biến môi trường:
#   DEMO_DATA_DIR  thư mục chứa DataCoSupplyChainDataset.csv và oulad/, tính từ thư mục này (mặc định ../downloads)
#   KEEP_STAGING=1 giữ schema staging sau khi nạp (để so với CSV gốc); mặc định xóa, đỡ khoảng 730 MB
#   SKIP_DEMO_CAUSE=1 không chạy sql/plant_demo_cause.sql: sales giữ đúng như CSV. Mặc định có chạy, xem README.md
#                  mục "Nguyên nhân cài sẵn"

set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
data_dir=${DEMO_DATA_DIR:-../downloads}

for f in DataCoSupplyChainDataset.csv oulad/courses.csv oulad/assessments.csv oulad/studentInfo.csv \
    oulad/studentAssessment.csv oulad/studentRegistration.csv oulad/vle.csv oulad/studentVle.csv; do
    if [ ! -f "$data_dir/$f" ]; then
        echo "missing $data_dir/$f: run 'sh data/download.sh dataco' and 'sh data/download.sh oulad' first" >&2
        exit 1
    fi
done

docker compose up -d --wait demo-db

# Câu \copy của OULAD: file ASCII, có dòng tiêu đề. Dùng printf, không dùng echo: echo của dash hiểu "\c" trong
# "\copy" là lệnh dừng in, câu \copy mất mà không báo lỗi.
copy_oulad() {
    printf '%s\n' "\\copy staging.$1 from '/downloads/oulad/$2' with (format csv, header true)"
}

# Thứ tự: staging, chép CSV, schema và dữ liệu của edu rồi sales, cài nguyên nhân, quyền, xóa staging. Mọi file nối
# vào một luồng
# để chạy trong cùng một giao dịch.
{
    # Ẩn NOTICE "drop cascades to ..." khi xóa schema cũ; lỗi và cảnh báo vẫn in.
    echo "SET client_min_messages = warning;"
    # Ngày giờ trong DataCo không có múi giờ; đổi qua to_timestamp theo UTC để giờ không lệch theo múi giờ của máy.
    echo "SET TIME ZONE 'UTC';"
    cat sql/staging.sql
    # DataCo lưu bằng windows-1252 (có chữ như "Dublín"). ENCODING để Postgres đổi sang UTF-8 khi chép; thiếu nó
    # thì chữ có dấu thành ký tự lỗi hoặc COPY báo lỗi mã hóa.
    printf '%s\n' "\\copy staging.dataco from '/downloads/DataCoSupplyChainDataset.csv' with (format csv, header true, encoding 'WIN1252')"
    copy_oulad oulad_courses courses.csv
    copy_oulad oulad_assessments assessments.csv
    copy_oulad oulad_student_info studentInfo.csv
    copy_oulad oulad_student_assessment studentAssessment.csv
    copy_oulad oulad_student_registration studentRegistration.csv
    copy_oulad oulad_vle vle.csv
    copy_oulad oulad_student_vle studentVle.csv
    cat sql/edu_schema.sql sql/edu_load.sql sql/sales_schema.sql sql/sales_load.sql
    # Bước riêng, sau dữ liệu thật: cài một nguyên nhân biết trước vào sales.orders để thử câu hỏi tìm nguyên nhân.
    if [ "${SKIP_DEMO_CAUSE:-0}" != "1" ]; then
        printf '%s\n' "\\echo 'planting demo cause (sql/plant_demo_cause.sql); set SKIP_DEMO_CAUSE=1 to skip'"
        cat sql/plant_demo_cause.sql
    fi
    cat sql/grants.sql
    if [ "${KEEP_STAGING:-0}" != "1" ]; then
        echo "DROP SCHEMA staging CASCADE;"
    fi
    # Thống kê cho planner, để câu truy vấn đầu tiên sau khi nạp không chạy theo ước lượng sai.
    echo "ANALYZE;"
} | docker compose exec -T demo-db psql -U demo -d company_demo -v ON_ERROR_STOP=1 --single-transaction -q -f -

echo "loaded company_demo; row counts:"
docker compose exec -T demo-db psql -U demo -d company_demo -f - < sql/row_counts.sql
