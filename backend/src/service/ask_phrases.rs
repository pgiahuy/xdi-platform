// SPDX-License-Identifier: Apache-2.0
/* service/ask_phrases.rs - tách câu hỏi thành các cụm 1 đến 5 từ liền nhau, để service/ask_context.rs tìm cột và giá
trị mà câu hỏi nhắc tới. Mỗi cụm giữ hai dạng: chữ thường như người hỏi gõ, để khớp có dấu đúng nguyên văn xếp trên khớp
sau khi bỏ dấu; và dạng đã bỏ dấu, để so với tên cột, bí danh và để đặt id lựa chọn.

Bỏ dấu làm mất phân biệt "năm" (thời gian) với "Nam" (miền Nam, Việt Nam): không chặn thì "doanh thu năm 2017" thành
lọc theo miền Nam. Vì vậy từ chỉ thời gian, số, từ nối phổ biến (STOP_WORDS) không bao giờ tự thành cụm tìm giá trị, và
cụm có một từ như vậy chỉ nhận khớp bằng nhau, không nhận khớp gần: "miền Nam" vẫn ra South qua bí danh, còn "thu năm"
không khớp gần với "Nam". */

/// Một cụm dài tối đa ngần này từ (một từ ở đây là một âm tiết, phần giữa hai dấu cách). Đề xuất ban đầu là 3, nhưng
/// bí danh tiếng Việt hay dài 4, 5 âm tiết ("trạng thái giao hàng", "châu Á Thái Bình Dương" trong hồ sơ mẫu của
/// DataCo), và cụm 3 âm tiết không bao giờ bằng chúng.
pub const PHRASE_MAX_WORDS: usize = 5;

/// Cụm dài hơn ngần này từ chỉ nhận khớp bằng nhau. Khớp bằng nhau rẻ (một phép join trên chuỗi đã bỏ dấu), khớp gần
/// thì mỗi cụm phải so với từng bí danh (Store::find_values), nên giữ số cụm khớp gần như đề xuất ban đầu (1 đến 3 từ):
/// gõ sai trong một bí danh 4, 5 âm tiết thì không khớp.
const FUZZY_MAX_WORDS: usize = 3;

/// Chỉ xét ngần này từ đầu của câu hỏi. Số cụm tăng theo số từ, và mỗi cụm khớp gần phải so với các bí danh của phiên
/// bản (Store::find_values); 40 từ đủ cho một câu hỏi, câu dài hơn (dán cả đoạn văn) vẫn chạy mà không làm truy vấn
/// chậm theo.
pub const QUESTION_MAX_WORDS: usize = 40;

/// Cụm một từ ngắn hơn ngần này ký tự (sau bỏ dấu) không thành cụm: "au", "o" khớp gần với quá nhiều thứ.
const SHORT_WORD_CHARS: usize = 3;

/// Từ chỉ thời gian, số đếm, từ nối, từ so sánh, viết đã bỏ dấu. Chúng có trong câu hỏi nào cũng được mà không trỏ
/// tới giá trị nào; một số trùng chữ với giá trị sau khi bỏ dấu ("nam", "hai" như Hải Phòng), nên chỉ khớp bằng nhau
/// trong cụm dài hơn. Không có "so" (số, như "doanh số"): một mình nó đã ngắn hơn SHORT_WORD_CHARS.
const STOP_WORDS: &[&str] = &[
    // Thời gian.
    "nam", "thang", "quy", "ngay", "tuan", "gio", "phut", "giay", "hom", "nay", "qua", "truoc",
    "sau", "dau", "cuoi", "ky", "luc", "khi", "nua", "hien", "tai", // Số đếm.
    "mot", "hai", "ba", "bon", "tu", "lam", "bay", "tam", "chin", "muoi", "tram", "nghin", "ngan",
    "trieu", "ty", // Từ nối, từ hỏi.
    "va", "voi", "cua", "theo", "o", "trong", "ngoai", "cho", "cac", "nhung", "moi", "la", "co",
    "khong", "nao", "bao", "nhieu", "den", "toi", "hay", "hoac", "thi", "duoc", "bi", "nhu", "ve",
    "tren", "duoi", "giua", "ra", "vao", "boi", "ma", "do", "kia", "gi", "sao", "ai", "dang", "da",
    "se", "can", "xem", "liet", "ke", "biet", // So sánh, tổng hợp.
    "hon", "nhat", "cao", "thap", "lon", "nho", "tang", "giam", "top", "tong",
    // Tiếng Anh.
    "the", "of", "and", "or", "by", "in", "on", "for", "to", "per", "vs", "is", "are", "an", "from",
    "with", "at", "what", "how", "which", "many", "much", "total", "sum", "count",
];

/// Một cụm từ liền nhau của câu hỏi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    /// Chữ thường, còn dấu, các từ cách nhau một dấu cách.
    pub text: String,
    /// Như text, đã bỏ dấu.
    pub folded: String,
    /// Vị trí từ đầu tiên trong câu hỏi, tính từ 0.
    pub start: usize,
    /// Số từ.
    pub len: usize,
    /// Cụm dài hơn FUZZY_MAX_WORDS, hoặc có từ trong STOP_WORDS, từ một chữ cái, từ có chữ số: chỉ nhận khớp bằng
    /// nhau.
    pub exact_only: bool,
}

impl Phrase {
    /// Hai cụm có chung ít nhất một từ của câu hỏi.
    pub fn overlaps(&self, other_start: usize, other_len: usize) -> bool {
        self.start < other_start + other_len && other_start < self.start + self.len
    }
}

