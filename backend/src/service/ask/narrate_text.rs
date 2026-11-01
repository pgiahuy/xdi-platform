// SPDX-License-Identifier: Apache-2.0
/* service/ask/narrate_text.rs - sửa chữ của lời mô hình viết cho câu hỏi nguyên nhân (ask/narrate.rs) trước khi hiện, các
lỗi đã gặp với Gemini trên database mẫu (01/11): nhãn cột lặp ("Nhóm tuổi nhóm tuổi 0-35", "Vùng vùng Scotland"), đầu ý
viết thường ("loại bài đánh giá CMA: ..."), và "cao hơn mức chênh 3,88 điểm phần trăm". Chỉ bỏ chữ thừa hay viết hoa
chữ đầu: không đổi số, chiều hay tên nhóm, nên bước kiểm số đã chạy trên khẳng định vẫn đúng với lời sau khi sửa. Hàm
thuần, test ở ask/narrate_tests.rs. */

/// Lời đã sửa: bỏ nhãn cột lặp liền nhau, bỏ "mức chênh" đứng ngay sau "cao hơn", "thấp hơn". labels là nhãn cột của
/// bảng phân rã (DimensionBreakdown.label) và của cột hai nhóm đang so.
pub fn build_tidy_text(text: &str, labels: &[&str]) -> String {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    let before = words.len();
    for label in labels {
        let label: Vec<String> = label.split_whitespace().map(fold).collect();
        if !label.is_empty() {
            drop_repeats(&mut words, &label);
        }
    }
    drop_gap_word(&mut words);
    if words.len() == before {
        return text.to_string();
    }
    words.join(" ")
}

/// Viết hoa chữ đầu (đầu ý, câu mở đầu): "loại bài đánh giá CMA" thành "Loại bài đánh giá CMA".
pub fn upper_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// Chữ thường, bỏ dấu câu dính hai đầu, để "vùng" khớp "Vùng" và "tuổi," khớp "tuổi".
fn fold(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

// Nhãn đứng hai lần liền nhau thì bỏ lần sau: "Nhóm tuổi nhóm tuổi 0-35" thành "Nhóm tuổi 0-35". Lần đầu giữ nguyên chữ
// hoa của nó. Chỉ bỏ khi chữ cuối của lần đầu không dính dấu câu ("vùng, vùng" là hai ý, không phải lặp).
fn drop_repeats(words: &mut Vec<&str>, label: &[String]) {
    let n = label.len();
    let mut i = 0;
    while i + 2 * n <= words.len() {
        let same = |start: usize| (0..n).all(|k| fold(words[start + k]) == label[k]);
        let joined = words[i + n - 1]
            .chars()
            .last()
            .is_some_and(char::is_alphanumeric);
        if joined && same(i) && same(i + n) {
            words.drain(i + n..i + 2 * n);
        } else {
            i += 1;
        }
    }
}

// "cao hơn mức chênh 3,88" thành "cao hơn 3,88": "mức chênh" là tên của khoảng cách, không đi sau chữ chiều.
fn drop_gap_word(words: &mut Vec<&str>) {
    let mut i = 0;
    while i + 3 < words.len() {
        let direction =
            matches!(fold(words[i]).as_str(), "cao" | "thấp") && fold(words[i + 1]) == "hơn";
        // "chênh" so nguyên chữ: dính dấu câu ("mức chênh.") thì bỏ đi là mất dấu câu, nên giữ nguyên.
        let gap = fold(words[i + 2]) == "mức" && words[i + 3] == "chênh";
        if direction && gap {
            words.drain(i + 2..i + 4);
        }
        i += 1;
    }
}
