// SPDX-License-Identifier: Apache-2.0
/* engine/query.rs - chạy câu SQL đã qua guard trên DuckDB nhúng, trả tên cột, kiểu cột và các dòng. Tách khỏi
analytics.rs (phần nạp file) vì chạy truy vấn có giới hạn riêng: thời gian, bộ nhớ, số dòng giữ lại. service/ask_run.rs
gọi run_query trong spawn_blocking, đổi dòng sang ResultTable của hợp đồng và quyết định có nhờ mô hình sửa câu không.

Ô giữ nguyên kiểu duckdb::types::Value. Kiểu cột trả kèm vì giá trị không đủ để biết kiểu: Value::Timestamp dùng cho
cả TIMESTAMP lẫn TIMESTAMPTZ, và cột toàn null vẫn phải gửi đúng một dạng ô.

Giới hạn thời gian và bộ nhớ ở đây không chặn được mọi câu: DuckDB tính biểu thức hằng ngay lúc chuẩn bị câu, khi
interrupt chưa có tác dụng và ngoài memory_limit (chuỗi tạo lúc chạy thì vẫn trong trần, vượt là OutOfMemory), và một
lời gọi hàm dựng chuỗi rất dài không dừng được giữa chừng. Guard thu hẹp phần này bằng luật kích thước hàm
(guard/sizes.rs), số tầng câu (guard/depth.rs), số mảnh ghép (guard/pieces.rs), độ dài chuỗi hằng và số cột
(guard/width.rs); câu nhân theo số dòng của bảng thì chưa, và việc đổi kết quả sang Arrow, sang Value nằm ngoài trần của
DuckDB; bảo đảm thật cần tiến trình con (#119, docs/GUARD.md). */

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use duckdb::core::LogicalTypeId;
use duckdb::types::Value;
use duckdb::Connection;

use super::analytics;
use crate::error::AppError;

/// Trần bộ nhớ DuckDB quản lý cho một câu (bảng băm, sắp xếp, bộ đệm đọc file, chuỗi tạo lúc chạy); biểu thức hằng tính
/// lúc chuẩn bị câu thì không tính vào đây. Mỗi câu mở một DuckDB riêng trong bộ nhớ, nên đây là trần của từng câu; số
/// câu chạy cùng lúc do service giới hạn (AppState::query_jobs).
const MEMORY_LIMIT: &str = "1GB";

/// Sau khi hết giờ, gọi interrupt lại sau mỗi khoảng này tới khi câu dừng (xem run_query).
const INTERRUPT_EVERY: Duration = Duration::from_millis(100);

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
    /// DuckDB không chạy được câu: sai tên, sai kiểu, chuỗi ngày không đúng định dạng; hoặc cột có kiểu chưa đọc được.
    /// Mô hình thường sửa được. Lời báo nguyên văn của DuckDB, có thể chứa đường dẫn file Parquet: người gọi phải lọc
    /// trước khi đưa ra ngoài. File Parquet hỏng cũng ra loại này ("Invalid Input Error: No magic bytes found at end of
    /// file '...'"): DuckDB không tách nó khỏi lỗi của câu, chỉ lời báo có đường dẫn file; service/ask_run.rs nhận ra
    /// qua đường dẫn đó và coi là lỗi máy chủ.
    Sql(String),
    /// Chạy quá thời gian cho phép và đã bị dừng. Sửa câu thường không làm nó nhanh hơn, nên không vào vòng sửa.
    Timeout,
    /// Cần nhiều hơn MEMORY_LIMIT. Như Timeout, không vào vòng sửa; lời báo của DuckDB khuyên đổi cấu hình (SET
    /// memory_limit), mô hình đọc được sẽ viết câu SET và bị guard chặn.
    OutOfMemory,
    /// Lỗi của máy chủ, không phải của câu: không mở được DuckDB, lỗi đọc đĩa (IO Error, ví dụ file Parquet mất).
    Internal(AppError),
}

