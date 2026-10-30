// SPDX-License-Identifier: Apache-2.0
/* service/eval_grade.rs - so bảng kết quả với bảng của SQL đáp án (#37, #202), thuần, không cần Postgres hay DuckDB.
service/eval.rs gọi khi chấm từng câu, service/eval_check.rs gọi khi so đáp án với check_sql.

Luật so theo #202:
- Bỏ qua tên cột và vị trí cột: mỗi cột của đáp án ghép với một cột của kết quả có cùng giá trị. Cột thừa vẫn đạt nhưng
  được ghi lại; có required_columns thì chỉ các cột đó của đáp án bắt buộc có.
- Cùng số dòng; không xét thứ tự dòng trừ khi ordered.
- Số dạng chữ đổi về số trước khi so: BIGINT và DECIMAL tới đây là chuỗi (build_result_table giữ đủ chữ số), nên
  thiếu bước này thì SUM trên cột nguyên khác kiểu với SUM trên cột thực và không bao giờ khớp.
- Số không nguyên so sau khi làm tròn 4 chữ số có nghĩa, để đáp án tỷ lệ chính xác khớp kết quả mô hình đã
  ROUND(x, 4). Hai số nguyên so đúng từng đơn vị, vì đếm lệch một dòng là sai chứ không phải sai số làm tròn. */

use std::cmp::Ordering;

use crate::service::ask::contract::CellValue;

/// Số chữ số có nghĩa khi so hai số không nguyên.
const SIGNIFICANT_DIGITS: i32 = 4;

type Rows = [Vec<Option<CellValue>>];

/// Một ô kết quả đưa về dạng so được.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Null,
    Num(f64),
    Str(String),
    Bool(bool),
}

fn build_cell(cell: &Option<CellValue>) -> Cell {
    match cell {
        None => Cell::Null,
        Some(CellValue::Number(n)) => Cell::Num(*n),
        // Chỉ số hữu hạn: parse nhận cả "inf" và "NaN", mà trong dữ liệu đó là chữ.
        Some(CellValue::String(s)) => match s.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => Cell::Num(n),
            _ => Cell::Str(s.clone()),
        },
        Some(CellValue::Boolean(b)) => Cell::Bool(*b),
    }
}

fn build_table(rows: &Rows) -> Vec<Vec<Cell>> {
    rows.iter()
        .map(|row| row.iter().map(build_cell).collect())
        .collect()
}

fn round_significant(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let scale = 10f64.powi(SIGNIFICANT_DIGITS - 1 - x.abs().log10().floor() as i32);
    (x * scale).round() / scale
}

fn numbers_match(a: f64, e: f64) -> bool {
    // Sai số dấu phẩy động của cùng một phép tính viết theo hai cách, ví dụ SUM theo thứ tự dòng khác nhau.
    if (a - e).abs() <= 1e-9 * e.abs().max(1.0) {
        return true;
    }
    if a.fract() == 0.0 && e.fract() == 0.0 {
        return false;
    }
    round_significant(a) == round_significant(e)
}

fn cells_match(a: &Cell, e: &Cell) -> bool {
    match (a, e) {
        (Cell::Num(x), Cell::Num(y)) => numbers_match(*x, *y),
        _ => a == e,
    }
}

