// SPDX-License-Identifier: Apache-2.0
/* engine/guard/tests.rs - test guard SQL: câu nào qua, câu nào bị chặn, câu được sửa thành gì. Test cuối chạy trên
DuckDB thật vì so chuỗi không chứng minh được DuckDB chạy được bản đã đổi. Nhiều test so nguyên chuỗi sqlparser in ra,
nên nâng phiên bản sqlparser có thể phải sửa chuỗi mong đợi. */

use std::path::PathBuf;

use super::{build_guarded_sql, AllowedTable, GuardError, GuardedSql};
use crate::engine::analytics::parquet_glob;

const MAX_ROWS: u64 = 100;

fn table(name: &str, columns: &[&str], parquet_dir: PathBuf) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir,
    }
}

fn tables_in(dir: &str) -> Vec<AllowedTable> {
    let dir = PathBuf::from(dir);
    vec![
        table(
            "sales",
            &[
                "region",
                "amount",
                "order_date",
                "customer_id",
                "Order Item Total",
            ],
            dir.join("sales"),
        ),
        table(
            "customers",
            &["customer_id", "segment"],
            dir.join("customers"),
        ),
    ]
}

fn guard(sql: &str) -> Result<GuardedSql, GuardError> {
    build_guarded_sql(sql, &tables_in("/data"), MAX_ROWS)
}

/// Phần read_parquet mà guard thay vào cho một bảng trong tables_in("/data"). Tính bằng parquet_glob thay vì viết
/// tay: Windows nối đường dẫn bằng dấu \, nên chuỗi viết tay "/data/sales/*.parquet" chỉ khớp trên Linux.
fn expected_read_parquet(table: &str) -> String {
    parquet_glob(&PathBuf::from("/data").join(table)).unwrap()
}

fn assert_accepted(sql: &str) -> GuardedSql {
    match guard(sql) {
        Ok(guarded) => guarded,
        Err(e) => panic!("guard rejected {sql:?}: {e:?}"),
    }
}

/// Trả lý do từ chối để test kiểm được lý do có nói đúng chỗ sai không. Nhận cả hai loại từ chối.
fn assert_rejected(sql: &str) -> String {
    match guard(sql) {
        Err(GuardError::Rejected(reason) | GuardError::UnknownName(reason)) => reason,
        other => panic!("guard should reject {sql:?}, got {other:?}"),
    }
}

/// Như assert_rejected, nhưng phải là loại sai tên bảng hay cột, loại service gửi lại cho mô hình sửa.
fn assert_unknown_name(sql: &str) -> String {
    match guard(sql) {
        Err(GuardError::UnknownName(reason)) => reason,
        other => panic!("guard should reject {sql:?} as UnknownName, got {other:?}"),
    }
}

/// Loại từ chối mà sửa tên không giúp gì: service dừng câu hỏi, không nhờ mô hình sửa.
fn assert_rejected_for_other_reason(sql: &str) {
    match guard(sql) {
        Err(GuardError::Rejected(_)) => {}
        other => panic!("guard should reject {sql:?} as Rejected, got {other:?}"),
    }
}

// Câu được qua

#[test]
fn accepts_plain_select() {
    assert_accepted(
        "SELECT region, SUM(amount) AS total FROM sales GROUP BY region ORDER BY total DESC",
    );
}

#[test]
fn accepts_with_union_and_subqueries() {
    assert_accepted(
        "WITH s AS (SELECT region, amount FROM sales) \
         SELECT region FROM s UNION ALL \
         SELECT region FROM (SELECT region FROM sales) t WHERE amount > (SELECT AVG(amount) FROM sales)",
    );
}

#[test]
fn accepts_join_with_table_aliases() {
    assert_accepted(
        "SELECT c.segment, SUM(s.amount) FROM sales s JOIN customers c ON s.customer_id = c.customer_id \
         GROUP BY c.segment",
    );
}

#[test]
fn accepts_names_in_any_case_and_quoted_columns() {
    assert_accepted("SELECT REGION, SUM(\"Order Item Total\") FROM Sales GROUP BY Region");
    assert_accepted("SELECT \"order item total\" FROM \"SALES\"");
}

