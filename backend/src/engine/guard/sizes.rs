// SPDX-License-Identifier: Apache-2.0
/* engine/guard/sizes.rs - luật riêng cho các hàm trong danh sách được phép mà tham số quyết định kích thước kết quả:
lpad, rpad dựng chuỗi dài đúng bằng tham số độ dài; replace, regexp_replace làm chuỗi dài ra sau mỗi lần thay, và lồng
nhau thì dài ra theo cấp số nhân; strftime in mỗi mã định dạng hai byte thành nhiều byte (`%c` thành cả ngày giờ).
ExprChecker (mod.rs) gọi check_function_size sau khi tên hàm đã có trong danh sách.

Chặn ở guard vì giới hạn lúc chạy (engine/query.rs) không đỡ được các hàm này: DuckDB tính biểu thức hằng ngay lúc
chuẩn bị câu, khi interrupt chưa có tác dụng và ngoài trần memory_limit; một lời gọi hàm dựng chuỗi rất dài lúc chạy
cũng không dừng được giữa chừng. Đo bằng run_query trên máy phát triển: `SELECT length(lpad('x', 300000000, 'y'))`
chạy 32 giây với giới hạn 1 giây; độ dài lấy từ cột (`lpad(x, 300000000 + n, 'y')`) chạy 87 giây và dùng 1,2 GB RAM
với trần 1 GB.

Độ dài so theo byte, không theo ký tự: thay 'a' bằng '😀' giữ số ký tự nhưng gấp 4 số byte, lồng nhau thì gấp 4 mỗi lần.
Câu vi phạm luật bị từ chối với lý do để mô hình viết lại (GuardError::Oversized, vào vòng sửa).

Trần của lpad, rpad và strftime giữ kết quả trong cỡ trần chuỗi hằng (1000 byte, guard/width.rs): ba hàm này là cách
dựng một chuỗi dài từ câu ngắn mà không viết chuỗi hằng. Ghép chuỗi qua nhiều tầng câu con thì guard/depth.rs và
guard/pieces.rs giới hạn. */

use sqlparser::ast::{Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Value};

/// Độ dài tối đa của chuỗi lpad, rpad dựng ra, tính bằng ký tự như hai hàm này đếm: đủ để căn lề mã số hay ngày tháng.
/// Một ký tự UTF-8 dài tới 4 byte (`lpad('😀', 1000, '😀')` ra 4000 byte), nên 250 ký tự là tối đa 1000 byte, bằng
/// trần chuỗi hằng (width.rs).
pub const MAX_PAD_LENGTH: u64 = 250;

/// Độ dài tối đa của chuỗi định dạng strftime, tính bằng byte: đủ cho mọi định dạng ngày giờ thường dùng. Mỗi mã hai
/// byte in ra không quá vài chục byte, nên kết quả vẫn cỡ trần chuỗi hằng.
pub const MAX_TIME_FORMAT_BYTES: usize = 100;

/// Err là lý do từ chối (tiếng Việt). name đã viết thường.
pub(super) fn check_function_size(name: &str, function: &Function) -> Result<(), String> {
    match name {
        "lpad" | "rpad" => check_pad(name, function),
        "replace" => check_replace(function),
        "regexp_replace" => check_regexp_replace(function),
        "strftime" => check_time_format(function),
        _ => Ok(()),
    }
}

/// Độ dài phải là số viết thẳng: biểu thức hay tên cột thì không biết trước giá trị.
fn check_pad(name: &str, function: &Function) -> Result<(), String> {
    let length = plain_args(function).and_then(|args| args.get(1).and_then(|e| number(e)));
    match length {
        Some(length) if length <= MAX_PAD_LENGTH => Ok(()),
        _ => Err(format!(
            "Hàm {name} chỉ nhận độ dài là một số viết thẳng, tối đa {MAX_PAD_LENGTH} ký tự"
        )),
    }
}

/// Định dạng phải viết thẳng ở tham số thứ hai, như chữ ký strftime(ngày giờ, định dạng) của DuckDB: định dạng lấy từ
/// cột hay biểu thức thì không biết trước độ dài kết quả.
fn check_time_format(function: &Function) -> Result<(), String> {
    let format = plain_args(function).and_then(|args| match args.as_slice() {
        [_, format] => text(format),
        _ => None,
    });
    match format {
        Some(format) if format.len() <= MAX_TIME_FORMAT_BYTES => Ok(()),
        _ => Err(format!(
            "Hàm strftime chỉ nhận strftime(ngày giờ, định dạng) với định dạng viết thẳng, tối đa \
             {MAX_TIME_FORMAT_BYTES} byte"
        )),
    }
}

