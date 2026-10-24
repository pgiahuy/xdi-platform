// SPDX-License-Identifier: Apache-2.0
/* engine/query.rs - chạy câu SQL đã qua guard trên DuckDB nhúng, trả tên cột, kiểu cột và các dòng. Tách khỏi
analytics.rs (phần nạp file) vì chạy truy vấn có giới hạn riêng: thời gian, bộ nhớ, số dòng giữ lại. service/ask_run.rs
gọi run_query trong spawn_blocking, đổi dòng sang ResultTable của hợp đồng và quyết định có nhờ mô hình sửa câu không.

Ô giữ nguyên kiểu duckdb::types::Value. Kiểu cột trả kèm vì giá trị không đủ để biết kiểu: Value::Timestamp dùng cho
cả TIMESTAMP lẫn TIMESTAMPTZ, và cột toàn null vẫn phải gửi đúng một dạng ô. */

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use duckdb::core::LogicalTypeId;
use duckdb::types::Value;
use duckdb::Connection;

use super::analytics;
use crate::error::AppError;

/// Bộ nhớ tối đa một câu được dùng. Mỗi câu mở một DuckDB riêng trong bộ nhớ, nên đây là trần của từng câu; số câu chạy
/// cùng lúc do service giới hạn (AppState::query_jobs).
const MEMORY_LIMIT: &str = "1GB";

#[derive(Debug)]
pub struct QueryColumn {
    /// Tên cột như DuckDB trả, có thể trùng nhau (`SELECT a, a`).
    pub name: String,
    pub type_id: LogicalTypeId,
}

#[derive(Debug)]
pub struct QueryOutput {
    pub columns: Vec<QueryColumn>,
    /// Tối đa keep_rows dòng đầu; mỗi dòng là các ô theo thứ tự cột.
    pub rows: Vec<Vec<Value>>,
    /// Số dòng câu trả về, kể cả các dòng không giữ lại.
    pub total_rows: u64,
}

#[derive(Debug)]
pub enum QueryError {
    /// DuckDB không chạy được câu: sai tên, sai kiểu, chuỗi ngày không đúng định dạng. Mô hình thường sửa được. Lời báo
    /// nguyên văn của DuckDB, có thể chứa đường dẫn file Parquet: người gọi phải lọc trước khi đưa ra ngoài.
    Sql(String),
    /// Chạy quá thời gian cho phép và đã bị dừng. Sửa câu thường không làm nó nhanh hơn, nên không vào vòng sửa.
    Timeout,
    /// Cần nhiều hơn MEMORY_LIMIT. Như Timeout, không vào vòng sửa; lời báo của DuckDB khuyên đổi cấu hình (SET
    /// memory_limit), mô hình đọc được sẽ viết câu SET và bị guard chặn.
    OutOfMemory,
    /// Lỗi của máy chủ, không phải của câu: không mở được DuckDB, không đọc được file Parquet trên đĩa.
    Internal(AppError),
}

/// Chạy một câu, giữ tối đa keep_rows dòng đầu. Hàm đồng bộ và tốn CPU: gọi trong tokio::task::spawn_blocking.
/// Quá timeout thì một luồng phụ gọi interrupt, DuckDB dừng câu đang chạy và báo lỗi; hàm trả QueryError::Timeout.
pub fn run_query(
    runnable: &str,
    keep_rows: usize,
    timeout: Duration,
) -> Result<QueryOutput, QueryError> {
    let conn = open_limited().map_err(QueryError::Internal)?;
    let interrupt = conn.interrupt_handle();
    let (done, wait_done) = mpsc::channel::<()>();
    // Luồng canh giờ: chờ tín hiệu xong, tối đa timeout. Hết giờ mà chưa có tín hiệu thì dừng câu. move: luồng giữ
    // interrupt và wait_done tới khi thoát, nên không mượn gì của hàm này.
    let watcher = thread::spawn(move || {
        let timed_out = wait_done.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout);
        if timed_out {
            interrupt.interrupt();
        }
        timed_out
    });
    let result = read_rows(&conn, runnable, keep_rows);
    // Luồng canh giờ đã thoát (hết giờ) thì không ai nhận tín hiệu; send lỗi và bỏ qua được.
    let _ = done.send(());
    let timed_out = watcher.join().unwrap_or(false);
    match result {
        Ok(output) => Ok(output),
        // Câu xong đúng lúc hết giờ thì vẫn lấy kết quả ở trên; chỉ lỗi xảy ra khi đã gọi interrupt mới là quá giờ.
        Err(_) if timed_out => Err(QueryError::Timeout),
        Err(e) => Err(build_query_error(e.to_string())),
    }
}

