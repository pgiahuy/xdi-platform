#!/bin/sh
# scripts/bench-snapshot.sh - đo chụp bảng Postgres (service/sources/) trên bảng sinh bằng generate_series, để đặt trần
# SOURCE_SNAPSHOT_MAX_MB, SOURCE_TABLE_MAX_MB, SOURCE_TIMEOUT_SECS từ số đo (docs/PERFORMANCE.md, mục Chụp bảng lớn).
#
# Cách dùng:
#   scripts/bench-snapshot.sh setup   tạo schema bench trong BENCH_URL: fact_1m, fact_5m, fact_10m, fact_20m (6 cột),
#                                     wide_1m, wide_5m (60 cột)
#   scripts/bench-snapshot.sh run [bảng...]
#                                     mỗi bảng: database ứng dụng và DATA_DIR mới, một tiến trình backend mới, một lần
#                                     chụp qua HTTP API; in một dòng CSV
#   scripts/bench-snapshot.sh defaults
#                                     một lần chụp mọi bảng (thân rỗng) với trần mặc định của backend: thời gian, bảng
#                                     nào chép, bảng nào chỉ lưu metadata
#
# Biến (mặc định là Postgres của make dev, cổng 5433):
#   BENCH_URL   database chứa bảng đo, ví dụ postgres://xdi:xdi@127.0.0.1:5433/xdi_run_bench (tạo trước bằng createdb)
#   APP_URL     database ứng dụng, bị xóa và tạo lại trước mỗi bảng (không dùng database xdi)
#   BACKEND     binary build --release; WORK thư mục DATA_DIR và log; EXTENSION file extension postgres của DuckDB
#   PYTHON      python3 có module duckdb (pip install duckdb==1.5.6) để đo một câu tổng hợp trên file Parquet;
#               không có thì bỏ cột đó
#
# Cột in ra: bảng, số dòng, số cột, dung lượng Postgres (pg_total_relation_size, MB), thời gian request chụp (giây),
# giao dịch dài nhất trên database đo trong lúc chụp (giây), dung lượng Parquet (MB), bộ nhớ đỉnh của backend (VmHWM, MiB), thời gian câu tổng hợp trên Parquet (giây).

set -eu

BENCH_URL=${BENCH_URL:-postgres://xdi:xdi@127.0.0.1:5433/xdi_run_bench}
APP_URL=${APP_URL:-postgres://xdi:xdi@127.0.0.1:5433/xdi_run_cap}
BACKEND=${BACKEND:-backend/target/release/xdi-backend}
WORK=${WORK:-$PWD/bench-snapshot}
EXTENSION=${EXTENSION:-$PWD/duckdb-extensions/postgres_scanner.duckdb_extension}
PYTHON=${PYTHON:-python3}
PORT=${PORT:-8790}

case "$APP_URL" in */xdi) echo "APP_URL must not be the xdi database" >&2; exit 1 ;; esac

# Bảng hẹp: khóa, hai mã, ngày, số tiền, nhóm chữ, như một bảng sự kiện bán hàng. setseed để lần sinh sau ra cùng dữ liệu.
narrow_sql() {
  cat <<SQL
SELECT setseed(0.42);
CREATE TABLE bench.$1 AS
SELECT i::bigint AS id,
       (i % 50000)::int AS customer_id,
       (i * 7919 % 2000)::int AS product_id,
       date '2020-01-01' + (i % 1500)::int AS order_date,
       round((random() * 1000)::numeric, 2)::numeric(12,2) AS amount,
       (ARRAY['north','south','east','west','central','online'])[1 + (i % 6)::int] AS region
FROM generate_series(1::bigint, $2) AS i;
ANALYZE bench.$1;
SQL
}

# Bảng rộng: 60 cột, gồm 20 số nguyên, 20 số thập phân, 10 chữ, 5 ngày, 4 cờ.
wide_sql() {
  cols="i::bigint AS id"
  n=1; while [ $n -le 20 ]; do cols="$cols, (i * $n % 100000)::int AS int_$n"; n=$((n + 1)); done
  n=1; while [ $n -le 20 ]; do cols="$cols, round((random() * 10000)::numeric, 2)::numeric(12,2) AS num_$n"; n=$((n + 1)); done
  n=1; while [ $n -le 10 ]; do cols="$cols, 'code_' || (i * $n % 5000)::text AS text_$n"; n=$((n + 1)); done
  n=1; while [ $n -le 5 ]; do cols="$cols, date '2020-01-01' + ((i + $n) % 1500)::int AS date_$n"; n=$((n + 1)); done
  n=1; while [ $n -le 4 ]; do cols="$cols, (i % ($n + 1) = 0) AS flag_$n"; n=$((n + 1)); done
  printf 'SELECT setseed(0.42);\nCREATE TABLE bench.%s AS SELECT %s FROM generate_series(1::bigint, %s) AS i;\nANALYZE bench.%s;\n' \
    "$1" "$cols" "$2" "$1"
}

setup() {
  psql "$BENCH_URL" -v ON_ERROR_STOP=1 -qc "CREATE SCHEMA IF NOT EXISTS bench"
  for spec in fact_1m:1000000 fact_5m:5000000 fact_10m:10000000 fact_20m:20000000; do
    name=${spec%%:*}; rows=${spec#*:}
    psql "$BENCH_URL" -v ON_ERROR_STOP=1 -qc "DROP TABLE IF EXISTS bench.$name"
    narrow_sql "$name" "$rows" | psql "$BENCH_URL" -v ON_ERROR_STOP=1 -q >/dev/null
    echo "created bench.$name"
  done
  for spec in wide_1m:1000000 wide_5m:5000000; do
    name=${spec%%:*}; rows=${spec#*:}
    psql "$BENCH_URL" -v ON_ERROR_STOP=1 -qc "DROP TABLE IF EXISTS bench.$name"
    wide_sql "$name" "$rows" | psql "$BENCH_URL" -v ON_ERROR_STOP=1 -q >/dev/null
    echo "created bench.$name"
  done
}

# Phần của URL postgres://user:pw@host:port/db.
url_field() {
  rest=${1#*://}; auth=${rest%%@*}; hostdb=${rest#*@}; hostport=${hostdb%%/*}
  case "$2" in
    user) echo "${auth%%:*}" ;; password) echo "${auth#*:}" ;; host) echo "${hostport%%:*}" ;;
    port) echo "${hostport#*:}" ;; db) echo "${hostdb#*/}" ;;
  esac
}

