// SPDX-License-Identifier: Apache-2.0
/* service/ask_match.rs - chấm điểm khớp giữa một cụm của câu hỏi (service/ask_phrases.rs) với tên, bí danh, mô tả của
cột và với giá trị Postgres đã tìm (Store::find_values). service/ask_context.rs dùng điểm này để chọn cột, hỏi lại
khi mơ hồ và chọn giá trị đưa vào ngữ cảnh. Hàm thuần, không đọc database.

Postgres chỉ lọc thô (bằng nhau sau bỏ dấu, hoặc similarity sau bỏ dấu cùng số từ); điểm cuối tính ở đây:
- Cụm gõ có dấu chỉ khớp chữ có đúng dấu đó (khớp gần thì lấy điểm thấp hơn của so có dấu và so bỏ dấu); cụm không
  dấu so sau bỏ dấu. So có dấu làm ở Rust vì lower() và cách pg_trgm tách từ theo locale của database.
- Khớp gần chỉ giữa hai chuỗi cùng số từ: gõ sai hiếm khi làm đổi số âm tiết, còn một âm tiết so với giá trị hai âm
  tiết chứa nó thì similarity thường vượt ngưỡng ("châu" với "châu Âu" 0.714), biến từ chung chung thành bộ lọc.
  Từng âm tiết cũng phải gần nhau (is_close_by_syllable). */

use std::collections::{HashMap, HashSet};

use super::ask_context::{
    ColumnProfile, ValueMatch, CLOSE_SCORE, VALUE_MIN_SIMILARITY, VALUE_MIN_SIMILARITY_WORDS,
};
use super::ask_phrases::{build_key, fold, Phrase};
use crate::engine::store::ValueHit;

/// Số giá trị tối đa giữ cho một cụm.
const VALUES_PER_PHRASE: usize = 5;

// Điểm của một khớp, từ 0 tới 1. Ba bậc cách nhau quá CLOSE_SCORE, nên khớp ở bậc trên luôn thắng khớp ở bậc dưới:
// chữ thường bằng nhau đúng nguyên văn, rồi bằng nhau sau khi bỏ dấu (chỉ khi người hỏi gõ không dấu), rồi khớp gần
// (tối đa 0.7, kể cả khi similarity là 1 vì chỉ đảo thứ tự từ).
const SCORE_SAME_TEXT: f32 = 1.0;
const SCORE_SAME_FOLDED: f32 = 0.85;
const SCORE_FUZZY_MAX: f32 = 0.7;

/// Khớp gần có similarity từ ngần này là khớp chắc: cụm dài khớp gần như vậy được xét trước cụm ngắn nằm trong nó
/// khớp bằng nhau, để "southen europe" (0.722 với "Southern Europe") không bị "europe" (Market = Europe) giữ chỗ.
const STRONG_SIMILARITY: f32 = 0.7;

/// Tên, bí danh, mô tả của một cột, viết sẵn ở dạng so được với cụm (build_key) và dạng đã bỏ dấu.
pub struct ColumnText {
    /// Tên và từng bí danh.
    names: Vec<String>,
    folded_names: Vec<String>,
    description: String,
    folded_description: String,
}

pub fn build_column_text(column: &ColumnProfile) -> ColumnText {
    let names: Vec<String> = std::iter::once(&column.name)
        .chain(&column.aliases)
        .map(|n| build_key(n))
        .collect();
    let description = build_key(column.description.as_deref().unwrap_or(""));
    ColumnText {
        folded_names: names.iter().map(|n| fold(n)).collect(),
        names,
        folded_description: fold(&description),
        description,
    }
}

/// similarity của pg_trgm tính ở Rust, trên chuỗi đã qua build_key (hoặc fold): mỗi từ thêm hai dấu cách đầu, một
/// dấu cách cuối, lấy mọi đoạn 3 ký tự; điểm là số đoạn chung chia số đoạn của cả hai.
pub fn trigram_similarity(a: &str, b: &str) -> f32 {
    let (left, right) = (build_trigrams(a), build_trigrams(b));
    let common = left.intersection(&right).count();
    let total = left.len() + right.len() - common;
    if total == 0 {
        return 0.0;
    }
    common as f32 / total as f32
}

