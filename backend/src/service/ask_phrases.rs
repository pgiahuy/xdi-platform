// SPDX-License-Identifier: Apache-2.0
/* service/ask_phrases.rs - tách câu hỏi thành các cụm 1 đến 5 từ liền nhau, để service/ask_context.rs tìm cột và giá
trị mà câu hỏi nhắc tới. Mỗi cụm giữ chữ thường như người hỏi gõ (NFC, dấu thanh đặt một chỗ cố định, xem
build_tone_placed) và dạng đã bỏ dấu.

So có dấu chỉ khi cả cụm lẫn chữ đích đều có dấu; một bên không dấu thì so sau bỏ dấu (service/ask_match.rs). Bỏ dấu
làm mất phân biệt "năm" (thời gian) với "Nam" (miền Nam, Việt Nam), nên:
- Từ chỉ thời gian, số, từ nối (STOP_WORDS) không tự thành cụm, và cụm có một từ như vậy chỉ nhận khớp bằng nhau.
- Câu có dấu ở bất kỳ đâu thì so từ với STOP_WORDS có dấu: người gõ có dấu viết "năm" cho thời gian, nên "nam" của
  họ là Nam. Câu không dấu thì so sau bỏ dấu, "nam" là từ dừng vì không biết người hỏi muốn chữ nào.
Vì vậy "doanh thu từng miền năm 2017" không ra miền Nam ("miền năm" khác "miền Nam" ở dấu), còn "miền Nam" và
"mien nam" vẫn ra South qua bí danh. */

use unicode_normalization::UnicodeNormalization;

/// Một cụm dài tối đa ngần này từ (một từ ở đây là một âm tiết, phần giữa hai dấu cách). Đề xuất ban đầu là 3, nhưng
/// bí danh tiếng Việt hay dài 4, 5 âm tiết ("trạng thái giao hàng", "châu Á Thái Bình Dương" trong hồ sơ mẫu của
/// DataCo), và cụm 3 âm tiết không bao giờ bằng chúng.
pub const PHRASE_MAX_WORDS: usize = 5;

/// Cụm dài hơn ngần này từ chỉ nhận khớp bằng nhau: khớp bằng nhau rẻ (tra bảng băm), khớp gần thì phải so với từng
/// giá trị và bí danh cùng số từ, nên giữ cụm khớp gần ở 1 đến 3 từ như đề xuất ban đầu. Gõ sai trong bí danh 4, 5 âm
/// tiết thì không khớp.
const FUZZY_MAX_WORDS: usize = 3;

/// Chỉ xét ngần này từ đầu của câu hỏi, để số cụm có trần (khoảng 5 lần số từ). Từ quá dài thì xem WORD_MAX_CHARS;
/// số cụm khớp gần còn có trần riêng ở service/ask_context.rs.
pub const QUESTION_MAX_WORDS: usize = 40;

/// Từ dài hơn ngần này ký tự không dùng để tìm, và không cụm nào chứa nó. Giá trị của cột chiều hầu như không có từ
/// dài như vậy, còn một từ dài (dán một chuỗi 100 000 ký tự) làm mỗi phép so trigram chậm theo độ dài: với bản đầu
/// của truy vấn, trên 20 000 giá trị, một từ 100 000 ký tự mất 100 giây.
const WORD_MAX_CHARS: usize = 40;

/// Từ ngắn hơn ngần này ký tự (sau bỏ dấu), hoặc có chữ số mà không toàn chữ số ("b2b", "5g"): đứng một mình thì chỉ
/// khớp đúng nguyên văn ("B2B"), không khớp sau bỏ dấu hay khớp gần, vì "nu" hay "au" bằng quá nhiều thứ khi bỏ dấu.
/// Giá trị một âm tiết tiếng Việt ("Nữ", "Nam") thì chỉ khớp qua "tên cột + giá trị" (service/ask_values.rs).
const SHORT_WORD_CHARS: usize = 3;

