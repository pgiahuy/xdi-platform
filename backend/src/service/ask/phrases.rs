// SPDX-License-Identifier: Apache-2.0
/* service/ask/phrases.rs - tách câu hỏi thành các cụm 1 đến 5 từ liền nhau, để service/ask/context.rs tìm cột mà câu
hỏi nhắc tới và tìm gợi ý giá trị khớp chính xác. Mỗi cụm giữ ba dạng: chữ người gõ (NFC, chữ thường) để hiển thị, dạng
so được (dấu thanh đặt một chỗ cố định, xem build_tone_placed) và dạng đã bỏ dấu.

Ánh xạ chữ người dùng sang giá trị ("miền Nam" thành South) là việc của mô hình, trong cùng lần sinh SQL (#29); ở đây
chỉ có khớp chính xác, và chỉ cho cụm từ hai âm tiết trở lên không chứa từ dừng (Phrase::is_hint_phrase). Luật khớp
gần và luật cho âm tiết đơn đã thử ba vòng, vòng nào cũng sinh khớp nhầm mới, vì nhiều âm tiết tiếng Việt vừa là từ
thường vừa là giá trị ("năm" và "Nam", "trung bình" và "Trung", "đồng" và "Dong").

Từ dừng (STOP_WORDS) so theo cách người hỏi gõ: câu có dấu ở bất kỳ đâu thì so với chữ có dấu, vì người gõ có dấu viết
"năm" cho thời gian, nên "nam" của họ là Nam; câu không dấu thì so sau bỏ dấu, "nam" là từ dừng vì không biết người hỏi
muốn chữ nào. */

use unicode_normalization::UnicodeNormalization;

/// Một cụm dài tối đa ngần này từ (một từ ở đây là một âm tiết, phần giữa hai dấu cách). Bí danh tiếng Việt hay dài 4, 5
/// âm tiết ("trạng thái giao hàng", "châu Á Thái Bình Dương" trong hồ sơ mẫu của DataCo).
pub const PHRASE_MAX_WORDS: usize = 5;

/// Chỉ xét ngần này từ đầu của câu hỏi, để số cụm có trần (khoảng 5 lần số từ).
pub const QUESTION_MAX_WORDS: usize = 40;

/// Từ dài hơn ngần này ký tự không vào cụm nào: không tên cột hay giá trị đã lưu nào có từ dài như vậy, còn một chuỗi
/// dán vào (100 000 ký tự liền) chỉ làm tốn công tách và so.
const WORD_MAX_CHARS: usize = 40;

/// Từ chỉ thời gian, số đếm, từ nối, từ hỏi, từ so sánh, viết có dấu. Chúng có trong câu hỏi nào cũng được mà không
/// trỏ tới giá trị nào. Không có "số" (như "doanh số"); "từng", "mọi" có vì "theo từng châu" là gom nhóm, không lọc.
/// Không có "giờ", "phút", "giây": hiếm khi có trong câu hỏi kinh doanh, mà gõ không dấu thì trùng "giỏ", "giày".
/// Các từ còn lại vẫn có thể trùng một từ thường khi gõ không dấu ("hai" với "Hải", "dau" với "dầu", "tai" với
/// "tài"): khi đó cụm có từ ấy không thành gợi ý; mô hình vẫn thấy danh sách giá trị và tự ánh xạ.
const STOP_WORDS: &[&str] = &[
    // Thời gian.
    "năm", "tháng", "quý", "ngày", "tuần", "hôm", "nay", "qua", "trước", "sau", "đầu", "cuối", "kỳ",
    "lúc", "khi", "nửa", "hiện", "tại", // Số đếm.
    "một", "hai", "ba", "bốn", "tư", "lăm", "sáu", "bảy", "tám", "chín", "mười", "mươi", "trăm",
    "nghìn", "ngàn", "triệu", "tỷ", "tỉ", // Từ nối, từ hỏi.
    "và", "với", "của", "theo", "ở", "trong", "ngoài", "cho", "các", "những", "mỗi", "mọi", "từng",
    "cả", "là", "có", "không", "nào", "bao", "nhiêu", "từ", "đến", "tới", "hay", "hoặc", "thì",
    "được", "bị", "như", "về", "trên", "dưới", "giữa", "ra", "vào", "bởi", "mà", "đó", "kia", "gì",
    "sao", "ai", "đang", "đã", "sẽ", "cần", "xem", "liệt", "kê", "biết",
    // So sánh, tổng hợp.
    "hơn", "nhất", "cao", "thấp", "lớn", "nhỏ", "tăng", "giảm", "top", "tổng",
    // Tiếng Anh.
    "the", "of", "and", "or", "by", "in", "on", "for", "to", "per", "vs", "is", "are", "an", "from",
    "with", "at", "what", "how", "which", "many", "much", "total", "sum", "count",
];