fn build_trigrams(text: &str) -> HashSet<[char; 3]> {
    let mut trigrams = HashSet::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let padded: Vec<char> = "  ".chars().chain(word.chars()).chain([' ']).collect();
        for window in padded.windows(3) {
            trigrams.insert([window[0], window[1], window[2]]);
        }
    }
    trigrams
}

/// Hai chuỗi cùng số từ giống nhau theo từng âm tiết (so âm tiết thứ i với âm tiết thứ i): âm tiết khác nhau chỉ được
/// sai một chữ (thêm, bớt, đổi một chữ, hoặc đảo hai chữ liền nhau), trừ một âm tiết khác hẳn khi ít nhất hai âm tiết
/// khác giống hệt. similarity không tách được gõ sai với từ khác: "mien nma" với "mien nam" và "chau my" với "chau
/// au" cùng được 0.5. Ngoại lệ hai âm tiết giống hệt để "hàng tiêu chuẩn" (trong "giao hàng tiêu chuẩn") vẫn khớp
/// "giao tiêu chuẩn".
fn is_close_by_syllable(a: &str, b: &str) -> bool {
    let pairs: Vec<(&str, &str)> = a.split(' ').zip(b.split(' ')).collect();
    let same = pairs.iter().filter(|(x, y)| x == y).count();
    let far = pairs
        .iter()
        .filter(|(x, y)| build_edit_distance(x, y) > 1)
        .count();
    far == 0 || (far == 1 && same >= 2)
}

/// Số phép sửa một chữ (thêm, bớt, đổi, đảo hai chữ liền nhau) để biến a thành b, tính theo ký tự.
fn build_edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    // d[i][j]: số phép sửa giữa i chữ đầu của a và j chữ đầu của b.
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

fn word_count(text: &str) -> usize {
    text.split(' ').filter(|w| !w.is_empty()).count()
}

/// Điểm khi chuỗi (tên cột, bí danh) bằng cụm: đúng nguyên văn, hoặc bằng sau bỏ dấu khi người hỏi gõ không dấu.
/// Cụm chỉ khớp đúng nguyên văn (từ ngắn) thì không nhận dạng bỏ dấu.
fn build_equal_score(
    text: &str,
    folded: &str,
    accented: bool,
    text_only: bool,
    names: &[String],
    folded_names: &[String],
) -> Option<f32> {
    if names.iter().any(|n| n == text) {
        Some(SCORE_SAME_TEXT)
    } else if !accented && !text_only && folded_names.iter().any(|n| n == folded) {
        Some(SCORE_SAME_FOLDED)
    } else {
        None
    }
}

fn build_column_score(phrase: &Phrase, text: &ColumnText) -> Option<f32> {
    build_equal_score(
        &phrase.text,
        &phrase.folded,
        phrase.accented,
        phrase.text_only,
        &text.names,
        &text.folded_names,
    )
}

/// Điểm của một giá trị Postgres đã tìm cho cụm, kèm similarity; None khi giá trị không khớp theo luật ở đầu file.
fn build_value_score(phrase: &Phrase, hit: &ValueHit) -> Option<(f32, f32)> {
    let key = build_key(&hit.matched_text);
    if key == phrase.text {
        return Some((SCORE_SAME_TEXT, 1.0));
    }
    if phrase.text_only {
        return None;
    }
    if !phrase.accented && hit.same_folded {
        return Some((SCORE_SAME_FOLDED, 1.0));
    }
    if phrase.exact_only {
        return None;
    }
    let folded_key = fold(&key);
    if word_count(&phrase.folded) != word_count(&folded_key) {
        return None;
    }
    let folded_similarity = trigram_similarity(&phrase.folded, &folded_key);
    // Cụm có dấu lấy số nhỏ hơn của hai cách so: so có dấu chỉ được khắt khe hơn so bỏ dấu. Tập trigram có dấu đôi khi
    // gộp hai đoạn trùng làm điểm cao lên: "châu mỹ" với "châu âu" được 0.5 có dấu, 0.455 bỏ dấu.
    let (similarity, syllables_close) = if phrase.accented {
        (
            folded_similarity.min(trigram_similarity(&phrase.text, &key)),
            is_close_by_syllable(&phrase.text, &key),
        )
    } else {
        (
            folded_similarity,
            is_close_by_syllable(&phrase.folded, &folded_key),
        )
    };
    if !syllables_close {
        return None;
    }
    let threshold = if phrase.len == 1 {
        VALUE_MIN_SIMILARITY
    } else {
        VALUE_MIN_SIMILARITY_WORDS
    };
    (similarity >= threshold).then_some((SCORE_FUZZY_MAX * similarity, similarity))
}

