// SPDX-License-Identifier: Apache-2.0
/* service/overview/query.rs - chạy một câu SQL do code của trang tổng quan dựng (service/overview/sql.rs): qua guard với
bảng của người xem (lọc dòng theo vai dữ liệu như luồng hỏi và truy dòng nguồn), tìm trong cache theo phiên bản
(overview/cache.rs), chưa có thì chạy bằng run_code_query của câu "tại sao" (explain/breakdown.rs) để chung hàng lượt
DuckDB và cách báo lỗi. */

use std::sync::Arc;

use uuid::Uuid;

use crate::engine::guard::{build_guarded_sql, GuardError};
use crate::error::AppError;
use crate::service::ask::contract::ResultTable;
use crate::service::explain::breakdown::{run_code_query, CodeQuery, QueryEnv};
use crate::service::overview::cache::OverviewCache;

/// Kết quả một câu của trang tổng quan.
pub enum Ran {
    Done {
        /// Bản đã qua guard, trên tên logic: giao diện hiện cho người xem.
        shown: String,
        result: Arc<ResultTable>,
    },
    /// Guard từ chối hay DuckDB lỗi; lời tiếng Việt, đã bỏ đường dẫn file. Trang vẫn trả, ghi phần này là thiếu.
    Failed(String),
}

/// Err chỉ cho lỗi máy chủ (không mở được DuckDB, file Parquet hỏng), như run_code_query.
pub async fn run_cached(
    env: &QueryEnv<'_>,
    cache: &OverviewCache,
    version_id: Uuid,
    sql: &str,
) -> Result<Ran, AppError> {
    // Guard trước khi tìm cache: khóa cache là câu chạy, đã gồm điều kiện lọc dòng của người xem.
    let guarded = match build_guarded_sql(sql, env.tables, env.max_rows) {
        Ok(guarded) => guarded,
        Err(GuardError::Internal(e)) => return Err(e),
        Err(
            GuardError::UnknownName(reason)
            | GuardError::Oversized(reason)
            | GuardError::Rejected(reason),
        ) => return Ok(Ran::Failed(reason)),
    };
    if let Some(result) = cache.get(version_id, &guarded.runnable) {
        return Ok(Ran::Done {
            shown: guarded.shown,
            result,
        });
    }
    // run_code_query guard lại câu một lần nữa: tốn ít so với chạy câu, đổi lại không phải chép cách báo lỗi.
    match run_code_query(env, sql, None).await? {
        CodeQuery::Ran { shown, result, .. } => {
            let result = Arc::new(result);
            // Chỉ giữ câu chạy được: lỗi quá giờ có thể tự hết ở lần mở sau.
            cache.put(version_id, guarded.runnable, Arc::clone(&result));
            Ok(Ran::Done { shown, result })
        }
        CodeQuery::Refused(reason) | CodeQuery::Failed(reason) => Ok(Ran::Failed(reason)),
    }
}
