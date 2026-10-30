// SPDX-License-Identifier: Apache-2.0
/* engine/guard/tests.rs - test guard SQL: câu nào qua, câu nào bị chặn, câu được sửa thành gì. Hai test cuối chạy
trên DuckDB thật vì so chuỗi không chứng minh được DuckDB chạy được bản đã đổi. Nhiều test so nguyên chuỗi sqlparser in
ra, nên nâng phiên bản sqlparser có thể phải sửa chuỗi mong đợi. */

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sqlparser::ast::Statement;
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;

use super::{build_guarded_sql, AllowedTable, GuardError, GuardedSql, RowFilter};
use crate::engine::analytics::parquet_glob;
use crate::error::AppError;

const MAX_ROWS: u64 = 100;

fn table(name: &str, columns: &[&str], parquet_dir: PathBuf) -> AllowedTable {
    AllowedTable {
        name: name.to_string(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        parquet_dir,
        row_filters: vec![],
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

/// Như assert_rejected, nhưng phải là loại sai tên bảng hay cột (GuardError::UnknownName).
fn assert_unknown_name(sql: &str) -> String {
    match guard(sql) {
        Err(GuardError::UnknownName(reason)) => reason,
        other => panic!("guard should reject {sql:?} as UnknownName, got {other:?}"),
    }
}

/// Câu có thể dựng giá trị quá lớn: service nhờ mô hình viết lại gọn hơn.
fn assert_oversized(sql: &str) -> String {
    match guard(sql) {
        Err(GuardError::Oversized(reason)) => reason,
        other => panic!("guard should reject {sql:?} as Oversized, got {other:?}"),
    }
}

/// Loại từ chối không phải sai tên hay quá lớn (GuardError::Rejected).
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

/// FOR UPDATE, FOR SHARE xin khóa dòng để ghi sau; chặn ở câu ngoài cùng và mọi câu con, loại dừng ngay.
#[test]
fn rejects_row_locks() {
    for sql in [
        "SELECT region FROM sales FOR UPDATE",
        "SELECT region FROM sales FOR SHARE",
        "SELECT region FROM sales WHERE region IN (SELECT region FROM sales FOR UPDATE)",
        "SELECT r FROM (SELECT region AS r FROM sales FOR UPDATE) t",
        "WITH t AS (SELECT region FROM sales FOR UPDATE) SELECT region FROM t",
    ] {
        let reason = assert_rejected(sql);
        assert!(reason.contains("FOR UPDATE"), "{sql}: {reason}");
        assert_rejected_for_other_reason(sql);
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

/// Câu dài sát giới hạn, cây lồng sâu nhất có thể, không tràn stack của luồng test (2 MB, bằng stack mặc định của luồng
/// tokio). Chuỗi UNION giờ bị luật số nhánh (width.rs) từ chối, nhưng chỉ sau khi các lượt duyệt trước đã đi hết cây;
/// chuỗi OR thì được nhận, nên đi tiếp cả bước in lại câu.
#[test]
fn sql_just_under_the_size_limit_does_not_overflow() {
    let part = "SELECT region FROM sales";
    let count = super::MAX_SQL_BYTES / (part.len() + " UNION ALL ".len());
    let sql = vec![part; count].join(" UNION ALL ");
    assert!(sql.len() <= super::MAX_SQL_BYTES);
    assert_oversized(&sql);

    let head = "SELECT region FROM sales WHERE amount = 1";
    let count = (super::MAX_SQL_BYTES - head.len()) / " OR amount = 1".len();
    let sql = format!("{head}{}", " OR amount = 1".repeat(count));
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

/// Các lý do ngoài sai tên và quá lớn (câu ghi, hàm bị cấm, nhiều câu, câu sai cú pháp, hàm ở chỗ bảng, WITH RECURSIVE,
/// FETCH FIRST) ra loại Rejected.
#[test]
fn other_refusals_are_rejected() {
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

// Hàm dựng giá trị lớn từ tham số (guard/sizes.rs)

/// Câu đầu là câu được báo vượt cả giới hạn thời gian lẫn bộ nhớ: DuckDB tính lpad hằng lúc chuẩn bị câu (sizes.rs).
#[test]
fn rejects_padding_to_huge_lengths() {
    for sql in [
        "SELECT length(lpad('x', 1900000000, 'y'))",
        "SELECT length(RPAD('x', 1900000000, 'y'))",
        "SELECT lpad(region, 251, 'y') FROM sales",
        // Đếm ký tự: 250 ký tự 4 byte đã là 1000 byte.
        "SELECT lpad('😀', 1000, '😀')",
        "SELECT lpad(region, 1000 * 1000, 'y') FROM sales",
        "SELECT lpad(region, amount, 'y') FROM sales",
        "SELECT lpad(region, (SELECT 5), 'y') FROM sales",
        "SELECT region FROM sales WHERE lpad(region, 99999999, ' ') = 'x'",
    ] {
        let reason = assert_oversized(sql);
        assert!(reason.contains("250"), "{sql}: {reason}");
    }
    assert_accepted("SELECT lpad(region, 250, ' ') FROM sales");
}

/// strftime in mỗi mã hai byte thành nhiều byte: định dạng phải viết thẳng, tối đa 100 byte.
#[test]
fn strftime_needs_a_short_written_format() {
    let format = |bytes: usize| {
        format!(
            "SELECT strftime(order_date, '{}') FROM sales",
            "%c".repeat(bytes / 2)
        )
    };
    assert_accepted(&format(100));
    assert_accepted("SELECT strftime(order_date, '%Y-%m') AS month FROM sales");
    for sql in [
        format(102),
        "SELECT strftime(order_date, region) FROM sales".to_string(),
        "SELECT strftime('%Y', order_date) FROM sales".to_string(),
        "SELECT strftime(order_date, '%Y' || region) FROM sales".to_string(),
    ] {
        assert_oversized(&sql);
    }
}

#[test]
fn rejects_replacements_that_grow_the_text() {
    for sql in [
        "SELECT replace(region, 'a', 'aa') FROM sales",
        "SELECT replace(replace('y', 'y', 'yyyyyyyyyy'), 'y', 'yyyyyyyyyy')",
        "SELECT replace(region, 'a', region) FROM sales",
        "SELECT replace(region, region, 'x') FROM sales",
        // Cùng một ký tự nhưng 4 byte thay 1 byte: lồng nhau thì gấp 4 mỗi lần.
        "SELECT replace(replace(region, 'a', '😀'), 'a', '😀') FROM sales",
        // Mẫu khớp được chuỗi rỗng: khớp ở mọi vị trí giữa hai ký tự.
        "SELECT regexp_replace(region, '', 'x', 'g') FROM sales",
        "SELECT regexp_replace(region, 'x*', ' ', 'g') FROM sales",
        "SELECT regexp_replace(region, 'a?', '-', 'g') FROM sales",
        "SELECT regexp_replace(region, 'a|', '-', 'g') FROM sales",
        "SELECT regexp_replace(region, '^', '-', 'g') FROM sales",
        "SELECT regexp_replace(region, '\\b', '-', 'g') FROM sales",
        "SELECT regexp_replace(region, '()', '-', 'g') FROM sales",
        "SELECT regexp_replace(region, '\\Q\\E', '-', 'g') FROM sales",
        // Chuỗi thay dài hơn một byte, kể cả nhắc lại nhóm.
        "SELECT regexp_replace(region, '(a)', '\\1\\1', 'g') FROM sales",
        "SELECT regexp_replace(region, '\\s+', '  ', 'g') FROM sales",
        "SELECT regexp_replace(region, '\\s+', '😀', 'g') FROM sales",
        "SELECT regexp_replace(region, region, ' ', 'g') FROM sales",
    ] {
        assert_oversized(sql);
    }
}

#[test]
fn accepts_size_functions_used_for_cleaning() {
    for sql in [
        "SELECT lpad(CAST(customer_id AS VARCHAR), 8, '0') FROM sales",
        "SELECT rpad(region, 250, ' ') FROM sales",
        "SELECT replace(region, ',', '') FROM sales",
        "SELECT replace(region, 'South', 'Nam') FROM sales",
        "SELECT regexp_replace(region, '[0-9]+', '', 'g') FROM sales",
        "SELECT regexp_replace(region, '[0-9]*', '', 'g') FROM sales",
        // Gộp khoảng trắng: mỗi lần khớp chiếm ít nhất một byte, thay bằng một byte.
        "SELECT regexp_replace(region, '\\s+', ' ', 'g') FROM sales",
        "SELECT regexp_replace(regexp_replace(region, '[^a-z]', '-', 'g'), '-+', '-', 'g') FROM sales",
    ] {
        assert_accepted(sql);
    }
}

// Số tầng câu lồng nhau (guard/depth.rs)

/// Câu có `levels` tầng: mỗi tầng là một câu con ở FROM của tầng ngoài.
fn nested(levels: usize) -> String {
    let mut sql = "SELECT region AS v FROM sales".to_string();
    for i in 1..levels {
        sql = format!("SELECT v FROM ({sql}) t{i}");
    }
    sql
}

#[test]
fn accepts_up_to_four_levels() {
    assert_accepted(&nested(4));
    for sql in [
        "SELECT region FROM sales",
        "SELECT r, total FROM (SELECT region AS r, SUM(amount) AS total FROM sales GROUP BY region) t",
        "SELECT region, (SELECT COUNT(*) FROM customers) AS n FROM sales",
        "SELECT region FROM sales UNION ALL SELECT segment FROM customers",
        // Bảng tạm kèm câu con, dạng mô hình hay viết: 3 tầng.
        "WITH monthly AS (SELECT date_trunc('month', order_date) AS m, SUM(amount) AS revenue FROM sales \
         WHERE customer_id IN (SELECT customer_id FROM customers WHERE segment = 'Retail') GROUP BY m) \
         SELECT m, revenue, revenue - LAG(revenue) OVER (ORDER BY m) AS change FROM monthly ORDER BY m",
        // Hai bảng tạm nối nhau, rồi câu con trên bảng tạm sau: 4 tầng.
        "WITH a AS (SELECT region, amount FROM sales), b AS (SELECT region, SUM(amount) AS total FROM a GROUP BY region) \
         SELECT region, total FROM (SELECT region, total, rank() OVER (ORDER BY total DESC) AS r FROM b) x WHERE r <= 3",
    ] {
        assert_accepted(sql);
    }
}

/// Mọi kiểu lồng đều tính: câu con trong FROM, WHERE, SELECT, bảng tạm, và bảng tạm đọc từ bảng tạm khác dù không
/// viết lồng. Từ chối thuộc loại vào vòng sửa.
#[test]
fn rejects_five_levels() {
    let five = [
        nested(5),
        format!("SELECT region FROM sales WHERE region IN ({})", nested(4)),
        format!("SELECT ({}) AS m", nested(4).replacen("SELECT v", "SELECT MAX(v)", 1)),
        format!("WITH t AS ({}) SELECT * FROM t", nested(4)),
        "WITH a AS (SELECT region AS v FROM sales), b AS (SELECT v FROM a), c AS (SELECT v FROM b), \
         d AS (SELECT v FROM c) SELECT v FROM d"
            .to_string(),
    ];
    for sql in five {
        let reason = assert_oversized(&sql);
        assert!(reason.contains("tối đa 4"), "{sql}: {reason}");
    }
}

/// Bảng tạm cùng tên trong một WITH lồng chỉ che bảng tạm ngoài trong câu đó: số tầng của bảng tạm ngoài giữ nguyên.
/// Câu dưới thật sự 5 tầng (c1, c2, c3, c4, câu chính); nếu bảng tạm c2 bên trong r2 ghi đè số tầng của c2 ngoài thì
/// câu đếm ra 4 và lọt.
#[test]
fn nested_with_does_not_hide_depth_of_outer_tables() {
    let sql = "WITH c1 AS (SELECT region AS v FROM sales), c2 AS (SELECT v FROM c1), \
               r2 AS (WITH c2 AS (SELECT region AS v FROM sales) SELECT v FROM c2), \
               c3 AS (SELECT v FROM c2), c4 AS (SELECT v FROM c3) SELECT v FROM c4";
    let reason = assert_oversized(sql);
    assert!(reason.contains("5 tầng"), "{reason}");
    // Bên trong r2, c2 là bảng tạm trong (1 tầng), nên r2 chỉ 2 tầng.
    assert_accepted(
        "WITH c1 AS (SELECT region AS v FROM sales), c2 AS (SELECT v FROM c1), c3 AS (SELECT v FROM c2), \
         r2 AS (WITH c2 AS (SELECT region AS v FROM sales) SELECT v FROM c2) SELECT v FROM r2",
    );
}

// Số mảnh ghép (guard/pieces.rs)

/// Gọi hàm dạng phương thức: biểu thức trước dấu chấm là tham số đầu mà luật đếm không thấy (`(a).concat(a)` nhân đôi
/// chuỗi mà chỉ đếm một mảnh). Từ chối loại vào vòng sửa.
#[test]
fn method_call_syntax_is_rejected() {
    for sql in [
        "SELECT (region).concat(region) FROM sales",
        "SELECT ('ab').concat('cd', 'ef') AS x",
        "SELECT (region).concat_ws(region, region) FROM sales",
        "SELECT region.lower() FROM sales",
        "SELECT region.lpad(3, '0') FROM sales",
        "SELECT main.sum(amount) FROM sales",
    ] {
        let reason = assert_oversized(sql);
        assert!(reason.contains("dạng phương thức"), "{sql}: {reason}");
    }
    // Truy cập trường, phần tử không phải gọi hàm.
    assert_accepted("SELECT lower(region) FROM sales");
}

/// Bí danh dùng lại trong cùng danh sách cột chép cả giá trị: mỗi chỗ dùng cộng số mảnh của bí danh.
#[test]
fn reused_column_aliases_count_their_pieces() {
    let sql =
        "SELECT 'ab' AS a0, a0 || a0 || a0 AS a1, a1 || a1 || a1 AS a2, a2 || a2 || a2 AS a3, \
               a3 || a3 || a3 AS a4, length(a4) AS n";
    assert_oversized(sql);
    // Dùng lại bí danh không ghép, hay ghép ít, vẫn qua.
    assert_accepted("SELECT region AS r, lower(r) AS lr, r || '-' || lr AS label FROM sales");
}

/// Tuple và mảng ở vế phải của IN, ANY, ALL chỉ để so sánh, không ghép thành chuỗi.
#[test]
fn compared_tuples_and_arrays_are_not_pieces() {
    let pairs: Vec<String> = (0..15).map(|i| format!("('M{i}', 'R{i}')")).collect();
    assert_accepted(&format!(
        "SELECT region FROM sales WHERE (region, order_date) IN ({})",
        pairs.join(", ")
    ));
    let values: Vec<String> = (0..30).map(|i| format!("'C{i}'")).collect();
    assert_accepted(&format!(
        "SELECT region FROM sales WHERE region = ANY(ARRAY[{}])",
        values.join(", ")
    ));
    assert_accepted(&format!(
        "SELECT region FROM sales WHERE region <> ALL(ARRAY[{}])",
        values.join(", ")
    ));
    // Mảng không đứng ở chỗ so sánh vẫn đếm.
    assert_oversized(&format!(
        "SELECT CAST([{}] AS VARCHAR) FROM sales",
        values.join(", ")
    ));
}

#[test]
fn concat_pieces_are_limited_to_twenty() {
    let pipes = |n: usize| format!("SELECT {} AS v FROM sales", vec!["region"; n].join(" || "));
    let concat = |n: usize| {
        format!(
            "SELECT concat({}) AS v FROM sales",
            vec!["region"; n].join(", ")
        )
    };
    assert_accepted(&pipes(20));
    assert_accepted(&concat(20));
    assert_oversized(&pipes(21));
    assert_oversized(&concat(21));
    // Đếm trên cả câu, cộng qua các cột và các tầng.
    assert_accepted(&format!(
        "SELECT {} || v FROM ({}) t",
        ["v"; 9].join(" || "),
        pipes(10)
    ));
    assert_oversized(&format!(
        "SELECT {} || v FROM ({}) t",
        ["v"; 10].join(" || "),
        pipes(10)
    ));
    // concat_ws: dấu ngăn viết thẳng không tính; dấu ngăn là biểu thức thì được ghép vào giữa mỗi cặp.
    let ws = |sep: &str, n: usize| {
        format!(
            "SELECT concat_ws({sep}, {}) FROM sales",
            vec!["region"; n].join(", ")
        )
    };
    assert_accepted(&ws("', '", 20));
    assert_oversized(&ws("', '", 21));
    assert_accepted(&ws("region", 10));
    assert_oversized(&ws("region", 11));
    // Phần tử danh sách và struct viết thẳng cũng là mảnh: đổi sang VARCHAR là ghép.
    assert_oversized(&format!(
        "SELECT CAST([{}] AS VARCHAR) FROM sales",
        vec!["region"; 21].join(", ")
    ));
    assert_oversized(&format!(
        "SELECT CAST({{{}}} AS VARCHAR) FROM sales",
        (0..21)
            .map(|i| format!("'k{i}': region"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

// Độ dài chuỗi hằng, số cột, số nguồn (guard/width.rs)

/// `*` trên nhiều nguồn nối cột của mọi nguồn: tính theo số cột thật, từ chối khi có bảng tạm, câu con.
#[test]
fn star_over_several_sources_counts_real_columns() {
    // Bảng dữ liệu: biết số cột từ hồ sơ (sales 5 cột, customers 2 cột).
    assert_accepted("SELECT * FROM sales s JOIN customers c ON s.customer_id = c.customer_id");
    assert_accepted(
        "SELECT s.*, c.segment FROM sales s JOIN customers c ON s.customer_id = c.customer_id",
    );
    // Một nguồn: `*` là một mục, kể cả bảng tạm.
    assert_accepted("WITH w AS (SELECT region, amount FROM sales) SELECT * FROM w");
    // JOIN bảng tạm với chính nó: số cột nhân theo số lần JOIN, mà bảng tạm không biết số cột.
    let cols: Vec<String> = (1..=200).map(|i| format!("v AS c{i}")).collect();
    let star_join = |n: usize| {
        let sources: Vec<String> = (1..=n).map(|i| format!("w a{i}")).collect();
        format!(
            "WITH w AS (SELECT {} FROM (SELECT region AS v FROM sales) t) SELECT * FROM {}",
            cols.join(", "),
            sources.join(", ")
        )
    };
    for n in [2, 4] {
        let reason = assert_oversized(&star_join(n));
        assert!(reason.contains("liệt kê các cột"), "{reason}");
    }
    assert_oversized(&star_join(100));
    assert_oversized("WITH w AS (SELECT region FROM sales) SELECT a.* FROM w a, w b");
}

/// Mỗi nhánh UNION ALL cho một dòng chép lại giá trị: tối đa 10 nhánh trong cả câu, cộng qua mọi tầng.
#[test]
fn set_operations_have_at_most_ten_branches() {
    let union = |n: usize| vec!["SELECT region FROM sales"; n].join(" UNION ALL ");
    assert_accepted(&union(10));
    let reason = assert_oversized(&union(11));
    assert!(
        reason.contains("11 nhánh") && reason.contains("tối đa 10"),
        "{reason}"
    );
    // INTERSECT, EXCEPT, nhánh trong ngoặc cũng tính.
    assert_oversized(&format!("({}) EXCEPT ({})", union(6), union(5)));
    let intersect = ["SELECT region FROM sales"; 11].join(" INTERSECT ");
    assert_oversized(&intersect);
    // Cộng qua các tầng: 6 nhánh trong bảng tạm và 5 nhánh ở câu chính.
    assert_oversized(&format!(
        "WITH u AS ({}) SELECT region FROM u UNION ALL {}",
        union(6),
        union(4)
    ));
    assert_accepted(&format!(
        "WITH u AS ({}) SELECT region FROM u UNION ALL {}",
        union(5),
        union(4)
    ));
}

#[test]
fn from_has_at_most_six_sources() {
    let from = |n: usize| {
        (1..=n)
            .map(|i| format!("sales s{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    assert_accepted(&format!("SELECT s1.region FROM {}", from(6)));
    let reason = assert_oversized(&format!("SELECT s1.region FROM {}", from(7)));
    assert!(reason.contains("tối đa 6"), "{reason}");
    // JOIN, JOIN trong ngoặc cũng tính.
    assert_oversized(
        "SELECT a.region FROM sales a JOIN sales b ON a.region = b.region JOIN (sales c JOIN sales d ON c.region = d.region) \
         ON a.region = c.region JOIN customers e ON a.customer_id = e.customer_id JOIN customers f \
         ON a.customer_id = f.customer_id JOIN customers g ON a.customer_id = g.customer_id",
    );
}

/// Guard báo đúng các bảng câu đọc, mỗi bảng một lần, kể cả bảng chỉ đọc trong bảng tạm; không tính bảng tạm.
#[test]
fn tables_read_lists_each_data_table_once() {
    let guarded = assert_accepted(
        "WITH c AS (SELECT * FROM customers) SELECT s.region FROM sales s JOIN c ON s.customer_id = c.customer_id \
         JOIN sales t ON t.region = s.region",
    );
    assert_eq!(guarded.tables_read, ["customers", "sales"]);
    let guarded = assert_accepted("SELECT region FROM sales");
    assert_eq!(guarded.tables_read, ["sales"]);
}

#[test]
fn string_literals_are_limited_to_1000_bytes() {
    let literal = |bytes: usize| {
        format!(
            "SELECT region FROM sales WHERE region = '{}'",
            "y".repeat(bytes)
        )
    };
    assert_accepted(&literal(1000));
    let reason = assert_oversized(&literal(1001));
    assert!(reason.contains("1001"), "{reason}");
    // Tính bằng byte: 334 chữ "ộ" (3 byte mỗi chữ) là 1002 byte.
    assert_oversized(&format!("SELECT '{}' AS v", "ộ".repeat(334)));
    // Mọi chỗ có chuỗi: IN (...), DATE '...', đối số hàm.
    assert_oversized(&format!(
        "SELECT region FROM sales WHERE region IN ('a', '{}')",
        "y".repeat(1001)
    ));
    assert_oversized(&format!("SELECT lower('{}') AS v", "y".repeat(1001)));
}

#[test]
fn select_lists_are_limited_to_200_items() {
    let columns = |n: usize| format!("SELECT {} FROM sales", vec!["region"; n].join(", "));
    assert_accepted(&columns(200));
    let reason = assert_oversized(&columns(201));
    assert!(reason.contains("201"), "{reason}");
    // Mỗi SELECT tính riêng, kể cả câu con; * là một mục.
    assert_oversized(&format!("SELECT * FROM ({}) t", columns(201)));
    assert_accepted(&format!(
        "SELECT *, {} FROM sales",
        vec!["region"; 199].join(", ")
    ));
}

// Lọc dòng theo vai dữ liệu

/// tables_in(dir) kèm chính sách cho sales; customers không có chính sách nên thấy hết.
fn tables_with_sales_policy(dir: &str, row_filters: Vec<RowFilter>) -> Vec<AllowedTable> {
    let mut tables = tables_in(dir);
    for table in &mut tables {
        if table.name == "sales" {
            table.row_filters = row_filters;
            break;
        }
    }
    tables
}

fn row_filter(column: &str, allowed_values: &[&str]) -> RowFilter {
    RowFilter {
        column: column.to_string(),
        allowed_values: allowed_values.iter().map(|v| v.to_string()).collect(),
    }
}

/// Thành viên có data_role chỉ được thấy miền South.
fn guard_as_south_member(sql: &str) -> Result<GuardedSql, GuardError> {
    let tables = tables_with_sales_policy("/data", vec![row_filter("region", &["South"])]);
    build_guarded_sql(sql, &tables, MAX_ROWS)
}

fn assert_accepted_for_south_member(sql: &str) -> GuardedSql {
    match guard_as_south_member(sql) {
        Ok(guarded) => guarded,
        Err(e) => panic!("guard rejected {sql:?}: {e:?}"),
    }
}

/// Nhận cả hai loại từ chối như assert_rejected.
fn assert_rejected_for_south_member(sql: &str) {
    match guard_as_south_member(sql) {
        Err(GuardError::Rejected(_) | GuardError::UnknownName(_)) => {}
        other => panic!("guard should reject {sql:?}, got {other:?}"),
    }
}

/// Câu con guard bọc quanh mỗi chỗ đọc sales, trong bản `shown`.
const SOUTH_SALES: &str = "(SELECT * FROM sales WHERE \"region\" IN ('South'))";

#[test]
fn filters_a_plain_read() {
    let guarded =
        assert_accepted_for_south_member("SELECT region, SUM(amount) FROM sales GROUP BY region");
    assert_eq!(
        guarded.shown,
        format!("SELECT region, SUM(amount) FROM {SOUTH_SALES} AS sales GROUP BY region LIMIT 100")
    );
}

#[test]
fn filter_keeps_the_alias_written_in_the_query() {
    let guarded = assert_accepted_for_south_member("SELECT s.region FROM sales s");
    assert_eq!(
        guarded.shown,
        format!("SELECT s.region FROM {SOUTH_SALES} s LIMIT 100")
    );
    let guarded = assert_accepted_for_south_member("SELECT s.region FROM sales AS s");
    assert_eq!(
        guarded.shown,
        format!("SELECT s.region FROM {SOUTH_SALES} AS s LIMIT 100")
    );
}

#[test]
fn filters_only_tables_with_a_policy_in_a_join() {
    let guarded = assert_accepted_for_south_member(
        "SELECT c.segment, SUM(s.amount) FROM sales s JOIN customers c ON s.customer_id = c.customer_id \
         GROUP BY c.segment",
    );
    assert_eq!(
        guarded.shown,
        format!(
            "SELECT c.segment, SUM(s.amount) FROM {SOUTH_SALES} s JOIN customers c ON s.customer_id = c.customer_id \
             GROUP BY c.segment LIMIT 100"
        )
    );
}

#[test]
fn filters_reads_inside_subqueries() {
    let guarded = assert_accepted_for_south_member(
        "SELECT region FROM (SELECT region, amount FROM sales) t WHERE amount > (SELECT AVG(amount) FROM sales)",
    );
    assert_eq!(
        guarded.shown,
        format!(
            "SELECT region FROM (SELECT region, amount FROM {SOUTH_SALES} AS sales) t \
             WHERE amount > (SELECT AVG(amount) FROM {SOUTH_SALES} AS sales) LIMIT 100"
        )
    );
}

#[test]
fn filters_reads_inside_exists_and_in() {
    let guarded = assert_accepted_for_south_member(
        "SELECT segment FROM customers c \
         WHERE EXISTS (SELECT 1 FROM sales s WHERE s.customer_id = c.customer_id) \
         OR c.customer_id IN (SELECT customer_id FROM sales)",
    );
    assert_eq!(
        guarded.shown.matches(SOUTH_SALES).count(),
        2,
        "{}",
        guarded.shown
    );
}

/// Bảng tạm t không bị bọc vì nó không phải bảng dữ liệu; sales bên trong định nghĩa của t thì có.
#[test]
fn filters_reads_inside_with() {
    let guarded = assert_accepted_for_south_member(
        "WITH t AS (SELECT region FROM sales) SELECT region FROM t",
    );
    assert_eq!(
        guarded.shown,
        format!(
            "WITH t AS (SELECT region FROM {SOUTH_SALES} AS sales) SELECT region FROM t LIMIT 100"
        )
    );
}

#[test]
fn filters_every_read_of_union_and_self_join() {
    for sql in [
        "SELECT region FROM sales UNION ALL SELECT region FROM sales WHERE amount > 0",
        "SELECT a.region FROM sales a JOIN sales b ON a.customer_id = b.customer_id",
    ] {
        let guarded = assert_accepted_for_south_member(sql);
        assert_eq!(
            guarded.shown.matches(SOUTH_SALES).count(),
            2,
            "{}",
            guarded.shown
        );
    }
}

/// Tên bảng so không phân biệt hoa thường như DuckDB, nên viết "SALES" hay Sales không lách được lọc.
#[test]
fn filters_table_name_written_in_any_case() {
    let guarded = assert_accepted_for_south_member("SELECT region FROM \"SALES\"");
    assert_eq!(
        guarded.shown,
        "SELECT region FROM (SELECT * FROM \"SALES\" WHERE \"region\" IN ('South')) AS \"SALES\" LIMIT 100"
    );
    let guarded = assert_accepted_for_south_member("SELECT region FROM Sales");
    assert!(guarded.shown.contains("IN ('South')"), "{}", guarded.shown);
}

/// Lọc theo bảng được đọc, không theo bí danh: `sales AS customers` vẫn bị lọc, `customers AS sales` thì không.
#[test]
fn filter_follows_the_table_not_its_alias() {
    let guarded =
        assert_accepted_for_south_member("SELECT customers.region FROM sales AS customers");
    assert_eq!(
        guarded.shown,
        format!("SELECT customers.region FROM {SOUTH_SALES} AS customers LIMIT 100")
    );
    let guarded = assert_accepted_for_south_member("SELECT sales.segment FROM customers AS sales");
    assert_eq!(
        guarded.shown,
        "SELECT sales.segment FROM customers AS sales LIMIT 100"
    );
}

#[test]
fn joins_several_policies_of_one_table_with_and() {
    let tables = tables_with_sales_policy(
        "/data",
        vec![
            row_filter("region", &["South", "East"]),
            row_filter("customer_id", &["1"]),
        ],
    );
    let guarded = build_guarded_sql("SELECT region FROM sales", &tables, MAX_ROWS).unwrap();
    assert_eq!(
        guarded.shown,
        "SELECT region FROM (SELECT * FROM sales WHERE \"region\" IN ('South', 'East') AND \"customer_id\" IN ('1')) \
         AS sales LIMIT 100"
    );
}

#[test]
fn empty_allowed_list_shows_no_rows() {
    let tables = tables_with_sales_policy("/data", vec![row_filter("region", &[])]);
    let guarded = build_guarded_sql("SELECT region FROM sales", &tables, MAX_ROWS).unwrap();
    assert_eq!(
        guarded.shown,
        "SELECT region FROM (SELECT * FROM sales WHERE false) AS sales LIMIT 100"
    );
}

/// Giá trị đi vào cây cú pháp, không ghép chuỗi: dấu nháy được thoát, nên "South') OR ('1' = '1" chỉ là một chuỗi để
/// so, không thành điều kiện mới.
#[test]
fn quotes_in_policy_values_stay_inside_the_string() {
    let tables = tables_with_sales_policy(
        "/data",
        vec![row_filter(
            "Order Item Total",
            &["O'Brien", "South') OR ('1' = '1"],
        )],
    );
    let guarded = build_guarded_sql("SELECT region FROM sales", &tables, MAX_ROWS).unwrap();
    assert_eq!(
        guarded.shown,
        "SELECT region FROM (SELECT * FROM sales \
         WHERE \"Order Item Total\" IN ('O''Brien', 'South'') OR (''1'' = ''1')) AS sales LIMIT 100"
    );
}

#[test]
fn runnable_filters_the_parquet_read() {
    let guarded = assert_accepted_for_south_member("SELECT region FROM sales");
    let sales = expected_read_parquet("sales");
    assert_eq!(
        guarded.runnable,
        format!(
            "SELECT region FROM (SELECT * FROM {sales} AS sales WHERE \"region\" IN ('South')) AS sales LIMIT 100"
        )
    );
}

/// Mọi cách đọc dữ liệu không qua tên bảng đều bị bước 2 từ chối, nên không có đường tới dòng ngoài phạm vi mà không
/// đi qua chỗ bọc.
#[test]
fn member_cannot_read_around_the_filter() {
    for sql in [
        "SELECT * FROM read_parquet('/data/sales/*.parquet')",
        "SELECT * FROM parquet_scan('/data/sales/part-0.parquet')",
        "SELECT * FROM '/data/sales/part-0.parquet'",
        "SELECT * FROM \"/data/sales/part-0.parquet\"",
        "SELECT * FROM glob('/data/sales/*')",
        "SELECT * FROM main.sales",
        "SELECT * FROM s\u{0430}les",
        "SELECT region FROM sales UNION ALL SELECT region FROM read_csv('/data/sales/part-0.csv')",
        "SELECT * FROM duckdb_tables()",
        "SELECT * FROM information_schema.columns",
        "TABLE sales",
        "COPY sales TO 'all.csv'",
        "ATTACH '/data/workspace.duckdb' AS w",
        "SELECT region FROM sales; SELECT * FROM read_parquet('/data/sales/*.parquet')",
    ] {
        assert_rejected_for_south_member(sql);
    }
}

/// Bảng tạm trùng tên bảng dữ liệu bị bước 2 từ chối, nên không đặt được bảng tạm tên sales để `FROM sales` trỏ vào
/// thứ khác ngoài bảng đã bọc.
#[test]
fn with_name_like_a_table_cannot_dodge_the_filter() {
    for sql in [
        "WITH sales AS (SELECT * FROM customers) SELECT * FROM sales",
        "WITH \"SALES\" AS (SELECT 1 AS region) SELECT region FROM sales",
        "SELECT * FROM (WITH Sales AS (SELECT 1 AS region) SELECT region FROM sales) t",
    ] {
        assert_rejected_for_south_member(sql);
    }
}

/// Các dạng câu issue #31 liệt kê, so nguyên `shown`: mọi chỗ đọc sales được bọc, câu ngoài giữ nguyên.
#[test]
fn filters_every_listed_query_shape() {
    for (sql, expected) in [
        (
            "SELECT region FROM sales WHERE amount > 5",
            format!("SELECT region FROM {SOUTH_SALES} AS sales WHERE amount > 5 LIMIT 100"),
        ),
        (
            "SELECT region FROM sales WHERE amount > 5 OR region = 'North'",
            format!(
                "SELECT region FROM {SOUTH_SALES} AS sales WHERE amount > 5 OR region = 'North' LIMIT 100"
            ),
        ),
        (
            "SELECT region, SUM(amount) FROM sales GROUP BY region HAVING SUM(amount) > 10",
            format!(
                "SELECT region, SUM(amount) FROM {SOUTH_SALES} AS sales GROUP BY region HAVING SUM(amount) > 10 \
                 LIMIT 100"
            ),
        ),
        (
            "SELECT a.region FROM sales a JOIN sales b ON a.customer_id = b.customer_id",
            format!(
                "SELECT a.region FROM {SOUTH_SALES} a JOIN {SOUTH_SALES} b ON a.customer_id = b.customer_id \
                 LIMIT 100"
            ),
        ),
        (
            "SELECT a.region, b.amount FROM sales a LEFT JOIN sales b ON a.customer_id = b.customer_id",
            format!(
                "SELECT a.region, b.amount FROM {SOUTH_SALES} a LEFT JOIN {SOUTH_SALES} b \
                 ON a.customer_id = b.customer_id LIMIT 100"
            ),
        ),
        (
            "SELECT a.region FROM sales a, sales b WHERE a.customer_id = b.customer_id",
            format!(
                "SELECT a.region FROM {SOUTH_SALES} a, {SOUTH_SALES} b WHERE a.customer_id = b.customer_id \
                 LIMIT 100"
            ),
        ),
        (
            "SELECT segment FROM customers WHERE customer_id IN (SELECT customer_id FROM sales)",
            format!(
                "SELECT segment FROM customers WHERE customer_id IN (SELECT customer_id FROM {SOUTH_SALES} AS sales) \
                 LIMIT 100"
            ),
        ),
        (
            "SELECT region FROM sales UNION ALL SELECT segment FROM customers",
            format!(
                "SELECT region FROM {SOUTH_SALES} AS sales UNION ALL SELECT segment FROM customers LIMIT 100"
            ),
        ),
        (
            "WITH a AS (SELECT region, amount FROM sales), b AS (SELECT region FROM a) SELECT region FROM b",
            format!(
                "WITH a AS (SELECT region, amount FROM {SOUTH_SALES} AS sales), b AS (SELECT region FROM a) \
                 SELECT region FROM b LIMIT 100"
            ),
        ),
        (
            "SELECT s.region, t.total FROM sales s, \
             LATERAL (SELECT SUM(amount) AS total FROM sales x WHERE x.customer_id = s.customer_id) t",
            format!(
                "SELECT s.region, t.total FROM {SOUTH_SALES} s, \
                 LATERAL (SELECT SUM(amount) AS total FROM {SOUTH_SALES} x WHERE x.customer_id = s.customer_id) t \
                 LIMIT 100"
            ),
        ),
    ] {
        assert_eq!(assert_accepted_for_south_member(sql).shown, expected);
    }
}

/// Hai bảng cùng có chính sách: mỗi chỗ đọc mang điều kiện của chính bảng đó, và row_filters liệt kê mỗi điều kiện một
/// lần dù sales được đọc hai chỗ.
#[test]
fn lists_each_added_condition_once() {
    let mut tables = tables_with_sales_policy("/data", vec![row_filter("region", &["South"])]);
    for table in &mut tables {
        if table.name == "customers" {
            table.row_filters = vec![row_filter("segment", &["Retail"])];
        }
    }
    let guarded = build_guarded_sql(
        "SELECT c.segment FROM sales s JOIN customers c ON s.customer_id = c.customer_id \
         UNION ALL SELECT region FROM sales",
        &tables,
        MAX_ROWS,
    )
    .unwrap();
    assert_eq!(
        guarded.shown,
        format!(
            "SELECT c.segment FROM {SOUTH_SALES} s \
             JOIN (SELECT * FROM customers WHERE \"segment\" IN ('Retail')) c ON s.customer_id = c.customer_id \
             UNION ALL SELECT region FROM {SOUTH_SALES} AS sales LIMIT 100"
        )
    );
    assert_eq!(
        guarded.row_filters,
        vec!["\"region\" IN ('South')", "\"segment\" IN ('Retail')"]
    );
}

#[test]
fn no_policy_adds_no_condition() {
    let guarded = assert_accepted("SELECT region FROM sales");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
    assert!(guarded.row_filters.is_empty(), "{:?}", guarded.row_filters);
}

/// Chính sách ghi cột sales không có. Không chặn thì DuckDB lấy customers.segment, điều kiện thành lọc khách hàng và mọi
/// dòng sales của khách Retail lọt ra, kể cả dòng North.
#[test]
fn policy_on_a_column_the_table_lacks_blocks_the_query() {
    let tables = tables_with_sales_policy("/data", vec![row_filter("segment", &["Retail"])]);
    let result = build_guarded_sql(
        "SELECT s.region FROM customers c, sales s",
        &tables,
        MAX_ROWS,
    );
    assert!(matches!(result, Err(GuardError::Internal(_))), "{result:?}");
}

/// Gọi thẳng bước lọc, bỏ qua bước 2, để kiểm bước lọc tự chặn chỗ đọc nó không nhận ra thay vì để nó chạy không lọc.
fn filter_without_name_check(sql: &str) -> Result<Vec<String>, AppError> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql).unwrap();
    let Statement::Query(mut query) = statements.remove(0) else {
        panic!("not a query: {sql}");
    };
    let tables = tables_with_sales_policy("/data", vec![row_filter("region", &["South"])]);
    super::apply_row_filters(&mut query, &tables, &BTreeSet::new()).map(|(added, _)| added)
}

#[test]
fn row_filter_step_blocks_reads_it_cannot_filter() {
    for sql in [
        "SELECT * FROM main.sales",
        "SELECT * FROM 'sales.parquet'",
        "SELECT * FROM read_parquet('/data/sales/*.parquet')",
        "SELECT * FROM sales()",
        "SELECT * FROM orders",
        "SELECT * FROM UNNEST([1, 2])",
    ] {
        let result = filter_without_name_check(sql);
        assert!(
            matches!(result, Err(AppError::Internal(_))),
            "{sql:?} gave {result:?}"
        );
    }
}

// Giới hạn dòng

#[test]
fn adds_limit_when_missing() {
    let guarded = assert_accepted("SELECT region FROM sales");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
    assert_eq!(guarded.limit_note.as_deref(), Some("Thêm LIMIT 100"));
    let guarded = assert_accepted("SELECT region FROM sales OFFSET 5");
    assert_eq!(guarded.limit_note.as_deref(), Some("Thêm LIMIT 100"));
}

#[test]
fn lowers_limit_above_max_rows() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 5000");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 100");
    assert_eq!(guarded.limit_note.as_deref(), Some("Hạ LIMIT xuống 100"));
}

#[test]
fn keeps_smaller_limit_and_offset() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 10 OFFSET 5");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 10 OFFSET 5");
    assert_eq!(guarded.limit_note, None);
}

/// DuckDB đọc `1_000` là 1000: LIMIT viết có gạch dưới vẫn được giữ, không bị nâng thành trần.
#[test]
fn keeps_limit_written_with_underscores() {
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 1_0");
    assert_eq!(guarded.shown, "SELECT region FROM sales LIMIT 1_0");
    assert_eq!(guarded.limit_note, None);
    let guarded = assert_accepted("SELECT region FROM sales LIMIT 1_000");
    assert_eq!(guarded.limit_note.as_deref(), Some("Hạ LIMIT xuống 100"));
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
        "SELECT x.region FROM (WITH \"secret.csv\" AS (SELECT region FROM sales) SELECT region FROM \"secret.csv\") x, \
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

/// Ghi sales và customers thành file Parquet trong `dir`, mỗi bảng một thư mục như dataset đã nạp.
fn write_sample_parquet(conn: &duckdb::Connection, dir: &Path) {
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
}

#[test]
fn runnable_runs_on_duckdb() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-{}", uuid::Uuid::new_v4()));
    let conn = duckdb::Connection::open_in_memory().unwrap();
    write_sample_parquet(&conn, &dir);

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

fn run_and_collect_regions(
    conn: &duckdb::Connection,
    sql: &str,
    tables: &[AllowedTable],
) -> Vec<String> {
    let guarded = build_guarded_sql(sql, tables, MAX_ROWS).unwrap();
    let mut statement = conn.prepare(&guarded.runnable).unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Thành viên chỉ thấy South cố lấy dòng North bằng nhiều cách viết; trên DuckDB thật, không câu nào ra dòng North.
#[test]
fn member_sees_only_rows_of_the_policy_on_duckdb() {
    let dir = std::env::temp_dir().join(format!("xdi-guard-{}", uuid::Uuid::new_v4()));
    let conn = duckdb::Connection::open_in_memory().unwrap();
    write_sample_parquet(&conn, &dir);
    let tables = tables_with_sales_policy(
        dir.to_str().unwrap(),
        vec![row_filter("region", &["South"])],
    );

    let all_rows = run_and_collect_regions(&conn, "SELECT region FROM sales", &tables);
    let tricky_queries = [
        "SELECT region FROM sales WHERE region = 'North' OR true",
        "SELECT region FROM sales WHERE region <> 'South'",
        "SELECT region FROM sales GROUP BY region",
        "SELECT s.region FROM customers c JOIN sales s ON s.customer_id = c.customer_id",
        "SELECT region FROM (SELECT * FROM sales) AS customers",
        "WITH t AS (SELECT * FROM sales) SELECT region FROM t",
        "SELECT region FROM sales UNION ALL SELECT region FROM sales WHERE region = 'North'",
        // COALESCE vì câu con không còn dòng nào sẽ ra NULL, mà hàm đọc kết quả nhận chuỗi.
        "SELECT COALESCE((SELECT MAX(region) FROM sales WHERE region = 'North'), 'none') AS region",
        "SELECT customers.region FROM sales AS customers",
        "SELECT region FROM \"SALES\"",
        "SELECT b.region FROM sales a LEFT JOIN sales b ON a.customer_id = b.customer_id",
        "SELECT b.region FROM sales a, sales b WHERE a.customer_id = b.customer_id",
        "SELECT t.region FROM sales s, LATERAL (SELECT region FROM sales x WHERE x.customer_id = s.customer_id) t",
        "WITH a AS (SELECT * FROM sales), b AS (SELECT * FROM a) SELECT region FROM b",
        "SELECT region FROM sales GROUP BY region HAVING COUNT(*) > 0",
    ];
    let tricky_results: Vec<(&str, Vec<String>)> = tricky_queries
        .iter()
        .map(|sql| (*sql, run_and_collect_regions(&conn, sql, &tables)))
        .collect();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(all_rows, vec!["South".to_string(), "South".to_string()]);
    for (sql, regions) in tricky_results {
        assert!(
            !regions.iter().any(|region| region == "North"),
            "{sql:?} returned {regions:?}"
        );
    }
}