/// Cụm là một đoạn từ liền nhau của tên hay bí danh ("region" trong Order Region), hoặc của mô tả với cụm từ hai từ
/// (một từ lẻ có trong quá nhiều mô tả). Khớp kiểu này chỉ giữ cột trong ngữ cảnh, không thành ứng viên của cụm: nó
/// sinh nhiều cột ngang điểm ("khách hàng" có trong bí danh của năm cột) và câu hỏi sẽ bị hỏi lại vô cớ.
pub fn is_partial_match(phrase: &Phrase, text: &ColumnText) -> bool {
    if phrase.exact_only {
        return false;
    }
    let (needle, names, description) = if phrase.accented {
        (&phrase.text, &text.names, &text.description)
    } else {
        (&phrase.folded, &text.folded_names, &text.folded_description)
    };
    let inside = |haystack: &str| format!(" {haystack} ").contains(&format!(" {needle} "));
    names.iter().any(|n| inside(n)) || (phrase.len > 1 && inside(description))
}

/// Một cụm mở đầu bằng tên hay bí danh của một cột, phần còn lại có thể là một giá trị của cột đó: "miền nam" là cột
/// "miền" với giá trị "Nam". Phần còn lại cần được tìm như một cụm chỉ khớp bằng nhau, kể cả khi nó là từ dừng.
pub struct ColumnValuePair {
    /// Vị trí của cụm trong danh sách phrases.
    pub phrase: usize,
    /// Chỉ số cột trong columns.
    pub column: usize,
    /// Phần còn lại: chữ thường còn dấu, và đã bỏ dấu.
    pub tail: String,
    tail_folded: String,
}

pub fn build_column_value_pairs(phrases: &[Phrase], texts: &[ColumnText]) -> Vec<ColumnValuePair> {
    let mut pairs = Vec::new();
    for (index, phrase) in phrases.iter().enumerate() {
        let words: Vec<&str> = phrase.text.split(' ').collect();
        let folded: Vec<&str> = phrase.folded.split(' ').collect();
        // deunicode có thể đổi số từ (chữ không phải Latin); khi đó không tách được tương ứng, bỏ qua.
        if words.len() != folded.len() {
            continue;
        }
        for split in 1..words.len() {
            let head = words[..split].join(" ");
            let head_folded = folded[..split].join(" ");
            for (column, text) in texts.iter().enumerate() {
                let accented = head != head_folded;
                if build_equal_score(
                    &head,
                    &head_folded,
                    accented,
                    false,
                    &text.names,
                    &text.folded_names,
                )
                .is_some()
                {
                    pairs.push(ColumnValuePair {
                        phrase: index,
                        column,
                        tail: words[split..].join(" "),
                        tail_folded: folded[split..].join(" "),
                    });
                }
            }
        }
    }
    pairs
}

/// Điểm của giá trị khớp phần còn lại của một cặp cột và giá trị: chỉ khớp bằng nhau, theo cách gõ của phần đó.
fn build_tail_score(pair: &ColumnValuePair, hit: &ValueHit) -> Option<f32> {
    if build_key(&hit.matched_text) == pair.tail {
        Some(SCORE_SAME_TEXT)
    } else if pair.tail == pair.tail_folded && hit.same_folded {
        Some(SCORE_SAME_FOLDED)
    } else {
        None
    }
}

pub struct ScoredHit<'a> {
    hit: &'a ValueHit,
    score: f32,
    similarity: f32,
}

