// SPDX-License-Identifier: Apache-2.0
/* service/explain/number.rs - cách viết số trong lời diễn giải của câu "tại sao" (#38): kiểu Việt Nam như bảng phân rã
trên giao diện (cellFormat.ts), nhóm nghìn bằng dấu chấm, phần thập phân sau dấu phẩy, tối đa hai chữ số lẻ. explain/mod.rs
dùng cùng một hàm cho prompt narrate, tập số được phép của bước kiểm và câu khuôn, nên số mô hình chép từ prompt khớp
đúng chuỗi bước kiểm chờ. Hàm thuần, test ở ask/narrate_tests.rs.

Số của hợp đồng (Explanation) vẫn là chuỗi chữ số thô ("-1152666.24"): giao diện tự định dạng, CSV và drill dùng số
đủ chữ số. Chỉ chữ cho người đọc mới đi qua file này. */

use rust_decimal::{Decimal, RoundingStrategy};

/// Số chữ số lẻ tối đa khi viết số trong câu, như cột số của bảng kết quả (cellFormat.ts). Số có ít chữ số lẻ hơn giữ
/// nguyên số chữ số của nó ("663000.00" thành "663.000,00", "5000" thành "5.000"), để câu và bảng phân rã in cùng một
/// kiểu.
pub const MAX_FRACTION_DIGITS: u32 = 2;

/// "-1152666.24" thành "-1.152.666,24". Hơn hai chữ số lẻ thì làm tròn nửa xa số 0 (2.495 thành 2,50), như
/// Intl.NumberFormat của giao diện. Chuỗi không đọc được thành số thì giữ nguyên, để câu vẫn có chữ của bảng.
pub fn build_number_text(text: &str) -> String {
    build_number_text_with(text, MAX_FRACTION_DIGITS)
}

/// Như build_number_text với số chữ số lẻ tối đa do bên gọi chọn: độ đo tỷ lệ giữ đủ RATE_SCALE chữ số
/// (explain/ratio.rs), vì làm tròn hai chữ số thì tỷ lệ 0.0014 thành "0,00".
pub fn build_number_text_with(text: &str, max_fraction_digits: u32) -> String {
    let Ok(value) = text.parse::<Decimal>() else {
        return text.to_string();
    };
    let scale = value.scale().min(max_fraction_digits);
    let rounded = value.round_dp_with_strategy(scale, RoundingStrategy::MidpointAwayFromZero);
    // -0.001 làm tròn còn -0.00: không in dấu trừ cho số 0.
    let sign = if rounded.is_sign_negative() && !rounded.is_zero() {
        "-"
    } else {
        ""
    };
    let digits = rounded.abs().to_string();
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits.as_str(), None),
    };
    let grouped = build_grouped(whole);
    match fraction {
        Some(fraction) => format!("{sign}{grouped},{fraction}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Một chênh lệch đọc tự nhiên: chữ chiều ("tăng", "giảm", "không đổi") và độ lớn không dấu, để câu nói "giảm
/// 1.152.666,24" thay vì "-1.152.666,24". Chuỗi không đọc được thì chiều là None và độ lớn giữ nguyên chữ.
/// `max_fraction_digits` như build_number_text_with.
pub fn build_change_parts(text: &str, max_fraction_digits: u32) -> (Option<&'static str>, String) {
    let direction = match text.parse::<Decimal>() {
        Ok(value) if value.is_zero() => Some("không đổi"),
        Ok(value) if value < Decimal::ZERO => Some("giảm"),
        Ok(_) => Some("tăng"),
        Err(_) => None,
    };
    let number = build_number_text_with(text, max_fraction_digits);
    let amount = number.strip_prefix('-').unwrap_or(&number).to_string();
    (direction, amount)
}

// "1152666" thành "1.152.666": chèn dấu chấm trước mỗi nhóm ba chữ số tính từ bên phải.
fn build_grouped(whole: &str) -> String {
    let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(digit);
    }
    grouped
}

/// "12.340.831,43" thành "khoảng 12,3 triệu": số viết kiểu Việt Nam từ một triệu trở lên, ba chữ số có nghĩa, đơn vị
/// triệu hay tỷ. None khi chuỗi không phải số như vậy (số nhỏ hơn, chữ, số viết kiểu máy).
pub fn build_readable_number(token: &str) -> Option<String> {
    let (whole, fraction) = token.split_once(',').unwrap_or((token, ""));
    let groups: Vec<&str> = whole.split('.').collect();
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    let grouped = groups.len() >= 3
        && (1..=3).contains(&groups[0].len())
        && groups[1..].iter().all(|g| g.len() == 3)
        && groups.iter().all(|g| digits(g))
        && digits(fraction);
    if !grouped {
        return None;
    }
    let value: Decimal = format!(
        "{}.{}",
        groups.concat(),
        if fraction.is_empty() { "0" } else { fraction }
    )
    .parse()
    .ok()?;
    let billion = Decimal::from(1_000_000_000);
    let (scaled, unit) = if value >= billion {
        (value / billion, "tỷ")
    } else {
        (value / Decimal::from(1_000_000), "triệu")
    };
    let places = if scaled >= Decimal::from(100) {
        0
    } else if scaled >= Decimal::from(10) {
        1
    } else {
        2
    };
    let rounded = scaled
        .round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    Some(format!(
        "khoảng {} {unit}",
        rounded.to_string().replace('.', ",")
    ))
}

/// Mỗi số từ một triệu trở lên trong câu viết thêm dạng rút gọn đứng trước, số đủ trong ngoặc: "đạt khoảng 12,3 triệu
/// (12.340.831,43)". Người đọc thấy ngay độ lớn mà vẫn đối chiếu được với bảng. Số có dấu trừ liền trước thì giữ nguyên,
/// để không thành "-khoảng ..."; câu của code nói chiều bằng chữ nên hiếm gặp. Số đang nằm trong ngoặc cũng giữ nguyên,
/// để không ra ngoặc lồng ngoặc ("Africa (2.294.452,93)"); nhờ vậy gọi hai lần trên cùng câu vẫn ra một câu.
pub fn apply_readable_numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let joins = |c: char| c.is_ascii_digit() || c == '.' || c == ',';
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let starts =
            chars[i].is_ascii_digit() && (i == 0 || !(joins(chars[i - 1]) || chars[i - 1] == '-'));
        if !starts {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let mut end = i;
        while end < chars.len() && joins(chars[end]) {
            end += 1;
        }
        // Dấu chấm hay dấu phẩy cuối là dấu câu, không thuộc số.
        while end > i && !chars[end - 1].is_ascii_digit() {
            end -= 1;
        }
        let token: String = chars[i..end].iter().collect();
        match build_readable_number(&token).filter(|_| depth == 0) {
            Some(short) => out.push_str(&format!("{short} ({token})")),
            None => out.push_str(&token),
        }
        i = end;
    }
    out
}
