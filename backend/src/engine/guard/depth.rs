// SPDX-License-Identifier: Apache-2.0
/* engine/guard/depth.rs - giới hạn số tầng câu lồng nhau: câu con ở FROM, WHERE, SELECT, và bảng tạm (WITH), kể cả bảng
tạm đọc từ bảng tạm khác. build_guarded_sql (mod.rs) gọi check_query_depth sau khi kiểm tên.

Vì sao: mỗi tầng ghép được giá trị của tầng dưới nhiều lần (`concat(v, v, ...)`, `v || v`), nên độ dài chuỗi nhân lên
theo từng tầng, và DuckDB tính các chuỗi hằng như vậy lúc chuẩn bị câu, khi giới hạn thời gian chưa có tác dụng. Đo qua
guard khi chỉ có luật này (chạy bằng run_query, giới hạn 1 giây): 3 tầng với câu 11,6 KB đã cho chuỗi 625 MB, RSS đỉnh
674 MB. Trần 4 tầng đủ cho câu hỏi thật (bảng tạm kèm câu con) chỉ an toàn cùng ba luật khác: tối đa 20 mảnh ghép
(guard/pieces.rs), chuỗi hằng tối đa 1000 byte và 200 cột mỗi SELECT (guard/width.rs); bảng số đo của cả bốn luật ở
docs/GUARD.md, mục Số tầng, mảnh ghép, độ rộng. Câu sâu hơn bị từ chối với lý do để mô hình viết lại gọn hơn trong vòng
sửa (GuardError::Oversized). */

use std::collections::HashMap;
use std::ops::ControlFlow;

use sqlparser::ast::{Query, TableFactor, Visit, Visitor};

use super::{name_key, single_ident};

/// Số tầng câu tối đa, tính cả câu ngoài cùng.
pub const MAX_QUERY_DEPTH: usize = 4;

/// Err là lý do từ chối (tiếng Việt).
pub(super) fn check_query_depth(query: &Query) -> Result<(), String> {
    let mut counter = DepthCounter::default();
    let _ = query.visit(&mut counter);
    if counter.deepest <= MAX_QUERY_DEPTH {
        return Ok(());
    }
    Err(format!(
        "Câu SQL lồng {} tầng (tính cả câu con và bảng tạm WITH), tối đa {MAX_QUERY_DEPTH}: viết lại gọn hơn, ví dụ gộp \
         bảng tạm hay câu con vào câu chính",
        counter.deepest
    ))
}

/// Số tầng của một câu là 1 cộng số tầng lớn nhất của các câu con bên trong nó. Đọc từ bảng tạm tính như có câu con là
/// thân bảng tạm đó, vì chuỗi `WITH a AS (...), b AS (SELECT ... FROM a)` lồng nhau như câu con mà không viết lồng.
#[derive(Default)]
struct DepthCounter {
    /// Một mục cho mỗi câu đang duyệt dở: số tầng lớn nhất của các câu con đã gặp bên trong nó.
    open: Vec<usize>,
    /// Thân các bảng tạm đã gặp, theo địa chỉ trong cây, kèm tên viết thường. Chỉ so địa chỉ, không đọc qua con trỏ.
    cte_bodies: Vec<(*const Query, String)>,
    /// Số tầng của thân từng bảng tạm đã duyệt xong.
    cte_depths: HashMap<String, usize>,
    /// Số tầng của câu ngoài cùng, có khi câu đó duyệt xong.
    deepest: usize,
}

impl DepthCounter {
    fn add_inner(&mut self, depth: usize) {
        if let Some(current) = self.open.last_mut() {
            *current = (*current).max(depth);
        }
    }
}

impl Visitor for DepthCounter {
    type Break = ();

    // Visitor duyệt phần WITH trước thân câu, nên bảng tạm đã có số tầng khi thân câu (hay bảng tạm sau) đọc tới nó.
    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        for cte in query.with.iter().flat_map(|with| &with.cte_tables) {
            let body: *const Query = &*cte.query;
            self.cte_bodies.push((body, name_key(&cte.alias.name)));
        }
        self.open.push(0);
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        let depth = self.open.pop().unwrap_or(0) + 1;
        let address: *const Query = query;
        let cte = self.cte_bodies.iter().find(|(body, _)| *body == address);
        if let Some((_, name)) = cte {
            self.cte_depths.insert(name.clone(), depth);
        }
        if self.open.is_empty() {
            self.deepest = depth;
        } else {
            self.add_inner(depth);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        if let TableFactor::Table { name, .. } = factor {
            let depth = single_ident(name).and_then(|ident| self.cte_depths.get(&name_key(ident)));
            if let Some(&depth) = depth {
                self.add_inner(depth);
            }
        }
        ControlFlow::Continue(())
    }
}