/// Một cụm cùng giá trị đã tìm và cột ứng viên của nó.
pub struct ScoredPhrase<'a> {
    pub phrase: &'a Phrase,
    pub hits: Vec<ScoredHit<'a>>,
    pub candidates: Vec<(usize, f32)>,
}

impl ScoredPhrase<'_> {
    pub fn best(&self) -> f32 {
        self.candidates.first().map_or(0.0, |c| c.1)
    }

    /// Khớp bằng nhau, hoặc khớp gần với similarity từ STRONG_SIMILARITY.
    fn is_strong(&self) -> bool {
        self.best() >= SCORE_SAME_FOLDED
            || self.hits.iter().any(|h| h.similarity >= STRONG_SIMILARITY)
    }
}

/// Cột ứng viên của một cụm và điểm tốt nhất của mỗi cột, qua tên, bí danh hoặc giá trị; điểm cao trước, bằng điểm
/// thì theo thứ tự cột. Phần tử là chỉ số trong columns. Đã có cột khớp bằng nhau thì bỏ cột chỉ khớp gần: "Africa"
/// bằng đúng Market = Africa, không nên thành giả định đổi được sang một cột chỉ khớp gần.
fn build_candidates(
    phrase: &Phrase,
    columns: &[ColumnProfile],
    texts: &[ColumnText],
    hits: &[ScoredHit],
) -> Vec<(usize, f32)> {
    let mut best: HashMap<usize, f32> = HashMap::new();
    for (i, text) in texts.iter().enumerate() {
        if let Some(score) = build_column_score(phrase, text) {
            best.insert(i, score);
        }
    }
    for h in hits {
        let Some(i) = columns.iter().position(|c| c.name == h.hit.column_name) else {
            continue;
        };
        let entry = best.entry(i).or_insert(h.score);
        *entry = entry.max(h.score);
    }
    let mut candidates: Vec<(usize, f32)> = best.into_iter().collect();
    if candidates
        .iter()
        .any(|(_, score)| *score >= SCORE_SAME_FOLDED)
    {
        candidates.retain(|(_, score)| *score >= SCORE_SAME_FOLDED);
    }
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    candidates
}

/// Giá trị đã chấm điểm của một cụm: giá trị Postgres tìm cho chính cụm, cộng giá trị của các cặp cột và giá trị
/// nằm trong cụm.
fn build_scored_hits<'a>(
    index: usize,
    phrase: &Phrase,
    columns: &[ColumnProfile],
    hits: &'a [ValueHit],
    pairs: &[ColumnValuePair],
) -> Vec<ScoredHit<'a>> {
    let mut scored: Vec<ScoredHit> = hits
        .iter()
        .filter(|h| h.phrase == phrase.text)
        .filter_map(|h| {
            let (score, similarity) = build_value_score(phrase, h)?;
            Some(ScoredHit {
                hit: h,
                score,
                similarity,
            })
        })
        .collect();
    for pair in pairs.iter().filter(|p| p.phrase == index) {
        let column = &columns[pair.column].name;
        for h in hits
            .iter()
            .filter(|h| h.phrase == pair.tail && &h.column_name == column)
        {
            if let Some(score) = build_tail_score(pair, h) {
                scored.push(ScoredHit {
                    hit: h,
                    score,
                    similarity: 1.0,
                });
            }
        }
    }
    scored
}