/// Từ chỉ thời gian, số đếm, từ nối, từ hỏi, từ so sánh, viết có dấu. Chúng có trong câu hỏi nào cũng được mà không
/// trỏ tới giá trị nào. Không có "số" (như "doanh số"); "từng", "mọi" có vì "theo từng châu" là gom nhóm, không lọc.
/// Không có "giờ", "phút", "giây": hiếm khi có trong câu hỏi kinh doanh, mà gõ không dấu thì trùng "giỏ", "giày".
/// Các từ còn lại vẫn có thể trùng một từ thường khi gõ không dấu ("hai" với "Hải", "dau" với "dầu", "tai" với
/// "tài"): khi đó cụm chỉ gồm những từ như vậy bị bỏ, cụm có thêm từ khác vẫn khớp bằng nhau.
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
    /// Chữ thường, còn dấu, NFC, các từ cách nhau một dấu cách.
    pub text: String,
    /// Như text, đã bỏ dấu.
    pub folded: String,
    /// Vị trí từ đầu tiên trong câu hỏi, tính từ 0.
    pub start: usize,
    /// Số từ.
    pub len: usize,
    /// Người hỏi gõ có dấu (text khác folded): chỉ so với chữ có đúng dấu.
    pub accented: bool,
    /// Chỉ nhận khớp bằng nhau: cụm dài hơn FUZZY_MAX_WORDS, hoặc có từ dừng, từ một chữ cái, từ có chữ số. Vì vậy gõ
    /// sai trong cụm có từ dừng không được nhận: câu không dấu "mie nam" ("nam" có thể là "năm") ra rỗng.
    pub exact_only: bool,
    /// Chỉ nhận khớp đúng nguyên văn, kể cả dấu: cụm một từ ngắn hoặc có chữ số (xem SHORT_WORD_CHARS).
    pub text_only: bool,
}

impl Phrase {
    /// Hai cụm có chung ít nhất một từ của câu hỏi.
    pub fn overlaps(&self, other_start: usize, other_len: usize) -> bool {
        self.start < other_start + other_len && other_start < self.start + self.len
    }
}

/// Loại của một từ trong câu hỏi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordKind {
    Plain,
    /// Từ trong STOP_WORDS, hoặc toàn chữ số.
    Stop,
    /// Ngắn hơn SHORT_WORD_CHARS hoặc có chữ số.
    Short,
    /// Dài hơn WORD_MAX_CHARS: không cụm nào chứa nó.
    TooLong,
}

/// Mọi cụm dùng được để tìm cột và giá trị, cụm dài trước, cùng độ dài thì theo thứ tự trong câu. Bỏ cụm toàn từ dừng
/// hay số ("năm 2017") và cụm một từ dừng.
pub fn build_phrases(question: &str) -> Vec<Phrase> {
    let words: Vec<String> = build_words(question)
        .into_iter()
        .take(QUESTION_MAX_WORDS)
        .collect();
    let folded: Vec<String> = words.iter().map(|w| fold(w)).collect();
    let question_accented = words.iter().zip(&folded).any(|(w, f)| w != f);
    let kinds: Vec<WordKind> = words
        .iter()
        .zip(&folded)
        .map(|(w, f)| build_word_kind(w, f, question_accented))
        .collect();

    let mut phrases = Vec::new();
    for len in (1..=PHRASE_MAX_WORDS).rev() {
        for start in 0..words.len().saturating_sub(len - 1) {
            let range = start..start + len;
            let kinds = &kinds[range.clone()];
            if kinds.contains(&WordKind::TooLong) || kinds.iter().all(|k| *k == WordKind::Stop) {
                continue;
            }
            let text = words[range.clone()].join(" ");
            let folded = folded[range].join(" ");
            let has_letter_word = words[start..start + len]
                .iter()
                .any(|w| w.chars().count() == 1);
            let has_digit = words[start..start + len]
                .iter()
                .any(|w| w.chars().any(|c| c.is_ascii_digit()));
            let text_only = len == 1 && kinds[0] == WordKind::Short;
            phrases.push(Phrase {
                accented: text != folded,
                exact_only: text_only
                    || len > FUZZY_MAX_WORDS
                    || kinds.contains(&WordKind::Stop)
                    || has_letter_word
                    || has_digit,
                text_only,
                text,
                folded,
                start,
                len,
            });
        }
    }
    phrases
}

fn build_word_kind(word: &str, folded: &str, question_accented: bool) -> WordKind {
    if word.chars().count() > WORD_MAX_CHARS {
        return WordKind::TooLong;
    }
    let is_stop = if question_accented {
        STOP_WORDS.contains(&word)
    } else {
        STOP_WORDS.iter().any(|s| fold(s) == folded)
    };
    if is_stop || word.chars().all(|c| c.is_ascii_digit()) {
        WordKind::Stop
    } else if folded.chars().count() < SHORT_WORD_CHARS || word.chars().any(|c| c.is_ascii_digit())
    {
        WordKind::Short
    } else {
        WordKind::Plain
    }
}

