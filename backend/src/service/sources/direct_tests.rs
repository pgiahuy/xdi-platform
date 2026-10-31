// SPDX-License-Identifier: Apache-2.0
/* service/sources/direct_tests.rs - hai cách chép một bảng (COPY ra CSV ở postgres.rs, DuckDB đọc thẳng ở direct.rs)
cho cùng kiểu cột và cùng dữ liệu sau khi đổi sang Parquet (engine/analytics.rs), trên một bảng có đủ loại kiểu Postgres.
Cần DATABASE_URL (database riêng của test) và file extension postgres của DuckDB (make duckdb-extension, hoặc
SOURCE_POSTGRES_EXTENSION); thiếu một trong hai thì bỏ qua. Lúc extension thiếu, lần chụp quay về COPY: test qua route ở
api/source_snapshot_tests.rs. */

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::direct::{self, DirectError, DirectTable};
use super::pg_types;
use super::postgres::{self, ConnectInfo, SourceTable, TableCopy};
use crate::engine::analytics::{self, InputFile};
use crate::engine::store::test_support::{database_url, exec, fresh_database};

/// File extension để test: SOURCE_POSTGRES_EXTENSION nếu đặt, không thì chỗ make duckdb-extension ghi trong repo.
fn test_extension() -> Option<PathBuf> {
    let path = std::env::var("SOURCE_POSTGRES_EXTENSION")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../duckdb-extensions/postgres_scanner.duckdb_extension")
        });
    if path.is_file() {
        Some(path)
    } else {
        eprintln!(
            "skipping: no DuckDB postgres extension at {} (make duckdb-extension)",
            path.display()
        );
        None
    }
}

/// Thông tin kết nối tới database của test, từ DATABASE_URL dạng postgres://user:pw@host:port/db.
fn connect_info(database: &str) -> ConnectInfo {
    let url = database_url().unwrap();
    let rest = url.split_once("://").unwrap().1;
    let authority = rest.split(['/', '?']).next().unwrap();
    let (user_info, host_port) = authority.rsplit_once('@').unwrap();
    let (user, password) = user_info.split_once(':').unwrap_or((user_info, ""));
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    ConnectInfo {
        host: host.to_string(),
        port: port.parse().unwrap(),
        database: database.to_string(),
        user: user.to_string(),
        password: password.to_string(),
        sslmode: "disable".to_string(),
        ssl_root_cert: None,
    }
}

const CREATE: &str = "
CREATE TYPE mood AS ENUM ('calm', 'busy');
CREATE TABLE mixed (
    small int2, id int4, big int8, f4 float4, f8 float8, price numeric(10,2), ratio numeric, ok boolean,
    day date, at timestamp, at_tz timestamptz, code text, note varchar(20), fixed char(4), uid uuid, doc jsonb,
    tags int[], span interval, raw bytea, feeling mood, addr inet
)";

const INSERT: &str = "
INSERT INTO mixed
SELECT i, i * 10, i * 100000000000, i / 10.0, i / 3.0, i * 1.25, i / 7.0, i % 2 = 0,
       date '2024-01-01' + i, timestamp '2024-03-17 10:00:00' + i * interval '1 second',
       timestamptz '2024-03-17 10:00:00+07' + i * interval '1 minute', lpad(i::text, 5, '0'), 'n' || i, 'ab',
       md5(i::text)::uuid, jsonb_build_object('k', i, 'v', 'x y'), ARRAY[i, i + 1], i * interval '1 hour',
       decode(lpad(to_hex(i), 4, '0'), 'hex'), CASE WHEN i % 2 = 0 THEN 'calm' ELSE 'busy' END::mood, ('10.0.' || (i % 250) || '.1')::inet
FROM generate_series(1, 2000) AS i;
INSERT INTO mixed (id, f4, code, note, fixed) VALUES
    (-1, 0.1, '', 'a,\"b\"' || chr(10) || 'c', 'x'),
    (-2, 'NaN', NULL, NULL, NULL)";

/// Chép bằng COPY rồi đổi sang Parquet như service/ingest.rs; trả thư mục Parquet.
async fn copy_with_copy(
    conn: &mut sqlx::PgConnection,
    table: &SourceTable,
    dir: &Path,
) -> (TableCopy, PathBuf) {
    let csv = dir.join("copy.csv");
    let copy = postgres::copy_to_csv(conn, table, Some(1_000_000), 1 << 30, &csv)
        .await
        .unwrap();
    let out = materialize(&csv, &copy, dir.join("copy-parquet"));
    (copy, out)
}

