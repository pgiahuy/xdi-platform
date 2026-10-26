// SPDX-License-Identifier: Apache-2.0
/* service/ask_chart.rs - gợi ý biểu đồ cho kết quả của câu hỏi (AskResponse.chart), theo luật ở ARCHITECTURE mục 6:
một chiều một độ đo thì cột, có thời gian thì đường. build_answer_view (ask_view.rs) gọi build_chart với vai trò cột
của build_column_roles (understanding.rs); giao diện vẽ theo gợi ý ở frontend/src/components/ask/ResultChart.tsx,
không có gợi ý thì chỉ hiện bảng.

Luật đọc hai nguồn. Vai trò cột (time, dimension, measure) lấy từ cây cú pháp của câu đã chạy, nên biết cột nào là
khóa nhóm kể cả khi khóa là số: năm, tháng dạng số nguyên (year(), EXTRACT) là time, mã sản phẩm dạng số là dimension.
Kiểu cột (ValueKind) và giá trị trục x bù chỗ vai trò không thấy: cột chữ mà mọi giá trị có dạng YYYY-MM hay YYYY-MM-DD
là thời gian kể cả khi vai trò là dimension, vì mô hình hay gộp tháng bằng strftime(..., '%Y-%m') (thấy khi chạy thử
với Gemini trên DataCo), và hồ sơ của cột nguồn có thể không ghi vai trò time. Câu không đọc được vai trò (mọi cột là
other, ví dụ câu không GROUP BY) thì chỉ theo kiểu cột: cột số là độ đo, cột chữ là chiều, cột ngày là thời gian.

Không đoán theo tên cột: tên do mô hình đặt nên đoán theo tên dễ sai. Không chắc thì trả None: bảng luôn có đủ số,
một biểu đồ sai dạng làm người đọc hiểu sai. */

use super::ask_contract::{
    CellValue, ChartKind, ChartSuggestion, ColumnRole, ResultTable, ValueKind,
};

/// Số nhóm tối đa của biểu đồ cột. Khung biểu đồ của giao diện cao cố định và rộng theo màn; quá khoảng 20 cột thì ở
/// bề ngang điện thoại mỗi cột chỉ còn vài pixel và trục x bỏ bớt nhãn (interval preserveStartEnd), người đọc không
/// biết cột nào là nhóm nào. Nhiều nhóm hơn thì bảng dễ đọc hơn.
const MAX_BAR_GROUPS: u64 = 20;

/// Phần một cột góp vào biểu đồ.
#[derive(PartialEq)]
enum Part {
    Time,
    Dimension,
    Measure,
    /// Cột không phải khóa cũng không phải độ đo (ví dụ upper(market) bên cạnh khóa market): không vẽ, không chặn.
    Skip,
}

/// Gợi ý biểu đồ cho một kết quả, hoặc None khi chỉ nên hiện bảng. `roles` theo thứ tự cột của result.
/// - đúng một cột thời gian và ít nhất một độ đo: đường của độ đo đầu, khi các mốc tăng dần và kết quả đủ dòng;
/// - đúng một cột chiều và đúng một độ đo: cột, khi số nhóm không quá MAX_BAR_GROUPS.
///
/// Đường chỉ vẽ độ đo đầu và cột cần đúng một độ đo: các độ đo thường khác đơn vị (doanh thu và số đơn), vẽ chung một
/// trục thì độ đo nhỏ thành đường phẳng. Biểu đồ cột không vẽ riêng độ đo đầu như đường, vì người đọc biểu đồ cột sẽ
/// nghĩ các độ đo kia bị bỏ sót. Mọi trường hợp khác là None: dưới 2 dòng, không có độ đo, hai khóa trở lên (bảng
/// chéo, hay chiều cộng thời gian cần nhiều đường), có cột boolean hay other mà không biết vai trò.
pub fn build_chart(result: &ResultTable, roles: &[ColumnRole]) -> Option<ChartSuggestion> {
    // Một dòng là một con số, không có gì để so trên biểu đồ.
    if result.total_rows < 2 {
        return None;
    }
    let roles_known = roles.iter().any(|role| *role != ColumnRole::Other);
    let mut parts = Vec::with_capacity(result.columns.len());
    for index in 0..result.columns.len() {
        let role = roles.get(index).copied().unwrap_or(ColumnRole::Other);
        parts.push(build_part(result, index, role, roles_known)?);
    }
    let axes: Vec<usize> = (0..parts.len())
        .filter(|&i| matches!(parts[i], Part::Time | Part::Dimension))
        .collect();
    let measures: Vec<usize> = (0..parts.len())
        .filter(|&i| parts[i] == Part::Measure)
        .collect();
    let ([x_index], [first_measure, ..]) = (axes.as_slice(), measures.as_slice()) else {
        return None;
    };
    let kind = match parts[*x_index] {
        Part::Time if is_time_line(result, *x_index) => ChartKind::Line,
        Part::Dimension if measures.len() == 1 && result.total_rows <= MAX_BAR_GROUPS => {
            ChartKind::Bar
        }
        // Mốc thời gian không vẽ được thành đường thì cũng không đổi sang cột: cột của các mốc lộn xộn vẫn đọc như
        // một chuỗi thời gian.
        _ => return None,
    };
    Some(ChartSuggestion {
        kind,
        x: result.columns[*x_index].name.clone(),
        y: vec![result.columns[*first_measure].name.clone()],
    })
}

