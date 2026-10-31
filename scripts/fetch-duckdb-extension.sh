#!/bin/sh
# scripts/fetch-duckdb-extension.sh - tải extension postgres của DuckDB (tên file postgres_scanner) đúng phiên bản
# DuckDB mà crate duckdb biên dịch kèm, kiểm SHA-256, ghi ra một file. Chỉ chạy lúc build (Dockerfile, make
# duckdb-extension, make release-artifacts); backend nạp file này bằng đường dẫn cục bộ và không bao giờ tải gì lúc chạy
# (service/sources/direct.rs).
#
# Cách dùng: scripts/fetch-duckdb-extension.sh <file đích> [nền tảng]
#   nền tảng: linux_amd64 hoặc linux_arm64; bỏ trống thì lấy theo uname -m của máy đang chạy.
#
# Nâng crate duckdb (backend/Cargo.toml) sang bản DuckDB khác thì sửa DUCKDB_VERSION và hai checksum dưới đây: tải file
# mới, kiểm bằng sha256sum sau khi giải nén. Test duckdb_version_matches_the_extension_pin (service/sources/direct.rs)
# báo lỗi khi DUCKDB_VERSION lệch bản DuckDB trong binary.

set -eu

DUCKDB_VERSION=v1.5.6
SHA256_linux_amd64=6706047a549bbb4b73afe6c81c87df2f6daf863545bf38b8af2fc2d60e501fc3
SHA256_linux_arm64=e8de4878f83574436f86159dbdad4839584bfcabe95411bd39eebb81dfd59623

out=${1:?usage: fetch-duckdb-extension.sh <output file> [linux_amd64|linux_arm64]}
platform=${2:-}
if [ -z "$platform" ]; then
  case "$(uname -m)" in
    x86_64) platform=linux_amd64 ;;
    aarch64 | arm64) platform=linux_arm64 ;;
    *) echo "no pinned DuckDB extension for $(uname -m)" >&2; exit 1 ;;
  esac
fi
case "$platform" in
  linux_amd64) expected=$SHA256_linux_amd64 ;;
  linux_arm64) expected=$SHA256_linux_arm64 ;;
  *) echo "no pinned DuckDB extension for platform $platform" >&2; exit 1 ;;
esac

# Đã có file đúng checksum thì thôi, để make dev không tải lại mỗi lần.
if [ -f "$out" ] && echo "$expected  $out" | sha256sum -c --status; then
  echo "$out is already the pinned extension ($DUCKDB_VERSION, $platform)"
  exit 0
fi

url="https://extensions.duckdb.org/$DUCKDB_VERSION/$platform/postgres_scanner.duckdb_extension.gz"
mkdir -p "$(dirname "$out")"
tmp="$out.download"
# Ghi ra file tạm rồi mới đổi tên: tải dở hay sai checksum không để lại file trông như dùng được.
trap 'rm -f "$tmp" "$tmp.gz"' EXIT
if command -v curl > /dev/null; then
  curl -fsSL -o "$tmp.gz" "$url"
else
  wget -q -O "$tmp.gz" "$url"
fi
gunzip -c "$tmp.gz" > "$tmp"
if ! echo "$expected  $tmp" | sha256sum -c --status; then
  echo "checksum mismatch for $url" >&2
  exit 1
fi
mv "$tmp" "$out"
echo "wrote $out ($DUCKDB_VERSION, $platform)"
