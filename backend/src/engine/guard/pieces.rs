// SPDX-License-Identifier: Apache-2.0
/* engine/guard/pieces.rs - giới hạn tổng số mảnh được ghép lại trong cả câu: toán hạng của `||`, tham số của concat,
concat_ws, phần tử của danh sách, struct, map viết thẳng (`[a, b]`, `{'k': a}`, `MAP {...}`, `(a, b)`), vì đổi chúng
sang VARCHAR cũng là ghép. build_guarded_sql (mod.rs) gọi check_concat_pieces cùng lúc với check_query_depth.

Vì sao: mỗi tầng câu (guard/depth.rs) ghép được giá trị của tầng dưới nhiều lần, nên độ dài nhân lên qua các tầng. Với
tổng số mảnh cố định S, tích các hệ số nhân lớn nhất khi chia đều; 20 mảnh qua 3 tầng ghép (4 tầng câu) cho tối đa
7 x 7 x 6 = 294 lần chuỗi nguồn dài nhất, cỡ 1000 byte (guard/width.rs). Đo qua guard: chuỗi 294 KB, RSS đỉnh 66 MB.
Bảng số đo đủ các ca ở docs/GUARD.md, mục Số tầng, mảnh ghép, độ rộng.

Cách đếm: `a || b || c` là 3 mảnh; concat(a, b) là 2; concat_ws(sep, a, b, c) là 3 khi sep viết thẳng, 5 khi sep là biểu
thức (sep được ghép vào giữa mỗi cặp, nên cũng nhân giá trị lên). Ngoặc quanh một vế `(a || b) || c` làm đếm dư một,
không đếm thiếu. */

use std::ops::ControlFlow;

use sqlparser::ast::{BinaryOperator, Expr, Function, FunctionArguments, Query, Visit, Visitor};

use super::sizes::{plain_args, text};
use super::{name_key, single_ident};

/// Tổng số mảnh ghép tối đa trong một câu.
pub const MAX_CONCAT_PIECES: usize = 20;

/// Err là lý do từ chối (tiếng Việt).
pub(super) fn check_concat_pieces(query: &Query) -> Result<(), String> {
    let mut counter = PieceCounter { pieces: 0 };
    let _ = query.visit(&mut counter);
    if counter.pieces <= MAX_CONCAT_PIECES {
        return Ok(());
    }
    Err(format!(
        "Câu SQL ghép {} mảnh (toán hạng của ||, tham số của concat, concat_ws, phần tử danh sách hay struct), tối đa \
         {MAX_CONCAT_PIECES}: viết lại ghép ít hơn",
        counter.pieces
    ))
}

struct PieceCounter {
    pieces: usize,
}

impl Visitor for PieceCounter {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        self.pieces += match expr {
            // Chuỗi `a || b || c` là hai nút lồng nhau; mỗi nút chỉ đếm vế không phải nút `||` khác.
            Expr::BinaryOp {
                left,
                op: BinaryOperator::StringConcat,
                right,
            } => usize::from(!is_concat(left)) + usize::from(!is_concat(right)),
            Expr::Function(function) => function_pieces(function),
            Expr::Array(array) => array.elem.len(),
            Expr::Tuple(items) => items.len(),
            Expr::Struct { values, .. } => values.len(),
            Expr::Dictionary(fields) => fields.len(),
            Expr::Map(map) => map.entries.len() * 2,
            _ => 0,
        };
        ControlFlow::Continue(())
    }
}

fn is_concat(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::BinaryOp {
            op: BinaryOperator::StringConcat,
            ..
        }
    )
}

/// Đếm theo số tham số viết ra, kể cả dạng không thường (có tên, có mệnh đề): DuckDB không nhận concat dạng đó, nhưng
/// đếm vẫn không thiếu.
fn function_pieces(function: &Function) -> usize {
    let Some(name) = single_ident(&function.name).map(name_key) else {
        return 0;
    };
    let count = match &function.args {
        FunctionArguments::List(list) => list.args.len(),
        _ => 0,
    };
    match name.as_str() {
        "concat" => count,
        "concat_ws" => {
            let values = count.saturating_sub(1);
            let separator_is_text = plain_args(function)
                .and_then(|args| args.first().and_then(|e| text(e)))
                .is_some();
            if separator_is_text {
                values
            } else {
                (2 * values).saturating_sub(1)
            }
        }
        _ => 0,
    }
}