/// Mọi cụm dùng được để tìm cột và giá trị, cụm dài trước, cùng độ dài thì theo thứ tự trong câu. Bỏ cụm toàn từ
/// trong STOP_WORDS hay số ("năm 2017"), và cụm một từ ngắn.
pub fn build_phrases(question: &str) -> Vec<Phrase> {
    let words: Vec<String> = build_words(question)
        .into_iter()
        .take(QUESTION_MAX_WORDS)
        .collect();
    let folded: Vec<String> = words.iter().map(|w| fold(w)).collect();
    let plain: Vec<bool> = folded.iter().map(|w| !is_stop_or_number(w)).collect();

    let mut phrases = Vec::new();
    for len in (1..=PHRASE_MAX_WORDS).rev() {
        for start in 0..words.len().saturating_sub(len - 1) {
            let range = start..start + len;
            if !plain[range.clone()].iter().any(|p| *p) {
                continue;
            }
            if len == 1 && folded[start].chars().count() < SHORT_WORD_CHARS {
                continue;
            }
            phrases.push(Phrase {
                text: words[range.clone()].join(" "),
                folded: folded[range.clone()].join(" "),
                start,
                len,
                exact_only: len > FUZZY_MAX_WORDS || !plain[range].iter().all(|p| *p),
            });
        }
    }
    phrases
}

/// Các từ của một đoạn chữ, viết thường, còn dấu. Từ là dãy chữ cái và chữ số liền nhau; mọi ký tự khác (dấu cách,
/// dấu câu, gạch nối) là chỗ ngăn từ. Tên cột và bí danh cũng tách bằng hàm này trước khi so với cụm, để
/// "Days for shipping (real)" và "days for shipping real" là một.
pub fn build_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !is_word_char(c))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Bỏ dấu và viết thường. deunicode cho cùng kết quả với immutable_unaccent của Postgres trên chữ tiếng Việt ("đ"
/// thành "d"); với chữ không phải Latin hai bên có thể khác nhau, chỉ làm mất khớp chứ không sinh khớp sai.
pub fn fold(text: &str) -> String {
    let ascii = deunicode::deunicode(text).to_lowercase();
    // deunicode có thể đổi một ký tự thành nhiều từ ("北京" thành "Bei Jing "), nên tách lại từ cho gọn.
    build_words(&ascii).join(" ")
}

// Dấu thanh gõ dạng tổ hợp (chữ cái rồi dấu, U+0300 đến U+036F, như bàn phím một số máy Mac) không phải chữ cái
// theo is_alphanumeric; không giữ thì "Miền" gõ kiểu đó bị cắt làm hai từ.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || ('\u{300}'..='\u{36f}').contains(&c)
}

// Từ một chữ cái tính như từ trong STOP_WORDS: nó chỉ góp hai trigram, khớp gần không phân biệt được nó với từ ngắn
// khác, nên "châu Á" sẽ khớp gần "châu Âu" (similarity 0.67) dù là hai châu khác nhau.
fn is_stop_or_number(folded_word: &str) -> bool {
    folded_word.chars().count() < 2
        || folded_word.chars().any(|c| c.is_ascii_digit())
        || STOP_WORDS.contains(&folded_word)
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

    #[test]
    fn phrases_are_lowercase_with_accents_and_folded() {
        let phrases = build_phrases("MIỀN NAM");
        assert_eq!(phrases[0].text, "miền nam");
        assert_eq!(phrases[0].folded, "mien nam");
        // "nam" là từ chỉ thời gian khi bỏ dấu, nên "miền nam" chỉ nhận khớp bằng nhau, và "nam" không thành cụm.
        assert!(phrases[0].exact_only);
        assert_eq!(texts("MIỀN NAM"), ["miền nam", "miền"]);
        // Dấu gõ dạng tổ hợp vẫn là một từ.
        assert_eq!(build_phrases("Mie\u{302}\u{300}n")[0].folded, "mien");
    }

    #[test]
    fn time_words_numbers_and_short_words_are_not_phrases_alone() {
        let all = texts("Doanh thu năm 2017");
        assert_eq!(
            all,
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
        let phrases = build_phrases("doanh thu nam 2017");
        assert!(!phrases
            .iter()
            .any(|p| p.text == "nam" || p.text == "nam 2017"));
        let thu_nam = phrases.iter().find(|p| p.text == "thu nam").unwrap();
        assert!(thu_nam.exact_only);
        assert!(
            !phrases
                .iter()
                .find(|p| p.text == "doanh thu")
                .unwrap()
                .exact_only
        );
        for word in ["tháng 5", "quý 3", "ngày", "và", "của"] {
            assert!(texts(word).is_empty(), "{word}");
        }
        // "âu" một mình quá ngắn; "châu Âu" là một cụm.
        assert_eq!(texts("châu Âu"), ["châu âu", "châu"]);
        // Từ một chữ cái: cụm chứa nó chỉ nhận khớp bằng nhau.
        let asia = build_phrases("châu Á");
        assert_eq!(asia[0].text, "châu á");
        assert!(asia[0].exact_only);
    }

    #[test]
    fn punctuation_splits_words_and_long_questions_are_capped() {
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
        // Cụm 4, 5 từ chỉ nhận khớp bằng nhau.
        assert!(phrases.iter().all(|p| p.exact_only == (p.len > 3)));
    }

    #[test]
    fn column_names_split_like_questions() {
        assert_eq!(
            build_words("Days for shipping (real)"),
            ["days", "for", "shipping", "real"]
        );
        assert_eq!(fold("Đơn  hàng"), "don hang");
        assert_eq!(fold("北京"), "bei jing");
    }
}
