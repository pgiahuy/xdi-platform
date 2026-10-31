#!/bin/sh
# data/postgres-demo/load.sh - nạp DataCo vào database sales_demo và OULAD vào database edu_demo, cùng một container
# demo-db. Mỗi database một mảng nghiệp vụ, để một câu hỏi trên một database chỉ thấy bảng của mảng đó.
#
# Cách chạy, từ bất kỳ thư mục nào:
#   sh data/postgres-demo/load.sh          nạp cả hai database
#   sh data/postgres-demo/load.sh sales    chỉ sales_demo (edu: chỉ edu_demo)
# Chưa có database thì tạo; database khác trong container (ví dụ company_demo của bản cũ) không bị đụng tới. Chạy lại
# bao nhiêu lần cũng ra cùng kết quả: mỗi lần xóa và tạo lại schema staging và schema dữ liệu (sales hay edu). Mỗi
# database nạp trong một psql, một giao dịch (--single-transaction, đọc lệnh từ stdin bằng -f -): lỗi giữa chừng thì
# dữ liệu cũ của database đó còn nguyên.
#
# psql chạy trong container (docker compose exec), CSV đọc từ /downloads, là thư mục DEMO_DATA_DIR (mặc định
# data/downloads) gắn chỉ đọc. Vì vậy máy chủ chỉ cần Docker, không cần cài psql hay iconv.
#
# Biến môi trường:
#   DEMO_DATA_DIR  thư mục chứa DataCoSupplyChainDataset.csv và oulad/, tính từ thư mục này (mặc định ../downloads)
#   KEEP_STAGING=1 giữ schema staging sau khi nạp (để so với CSV gốc bằng sql/verify_*.sql); mặc định xóa, đỡ khoảng
#                  730 MB
#   SKIP_DEMO_CAUSE=1 không chạy sql/plant_demo_cause_sales.sql và sql/plant_demo_cause_edu.sql: dữ liệu giữ đúng như
#                  CSV. Mặc định có chạy, xem README.md mục "Nguyên nhân cài sẵn"

set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
data_dir=${DEMO_DATA_DIR:-../downloads}
targets=${*:-sales edu}

need_files() {
    for f in "$@"; do
        if [ ! -f "$data_dir/$f" ]; then
            echo "missing $data_dir/$f: run 'sh data/download.sh dataco' and 'sh data/download.sh oulad' first" >&2
            exit 1
        fi
    done
}

for target in $targets; do
    case $target in
        sales) need_files DataCoSupplyChainDataset.csv ;;
        edu) need_files oulad/courses.csv oulad/assessments.csv oulad/studentInfo.csv oulad/studentAssessment.csv \
            oulad/studentRegistration.csv oulad/vle.csv oulad/studentVle.csv ;;
        *) echo "usage: sh load.sh [sales] [edu]" >&2; exit 2 ;;
    esac
done

docker compose up -d --wait demo-db

psql_demo() {
    docker compose exec -T demo-db psql -U demo -v ON_ERROR_STOP=1 "$@"
}

# CREATE DATABASE không chạy được trong giao dịch, nên tạo riêng, trước khi nạp. \gexec chạy câu lệnh mà SELECT trả
# về: database đã có thì SELECT không trả dòng nào, không làm gì.
create_database() {
    printf '%s\n' "SELECT format('CREATE DATABASE %I', '$1') WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '$1')\\gexec" \
        | psql_demo -d postgres -q -f -
}

# Câu \copy của OULAD: file ASCII, có dòng tiêu đề. Dùng printf, không dùng echo: echo của dash hiểu "\c" trong
# "\copy" là lệnh dừng in, câu \copy mất mà không báo lỗi.
copy_oulad() {
    printf '%s\n' "\\copy staging.$1 from '/downloads/oulad/$2' with (format csv, header true)"
}

# SQL riêng của sales_demo: staging, chép CSV, schema và dữ liệu, rồi bước cài nguyên nhân.
sales_sql() {
    cat sql/staging_sales.sql
    # DataCo lưu bằng windows-1252 (có chữ như "Dublín"). ENCODING để Postgres đổi sang UTF-8 khi chép; thiếu nó
    # thì chữ có dấu thành ký tự lỗi hoặc COPY báo lỗi mã hóa.
    printf '%s\n' "\\copy staging.dataco from '/downloads/DataCoSupplyChainDataset.csv' with (format csv, header true, encoding 'WIN1252')"
    cat sql/sales_schema.sql sql/sales_load.sql
    # Bước riêng, sau dữ liệu thật: cài một nguyên nhân biết trước vào sales.orders để thử câu hỏi tìm nguyên nhân.
    if [ "${SKIP_DEMO_CAUSE:-0}" != "1" ]; then
        printf '%s\n' "\\echo 'planting the demo cause (sql/plant_demo_cause_sales.sql); set SKIP_DEMO_CAUSE=1 to skip'"
        cat sql/plant_demo_cause_sales.sql
    fi
}

# SQL riêng của edu_demo, cùng thứ tự như sales_sql.
edu_sql() {
    cat sql/staging_edu.sql
    copy_oulad oulad_courses courses.csv
    copy_oulad oulad_assessments assessments.csv
    copy_oulad oulad_student_info studentInfo.csv
    copy_oulad oulad_student_assessment studentAssessment.csv
    copy_oulad oulad_student_registration studentRegistration.csv
    copy_oulad oulad_vle vle.csv
    copy_oulad oulad_student_vle studentVle.csv
    cat sql/edu_schema.sql sql/edu_load.sql
    if [ "${SKIP_DEMO_CAUSE:-0}" != "1" ]; then
        printf '%s\n' "\\echo 'planting the demo cause (sql/plant_demo_cause_edu.sql); set SKIP_DEMO_CAUSE=1 to skip'"
        cat sql/plant_demo_cause_edu.sql
    fi
}

# Nạp một database: $1 tên database, $2 schema dữ liệu, $3 hàm in SQL riêng của database đó. Phần chung (quyền, xóa
# staging, thống kê) nối vào cùng luồng để chạy trong cùng một giao dịch.
load_database() {
    create_database "$1"
    {
        # Ẩn NOTICE "drop cascades to ..." khi xóa schema cũ; lỗi và cảnh báo vẫn in.
        echo "SET client_min_messages = warning;"
        # Ngày giờ trong DataCo không có múi giờ; đổi qua to_timestamp theo UTC để giờ không lệch theo múi giờ của máy.
        echo "SET TIME ZONE 'UTC';"
        "$3"
        cat sql/grants.sql
        if [ "${KEEP_STAGING:-0}" != "1" ]; then
            echo "DROP SCHEMA staging CASCADE;"
        fi
        # Thống kê cho planner, để câu truy vấn đầu tiên sau khi nạp không chạy theo ước lượng sai.
        echo "ANALYZE;"
    } | psql_demo -d "$1" -v database="$1" -v schema="$2" --single-transaction -q -f -
    echo "loaded $1; row counts:"
    psql_demo -d "$1" -v schema="$2" -f - < sql/row_counts.sql
}

for target in $targets; do
    case $target in
        sales) load_database sales_demo sales sales_sql ;;
        edu) load_database edu_demo edu edu_sql ;;
    esac
done