/// Mỗi cụm kèm giá trị đã chấm điểm và các cột ứng viên, theo thứ tự xét.
pub fn build_scored_phrases<'a>(
    phrases: &'a [Phrase],
    columns: &[ColumnProfile],
    texts: &[ColumnText],
    hits: &'a [ValueHit],
) -> Vec<ScoredPhrase<'a>> {
    let pairs = build_column_value_pairs(phrases, texts);
    let mut scored: Vec<ScoredPhrase> = phrases
        .iter()
        .enumerate()
        .map(|(index, phrase)| {
            let hits = build_scored_hits(index, phrase, columns, hits, &pairs);
            let candidates = build_candidates(phrase, columns, texts, &hits);
            ScoredPhrase {
                phrase,
                hits,
                candidates,
            }
        })
        .collect();
    // Khớp chắc (bằng nhau, hoặc khớp gần từ STRONG_SIMILARITY) trước, cụm dài trước: "trạng thái giao hàng" trước
    // "giao hàng", "southen europe" trước "europe". Còn lại (khớp gần yếu) xếp theo điểm trước rồi mới tới độ dài: cụm
    // dài khớp gần kém hơn không được giữ chỗ của cụm ngắn khớp tốt hơn ("giao hàng tiêu" với "giao hạng hai" chặn
    // "hàng tiêu chuẩn" với "giao tiêu chuẩn").
    scored.sort_by(|a, b| {
        let (strong_a, strong_b) = (a.is_strong(), b.is_strong());
        let by_len = b.phrase.len.cmp(&a.phrase.len);
        let by_score = b.best().total_cmp(&a.best());
        let order = if strong_a {
            by_len.then(by_score)
        } else {
            by_score.then(by_len)
        };
        strong_b
            .cmp(&strong_a)
            .then(order)
            .then(a.phrase.start.cmp(&b.phrase.start))
    });
    scored
}

/// Giá trị của một cụm trong các cột đã chọn: điểm cao trước, chỉ những giá trị gần giá trị tốt nhất, để "Europe"
/// khớp đúng không kéo theo các khớp gần kém hơn.
pub fn build_phrase_values(
    phrase: &Phrase,
    hits: &[ScoredHit],
    columns: &[&str],
) -> Vec<ValueMatch> {
    let mut kept: Vec<&ScoredHit> = hits
        .iter()
        .filter(|h| columns.contains(&h.hit.column_name.as_str()))
        .collect();
    kept.sort_by(|a, b| b.score.total_cmp(&a.score));
    let Some(best) = kept.first().map(|h| h.score) else {
        return Vec::new();
    };
    let mut values: Vec<ValueMatch> = Vec::new();
    for h in kept.into_iter().filter(|h| best - h.score < CLOSE_SCORE) {
        // Giá trị khớp qua nhiều bí danh chỉ ghi một lần, với khớp tốt nhất (đã xếp trước).
        if values
            .iter()
            .any(|v| v.column == h.hit.column_name && v.value == h.hit.value)
        {
            continue;
        }
        values.push(ValueMatch {
            column: h.hit.column_name.clone(),
            value: h.hit.value.clone(),
            matched_text: h.hit.matched_text.clone(),
            phrase: phrase.text.clone(),
            score: h.score,
        });
        if values.len() == VALUES_PER_PHRASE {
            break;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cùng số với similarity của pg_trgm đo trên Postgres (số đo ở chú thích của VALUE_MIN_SIMILARITY và các test của ask_context_tests.rs).
    #[test]
    fn trigram_similarity_matches_pg_trgm() {
        let close = |a: &str, b: &str, expected: f32| {
            let got = trigram_similarity(a, b);
            assert!((got - expected).abs() < 0.001, "{a} / {b}: {got}");
        };
        close("mien nma", "mien nam", 0.5);
        close("chau", "chau au", 0.714);
        close("europe", "southern europe", 0.438);
        close("chau phi", "chau au", 0.455);
        close("europ", "europe", 0.625);
        close("doanh thu", "viet nam", 0.0);
        assert_eq!(trigram_similarity("", ""), 0.0);
    }

    #[test]
    fn syllables_must_be_close() {
        assert_eq!(build_edit_distance("nma", "nam"), 1);
        assert_eq!(build_edit_distance("mỹ", "âu"), 2);
        assert_eq!(build_edit_distance("latin", "latinh"), 1);
        assert!(is_close_by_syllable("mien nma", "mien nam"));
        assert!(is_close_by_syllable("southen europe", "southern europe"));
        assert!(!is_close_by_syllable("châu mỹ", "châu âu"));
        assert!(!is_close_by_syllable("tung chau", "chau au"));
        assert!(is_close_by_syllable("hàng tiêu chuẩn", "giao tiêu chuẩn"));
        assert!(!is_close_by_syllable("giao hàng tiêu", "giao tiêu chuẩn"));
    }
}