# Câu tổng hợp như câu hỏi "doanh thu theo vùng theo tháng" trên file Parquet, đo bằng DuckDB.
aggregate_secs() {
  file=$1; table=$2
  case "$table" in
    wide_*) sql="SELECT text_1, date_trunc('month', date_1) AS m, sum(num_1), count(*) FROM read_parquet('$file') GROUP BY 1, 2" ;;
    *) sql="SELECT region, date_trunc('month', order_date) AS m, sum(amount), count(*) FROM read_parquet('$file') GROUP BY 1, 2" ;;
  esac
  "$PYTHON" -I -c '
import sys, time, duckdb
con = duckdb.connect()
con.execute(sys.argv[1]).fetchall()
start = time.perf_counter(); con.execute(sys.argv[1]).fetchall()
print(f"{time.perf_counter() - start:.2f}")' "$sql" 2>/dev/null || echo ""
}

run_one() {
  table=$1
  app_db=$(url_field "$APP_URL" db)
  admin_url=${APP_URL%/*}/postgres
  psql "$admin_url" -qc "DROP DATABASE IF EXISTS $app_db" >/dev/null
  psql "$admin_url" -qc "CREATE DATABASE $app_db" >/dev/null
  rm -rf "$WORK/data"; mkdir -p "$WORK/data"
  # Đo từng bảng: trần đặt rất cao để mọi bảng đều chép. Lệnh defaults: trần mặc định của backend.
  caps="SOURCE_SNAPSHOT_MAX_MB=1000000 SOURCE_TABLE_MAX_MB=1000000 SOURCE_TIMEOUT_SECS=3600"
  [ "$table" = all ] && caps=""
  # $caps cố ý không có nháy kép: mỗi từ là một biến cho env.
  # shellcheck disable=SC2086
  env DATABASE_URL="$APP_URL" DATA_DIR="$WORK/data" BACKEND_PORT="$PORT" LLM_PROVIDER=mock SEED_SAMPLE=false \
    SOURCE_SECRET_KEY="$(head -c 32 /dev/urandom | base64)" SOURCE_ALLOWED_HOSTS="$(url_field "$BENCH_URL" host)" \
    SOURCE_MAX_ROWS="${SOURCE_MAX_ROWS:-}" SOURCE_POSTGRES_EXTENSION="$EXTENSION" $caps \
    "$BACKEND" >"$WORK/backend.log" 2>&1 &
  pid=$!
  trap 'kill "$pid" 2>/dev/null || true' EXIT
  i=0
  until curl -sf "http://127.0.0.1:$PORT/api/auth/setup" >/dev/null; do
    i=$((i + 1)); [ $i -gt 100 ] && { echo "backend did not start, see $WORK/backend.log" >&2; kill "$pid"; exit 1; }
    sleep 0.2
  done
  code=$(sed 's/\x1b\[[0-9;]*m//g' "$WORK/backend.log" | sed -n 's/.*setup_code=\([A-Za-z0-9-]*\).*/\1/p' | head -1)
  jar=$WORK/cookies
  curl -sf -c "$jar" -H 'content-type: application/json' "http://127.0.0.1:$PORT/api/auth/setup" \
    -d "{\"setup_code\":\"$code\",\"email\":\"bench@example.com\",\"password\":\"bench-password\",\"workspace_name\":\"Bench\"}" \
    >"$WORK/me.json"
  ws=$(jq -r '.workspaces[0].slug' "$WORK/me.json")
  api=http://127.0.0.1:$PORT/api/workspaces/$ws
  source_body=$(jq -n --arg h "$(url_field "$BENCH_URL" host)" --arg p "$(url_field "$BENCH_URL" port)" \
    --arg d "$(url_field "$BENCH_URL" db)" --arg u "$(url_field "$BENCH_URL" user)" \
    --arg w "$(url_field "$BENCH_URL" password)" \
    '{host: $h, port: ($p | tonumber), database: $d, user: $u, password: $w, sslmode: "disable"}')
  id=$(curl -sf -b "$jar" -H 'content-type: application/json' "$api/sources" -d "$source_body" | jq -r '.id')
  # Giao dịch dài nhất trên database đo trong lúc chụp (pg_stat_activity.xact_start), đọc mỗi 0,2 giây: đây là thời gian
  # database nguồn giữ ảnh REPEATABLE READ mở.
  bench_db=$(url_field "$BENCH_URL" db)
  : >"$WORK/xact"
  (while :; do
    psql "$BENCH_URL" -Atc "SELECT coalesce(max(extract(epoch FROM now() - xact_start)), 0) FROM pg_stat_activity
      WHERE datname = '$bench_db' AND pid <> pg_backend_pid() AND xact_start IS NOT NULL" >>"$WORK/xact" 2>/dev/null
    sleep 0.2
  done) &
  sampler=$!
  body="{\"tables\":[\"bench.$table\"]}"
  [ "$table" = all ] && body="{}"
  secs=$(curl -s -o "$WORK/snapshot.json" -w '%{time_total}' -b "$jar" -H 'content-type: application/json' \
    "$api/sources/$id/snapshot" -d "$body")
  kill "$sampler"
  xact=$(sort -n "$WORK/xact" | tail -1 | xargs printf '%.1f')
  hwm_kb=$(sed -n 's/^VmHWM:[[:space:]]*\([0-9]*\) kB/\1/p' "/proc/$pid/status")
  kill "$pid"; wait "$pid" 2>/dev/null || true
  if [ "$table" = all ]; then
    echo "all tables, default limits: $secs s, transaction $xact s, peak RSS $((hwm_kb / 1024)) MiB"
    jq -r '"copied: " + ([.datasets[].dataset.source_table] | join(" ")),
      (.metadata_only[] | "metadata only: " + .table + " (" + .cause + ")")' "$WORK/snapshot.json"
    return
  fi
  if [ "$(jq '.datasets | length' "$WORK/snapshot.json")" != 1 ]; then
    echo "$table: snapshot failed: $(jq -c . "$WORK/snapshot.json")" >&2
    return
  fi
  file=$(find "$WORK/data" -name '*.parquet' -not -path '*/staging/*' -printf '%s %p\n' | sort -n | tail -1 | cut -d' ' -f2)
  pq_mb=$(( $(stat -c %s "$file") / 1048576 ))
  meta=$(psql "$BENCH_URL" -Atc "SELECT c.reltuples::bigint, (SELECT count(*) FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped), pg_total_relation_size(c.oid) / 1048576 FROM pg_class c WHERE c.oid = 'bench.$table'::regclass")
  agg=$(aggregate_secs "$file" "$table")
  echo "$table,$(echo "$meta" | tr '|' ','),$secs,$xact,$pq_mb,$((hwm_kb / 1024)),$agg"
}

case "${1:-}" in
  setup) setup ;;
  run)
    shift
    [ $# -eq 0 ] && set -- fact_1m fact_5m fact_10m fact_20m wide_1m wide_5m
    mkdir -p "$WORK"
    echo "table,rows,columns,postgres_mb,snapshot_secs,transaction_secs,parquet_mb,peak_rss_mib,aggregate_secs"
    for t in "$@"; do run_one "$t"; done
    ;;
  defaults) mkdir -p "$WORK"; run_one all ;;
  *) sed -n '2,24p' "$0"; exit 1 ;;
esac
