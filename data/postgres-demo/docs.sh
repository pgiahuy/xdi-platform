#!/bin/sh
# data/postgres-demo/docs.sh - sinh lại tài liệu bảng (Markdown) và sơ đồ ER (SVG) của hai database từ container đang
# chạy, bằng tbls: docs/sales_demo/ theo .tbls.sales_demo.yml, docs/edu_demo/ theo .tbls.edu_demo.yml.
#
# Cách chạy, sau khi load.sh xong: TBLS=/đường/dẫn/tới/tbls sh data/postgres-demo/docs.sh
# tbls không nằm trong repo: tải bản phát hành ở https://github.com/k1LoW/tbls/releases (giấy phép MIT). Không đặt
# TBLS thì dùng lệnh tbls trên PATH.
#
# --rm-dist xóa thư mục đích trước khi sinh, để bảng đã bỏ khỏi schema không còn file cũ.

set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"
export DEMO_DB_PORT="${DEMO_DB_PORT:-5440}"
for database in sales_demo edu_demo; do
    "${TBLS:-tbls}" doc --rm-dist --config ".tbls.$database.yml"
done