#[test]
fn accepts_date_functions_and_column_aliases_of_with() {
    assert_accepted(
        "SELECT date_trunc('month', order_date) AS m, COUNT(*) FROM sales GROUP BY m ORDER BY m",
    );
    assert_accepted(
        "WITH t(r, total) AS (SELECT region, SUM(amount) FROM sales GROUP BY region) \
         SELECT r, total FROM t WHERE total > 0",
    );
}

#[test]
fn accepts_trailing_semicolon() {
    assert_accepted("SELECT 1;");
}

/// Guard đọc cây cú pháp nên SQL viết trong chuỗi chỉ là một giá trị để so, không phải câu lệnh.
#[test]
fn accepts_sql_text_inside_a_string_value() {
    let guarded =
        assert_accepted("SELECT region FROM sales WHERE region = '; DROP TABLE sales; --'");
    assert_eq!(
        guarded.shown,
        "SELECT region FROM sales WHERE region = '; DROP TABLE sales; --' LIMIT 100"
    );
}

// Đọc câu và chỉ nhận một câu SELECT

#[test]
fn rejects_unparsable_sql() {
    for sql in ["SELEC 1", "SELECT * FROM", "SELECT (1"] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_empty_sql() {
    for sql in ["", "   ", "-- only a comment"] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_more_than_one_statement() {
    assert_rejected("SELECT 1; DROP TABLE sales");
    assert_rejected("SELECT 1; SELECT 2");
}

/// Chú thích bị bỏ khi đọc: câu lệnh giấu trong chú thích không chạy, và không còn trong bản hiện cho người dùng.
#[test]
fn comments_cannot_hide_a_second_statement() {
    for sql in [
        "SELECT region FROM sales -- ; DROP TABLE sales",
        "SELECT region FROM sales /* ; DROP TABLE sales */",
    ] {
        assert_eq!(
            assert_accepted(sql).shown,
            "SELECT region FROM sales LIMIT 100"
        );
    }
    assert_rejected("SELECT region FROM sales; -- note\nDROP TABLE sales");
    assert_rejected("SELECT region FROM sales /* note */; DROP TABLE sales");
    // Chú thích chen giữa một từ khóa cắt nó thành hai từ lạ, không ghép lại thành SELECT.
    assert_rejected("SEL/**/ECT region FROM sales");
}

#[test]
fn rejects_write_statements() {
    for sql in [
        "INSERT INTO sales VALUES (1)",
        "UPDATE sales SET amount = 0",
        "DELETE FROM sales",
        "DROP TABLE sales",
        "CREATE TABLE copy AS SELECT * FROM sales",
    ] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_attach() {
    assert_rejected("ATTACH 'other.db' AS other");
}

#[test]
fn rejects_copy() {
    assert_rejected("COPY sales TO 'out.csv'");
    assert_rejected("COPY (SELECT * FROM sales) TO 'out.csv'");
}

#[test]
fn rejects_pragma() {
    assert_rejected("PRAGMA database_list");
}

#[test]
fn rejects_install_and_load_extension() {
    assert_rejected("INSTALL httpfs");
    assert_rejected("LOAD httpfs");
}

#[test]
fn rejects_statements_that_describe_the_database() {
    for sql in [
        "DESCRIBE sales",
        "SHOW TABLES",
        "SUMMARIZE sales",
        "EXPLAIN SELECT region FROM sales",
    ] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_statements_that_change_settings_or_databases() {
    for sql in [
        "SET threads = 1",
        "RESET threads",
        "CALL duckdb_settings()",
        "USE other",
        "DETACH other",
        "CREATE SECRET (TYPE s3, KEY_ID 'key', SECRET 'secret')",
        "EXPORT DATABASE 'out'",
        "CHECKPOINT",
    ] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_write_hidden_inside_select() {
    assert_rejected("WITH t AS (DELETE FROM sales RETURNING *) SELECT * FROM t");
    assert_rejected("SELECT * INTO copy FROM sales");
}

#[test]
fn rejects_table_shorthand() {
    assert_rejected("TABLE sales");
    assert_rejected("SELECT region FROM sales UNION TABLE customers");
}

// Bảng

#[test]
fn rejects_read_csv() {
    let reason = assert_rejected("SELECT * FROM read_csv('/etc/passwd')");
    assert!(reason.contains("read_csv"), "{reason}");
}

#[test]
fn rejects_glob() {
    assert_rejected("SELECT * FROM glob('/data/*')");
}

/// Mọi hàm đặt ở chỗ bảng đều bị chặn, kể cả read_parquet: LLM tự viết đường dẫn thì đọc được file của dataset hay
/// workspace khác. Chỉ guard được đổi bảng logic sang file.
#[test]
fn rejects_any_function_used_as_a_table() {
    for sql in [
        "SELECT * FROM read_parquet('/data/sales/*.parquet')",
        "SELECT * FROM parquet_scan('x.parquet')",
        "SELECT * FROM read_json_auto('x.json')",
        "SELECT * FROM generate_series(1, 10)",
        "SELECT * FROM range(10)",
        "SELECT region FROM sales JOIN read_csv('x.csv') r ON true",
    ] {
        assert_rejected(sql);
    }
}

#[test]
fn rejects_system_catalogs() {
    for sql in [
        "SELECT * FROM duckdb_tables()",
        "SELECT * FROM duckdb_settings()",
        "SELECT * FROM duckdb_tables",
        "SELECT * FROM pg_catalog.pg_tables",
        "SELECT * FROM sqlite_master",
    ] {
        assert_rejected(sql);
    }
}

/// UNION ghép kết quả từ hai nơi; vế sau cũng phải là bảng được phép.
#[test]
fn rejects_union_with_a_source_outside_the_list() {
    for sql in [
        "SELECT region FROM sales UNION ALL SELECT * FROM orders",
        "SELECT region FROM sales UNION ALL SELECT * FROM read_csv('x.csv')",
        "SELECT region FROM sales UNION ALL SELECT table_name FROM information_schema.tables",
        "SELECT region FROM sales UNION ALL SELECT * FROM 'secret.csv'",
    ] {
        assert_rejected(sql);
    }
}

/// Chuỗi UNION 20.000 vế từng làm tràn stack khi duyệt cây và hủy cả tiến trình. Giờ bị từ chối trước khi đọc.
#[test]
fn rejects_sql_over_the_size_limit() {
    let sql = vec!["SELECT region FROM sales"; 20_000].join(" UNION ALL ");
    let reason = assert_rejected(&sql);
    assert!(reason.contains("quá dài"), "{reason}");
}

/// Câu dài sát giới hạn, toàn vế UNION ngắn nhất guard nhận, vẫn qua được mà không tràn stack của luồng test (2 MB,
/// bằng stack mặc định của luồng tokio).
#[test]
fn sql_just_under_the_size_limit_does_not_overflow() {
    let part = "SELECT region FROM sales";
    let count = super::MAX_SQL_BYTES / (part.len() + " UNION ALL ".len());
    let sql = vec![part; count].join(" UNION ALL ");
    assert!(sql.len() <= super::MAX_SQL_BYTES);
    assert_accepted(&sql);
}

/// Chữ Kirin "а" (U+0430) và "ѕ" (U+0455) trông như chữ Latin. Tên phải khớp đúng từng ký tự, nên bảng giả bị từ chối.
#[test]
fn rejects_table_name_with_lookalike_unicode_letters() {
    for sql in [
        "SELECT region FROM s\u{0430}les",
        "SELECT region FROM \u{0455}ales",
        "SELECT region FROM \"s\u{0430}les\"",
    ] {
        let reason = assert_rejected(sql);
        assert!(reason.contains("sales"), "{reason}");
    }
}

/// Bí danh trùng tên bảng khác chỉ đổi tên gọi, không đổi nơi đọc: `customers AS sales` vẫn đọc file của customers, và
/// cột riêng của sales không dùng được qua bí danh đó.
#[test]
fn alias_named_like_another_table_still_reads_its_own_table() {
    let guarded = assert_accepted("SELECT sales.segment FROM customers AS sales");
    let customers = expected_read_parquet("customers");
    assert_eq!(
        guarded.runnable,
        format!("SELECT sales.segment FROM {customers} AS sales LIMIT 100")
    );
    assert_rejected("SELECT sales.amount FROM customers AS sales");
}

#[test]
fn rejects_information_schema() {
    assert_rejected("SELECT * FROM information_schema.tables");
    assert_rejected("SELECT column_name FROM information_schema.columns");
}

#[test]
fn rejects_file_path_written_as_table_name() {
    assert_rejected("SELECT * FROM 'secret.csv'");
    assert_rejected("SELECT * FROM \"secret.csv\"");
}

#[test]
fn rejects_unknown_table_and_lists_allowed_ones() {
    let reason = assert_unknown_name("SELECT * FROM orders");
    assert!(
        reason.contains("sales") && reason.contains("customers"),
        "{reason}"
    );
    // Cột của bảng sai tên cũng là tên lạ, nhưng lý do phải chỉ vào bảng, chỗ LLM cần sửa.
    let reason = assert_unknown_name("SELECT region FROM orders");
    assert!(reason.contains("Không có bảng orders"), "{reason}");
}

#[test]
fn rejects_table_with_schema() {
    assert_rejected("SELECT * FROM main.sales");
}

#[test]
fn rejects_read_function_hidden_in_subquery() {
    assert_rejected("SELECT region FROM sales WHERE region IN (SELECT * FROM read_csv('x.csv'))");
    assert_rejected("WITH t AS (SELECT * FROM glob('*')) SELECT * FROM t");
}

#[test]
fn rejects_recursive_with() {
    assert_rejected(
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM t) SELECT n FROM t",
    );
}

#[test]
fn rejects_with_name_that_shadows_a_table() {
    assert_rejected("WITH sales AS (SELECT 1 AS region) SELECT region FROM sales");
}

// Hàm

#[test]
fn rejects_functions_outside_the_allowed_list() {
    for sql in [
        "SELECT current_setting('home_directory')",
        "SELECT getenv('HOME')",
        "SELECT region FROM sales WHERE region = read_text('x')",
        "SELECT main.sum(amount) FROM sales",
        "SELECT \"getenv\"('HOME')",
        "SELECT COLUMNS('.*') FROM sales",
    ] {
        assert_rejected(sql);
    }
}

/// Hàm lạ bị bắt ở mọi chỗ có biểu thức, không chỉ trong danh sách cột của SELECT.
#[test]
fn rejects_functions_hidden_anywhere_in_the_query() {
    for sql in [
        "SELECT region FROM sales ORDER BY getenv('HOME')",
        "SELECT region FROM sales GROUP BY region HAVING SUM(amount) > length(read_text('x'))",
        "SELECT CASE WHEN amount > 0 THEN getenv('HOME') END FROM sales",
        "SELECT SUM(amount) FILTER (WHERE getenv('HOME') = '') FROM sales",
        "SELECT SUM(amount) OVER (PARTITION BY getenv('HOME')) FROM sales",
        "SELECT c.segment FROM sales s JOIN customers c ON current_setting('x') = c.segment",
    ] {
        assert_rejected(sql);
    }
}

// Cột

#[test]
fn rejects_unknown_column() {
    let reason = assert_unknown_name("SELECT profit FROM sales");
    assert!(reason.contains("profit"), "{reason}");
    assert_unknown_name("SELECT s.profit FROM sales s");
}

#[test]
fn rejects_column_name_with_lookalike_unicode_letters() {
    // "і" ở đây là chữ Ukraina U+0456, không phải chữ i Latin.
    let reason = assert_rejected("SELECT reg\u{0456}on FROM sales");
    assert!(reason.contains("reg\u{0456}on"), "{reason}");
}

#[test]
fn rejects_column_of_unknown_table_alias() {
    assert_unknown_name("SELECT x.region FROM sales s");
}

/// Chỉ sai tên bảng hay cột được gửi lại cho mô hình sửa; câu ghi, hàm bị cấm, nhiều câu thì dừng ngay.
#[test]
fn only_unknown_names_are_fixable() {
    for sql in [
        "DELETE FROM sales",
        "SELECT getenv('HOME')",
        "SELECT 1; SELECT 2",
        "SELEC region FROM sales",
        "SELECT * FROM read_csv('x.csv')",
        "WITH RECURSIVE t AS (SELECT 1) SELECT * FROM t",
        "SELECT region FROM sales FETCH FIRST 5 ROWS ONLY",
    ] {
        assert_rejected_for_other_reason(sql);
    }
}

#[test]
fn rejects_column_with_too_many_parts() {
    assert_rejected("SELECT main.sales.region FROM sales");
}

// Giới hạn dòng

#[test]
fn adds_limit_when_missing() {
    let guarded = assert_accepted("SELECT region FROM sales");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
}

#[test]
fn lowers_limit_above_max_rows() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 5000");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
}

#[test]
fn keeps_smaller_limit_and_offset() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 10 OFFSET 5");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 10 OFFSET 5");
}

#[test]
fn replaces_limit_that_is_not_a_plain_number() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT (SELECT 5000)");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
}