/// Chạy một câu, giữ tối đa keep_rows dòng đầu. Hàm đồng bộ và tốn CPU: gọi trong tokio::task::spawn_blocking.
/// Quá timeout thì một luồng phụ gọi interrupt, DuckDB dừng câu ở lần kiểm kế tiếp và báo lỗi; hàm trả
/// QueryError::Timeout. Phần DuckDB tính lúc chuẩn bị câu thì không dừng được (xem đầu file).
pub fn run_query(
    runnable: &str,
    keep_rows: usize,
    timeout: Duration,
) -> Result<QueryOutput, QueryError> {
    let conn = open_limited().map_err(QueryError::Internal)?;
    let interrupt = conn.interrupt_handle();
    let (done, wait_done) = mpsc::channel::<()>();
    // Luồng canh giờ: chờ tín hiệu xong, tối đa timeout; trả true khi đã phải dừng câu. move: luồng giữ interrupt và
    // wait_done tới khi thoát, nên không mượn gì của hàm này.
    let watcher = thread::spawn(move || {
        if wait_done.recv_timeout(timeout) != Err(RecvTimeoutError::Timeout) {
            return false;
        }
        // DuckDB xóa cờ interrupt khi bắt đầu chuẩn bị câu và khi bắt đầu chạy, nên một lần gọi rơi vào trước đó mất
        // tác dụng (timeout rất nhỏ thì câu chạy tới hết). Gọi lại tới khi câu dừng.
        loop {
            interrupt.interrupt();
            if wait_done.recv_timeout(INTERRUPT_EVERY) != Err(RecvTimeoutError::Timeout) {
                return true;
            }
        }
    });
    // duckdb-rs panic với vài kiểu nó chưa đọc được; bắt lại để câu hỏi nhận lời báo thay vì lỗi 500. AssertUnwindSafe:
    // sau panic không dùng lại conn hay dữ liệu dở nào, chỉ đóng kết nối.
    let result = panic::catch_unwind(AssertUnwindSafe(|| read_rows(&conn, runnable, keep_rows)))
        .unwrap_or_else(|payload| Err(build_panic_error(payload.as_ref())));
    // Luồng canh giờ đã thoát (hết giờ) thì không ai nhận tín hiệu; send lỗi và bỏ qua được.
    let _ = done.send(());
    let timed_out = watcher.join().unwrap_or(false);
    match result {
        Ok(output) => Ok(output),
        // Câu xong đúng lúc hết giờ thì vẫn lấy kết quả ở trên; chỉ lỗi xảy ra khi đã gọi interrupt mới là quá giờ.
        Err(_) if timed_out => Err(QueryError::Timeout),
        Err(e) => Err(e),
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

fn read_rows(conn: &Connection, sql: &str, keep_rows: usize) -> Result<QueryOutput, QueryError> {
    let mut statement = conn.prepare(sql).map_err(build_query_error)?;
    let mut rows = statement.query([]).map_err(build_query_error)?;
    // Tên và kiểu cột chỉ có sau khi câu đã chạy (query ở trên), nên đọc qua rows chứ không qua statement.
    let mut columns = Vec::new();
    if let Some(executed) = rows.as_ref() {
        for i in 0..executed.column_count() {
            columns.push(QueryColumn {
                name: executed.column_name(i).map_err(build_query_error)?.clone(),
                type_id: executed.column_logical_type(i).id(),
            });
        }
    }
    check_column_types(&columns)?;
    let mut kept = Vec::new();
    let mut total_rows = 0;
    while let Some(row) = rows.next().map_err(build_query_error)? {
        // Vẫn đi hết các dòng sau keep_rows để đếm total_rows; số dòng đã bị guard giới hạn trước đó.
        if kept.len() < keep_rows {
            let cells = (0..columns.len())
                .map(|i| row.get::<_, Value>(i))
                .collect::<Result<Vec<_>, _>>()
                .map_err(build_query_error)?;
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

/// Kiểu cột duckdb-rs không đọc được (TIME_NS làm nó panic) hoặc đọc ra mất thông tin (TIMETZ mất múi giờ, BIT và
/// BIGNUM ra byte thô). Báo như lỗi của câu để mô hình đổi cột sang VARCHAR; khi đó DuckDB tự in giá trị, đúng dạng.
/// Hạn chế: chỉ xét cột ngoài cùng; các kiểu này nằm trong danh sách hay struct thì vẫn in sai (service/ask_other_text.rs).
fn check_column_types(columns: &[QueryColumn]) -> Result<(), QueryError> {
    use LogicalTypeId as T;
    for column in columns {
        let type_name = match column.type_id {
            T::TimeNs => "TIME_NS",
            T::TimeTZ => "TIME WITH TIME ZONE",
            T::Bit => "BIT",
            T::Bignum => "BIGNUM",
            T::Variant => "VARIANT",
            T::Geometry => "GEOMETRY",
            T::Unsupported | T::Invalid => "không rõ",
            _ => continue,
        };
        let name = &column.name;
        return Err(QueryError::Sql(format!(
            "Cột {name} có kiểu {type_name}, kiểu backend chưa đọc được. Đổi cột đó sang VARCHAR, ví dụ CAST(\"{name}\" AS VARCHAR)"
        )));
    }
    Ok(())
}

/// Panic của duckdb-rs khi đọc kết quả: chi tiết vào log, câu hỏi nhận lời báo như lỗi của câu.
fn build_panic_error(payload: &(dyn std::any::Any + Send)) -> QueryError {
    let detail = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");
    tracing::error!(detail, "duckdb panicked while reading query result");
    QueryError::Sql(
        "Không đọc được kết quả của câu SQL, có thể do một cột có kiểu backend chưa đọc được. Đổi các cột kiểu ngày giờ \
         hay kiểu đặc biệt sang VARCHAR rồi chạy lại"
            .to_string(),
    )
}

/// Phân loại theo phần đầu lời báo của DuckDB ("IO Error: ...", "Out of Memory Error: ..."). Guard chỉ cho đọc file
/// Parquet của dataset, nên lỗi đọc đĩa là lỗi của máy chủ (file mất), mô hình không sửa được.
fn build_query_error(error: duckdb::Error) -> QueryError {
    let message = error.to_string();
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

    /// Timeout rất nhỏ: lần interrupt đầu rơi vào trước lúc DuckDB xóa cờ; câu vẫn phải bị dừng nhờ các lần gọi sau.
    /// Câu chạy được tới hết trong vài giây, nên khi chỉ interrupt một lần thì test nhận Ok và đỏ thay vì treo.
    #[test]
    fn tiny_timeout_still_stops_the_query() {
        let sql = "SELECT count(*) FROM range(40000) a, range(40000) b";
        for _ in 0..5 {
            let err = run_query(sql, 1000, Duration::from_nanos(1)).unwrap_err();
            assert!(matches!(err, QueryError::Timeout), "{err:?}");
        }
    }

    /// TIME_NS làm duckdb-rs panic; TIMETZ, BIT, BIGNUM đọc ra mất thông tin. Cả bốn thành lỗi sửa được, nêu tên cột.
    #[test]
    fn unreadable_column_types_are_sql_errors() {
        for (sql, type_name) in [
            ("SELECT CAST(TIME '01:02:03' AS TIME_NS) AS t", "TIME_NS"),
            ("SELECT TIMETZ '12:34:56+07' AS t", "TIME WITH TIME ZONE"),
            ("SELECT '0101'::BIT AS t", "BIT"),
            ("SELECT 12345::BIGNUM AS t", "BIGNUM"),
        ] {
            let err = run_query(sql, 1000, NO_LIMIT).unwrap_err();
            assert!(
                matches!(&err, QueryError::Sql(m) if m.contains(type_name) && m.contains("CAST(\"t\" AS VARCHAR)")),
                "{sql}: {err:?}"
            );
        }
    }

    /// TIME_NS lồng trong danh sách không bị check_column_types bắt; panic của duckdb-rs vẫn thành lời báo.
    #[test]
    fn panic_while_reading_rows_is_a_sql_error() {
        let sql = "SELECT [CAST(TIME '01:02:03' AS TIME_NS)] AS t";
        let err = run_query(sql, 1000, NO_LIMIT).unwrap_err();
        assert!(
            matches!(&err, QueryError::Sql(m) if m.starts_with("Không đọc được kết quả")),
            "{err:?}"
        );
    }

    #[test]
    fn query_over_the_memory_limit_is_stopped() {
        // Gom 300 triệu số vào danh sách cần vài GB, quá trần 1 GB.
        let sql = "SELECT list(range) FROM range(300000000)";
        let err = run_query(sql, 1000, NO_LIMIT).unwrap_err();
        assert!(matches!(err, QueryError::OutOfMemory), "{err:?}");
    }
}
