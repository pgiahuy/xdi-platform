#!/bin/sh
# scripts/schema-docs.sh - sinh lại sơ đồ ER của Postgres ứng dụng (docs/images/schema.svg) từ một database đã chạy đủ
# migration của nhánh hiện tại. Ý nghĩa từng bảng và ví dụ viết tay ở docs/SCHEMA.md; file này chỉ vẽ hình.
#
# Cách chạy: TBLS=/đường/dẫn/tới/tbls DATABASE_URL=postgres://xdi:xdi@localhost:5433/<database> make schema-docs
# tbls không nằm trong repo: tải bản phát hành ở https://github.com/k1LoW/tbls/releases (giấy phép MIT). Không đặt
# TBLS thì dùng lệnh tbls trên PATH. Cần thêm lệnh dot của Graphviz.
#
# Dùng một database riêng vừa migrate (backend tự migrate lúc khởi động), không dùng database xdi đang chạy: sơ đồ chỉ
# đọc cấu trúc, nhưng database riêng chắc chắn khớp đúng migration của nhánh.
#
# Không dùng ảnh tbls tự vẽ: bản tbls đã thử vẽ hỏng khi sơ đồ lớn. Thay vào đó xuất .dot, bỏ nhãn khóa ngoại dài ở
# mỗi mũi tên (câu FOREIGN KEY ... đầy đủ, làm sơ đồ rộng gấp mấy lần; ràng buộc đầy đủ đọc ở backend/migrations/),
# xếp bảng từ trái sang phải, rồi vẽ SVG bằng dot.

set -eu

: "${DATABASE_URL:?set DATABASE_URL to a database migrated by this branch, for example postgres://xdi:xdi@localhost:5433/xdi_schema}"

here=$(cd "$(dirname "$0")/.." && pwd)
out="$here/docs/images/schema.svg"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# tbls cần sslmode trong DSN; Postgres của compose không bật TLS.
dsn="$DATABASE_URL"
case "$dsn" in
    *sslmode=*) ;;
    *\?*) dsn="$dsn&sslmode=disable" ;;
    *) dsn="$dsn?sslmode=disable" ;;
esac

# _sqlx_migrations là bảng sổ sách của sqlx, không phải dữ liệu ứng dụng.
"${TBLS:-tbls}" out -t dot --exclude _sqlx_migrations -o "$work/raw.dot" "$dsn"

sed -E \
    -e '1s/^digraph .*/digraph "xdi-platform" {/' \
    -e 's/^  graph \[.*\];$/  graph [rankdir=LR, layout=dot, fontname="Arial", splines=true, nodesep=0.5, ranksep=1.4];/' \
    -e 's/^  edge \[.*\];$/  edge [fontsize=10, fontname="Arial", color="#7a8a90", arrowsize=0.7];/' \
    -e 's/, +taillabel=<<table.*<\/table>>\];$/];/' \
    "$work/raw.dot" > "$work/schema.dot"

if grep -q taillabel "$work/schema.dot"; then
    echo "schema-docs: tbls output changed, foreign key labels were not removed; check the sed patterns" >&2
    exit 1
fi

dot -Tsvg "$work/schema.dot" -o "$work/schema.svg"
mv "$work/schema.svg" "$out"
echo "wrote $out"