/// Một cụm từ liền nhau của câu hỏi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    /// Chữ người gõ: NFC, chữ thường, các từ cách nhau một dấu cách. Dùng để hiển thị.
    pub text: String,
    /// Như text, dấu thanh đặt một chỗ cố định ("hoà" và "hòa" đều thành "hoà"). Dùng để so.
    pub key: String,
    /// Như text, đã bỏ dấu.
    pub folded: String,
    /// Vị trí từ đầu tiên trong câu hỏi, tính từ 0.
    pub start: usize,
    /// Số từ.
    pub len: usize,
    /// Người hỏi gõ có dấu (key khác folded).
    pub accented: bool,
    /// Có từ dừng hoặc từ toàn chữ số.
    pub has_stop: bool,
}

impl Phrase {
    /// Hai cụm có chung ít nhất một từ của câu hỏi.
    pub fn overlaps(&self, other: &Phrase) -> bool {
        self.start < other.start + other.len && other.start < self.start + self.len
    }

    /// Cụm được dùng để tìm gợi ý giá trị khớp chính xác: từ hai âm tiết, không có từ dừng. Một âm tiết hay cụm có từ
    /// dừng quá dễ trùng chữ thường ("Trung" trong "trung bình", "miền năm" bỏ dấu thành "mien nam").
    pub fn is_hint_phrase(&self) -> bool {
        self.len >= 2 && !self.has_stop
    }
}

/// Loại của một từ trong câu hỏi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordKind {
    Plain,
    /// Từ trong STOP_WORDS, hoặc toàn chữ số.
    Stop,
    /// Dài hơn WORD_MAX_CHARS: không cụm nào chứa nó.
    TooLong,
}

/// Mọi cụm của câu hỏi, cụm dài trước, cùng độ dài thì theo thứ tự trong câu. Bỏ cụm toàn từ dừng hay số ("năm 2017").
pub fn build_phrases(question: &str) -> Vec<Phrase> {
    let words: Vec<String> = build_words(question)
        .into_iter()
        .take(QUESTION_MAX_WORDS)
        .collect();
    let keys: Vec<String> = words.iter().map(|w| build_tone_placed(w)).collect();
    let folded: Vec<String> = words.iter().map(|w| fold(w)).collect();
    let question_accented = keys.iter().zip(&folded).any(|(k, f)| k != f);
    let kinds: Vec<WordKind> = keys
        .iter()
        .zip(&folded)
        .map(|(k, f)| build_word_kind(k, f, question_accented))
        .collect();

    let mut phrases = Vec::new();
    for len in (1..=PHRASE_MAX_WORDS).rev() {
        for start in 0..words.len().saturating_sub(len - 1) {
            let range = start..start + len;
            let kinds = &kinds[range.clone()];
            if kinds.contains(&WordKind::TooLong) || kinds.iter().all(|k| *k == WordKind::Stop) {
                continue;
            }
            let key = keys[range.clone()].join(" ");
            let folded = folded[range.clone()].join(" ");
            phrases.push(Phrase {
                text: words[range].join(" "),
                accented: key != folded,
                has_stop: kinds.contains(&WordKind::Stop),
                key,
                folded,
                start,
                len,
            });
        }
    }
    phrases
}

fn build_word_kind(key: &str, folded: &str, question_accented: bool) -> WordKind {
    if key.chars().count() > WORD_MAX_CHARS {
        return WordKind::TooLong;
    }
    let is_stop = if question_accented {
        STOP_WORDS.iter().any(|s| build_tone_placed(s) == key)
    } else {
        STOP_WORDS.iter().any(|s| fold(s) == folded)
    };
    if is_stop || key.chars().all(|c| c.is_ascii_digit()) {
        WordKind::Stop
    } else {
        WordKind::Plain
    }
}

