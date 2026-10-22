// SPDX-License-Identifier: Apache-2.0
/* engine/excel_worker.rs - chạy engine/excel.rs trong một tiến trình con riêng: `xdi-backend excel-worker`.

Vì sao tách tiến trình: calamine tin những gì file tự khai. Một file xlsx vài KB cố ý làm sai có thể khiến nó cấp phát
hàng trăm GB (tiến trình chết ngay, không bắt lỗi được), giữ vài GB RAM, hoặc đọc hàng chục phút. Chặn từng cách ở
phía trước là chạy đua với mọi chi tiết của calamine. Tiến trình con tự đặt trần RAM và thời gian CPU (setrlimit);
vượt trần thì chỉ tiến trình con dừng, backend trả 400 cho đúng request đó và vẫn phục vụ người khác.

Giao tiếp: backend ghi một Request dạng JSON vào stdin của tiến trình con, tiến trình con in một Reply dạng JSON ra
stdout rồi thoát. Log của tiến trình con ra stderr, chung log với backend.

Request bị hủy giữa chừng (client ngắt kết nối, quá thời gian) thì tiến trình con bị tắt theo (kill_on_drop), không
chạy tiếp ngoài tầm kiểm soát. Test trong crate dùng ExcelWorker::InProcess vì binary của test không phải
xdi-backend; tests/excel_worker.rs chạy binary thật với file độc hại. */

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::engine::excel::{self, Limits, SheetCsv, SheetJob, SheetName, SheetSummary};
use crate::error::AppError;

/// Tham số dòng lệnh để main chạy tiến trình con thay vì server.
pub const WORKER_ARG: &str = "excel-worker";
/// Trần RAM ảo của tiến trình con. File thật cần ít hơn nhiều (Online Retail II, 1 triệu dòng: dưới 200 MB).
const MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
/// Trần thời gian CPU của tiến trình con.
const CPU_SECONDS: u64 = 300;
/// Trần thời gian chờ tính từ phía backend, phòng khi tiến trình con treo mà không tốn CPU.
const WALL_TIME: Duration = Duration::from_secs(360);
/// Trần dung lượng Reply backend đọc từ tiến trình con. Reply thật chỉ vài KB (tên sheet, tên cột); engine/excel.rs đã
/// giới hạn số sheet, độ dài tên sheet và tổng tên cột, trần này chặn thêm một lần phía backend.
const MAX_REPLY_BYTES: u64 = 8 * 1024 * 1024;