/// Kết nối như analytics::open, thêm trần bộ nhớ. temp_directory rỗng tắt việc ghi dữ liệu tạm ra đĩa khi chạm trần:
/// mặc định DuckDB ghi vào thư mục .tmp ngay chỗ backend đang chạy, ngoài DATA_DIR, không ai dọn; tắt thì câu chạm trần
/// báo OutOfMemory.
fn open_limited() -> Result<Connection, AppError> {
    let conn = analytics::open()?;
    conn.execute_batch(&format!(
        "SET memory_limit = '{MEMORY_LIMIT}'; SET temp_directory = '';"
    ))
    .map_err(|e| AppError::Internal(format!("limit duckdb memory: {e}")))?;
    Ok(conn)
}

fn read_rows(conn: &Connection, sql: &str, keep_rows: usize) -> Result<QueryOutput, duckdb::Error> {
    let mut statement = conn.prepare(sql)?;
    let mut rows = statement.query([])?;
    // Tên và kiểu cột chỉ có sau khi câu đã chạy (query ở trên), nên đọc qua rows chứ không qua statement.
    let mut columns = Vec::new();
    if let Some(executed) = rows.as_ref() {
        for i in 0..executed.column_count() {
            columns.push(QueryColumn {
                name: executed.column_name(i)?.clone(),
                type_id: executed.column_logical_type(i).id(),
            });
        }
    }
    let mut kept = Vec::new();
    let mut total_rows = 0;
    while let Some(row) = rows.next()? {
        // Vẫn đi hết các dòng sau keep_rows để đếm total_rows; số dòng đã bị guard giới hạn trước đó.
        if kept.len() < keep_rows {
            let cells = (0..columns.len())
                .map(|i| row.get::<_, Value>(i))
                .collect::<Result<Vec<_>, _>>()?;
            kept.push(cells);
        }
        total_rows += 1;
    }
    Ok(QueryOutput {
        columns,
        rows: kept,
        total_rows,
    })
}

/// Phân loại theo phần đầu lời báo của DuckDB ("IO Error: ...", "Out of Memory Error: ..."). Guard chỉ cho đọc file
/// Parquet của dataset, nên lỗi đọc đĩa là lỗi của máy chủ (file mất, hỏng), mô hình không sửa được.
fn build_query_error(message: String) -> QueryError {
    if message.starts_with("IO Error") {
        QueryError::Internal(AppError::Internal(format!("run query: {message}")))
    } else if message.starts_with("Out of Memory Error") {
        tracing::info!(error = %message, "query hit the memory limit");
        QueryError::OutOfMemory
    } else {
        QueryError::Sql(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_LIMIT: Duration = Duration::from_secs(30);

    #[test]
    fn keeps_first_rows_and_counts_all() {
        let output = run_query("SELECT range AS n FROM range(1500)", 1000, NO_LIMIT).unwrap();
        assert_eq!(output.rows.len(), 1000);
        assert_eq!(output.total_rows, 1500);
        assert_eq!(output.rows[999], vec![Value::BigInt(999)]);
        assert_eq!(output.columns[0].name, "n");
        assert_eq!(output.columns[0].type_id, LogicalTypeId::Bigint);
    }

    #[test]
    fn empty_result_still_has_columns() {
        let output = run_query("SELECT 1 AS a, 'x' AS a WHERE false", 1000, NO_LIMIT).unwrap();
        let names: Vec<&str> = output.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["a", "a"]);
        assert_eq!(output.columns[1].type_id, LogicalTypeId::Varchar);
        assert_eq!(output.total_rows, 0);
    }

    #[test]
    fn duckdb_errors_are_sql_errors() {
        let err = run_query("SELECT nope FROM range(3)", 1000, NO_LIMIT).unwrap_err();
        assert!(
            matches!(&err, QueryError::Sql(m) if m.contains("nope")),
            "{err:?}"
        );
    }

    #[test]
    fn missing_parquet_files_are_server_errors() {
        let sql = "SELECT * FROM read_parquet('/nonexistent-xdi/*.parquet')";
        let err = run_query(sql, 1000, NO_LIMIT).unwrap_err();
        assert!(matches!(err, QueryError::Internal(_)), "{err:?}");
    }

    /// Câu chạy rất lâu (tích chéo hàng nghìn tỷ dòng) bị dừng khi hết giờ, không chờ tới khi xong.
    #[test]
    fn long_query_is_stopped_at_the_timeout() {
        let started = std::time::Instant::now();
        let sql = "SELECT count(*) FROM range(10000000) a, range(10000000) b";
        let err = run_query(sql, 1000, Duration::from_millis(300)).unwrap_err();
        assert!(matches!(err, QueryError::Timeout), "{err:?}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn query_over_the_memory_limit_is_stopped() {
        // Gom 300 triệu số vào danh sách cần vài GB, quá trần 1 GB.
        let sql = "SELECT list(range) FROM range(300000000)";
        let err = run_query(sql, 1000, NO_LIMIT).unwrap_err();
        assert!(matches!(err, QueryError::OutOfMemory), "{err:?}");
    }
}