/// Chuỗi thay không dài hơn chuỗi tìm (tính bằng byte) thì kết quả không dài hơn chuỗi gốc, lồng bao nhiêu lần cũng
/// vậy. Đủ cho việc thường gặp: bỏ ký tự, đổi dấu phân cách.
fn check_replace(function: &Function) -> Result<(), String> {
    let pair = plain_args(function).and_then(|args| match args.as_slice() {
        [_, from, to] => Some((text(from)?, text(to)?)),
        _ => None,
    });
    match pair {
        Some((from, to)) if to.len() <= from.len() => Ok(()),
        _ => Err(
            "Hàm replace chỉ nhận chuỗi tìm và chuỗi thay viết thẳng, chuỗi thay không dài hơn chuỗi tìm".to_string(),
        ),
    }
}

/// Cho xóa (chuỗi thay rỗng) với mẫu bất kỳ, và thay bằng một byte (`' '`, `'-'`) khi mẫu chắc chắn không khớp chuỗi
/// rỗng: khi đó mỗi lần khớp chiếm ít nhất một byte và được thay bằng tối đa một byte, nên kết quả không dài hơn chuỗi
/// gốc, lồng bao nhiêu lần cũng vậy. Mẫu khớp được chuỗi rỗng (`''`, `x*`) thì khớp ở mọi vị trí giữa hai ký tự, thay
/// một ký tự cũng gần gấp đôi chuỗi mỗi lần lồng.
fn check_regexp_replace(function: &Function) -> Result<(), String> {
    let pair = plain_args(function).and_then(|args| match args.as_slice() {
        [_, pattern, replacement] | [_, pattern, replacement, _] => {
            Some((text(pattern)?, text(replacement)?))
        }
        _ => None,
    });
    match pair {
        Some((_, "")) => Ok(()),
        Some((pattern, replacement)) if replacement.len() == 1 && never_matches_empty(pattern) => {
            Ok(())
        }
        _ => Err(
            "Hàm regexp_replace chỉ nhận mẫu và chuỗi thay viết thẳng: chuỗi thay rỗng, hoặc một ký tự ASCII khi mẫu \
             không khớp được chuỗi rỗng (không dùng *, ?, {, |)"
                .to_string(),
        ),
    }
}

/// Mẫu không khớp được chuỗi rỗng, xét theo chữ. Không có `*`, `?`, `{` (lượng từ cho phép không lần nào), `|` (một
/// nhánh có thể rỗng) và `\Q` (đoạn nguyên văn, có thể rỗng) thì mọi phần của mẫu đều bắt buộc, nên mẫu chỉ khớp rỗng
/// khi mọi phần đều dài 0: neo (`^`, `$`, `\b`, `\B`, `\A`, `\z`, `\Z`), dấu nhóm, `+`. Có một phần chiếm ký tự (ký tự,
/// `.`, lớp `[...]`, `\s`, `\d`...) là đủ. Cách xét này từ chối nhầm vài mẫu vô hại (`a?b`), không nhận nhầm mẫu khớp
/// rỗng.
fn never_matches_empty(pattern: &str) -> bool {
    if pattern.contains(['*', '?', '{', '|']) || pattern.contains("\\Q") {
        return false;
    }
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '^' | '$' | '(' | ')' | '+' => {}
            '\\' => match chars.next() {
                Some('b' | 'B' | 'A' | 'z' | 'Z') => {}
                Some(_) => return true,
                None => return false,
            },
            _ => return true,
        }
    }
    false
}

/// Các tham số khi hàm gọi dạng thường `f(a, b, c)`; None khi có tham số đặt tên hay mệnh đề trong ngoặc, để luật ở
/// trên từ chối thay vì đoán.
pub(super) fn plain_args(function: &Function) -> Option<Vec<&Expr>> {
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

pub(super) fn text(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Value(v) => match &v.value {
            Value::SingleQuotedString(s) => Some(s.as_str()),
            _ => None,
        },
        _ => None,
    }
}