/// Cách chạy bộ đọc Excel.
#[derive(Debug, Clone)]
pub enum ExcelWorker {
    /// Chạy binary này với tham số excel-worker. Cách dùng khi chạy thật.
    Process(PathBuf),
    /// Chạy ngay trong tiến trình hiện tại (spawn_blocking), không có trần. Chỉ cho test trong crate.
    #[cfg(test)]
    InProcess,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Request {
    SheetNames {
        path: PathBuf,
        file_name: String,
        limits: Limits,
    },
    ListSheets {
        path: PathBuf,
        file_name: String,
        limits: Limits,
    },
    SheetsToCsv {
        path: PathBuf,
        file_name: String,
        jobs: Vec<SheetJob>,
        limits: Limits,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Reply {
    SheetNames(Vec<SheetName>),
    Sheets(Vec<SheetSummary>),
    Csv(Vec<SheetCsv>),
    Error { kind: ErrorKind, message: String },
}

/// Loại lỗi gửi qua JSON, để backend dựng lại đúng AppError (400, 413, 500).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ErrorKind {
    BadRequest,
    TooLarge,
    Internal,
}

pub async fn sheet_names(
    worker: &ExcelWorker,
    path: &Path,
    file_name: &str,
    limits: Limits,
) -> Result<Vec<SheetName>, AppError> {
    let request = Request::SheetNames {
        path: path.to_path_buf(),
        file_name: file_name.to_string(),
        limits,
    };
    match call(worker, request, file_name).await? {
        Reply::SheetNames(names) => Ok(names),
        other => Err(unexpected(&other)),
    }
}

pub async fn list_sheets(
    worker: &ExcelWorker,
    path: &Path,
    file_name: &str,
    limits: Limits,
) -> Result<Vec<SheetSummary>, AppError> {
    let request = Request::ListSheets {
        path: path.to_path_buf(),
        file_name: file_name.to_string(),
        limits,
    };
    match call(worker, request, file_name).await? {
        Reply::Sheets(sheets) => Ok(sheets),
        other => Err(unexpected(&other)),
    }
}

pub async fn sheets_to_csv(
    worker: &ExcelWorker,
    path: &Path,
    file_name: &str,
    jobs: Vec<SheetJob>,
    limits: Limits,
) -> Result<Vec<SheetCsv>, AppError> {
    let request = Request::SheetsToCsv {
        path: path.to_path_buf(),
        file_name: file_name.to_string(),
        jobs,
        limits,
    };
    match call(worker, request, file_name).await? {
        Reply::Csv(sheets) => Ok(sheets),
        other => Err(unexpected(&other)),
    }
}

fn unexpected(reply: &Reply) -> AppError {
    AppError::Internal(format!("unexpected excel worker reply: {reply:?}"))
}

async fn call(worker: &ExcelWorker, request: Request, file_name: &str) -> Result<Reply, AppError> {
    let reply = match worker {
        ExcelWorker::Process(binary) => call_process(binary, &request, file_name).await?,
        #[cfg(test)]
        ExcelWorker::InProcess => {
            // Đi qua JSON như tiến trình con thật, để test cả phần đổi Request, Reply.
            let input = serde_json::to_vec(&request)
                .map_err(|e| AppError::Internal(format!("encode excel request: {e}")))?;
            let output = tokio::task::spawn_blocking(move || {
                let request: Request = serde_json::from_slice(&input).expect("decode request");
                serde_json::to_vec(&handle(request)).expect("encode reply")
            })
            .await
            .map_err(|e| AppError::Internal(format!("excel task: {e}")))?;
            serde_json::from_slice(&output)
                .map_err(|e| AppError::Internal(format!("decode excel reply: {e}")))?
        }
    };
    match reply {
        Reply::Error { kind, message } => Err(match kind {
            ErrorKind::BadRequest => AppError::BadRequest(message),
            ErrorKind::TooLarge => AppError::TooLarge(message),
            ErrorKind::Internal => AppError::Internal(message),
        }),
        reply => Ok(reply),
    }
}

async fn call_process(
    binary: &Path,
    request: &Request,
    file_name: &str,
) -> Result<Reply, AppError> {
    let input = serde_json::to_vec(request)
        .map_err(|e| AppError::Internal(format!("encode excel request: {e}")))?;
    let mut command = tokio::process::Command::new(binary);
    // Tiến trình con không cần biến môi trường của server (DATABASE_URL, khóa API): xóa hết, chỉ giữ các biến điều
    // khiển log.
    command.env_clear();
    for name in ["RUST_LOG", "RUST_BACKTRACE", "NO_COLOR"] {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
    }
    // Trên Linux binary là /proc/self/exe; đặt tên hiện trong ps cho dễ nhận ra.
    #[cfg(unix)]
    command.arg0("xdi-backend");
    let mut child = command
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        // Future này bị hủy (client ngắt kết nối, hết WALL_TIME) thì tiến trình con bị tắt theo.
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::Internal(format!("start excel worker {}: {e}", binary.display())))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("excel worker has no stdin".to_string()))?;
    stdin
        .write_all(&input)
        .await
        .map_err(|e| AppError::Internal(format!("write excel request: {e}")))?;
    // Đóng stdin để tiến trình con biết đã hết Request.
    drop(stdin);

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("excel worker has no stdout".to_string()))?;
    // Đọc Reply (có trần) rồi chờ tiến trình con thoát. Hết WALL_TIME thì future bỏ dở, child bị drop và bị tắt.
    let finished = tokio::time::timeout(WALL_TIME, async {
        let mut reply = Vec::new();
        (&mut stdout)
            .take(MAX_REPLY_BYTES + 1)
            .read_to_end(&mut reply)
            .await?;
        if reply.len() as u64 > MAX_REPLY_BYTES {
            // Không đọc tiếp: tiến trình con sẽ kẹt khi ghi, tắt nó luôn.
            child.start_kill()?;
        }
        let status = child.wait().await?;
        Ok::<_, std::io::Error>((reply, status))
    })
    .await;
    let (reply, status) = match finished {
        Ok(result) => result.map_err(|e| AppError::Internal(format!("wait excel worker: {e}")))?,
        Err(_) => {
            return Err(AppError::BadRequest(format!(
                "Đọc file {file_name} quá {} phút: file quá lớn hoặc quá phức tạp",
                WALL_TIME.as_secs() / 60
            )))
        }
    };
    if reply.len() as u64 > MAX_REPLY_BYTES {
        return Err(AppError::BadRequest(format!(
            "File {file_name} có danh sách sheet hoặc tên cột quá dài"
        )));
    }
    if !status.success() {
        // Thường là vượt trần RAM hoặc CPU (tiến trình con bị dừng), do file. Ghi log để người vận hành thấy.
        tracing::warn!(file = file_name, %status, "excel worker stopped");
        return Err(AppError::BadRequest(format!(
            "Không đọc được file {file_name}: bộ đọc Excel dừng giữa chừng vì file cần quá nhiều bộ nhớ hoặc thời gian; \
             file hỏng hoặc quá phức tạp"
        )));
    }
    serde_json::from_slice(&reply)
        .map_err(|e| AppError::Internal(format!("decode excel worker reply: {e}")))
}