/// Các từ của một đoạn chữ, viết thường, còn dấu, chuẩn hóa NFC. Từ là dãy chữ cái và chữ số liền nhau; mọi ký tự khác
/// (dấu cách, dấu câu, gạch nối) là chỗ ngăn từ. Tên cột, bí danh và giá trị cũng qua hàm này (build_key) trước khi so
/// với cụm, để "Days for shipping (real)" và "days for shipping real" là một, và chữ gõ dạng tổ hợp (chữ cái rồi dấu,
/// như một số máy Mac) bằng chữ dựng sẵn.
pub fn build_words(text: &str) -> Vec<String> {
    let composed: String = text.nfc().collect();
    composed
        .to_lowercase()
        .split(|c: char| !is_word_char(c))
        .filter(|w| !w.is_empty())
        .map(build_tone_placed)
        .collect()
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

/// Chữ có dấu (kể cả "đ"): khác dạng đã bỏ dấu.
pub fn has_accent(text: &str) -> bool {
    build_key(text) != fold(text)
}

/// Một đoạn chữ ở dạng so được với Phrase::text: các từ của build_words nối bằng một dấu cách.
pub fn build_key(text: &str) -> String {
    build_words(text).join(" ")
}

/// Bỏ dấu và viết thường bằng deunicode ("đ" thành "d"). Cả câu hỏi lẫn giá trị đều bỏ dấu bằng hàm này, nên hai phía
/// luôn như nhau.
pub fn fold(text: &str) -> String {
    let ascii = deunicode::deunicode(text).to_lowercase();
    // deunicode có thể đổi một ký tự thành nhiều từ ("北京" thành "Bei Jing "), nên tách lại từ cho gọn.
    build_key(&ascii)
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
    fn phrases_keep_accents_as_typed() {
        let phrases = build_phrases("MIỀN NAM");
        assert_eq!(phrases[0].text, "miền nam");
        assert_eq!(phrases[0].folded, "mien nam");
        assert!(phrases[0].accented);
        // Câu có dấu: "nam" không phải "năm", nên là từ thường và "miền nam" nhận cả khớp gần.
        assert!(!phrases[0].exact_only);
        assert_eq!(texts("MIỀN NAM"), ["miền nam", "miền", "nam"]);
        // Câu không dấu: "nam" có thể là "năm", là từ dừng.
        let plain = build_phrases("mien nam");
        assert!(!plain[0].accented && plain[0].exact_only);
        assert_eq!(texts("mien nam"), ["mien nam", "mien"]);
        // Gõ dạng tổ hợp thành cùng chữ với dạng dựng sẵn.
        assert_eq!(build_phrases("Mie\u{302}\u{300}n Nam")[0].text, "miền nam");
    }

    #[test]
    fn time_words_numbers_and_connectors_are_not_phrases_alone() {
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
        assert!(find(&phrases, "miền năm").exact_only);
        assert!(!phrases.iter().any(|p| p.text == "năm" || p.text == "từng"));
        let plain = build_phrases("doanh thu nam 2017");
        assert!(!plain
            .iter()
            .any(|p| p.text == "nam" || p.text == "nam 2017"));
        assert!(find(&plain, "thu nam").exact_only);
        assert!(!find(&plain, "doanh thu").exact_only);
        for words in ["tháng 5", "quý 3", "ngày", "và", "của", "2017", "theo từng"] {
            assert!(texts(words).is_empty(), "{words}");
        }
        // Từ một chữ cái: cụm chứa nó chỉ nhận khớp bằng nhau; đứng một mình thì chỉ khớp đúng nguyên văn.
        let asia = build_phrases("châu Á");
        assert_eq!(asia[0].text, "châu á");
        assert!(asia[0].exact_only);
        assert_eq!(texts("châu Á"), ["châu á", "châu", "á"]);
        assert!(find(&asia, "á").text_only);
    }

    #[test]
    fn short_words_and_codes_only_match_as_typed() {
        for (question, word) in [
            ("khách nữ", "nữ"),
            ("doanh thu B2B", "b2b"),
            ("thuê bao 5G", "5g"),
        ] {
            let phrases = build_phrases(question);
            let phrase = find(&phrases, word);
            assert!(phrase.text_only && phrase.exact_only, "{question}");
        }
        // "mới" không phải "mỗi" khi câu có dấu.
        let phrases = build_phrases("đơn mới");
        assert!(!find(&phrases, "mới").text_only);
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
        // Mỗi độ dài n từ 1 tới 5 có QUESTION_MAX_WORDS - n + 1 cụm; cụm 4, 5 từ chỉ nhận khớp bằng nhau.
        assert_eq!(phrases.len(), 5 * QUESTION_MAX_WORDS - 10);
        assert!(phrases.iter().all(|p| p.exact_only == (p.len > 3)));
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
        assert!(has_accent("Hà Nội") && has_accent("đi"));
        assert!(!has_accent("Ha Noi") && !has_accent("khu_vuc"));
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