/// None khi cột làm cả kết quả không vẽ được.
fn build_part(
    result: &ResultTable,
    index: usize,
    role: ColumnRole,
    roles_known: bool,
) -> Option<Part> {
    let kind = result.columns[index].kind;
    let numeric = matches!(kind, ValueKind::Number | ValueKind::Decimal);
    match role {
        // Độ đo không phải số (MIN của một cột chữ) không vẽ được; bỏ nó đi thì người đọc tưởng câu chỉ đo các cột kia.
        ColumnRole::Measure => numeric.then_some(Part::Measure),
        ColumnRole::Other if roles_known => Some(Part::Skip),
        // Khóa boolean hay kiểu other không đặt được lên trục.
        _ if matches!(kind, ValueKind::Boolean | ValueKind::Other) => None,
        // Thứ tự của mốc (chữ "2017-Q1", số 2017, ngày) kiểm sau ở is_time_line.
        ColumnRole::Time => Some(Part::Time),
        ColumnRole::Other if numeric => Some(Part::Measure),
        ColumnRole::Dimension | ColumnRole::Other => Some(build_key_part(result, index)),
    }
}

/// Khóa là thời gian khi kiểu cột là ngày, giờ, hoặc cột chữ dạng YYYY-MM, YYYY-MM-DD. Năm đứng riêng ("2017") hay
/// quý ("2017-Q1") dạng chữ thì không, khi vai trò không nói là time: bốn chữ số cũng có thể là mã, còn quý mỗi câu
/// SQL viết một kiểu. Khóa dạng số mà vai trò là dimension (mã sản phẩm) thì là chiều.
fn build_key_part(result: &ResultTable, index: usize) -> Part {
    match result.columns[index].kind {
        ValueKind::Date | ValueKind::Timestamp => Part::Time,
        ValueKind::Text if is_text_time(result, index) => Part::Time,
        _ => Part::Dimension,
    }
}

/// Cột chữ mà mọi ô có dạng YYYY-MM hoặc YYYY-MM-DD, tháng từ 01 tới 12.
fn is_text_time(result: &ResultTable, x_index: usize) -> bool {
    result.rows.iter().all(|row| match row.get(x_index) {
        Some(Some(CellValue::String(text))) => is_month_or_date(text),
        _ => false,
    })
}

fn is_month_or_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    // Vị trí của dấu gạch trong "2017-01" và "2017-01-31"; mọi vị trí khác là chữ số.
    let dashes: &[usize] = match bytes.len() {
        7 => &[4],
        10 => &[4, 7],
        _ => return false,
    };
    let shape_ok = bytes.iter().enumerate().all(|(i, b)| {
        if dashes.contains(&i) {
            *b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    // Đã chắc mọi byte là ASCII nên cắt chuỗi theo byte không rơi giữa một ký tự.
    shape_ok && matches!(text[5..7].parse::<u8>(), Ok(1..=12))
}

/// Đường chỉ đúng khi có đủ dòng và các mốc tăng dần. Thiếu dòng (truncated) thì đường dừng giữa chừng mà trông như
/// hết dữ liệu. Giao diện nối điểm theo thứ tự dòng, không tự sắp: câu SQL sắp giảm dần hay không sắp sẽ vẽ ra đường
/// đi ngược hay rối; mốc trống hoặc trùng cũng không đặt được lên trục.
fn is_time_line(result: &ResultTable, x_index: usize) -> bool {
    if result.truncated {
        return false;
    }
    let cells = result
        .rows
        .iter()
        .map(|row| row.get(x_index).and_then(Option::as_ref));
    match result.columns[x_index].kind {
        // Năm, tháng dạng số: BIGINT gửi dạng chuỗi chữ số (ARCHITECTURE mục 8), so chuỗi thì "10" đứng trước "9",
        // nên đổi sang số rồi mới so.
        ValueKind::Number | ValueKind::Decimal => {
            let marks: Option<Vec<f64>> = cells
                .map(|cell| match cell {
                    Some(CellValue::Number(number)) => Some(*number),
                    Some(CellValue::String(text)) => text.parse().ok(),
                    _ => None,
                })
                .collect();
            marks.is_some_and(|marks| is_ascending(&marks))
        }
        // Ngày và giờ gửi dạng chuỗi ISO 8601 cùng một dạng trong một cột, nên so chuỗi là so thời gian. Năm ngoài
        // 0000 đến 9999 (dạng có dấu +, -) so chuỗi sai thứ tự, nhưng chỉ làm luật trả None, không vẽ sai.
        _ => {
            let marks: Option<Vec<&str>> = cells
                .map(|cell| match cell {
                    Some(CellValue::String(text)) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            marks.is_some_and(|marks| is_ascending(&marks))
        }
    }
}

/// Tăng ngặt: mốc trùng không đặt được lên trục. NaN so với gì cũng sai nên làm hàm trả false.
fn is_ascending<T: PartialOrd>(marks: &[T]) -> bool {
    marks.windows(2).all(|pair| pair[0] < pair[1])
}
