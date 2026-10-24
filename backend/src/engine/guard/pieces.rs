// SPDX-License-Identifier: Apache-2.0
/* engine/guard/pieces.rs - giới hạn tổng số mảnh được ghép lại trong cả câu: toán hạng của `||`, tham số của concat,
concat_ws, phần tử của danh sách, struct, map viết thẳng (`[a, b]`, `{'k': a}`, `MAP {...}`, `(a, b)`), vì đổi chúng
sang VARCHAR cũng là ghép. build_guarded_sql (mod.rs) gọi check_concat_pieces cùng lúc với check_query_depth.

Vì sao: mỗi tầng câu (guard/depth.rs) ghép được giá trị của tầng dưới nhiều lần, nên độ dài nhân lên qua các tầng. Với
tổng số mảnh cố định S, tích các hệ số nhân lớn nhất khi chia đều. 20 mảnh qua 4 tầng (tầng trong cùng ghép các chuỗi
hằng, mỗi chuỗi tối đa 1000 byte, guard/width.rs) cho tối đa 5 x 5 x 5 x 5 = 625 lần, tức chuỗi 625 KB; khi tầng ngoài
chép ra 200 cột thì 7 x 7 x 6 = 294 lần mỗi cột. Đo qua guard: 625 KB một cột, RSS đỉnh 67 MB; 200 cột 294 KB, RSS đỉnh
268 MB. Bảng số đo đủ các ca ở docs/GUARD.md, mục Số tầng, mảnh ghép, độ rộng.

Hai cách nhân không cần tầng câu cũng được đếm hay chặn: dùng lại bí danh trong cùng danh sách cột (alias_reuse_pieces
cộng mảnh của bí danh vào chỗ dùng), gọi hàm dạng phương thức `(a).concat(a)` (ExprChecker ở mod.rs từ chối, vì biểu
thức trước dấu chấm là tham số mà đếm ở đây không thấy).

Cách đếm: `a || b || c` là 3 mảnh; concat(a, b) là 2; concat_ws(sep, a, b, c) là 3 khi sep viết thẳng, 5 khi sep là biểu
thức (sep được ghép vào giữa mỗi cặp, nên cũng nhân giá trị lên). Ngoặc quanh một vế `(a || b) || c` làm đếm dư một,
không đếm thiếu. */

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArguments, Query, Select, SelectItem, Visit, Visitor,
};

use super::sizes::{plain_args, text};
use super::{name_key, single_ident};

/// Tổng số mảnh ghép tối đa trong một câu.
pub const MAX_CONCAT_PIECES: usize = 20;

/// Err là lý do từ chối (tiếng Việt).
pub(super) fn check_concat_pieces(query: &Query) -> Result<(), String> {
    let mut counter = PieceCounter::default();
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

#[derive(Default)]
struct PieceCounter {
    pieces: usize,
    /// Tuple và mảng ở chỗ chỉ để so sánh (`(a, b) IN ((..), (..))`, `= ANY(ARRAY[...])`): không thành chuỗi nên không
    /// đếm. Giữ theo địa chỉ trong cây, chỉ so địa chỉ.
    compared: HashSet<*const Expr>,
}

impl PieceCounter {
    fn mark_compared(&mut self, expr: &Expr) {
        self.compared.insert(expr);
        if let Expr::Nested(inner) = expr {
            self.compared.insert(&**inner);
        }
    }

    fn literal_pieces(&self, expr: &Expr, count: usize) -> usize {
        let address: *const Expr = expr;
        if self.compared.contains(&address) {
            0
        } else {
            count
        }
    }
}

impl Visitor for PieceCounter {
    type Break = ();

    // Visitor gặp nút IN, ANY trước các vế của nó, nên đánh dấu ở đây kịp trước khi gặp tuple, mảng bên trong.
    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        let pieces = match expr {
            // Chuỗi `a || b || c` là hai nút lồng nhau; mỗi nút chỉ đếm vế không phải nút `||` khác.
            Expr::BinaryOp {
                left,
                op: BinaryOperator::StringConcat,
                right,
            } => usize::from(!is_concat(left)) + usize::from(!is_concat(right)),
            Expr::Function(function) => function_pieces(function),
            Expr::InList {
                expr: left, list, ..
            } => {
                self.mark_compared(left);
                list.iter().for_each(|item| self.mark_compared(item));
                0
            }
            Expr::AnyOp { right, .. } | Expr::AllOp { right, .. } => {
                self.mark_compared(right);
                0
            }
            Expr::Array(array) => self.literal_pieces(expr, array.elem.len()),
            Expr::Tuple(items) => self.literal_pieces(expr, items.len()),
            Expr::Struct { values, .. } => values.len(),
            Expr::Dictionary(fields) => fields.len(),
            Expr::Map(map) => map.entries.len() * 2,
            _ => 0,
        };
        self.pieces = self.pieces.saturating_add(pieces);
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<()> {
        self.pieces = self.pieces.saturating_add(alias_reuse_pieces(select));
        ControlFlow::Continue(())
    }
}

/// DuckDB cho một cột dùng lại bí danh đặt trước nó trong cùng danh sách (`'ab' AS a0, a0 || a0 || a0 AS a1,
/// a1 || a1 || a1 AS a2`): mỗi lần dùng lại chép cả giá trị, nên nhân mà không cần tầng câu. Mỗi chỗ dùng lại cộng thêm
/// số mảnh của bí danh đó (kể cả phần nó dùng lại), nên chuỗi như trên vượt trần sau vài bước. Tên trùng cột của bảng
/// cũng bị cộng: đếm dư, không đếm thiếu.
fn alias_reuse_pieces(select: &Select) -> usize {
    let mut alias_pieces: HashMap<String, usize> = HashMap::new();
    let mut extra: usize = 0;
    for item in &select.projection {
        let (expr, alias) = match item {
            SelectItem::UnnamedExpr(expr) => (expr, None),
            SelectItem::ExprWithAlias { expr, alias } => (expr, Some(alias)),
            _ => continue,
        };
        let reused = identifiers(expr)
            .iter()
            .filter_map(|name| alias_pieces.get(name))
            .fold(0, |sum: usize, pieces| sum.saturating_add(*pieces));
        extra = extra.saturating_add(reused);
        if let Some(alias) = alias {
            let mut own = PieceCounter::default();
            let _ = expr.visit(&mut own);
            alias_pieces.insert(name_key(alias), own.pieces.saturating_add(reused));
        }
    }
    extra
}

/// Tên cột trơn (không kèm bảng) trong một biểu thức, viết thường, mỗi lần xuất hiện một mục.
fn identifiers(expr: &Expr) -> Vec<String> {
    struct Collect(Vec<String>);
    impl Visitor for Collect {
        type Break = ();
        fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
            if let Expr::Identifier(ident) = expr {
                self.0.push(name_key(ident));
            }
            ControlFlow::Continue(())
        }
    }
    let mut collect = Collect(Vec::new());
    let _ = expr.visit(&mut collect);
    collect.0
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
