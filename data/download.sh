#!/usr/bin/env sh
# Tải bản đầy đủ của các dataset mẫu vào data/downloads/ (git bỏ qua thư mục này). Nguồn, giấy phép: data/README.md.
#   sh data/download.sh            tải cả ba
#   sh data/download.sh dataco     chỉ DataCo Smart Supply Chain (CSV, khoảng 96 MB)
#   sh data/download.sh retail     chỉ Online Retail II (file .zip khoảng 46 MB, giải nén ra .xlsx)
#   sh data/download.sh oulad      chỉ Open University Learning Analytics (file .zip khoảng 45 MB, giải nén ra 7 CSV
#                                  trong oulad/, tổng khoảng 464 MB, riêng studentVle.csv 454 MB)
# Mỗi file tải vào <tên>.part, kiểm SHA-256 rồi mới đổi tên; file đã có và đúng checksum thì bỏ qua. Tải không được
# thì in trang nguồn để tải tay. Nạp file tải về ở trang Dữ liệu của giao diện (docs/INSTALL.md mục 7).
set -eu
cd "$(dirname "$0")"
OUT=downloads

# URL tải trực tiếp và SHA-256 lấy từ API công khai của Mendeley Data
# (https://data.mendeley.com/public-api/datasets/8gx2fvg2k6/files?folder_id=root&version=5).
DATACO_URL=https://data.mendeley.com/public-files/datasets/8gx2fvg2k6/files/72784be5-36d3-44fe-b75d-0edbf1999f65/file_downloaded
DATACO_SHA256=fa6d022ed437155e1a2f0378710602848703c8a7f203f7ff5d77805bf8480aa6
DATACO_FILE=DataCoSupplyChainDataset.csv
DATACO_PAGE=https://doi.org/10.17632/8gx2fvg2k6.5

# UCI không công bố checksum; SHA-256 dưới đây tính từ file tải về ngày 2026-10-27. UCI thay file thì checksum lệch và
# script dừng, báo tải tay.
RETAIL_URL=https://archive.ics.uci.edu/static/public/502/online+retail+ii.zip
RETAIL_SHA256=572e36277c2390fbfde10664750731e0a86f55e33470d91919085f0408e67bfb
RETAIL_ZIP=online_retail_ii.zip
RETAIL_FILE=online_retail_II.xlsx
RETAIL_PAGE=https://archive.ics.uci.edu/dataset/502/online+retail+ii

# Cùng cách với Online Retail II: SHA-256 tính từ file tải về ngày 2026-10-27.
OULAD_URL=https://archive.ics.uci.edu/static/public/349/open+university+learning+analytics+dataset.zip
OULAD_SHA256=f2ed1902616c1fe8d2824d872c0b7d2d72be435bf0124d077044fe4be2c6d3e4
OULAD_ZIP=oulad.zip
OULAD_DIR=oulad
OULAD_PAGE=https://archive.ics.uci.edu/dataset/349/open+university+learning+analytics+dataset

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# fetch <url> <sha256> <file trong downloads/> <trang nguồn>. Trả 0 khi file có và đúng checksum.
fetch() {
    url=$1 sum=$2 file=$OUT/$3 page=$4
    if [ -f "$file" ] && [ "$(sha256_of "$file")" = "$sum" ]; then
        echo "data/$file đã có, đúng checksum"
        return 0
    fi
    echo "Tải data/$file"
    if ! curl -fL --retry 3 -o "$file.part" "$url"; then
        rm -f "$file.part"
        echo "Không tải được $url. Tải tay từ $page rồi đặt vào data/$file" >&2
        return 1
    fi
    if [ "$(sha256_of "$file.part")" != "$sum" ]; then
        rm -f "$file.part"
        echo "Checksum của data/$file không khớp: file nguồn đã đổi hoặc tải hỏng. Tải tay từ $page" >&2
        return 1
    fi
    mv "$file.part" "$file"
}

dataco() {
    fetch "$DATACO_URL" "$DATACO_SHA256" "$DATACO_FILE" "$DATACO_PAGE" || return 1
    echo "DataCo: CC BY 4.0, ghi công: Constante, Silva, Pereira, DataCo Smart Supply Chain for Big Data Analysis, Mendeley Data, $DATACO_PAGE"
}

retail() {
    fetch "$RETAIL_URL" "$RETAIL_SHA256" "$RETAIL_ZIP" "$RETAIL_PAGE" || return 1
    if ! command -v unzip >/dev/null 2>&1; then
        echo "Thiếu lệnh unzip: giải nén data/$OUT/$RETAIL_ZIP bằng tay để lấy $RETAIL_FILE" >&2
        return 1
    fi
    unzip -o -q "$OUT/$RETAIL_ZIP" "$RETAIL_FILE" -d "$OUT"
    echo "Online Retail II: CC BY 4.0, ghi công: Chen, Online Retail II, UCI Machine Learning Repository, https://doi.org/10.24432/C5CG6D"
}

oulad() {
    fetch "$OULAD_URL" "$OULAD_SHA256" "$OULAD_ZIP" "$OULAD_PAGE" || return 1
    if ! command -v unzip >/dev/null 2>&1; then
        echo "Thiếu lệnh unzip: giải nén data/$OUT/$OULAD_ZIP bằng tay vào data/$OUT/$OULAD_DIR" >&2
        return 1
    fi
    unzip -o -q "$OUT/$OULAD_ZIP" '*.csv' -d "$OUT/$OULAD_DIR"
    echo "OULAD: CC BY 4.0, ghi công: Kuzilek, Hlosta, Zdrahal, Open University Learning Analytics dataset, UCI Machine Learning Repository, https://doi.org/10.24432/C5KK69"
}

command -v curl >/dev/null 2>&1 || { echo "Cần curl để tải dữ liệu" >&2; exit 1; }
mkdir -p "$OUT"

status=0
case "${1:-all}" in
    dataco) dataco || status=1 ;;
    retail) retail || status=1 ;;
    oulad) oulad || status=1 ;;
    all)
        dataco || status=1
        retail || status=1
        oulad || status=1
        ;;
    *) echo "Dùng: sh data/download.sh [dataco|retail|oulad]" >&2; exit 2 ;;
esac
exit $status