fn materialize(path: &Path, copy: &TableCopy, out: PathBuf) -> PathBuf {
    std::fs::create_dir_all(&out).unwrap();
    let input = InputFile {
        path: path.to_path_buf(),
        name: "postgres://test/mixed".to_string(),
        format: copy.format,
        column_types: copy.column_types.clone(),
    };
    analytics::materialize(&[input], &out).unwrap();
    out
}

fn query_strings(conn: &duckdb::Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[tokio::test]
async fn both_modes_give_the_same_types_and_rows() {
    let Some(extension) = test_extension() else {
        return;
    };
    let Some(db) = fresh_database().await else {
        return;
    };
    for statement in CREATE.split(';').chain(INSERT.split(';')) {
        if !statement.trim().is_empty() {
            exec(&db.store, statement).await;
        }
    }
    let dir = std::env::temp_dir().join(format!("xdi-direct-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let info = connect_info(db.name());
    let table = SourceTable {
        schema: "public".to_string(),
        name: "mixed".to_string(),
        kind: "table".to_string(),
    };
    let mut conn = postgres::connect(&info, Duration::from_secs(30))
        .await
        .unwrap();
    let (csv_copy, csv_out) = copy_with_copy(&mut conn, &table, &dir).await;

    let mut config = crate::config::Config::from_env();
    config.data_dir = dir.clone();
    config.source_postgres_extension = extension;
    let column_types = pg_types::column_types(&mut conn, &postgres::select_all(&table))
        .await
        .unwrap();
    // Chép hai lần: cùng bảng không đổi phải ra cùng SHA-256, nếu không mỗi lần chụp lại tạo một phiên bản trùng.
    let mut direct_copies = Vec::new();
    for n in 0..2 {
        let wanted = vec![DirectTable {
            table: table.clone(),
            column_types: column_types.clone(),
            out: dir.join(format!("direct-{n}.parquet")),
        }];
        match direct::copy_tables(&config, &info, wanted).await {
            Ok(mut copies) => direct_copies.push(copies.remove(0)),
            Err(DirectError::Unavailable(reason)) => panic!("direct unavailable: {reason}"),
            Err(DirectError::Failed(e)) => panic!("direct failed: {e:?}"),
        }
    }
    assert_eq!(direct_copies[0].sha256, direct_copies[1].sha256);
    assert_eq!(direct_copies[0].column_types, csv_copy.column_types);
    let direct_out = materialize(
        &dir.join("direct-0.parquet"),
        &direct_copies[0],
        dir.join("direct-parquet"),
    );

    let duck = duckdb::Connection::open_in_memory().unwrap();
    let read = |out: &Path| analytics::parquet_glob(out).unwrap();
    let describe = |out: &Path| {
        query_strings(
            &duck,
            &format!(
                "SELECT column_name || ' ' || column_type FROM (DESCRIBE SELECT * FROM {})",
                read(out)
            ),
        )
    };
    assert_eq!(describe(&csv_out), describe(&direct_out));
    // Mọi dòng giống hệt, kể cả số lần lặp: EXCEPT ALL hai chiều đều rỗng. So bằng chữ để NaN và NULL so được.
    let as_text = |out: &Path| {
        format!(
            "SELECT * EXCLUDE ({}) FROM {}",
            analytics::SOURCE_COLUMN,
            read(out)
        )
    };
    let mut diffs = Vec::new();
    for (left, right) in [(&csv_out, &direct_out), (&direct_out, &csv_out)] {
        diffs.extend(query_strings(
            &duck,
            &format!(
                "SELECT CAST(d AS VARCHAR) FROM ({} EXCEPT ALL {}) AS d LIMIT 2",
                as_text(left),
                as_text(right)
            ),
        ));
    }
    assert!(diffs.is_empty(), "rows differ: {diffs:#?}");
    let count: i64 = duck
        .query_row(
            &format!("SELECT count(*) FROM {}", read(&direct_out)),
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2002);
    // Thư mục tạm của passfile đã bị xóa: trong dir chỉ còn file của test.
    let leftovers: Vec<String> = std::fs::read_dir(dir.join("staging"))
        .map(|entries| {
            entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    let _ = std::fs::remove_dir_all(&dir);
    drop(conn);
    db.drop().await;
}