/// Điểm vào của tiến trình con (main gọi khi có tham số WORKER_ARG). Trả mã thoát.
pub fn serve() -> i32 {
    limit_resources();
    let mut input = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut input) {
        tracing::error!(error = %e, "excel worker cannot read stdin");
        return 2;
    }
    let reply = match serde_json::from_slice::<Request>(&input) {
        Ok(request) => handle(request),
        Err(e) => Reply::Error {
            kind: ErrorKind::Internal,
            message: format!("invalid excel worker request: {e}"),
        },
    };
    let written = serde_json::to_vec(&reply)
        .map_err(std::io::Error::other)
        .and_then(|bytes| {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&bytes)?;
            stdout.flush()
        });
    match written {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!(error = %e, "excel worker cannot write reply");
            2
        }
    }
}

fn handle(request: Request) -> Reply {
    let result = match request {
        Request::SheetNames {
            path,
            file_name,
            limits,
        } => excel::sheet_names(&path, &file_name, limits).map(Reply::SheetNames),
        Request::ListSheets {
            path,
            file_name,
            limits,
        } => excel::list_sheets(&path, &file_name, limits).map(Reply::Sheets),
        Request::SheetsToCsv {
            path,
            file_name,
            jobs,
            limits,
        } => excel::sheets_to_csv(&path, &file_name, &jobs, limits).map(Reply::Csv),
    };
    result.unwrap_or_else(|e| {
        let (kind, message) = match e {
            AppError::BadRequest(m) => (ErrorKind::BadRequest, m),
            AppError::TooLarge(m) => (ErrorKind::TooLarge, m),
            other => (ErrorKind::Internal, format!("{other:?}")),
        };
        Reply::Error { kind, message }
    })
}

/// Đặt trần RAM ảo và thời gian CPU cho chính tiến trình này. Cấp phát vượt trần thì tiến trình dừng; hết thời gian
/// CPU thì hệ điều hành dừng nó. Không ghi core dump: mỗi file độc hại làm tiến trình con dừng một lần, ghi core dump
/// hàng GB mỗi lần thì đầy đĩa.
#[cfg(unix)]
fn limit_resources() {
    use rlimit::Resource;
    let limits = [
        (Resource::AS, MEMORY_LIMIT),
        (Resource::CPU, CPU_SECONDS),
        (Resource::CORE, 0),
    ];
    for (resource, limit) in limits {
        if let Err(e) = rlimit::setrlimit(resource, limit, limit) {
            tracing::warn!(error = %e, "excel worker cannot set resource limit");
        }
    }
}

#[cfg(not(unix))]
fn limit_resources() {
    tracing::warn!("excel worker runs without memory and CPU limits on this platform");
}
