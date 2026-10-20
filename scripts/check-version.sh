#!/bin/sh
# check-version.sh - kiểm số phiên bản phát hành khớp ở mọi nơi khai báo, để tag v0.2.0 không phát hành binary 0.1.0.
# Dùng: scripts/check-version.sh 0.1.0
set -eu
want="${1:?usage: check-version.sh <x.y.z>}"
case "$want" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must look like x.y.z, got: $want" >&2; exit 1 ;;
esac
cd "$(dirname "$0")/.."
fail=0
cargo_version=$(sed -n 's/^version = "\(.*\)"/\1/p' backend/Cargo.toml | head -n 1)
if [ "$cargo_version" != "$want" ]; then
  echo "backend/Cargo.toml has $cargo_version, expected $want" >&2
  fail=1
fi
if [ -f frontend/package.json ]; then
  npm_version=$(sed -n 's/^  "version": "\(.*\)",/\1/p' frontend/package.json | head -n 1)
  if [ "$npm_version" != "$want" ]; then
    echo "frontend/package.json has $npm_version, expected $want" >&2
    fail=1
  fi
fi
exit $fail