/// Các từ của một đoạn chữ, viết thường, còn dấu, chuẩn hóa NFC. Từ là dãy chữ cái và chữ số liền nhau; mọi ký tự khác
/// (dấu cách, dấu câu, gạch nối) là chỗ ngăn từ, để "Days for shipping (real)" và "days for shipping real" là một, và
/// chữ gõ dạng tổ hợp (chữ cái rồi dấu, như một số máy Mac) bằng chữ dựng sẵn.
pub fn build_words(text: &str) -> Vec<String> {
    let composed: String = text.nfc().collect();
    composed
        .to_lowercase()
        .split(|c: char| !is_word_char(c))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Một đoạn chữ ở dạng so được với Phrase::key: các từ của build_words, dấu thanh đặt một chỗ, nối bằng một dấu cách.
pub fn build_key(text: &str) -> String {
    build_words(text)
        .iter()
        .map(|w| build_tone_placed(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Năm dấu thanh của tiếng Việt, dạng tổ hợp: huyền, sắc, ngã, hỏi, nặng.
const TONE_MARKS: [char; 5] = ['\u{300}', '\u{301}', '\u{303}', '\u{309}', '\u{323}'];

/// Một âm tiết với dấu thanh đặt ở một chỗ cố định. Chữ Việt có hai kiểu đặt dấu cùng tồn tại trong dữ liệu và cách
/// gõ ("hoà" và "hòa", "thuỷ" và "thủy"); sau NFC đó là hai chuỗi khác nhau. Bỏ dấu thanh ra rồi đặt lại theo một luật
/// thì hai kiểu thành một, mà "năm" vẫn khác "nam". Luật (kiểu mới): nguyên âm có mũ, móc, trăng (â ă ê ô ơ ư) nhận dấu,
/// có hai thì nguyên âm sau ("người"); không có thì một nguyên âm nhận dấu; vần đóng (có phụ âm cuối) thì nguyên âm cuối;
/// vần mở hai nguyên âm thì nguyên âm đầu, trừ "oa", "oe", "uy" thì nguyên âm sau; vần mở ba nguyên âm thì nguyên âm
/// giữa. "qu", "gi" đứng trước nguyên âm khác không tính là nguyên âm. Luật chỉ cần cho cùng kết quả ở hai phía, nên
/// với chữ không phải tiếng Việt ("méxico") chỗ đặt có thể khác chính tả, mà vẫn so đúng.
fn build_tone_placed(word: &str) -> String {
    let decomposed: Vec<char> = word.nfd().collect();
    let Some(tone) = decomposed.iter().copied().find(|c| TONE_MARKS.contains(c)) else {
        return word.to_string();
    };
    let base: Vec<char> = decomposed
        .iter()
        .filter(|c| !TONE_MARKS.contains(c))
        .collect::<String>()
        .nfc()
        .collect();
    let Some(position) = build_tone_position(&base) else {
        return word.to_string();
    };
    let mut placed: String = base[..=position].iter().collect();
    placed.push(tone);
    placed.extend(&base[position + 1..]);
    placed.nfc().collect()
}

/// Chỗ đặt dấu thanh trong một âm tiết đã bỏ dấu thanh (xem build_tone_placed); None khi không có nguyên âm.
fn build_tone_position(base: &[char]) -> Option<usize> {
    let is_vowel = |c: char| "aăâeêioôơuưy".contains(c);
    let mut vowels: Vec<usize> = (0..base.len()).filter(|&i| is_vowel(base[i])).collect();
    let glide = base.len() > 1 && matches!((base[0], base[1]), ('q', 'u') | ('g', 'i'));
    if glide && vowels.len() > 1 && vowels[0] == 1 {
        vowels.remove(0);
    }
    if let Some(&marked) = vowels.iter().rev().find(|&&i| "ăâêôơư".contains(base[i])) {
        return Some(marked);
    }
    let (&first, &last) = (vowels.first()?, vowels.last()?);
    if vowels.len() == 1 || last + 1 < base.len() {
        return Some(last);
    }
    if vowels.len() == 2 {
        let pair: String = [base[first], base[last]].iter().collect();
        return Some(if matches!(pair.as_str(), "oa" | "oe" | "uy") {
            last
        } else {
            first
        });
    }
    Some(vowels[1])
}

/// Bỏ dấu và viết thường bằng deunicode ("đ" thành "d"). Cả câu hỏi lẫn giá trị đều bỏ dấu bằng hàm này, nên hai phía
/// luôn như nhau.
pub fn fold(text: &str) -> String {
    let ascii = deunicode::deunicode(text).to_lowercase();
    // deunicode có thể đổi một ký tự thành nhiều từ ("北京" thành "Bei Jing "), nên tách lại từ cho gọn.
    build_words(&ascii).join(" ")
}

// Dấu tổ hợp còn lại sau NFC (không ghép được vào chữ trước nó) vẫn thuộc từ, không cắt từ làm hai.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || ('\u{300}'..='\u{36f}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(question: &str) -> Vec<String> {
        build_phrases(question)
            .into_iter()
            .map(|p| p.text)
            .collect()
    }

    fn find<'a>(phrases: &'a [Phrase], text: &str) -> &'a Phrase {
        phrases
            .iter()
            .find(|p| p.text == text)
            .unwrap_or_else(|| panic!("no phrase {text}"))
    }

    #[test]
    fn phrases_keep_what_was_typed_and_a_comparable_key() {
        let phrases = build_phrases("MIỀN NAM");
        assert_eq!(phrases[0].text, "miền nam");
        assert_eq!(phrases[0].folded, "mien nam");
        assert!(phrases[0].accented);
        // Câu có dấu: "nam" không phải "năm", nên "miền nam" là cụm gợi ý.
        assert!(phrases[0].is_hint_phrase());
        assert_eq!(texts("MIỀN NAM"), ["miền nam", "miền", "nam"]);
        // Câu không dấu: "nam" có thể là "năm", là từ dừng; "mien nam" không thành cụm gợi ý.
        let plain = build_phrases("mien nam");
        assert!(!plain[0].accented && !plain[0].is_hint_phrase());
        assert_eq!(texts("mien nam"), ["mien nam", "mien"]);
        // Gõ dạng tổ hợp thành cùng chữ với dạng dựng sẵn.
        assert_eq!(build_phrases("Mie\u{302}\u{300}n Nam")[0].text, "miền nam");
        // Chữ hiển thị giữ chỗ dấu người gõ; dạng so đặt dấu một chỗ.
        let typed = build_phrases("đã hủy giao");
        let phrase = find(&typed, "hủy giao");
        assert_eq!(phrase.key, build_key("huỷ giao"));
    }

    #[test]
    fn time_words_numbers_and_connectors_are_not_hint_phrases() {
        assert_eq!(
            texts("Doanh thu năm 2017"),
            [
                "doanh thu năm 2017",
                "doanh thu năm",
                "thu năm 2017",
                "doanh thu",
                "thu năm",
                "doanh",
                "thu"
            ]
        );
        let phrases = build_phrases("doanh thu từng miền năm 2017");
        assert!(!find(&phrases, "miền năm").is_hint_phrase());
        assert!(!phrases.iter().any(|p| p.text == "năm" || p.text == "từng"));
        let plain = build_phrases("doanh thu nam 2017");
        assert!(!plain
            .iter()
            .any(|p| p.text == "nam" || p.text == "nam 2017"));
        assert!(!find(&plain, "thu nam").is_hint_phrase());
        assert!(find(&plain, "doanh thu").is_hint_phrase());
        for words in ["tháng 5", "quý 3", "ngày", "và", "của", "2017", "theo từng"] {
            assert!(texts(words).is_empty(), "{words}");
        }
        // Một âm tiết không bao giờ là cụm gợi ý.
        assert!(!find(&build_phrases("doanh thu Europe"), "europe").is_hint_phrase());
        // "giày" không còn là từ dừng ("giây" đã bỏ khỏi danh sách).
        assert!(!find(&build_phrases("doanh thu giay nam"), "giay").has_stop);
    }

    #[test]
    fn long_words_and_long_questions_are_capped() {
        assert!(build_phrases("?!... ,;").is_empty());
        assert!(build_phrases("").is_empty());
        assert_eq!(texts("Europe/LATAM"), ["europe latam", "europe", "latam"]);
        let long = "doanh ".repeat(500);
        let phrases = build_phrases(&long);
        assert!(phrases
            .iter()
            .all(|p| p.start + p.len <= QUESTION_MAX_WORDS));
        // Mỗi độ dài n từ 1 tới 5 có QUESTION_MAX_WORDS - n + 1 cụm.
        assert_eq!(phrases.len(), 5 * QUESTION_MAX_WORDS - 10);
        // Một từ rất dài không vào cụm nào, các từ khác vẫn dùng được.
        let huge = format!("doanh thu {} europe", "x".repeat(100_000));
        assert_eq!(texts(&huge), ["doanh thu", "doanh", "thu", "europe"]);
    }

    #[test]
    fn tone_marks_are_placed_one_way() {
        for (old, new) in [
            ("Hoà Bình", "Hòa Bình"),
            ("Khánh Hoà", "Khánh Hòa"),
            ("Thanh Hoá", "Thanh Hóa"),
            ("thuỷ sản", "thủy sản"),
            ("hoá chất", "hóa chất"),
            ("khoẻ", "khỏe"),
        ] {
            assert_eq!(build_key(old), build_key(new), "{old}");
        }
        // Dấu thanh vẫn phân biệt từ; chữ đúng chính tả không đổi.
        assert_ne!(build_key("năm"), build_key("nam"));
        for word in [
            "người",
            "trường",
            "quý",
            "giày",
            "ngoài",
            "khuỷu",
            "mỹ",
            "nữ",
            "hoàng",
        ] {
            assert_eq!(build_key(word), word);
        }
    }

    #[test]
    fn column_names_split_like_questions() {
        assert_eq!(
            build_words("Days for shipping (real)"),
            ["days", "for", "shipping", "real"]
        );
        assert_eq!(fold("Đơn  hàng"), "don hang");
        assert_eq!(fold("北京"), "bei jing");
        assert_eq!(build_key("Mie\u{302}\u{300}n Tây"), "miền tây");
    }
}
