// SPDX-License-Identifier: Apache-2.0
/* engine/guard/sizes.rs - luật riêng cho các hàm trong danh sách được phép mà tham số quyết định kích thước kết quả:
lpad, rpad dựng chuỗi dài đúng bằng tham số độ dài; replace, regexp_replace làm chuỗi dài ra sau mỗi lần thay, và lồng
nhau thì dài ra theo cấp số nhân. ExprChecker (mod.rs) gọi check_function_size sau khi tên hàm đã có trong danh sách.

Chặn ở guard vì giới hạn lúc chạy (engine/query.rs) không đỡ được các hàm này: DuckDB tính biểu thức hằng ngay lúc
chuẩn bị câu, khi interrupt chưa có tác dụng, và chuỗi dựng ra không tính vào memory_limit. Đo bằng run_query trên máy
phát triển: `SELECT length(lpad('x', 300000000, 'y'))` chạy 32 giây với giới hạn 1 giây; độ dài lấy từ cột
(`lpad(x, 300000000 + n, 'y')`) chạy 87 giây và dùng 1,2 GB RAM với trần 1 GB.

Hạn chế: ghép chuỗi (`||`, concat) qua nhiều tầng câu con vẫn nhân đôi được chuỗi mỗi tầng, guard không chặn; xem
docs/GUARD.md, mục Hạn chế đã biết. */

use sqlparser::ast::{Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Value};

/// Độ dài tối đa của chuỗi lpad, rpad dựng ra: đủ để căn lề mã số hay ngày tháng, nhỏ tới mức tính tức thì.
pub const MAX_PAD_LENGTH: u64 = 10_000;

/// Err là lý do từ chối (tiếng Việt). name đã viết thường.
pub(super) fn check_function_size(name: &str, function: &Function) -> Result<(), String> {
    match name {
        "lpad" | "rpad" => check_pad(name, function),
        "replace" => check_replace(function),
        "regexp_replace" => check_regexp_replace(function),
        _ => Ok(()),
    }
}

/// Độ dài phải là số viết thẳng: biểu thức hay tên cột thì không biết trước giá trị.
fn check_pad(name: &str, function: &Function) -> Result<(), String> {
    let length = plain_args(function).and_then(|args| args.get(1).and_then(|e| number(e)));
    match length {
        Some(length) if length <= MAX_PAD_LENGTH => Ok(()),
        _ => Err(format!(
            "Hàm {name} chỉ nhận độ dài là một số viết thẳng, tối đa {MAX_PAD_LENGTH}"
        )),
    }
}

/// Chuỗi thay không dài hơn chuỗi tìm thì kết quả không dài hơn chuỗi gốc, lồng bao nhiêu lần cũng vậy. Đủ cho việc
/// thường gặp: bỏ ký tự, đổi dấu phân cách.
fn check_replace(function: &Function) -> Result<(), String> {
    let pair = plain_args(function).and_then(|args| match args.as_slice() {
        [_, from, to] => Some((text(from)?, text(to)?)),
        _ => None,
    });
    match pair {
        Some((from, to)) if to.chars().count() <= from.chars().count() => Ok(()),
        _ => Err(
            "Hàm replace chỉ nhận chuỗi tìm và chuỗi thay viết thẳng, chuỗi thay không dài hơn chuỗi tìm".to_string(),
        ),
    }
}

/// Mẫu regex khớp được chuỗi rỗng ở mọi vị trí, nên chuỗi thay dù ngắn cũng làm kết quả dài ra; chỉ cho xóa.
fn check_regexp_replace(function: &Function) -> Result<(), String> {
    let replacement = plain_args(function).and_then(|args| args.get(2).and_then(|e| text(e)));
    match replacement {
        Some("") => Ok(()),
        _ => Err("Hàm regexp_replace chỉ dùng để xóa phần khớp: chuỗi thay phải là ''".to_string()),
    }
}

/// Các tham số khi hàm gọi dạng thường `f(a, b, c)`; None khi có tham số đặt tên hay mệnh đề trong ngoặc, để luật ở
/// trên từ chối thay vì đoán.
fn plain_args(function: &Function) -> Option<Vec<&Expr>> {
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    if !list.clauses.is_empty() {
        return None;
    }
    list.args
        .iter()
        .map(|arg| match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => Some(expr),
            _ => None,
        })
        .collect()
}

fn number(expr: &Expr) -> Option<u64> {
    match expr {
        Expr::Value(v) => match &v.value {
            Value::Number(n, _) => n.parse().ok(),
            _ => None,
        },
        _ => None,
    }
}

fn text(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Value(v) => match &v.value {
            Value::SingleQuotedString(s) => Some(s.as_str()),
            _ => None,
        },
        _ => None,
    }
}