#[test]
fn limits_the_whole_union() {
    let guarded = assert_accepted("SELECT region FROM sales UNION SELECT segment FROM customers");
    assert_eq!(
        guarded.shown,
        "SELECT region FROM sales UNION SELECT segment FROM customers LIMIT 100"
    );
}

#[test]
fn rejects_fetch_first() {
    assert_rejected("SELECT region FROM sales FETCH FIRST 5 ROWS ONLY");
}

// Bản cho DuckDB chạy

#[test]
fn runnable_reads_parquet_and_keeps_table_name_as_alias() {
    let guarded = assert_accepted("SELECT sales.region FROM sales");
    let sales = expected_read_parquet("sales");
    assert_eq!(
        guarded.runnable,
        format!("SELECT sales.region FROM {sales} AS sales LIMIT 100")
    );
    assert!(!guarded.shown.contains("/data"), "{}", guarded.shown);
}

#[test]
fn runnable_keeps_existing_alias() {
    let guarded = assert_accepted("SELECT s.region FROM sales AS s");
    let sales = expected_read_parquet("sales");
    assert_eq!(
        guarded.runnable,
        format!("SELECT s.region FROM {sales} AS s LIMIT 100")
    );
}

#[test]
fn runnable_renames_with_tables() {
    let guarded = assert_accepted("WITH t AS (SELECT region FROM sales) SELECT t.region FROM t");
    let sales = expected_read_parquet("sales");
    assert_eq!(
        guarded.runnable,
        format!(
            "WITH guard_cte_1 AS (SELECT region FROM {sales} AS sales) \
             SELECT t.region FROM guard_cte_1 AS t LIMIT 100"
        )
    );
}