/// Thứ tự để sắp hai phía trước khi so ở chế độ không thứ tự: theo loại rồi theo giá trị. Số sắp theo giá trị
/// chính xác, không theo giá trị làm tròn: hai số nguyên gần nhau mà sắp ngang hàng thì giữ thứ tự vào, hai phía
/// có thể lệch thứ tự rồi so sai.
fn compare_cells(a: &Cell, b: &Cell) -> Ordering {
    let rank = |c: &Cell| match c {
        Cell::Null => 0,
        Cell::Bool(_) => 1,
        Cell::Num(_) => 2,
        Cell::Str(_) => 3,
    };
    match (a, b) {
        (Cell::Num(x), Cell::Num(y)) => x.total_cmp(y),
        (Cell::Str(x), Cell::Str(y)) => x.cmp(y),
        (Cell::Bool(x), Cell::Bool(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

fn compare_rows(a: &[Cell], b: &[Cell]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(x, y)| compare_cells(x, y))
        .find(|o| o.is_ne())
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

fn project(table: &[Vec<Cell>], columns: &[usize], ordered: bool) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = table
        .iter()
        .map(|row| columns.iter().map(|&c| row[c].clone()).collect())
        .collect();
    if !ordered {
        rows.sort_by(|a, b| compare_rows(a, b));
    }
    rows
}

/// So cột actual_cols của kết quả với cột expected_cols của đáp án, cột thứ k ứng với cột thứ k.
fn compare_projected(
    actual: &[Vec<Cell>],
    actual_cols: &[usize],
    expected: &[Vec<Cell>],
    expected_cols: &[usize],
    ordered: bool,
) -> Result<(), String> {
    let left = project(actual, actual_cols, ordered);
    let right = project(expected, expected_cols, ordered);
    for (r, (a, e)) in left.iter().zip(&right).enumerate() {
        for (k, (a, e)) in a.iter().zip(e).enumerate() {
            if !cells_match(a, e) {
                return Err(format!(
                    "dòng {r} cột {} lệch: {a:?} thay vì {e:?}",
                    expected_cols[k]
                ));
            }
        }
    }
    Ok(())
}

/// Ghép từng cột cần có của đáp án với một cột khác nhau của kết quả, thử lần lượt (số cột nhỏ nên thử hết được).
/// candidates[k] là các cột của kết quả có cùng giá trị với cột thứ k; ghép xong thì so cả dòng, vì từng cột khớp
/// riêng chưa chắc các dòng khớp.
fn find_mapping(
    candidates: &[Vec<usize>],
    chosen: &mut Vec<usize>,
    accept: &dyn Fn(&[usize]) -> bool,
) -> bool {
    let k = chosen.len();
    if k == candidates.len() {
        return accept(chosen);
    }
    for &c in &candidates[k] {
        if chosen.contains(&c) {
            continue;
        }
        chosen.push(c);
        if find_mapping(candidates, chosen, accept) {
            return true;
        }
        chosen.pop();
    }
    false
}

/// So bảng kết quả với bảng đáp án theo luật ở đầu file. Ok kèm ghi chú (khớp, thừa cột, cột khác thứ tự), Err kèm
/// chỗ lệch đầu tiên.
pub fn build_eval_verdict(
    actual: &Rows,
    expected: &Rows,
    ordered: bool,
    required_columns: &[usize],
) -> Result<String, String> {
    let actual = build_table(actual);
    let expected = build_table(expected);
    if actual.len() != expected.len() {
        return Err(format!(
            "số dòng khác: {} thay vì {}",
            actual.len(),
            expected.len()
        ));
    }
    if expected.is_empty() {
        return Ok("khớp (không dòng nào)".to_string());
    }
    let actual_width = actual[0].len();
    let expected_width = expected[0].len();
    let wanted: Vec<usize> = if required_columns.is_empty() {
        (0..expected_width).collect()
    } else {
        required_columns.to_vec()
    };
    if let Some(bad) = wanted.iter().find(|&&j| j >= expected_width) {
        return Err(format!(
            "required_columns có cột {bad}, đáp án chỉ có {expected_width} cột"
        ));
    }
    if actual_width < wanted.len() {
        return Err(format!(
            "thiếu cột: {actual_width} thay vì {}",
            wanted.len()
        ));
    }

    // Thử ghép đúng vị trí trước: đa số câu viết cột theo cùng thứ tự, và khi lệch thì lời báo chỉ đúng ô lệch.
    let identity_error = if actual_width == expected_width {
        match compare_projected(&actual, &wanted, &expected, &wanted, ordered) {
            Ok(()) => return Ok("khớp".to_string()),
            Err(why) => Some(why),
        }
    } else {
        None
    };

    let candidates: Vec<Vec<usize>> = wanted
        .iter()
        .map(|&j| {
            (0..actual_width)
                .filter(|&c| compare_projected(&actual, &[c], &expected, &[j], ordered).is_ok())
                .collect()
        })
        .collect();
    let accept =
        |chosen: &[usize]| compare_projected(&actual, chosen, &expected, &wanted, ordered).is_ok();
    let mut chosen = Vec::new();
    if find_mapping(&candidates, &mut chosen, &accept) {
        let extra = actual_width - wanted.len();
        return Ok(if extra > 0 {
            format!("khớp; thừa {extra} cột")
        } else {
            "khớp; cột khác thứ tự".to_string()
        });
    }
    if let Some(why) = identity_error {
        return Err(why);
    }
    match wanted.iter().zip(&candidates).find(|(_, c)| c.is_empty()) {
        Some((j, _)) => Err(format!("không cột nào của kết quả khớp cột {j} của đáp án")),
        None => Err("từng cột khớp nhưng các dòng không khớp".to_string()),
    }
}
