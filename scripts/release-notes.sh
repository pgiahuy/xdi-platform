#!/bin/sh
# release-notes.sh - in mục của một phiên bản trong CHANGELOG.md, dùng làm ghi chú cho GitHub Release.
# Dùng: scripts/release-notes.sh 0.1.0. Không có mục [0.1.0] thì báo lỗi, vì phát hành mà không có ghi chú là quên bước.
set -eu
version="${1:?usage: release-notes.sh <x.y.z>}"
cd "$(dirname "$0")/.."
notes=$(awk -v v="$version" '
  /^## \[/ { if (found) exit; if (index($0, "## [" v "]") == 1) { found = 1; next } }
  found { print }
' CHANGELOG.md)
if [ -z "$(printf '%s' "$notes" | tr -d '[:space:]')" ]; then
  echo "CHANGELOG.md has no section for [$version]" >&2
  exit 1
fi
printf '%s\n' "$notes"