/// Câu gọi bảng tạm ở ngoài chỗ nó được định nghĩa. Nếu tên giữ nguyên, DuckDB sẽ tìm file "secret.csv"; sau khi
/// đổi tên, chỗ gọi đó trỏ tới guard_cte_1 vốn không tồn tại ở ngoài, nên DuckDB chỉ báo lỗi.
#[test]
fn runnable_does_not_leave_with_names_that_look_like_files() {
    let guarded = assert_accepted(
        "SELECT * FROM (WITH \"secret.csv\" AS (SELECT region FROM sales) SELECT region FROM \"secret.csv\") x, \
         \"secret.csv\"",
    );
    assert!(
        !guarded.runnable.contains("FROM \"secret.csv\""),
        "{}",
        guarded.runnable
    );
    assert!(
        !guarded.runnable.contains(", \"secret.csv\""),
        "{}",
        guarded.runnable
    );
}

#[test]
fn runnable_runs_on_duckdb() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-{}", uuid::Uuid::new_v4()));
    let conn = duckdb::Connection::open_in_memory().unwrap();
    for (name, rows) in [
        (
            "sales",
            "SELECT * FROM (VALUES ('South', 10, 1), ('South', 5, 2), ('North', 7, 1)) \
             v(region, amount, customer_id)",
        ),
        (
            "customers",
            "SELECT * FROM (VALUES (1, 'Retail'), (2, 'Business')) v(customer_id, segment)",
        ),
    ] {
        let table_dir = dir.join(name);
        std::fs::create_dir_all(&table_dir).unwrap();
        let file = table_dir.join("part-0.parquet");
        conn.execute_batch(&format!(
            "COPY ({rows}) TO '{}' (FORMAT parquet)",
            file.display()
        ))
        .unwrap();
    }

    let sql = "WITH t AS (SELECT s.region, c.segment, s.amount FROM sales s \
               JOIN customers c ON s.customer_id = c.customer_id) \
               SELECT segment, SUM(amount) AS total FROM t GROUP BY segment ORDER BY total DESC";
    let guarded = build_guarded_sql(sql, &tables_in(dir.to_str().unwrap()), MAX_ROWS).unwrap();
    let mut statement = conn.prepare(&guarded.runnable).unwrap();
    let rows: Vec<(String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(
        rows,
        vec![("Retail".to_string(), 17), ("Business".to_string(), 5)]
    );
}
