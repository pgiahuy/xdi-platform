// SPDX-License-Identifier: Apache-2.0
/* service/ask_match.rs - chấm điểm khớp giữa một cụm của câu hỏi (service/ask_phrases.rs) với tên, bí danh, mô tả của
cột và với giá trị của phiên bản (service/ask_values.rs). service/ask_context.rs dùng điểm này để chọn cột, hỏi lại khi
mơ hồ và chọn giá trị đưa vào ngữ cảnh. Hàm thuần, không đọc database.

Luật, ưu tiên đúng hơn bắt được nhiều (thà bỏ sót còn hơn khớp nhầm):
- So có dấu chỉ khi cả cụm lẫn chữ đích đều có dấu ("miền năm" khác "miền Nam"); một bên không dấu thì so sau bỏ dấu
  ("Hà Nội" bằng "Ha Noi", "khu vực" bằng cột khu_vuc), vì bên không dấu không mang thông tin dấu để phân biệt.
- Khớp gần chỉ giữa hai chuỗi cùng số từ, similarity kiểu pg_trgm từ ngưỡng, và từng âm tiết khác nhau chỉ sai một chữ
  (is_close_by_syllable). Một âm tiết so với giá trị hai âm tiết chứa nó thì similarity thường vượt ngưỡng ("châu" với
  "châu Âu" 0.714), và similarity không tách được gõ sai với từ khác ("mien nma" với "mien nam", "chau my" với "chau
  au" cùng 0.5).
- Giá trị một âm tiết tiếng Việt chỉ khớp qua "tên cột + giá trị" (ValueTarget::standalone, ColumnValuePair). */

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::ask_context::{
    ColumnProfile, ValueMatch, CLOSE_SCORE, VALUE_MIN_SIMILARITY, VALUE_MIN_SIMILARITY_WORDS,
};
use super::ask_phrases::{build_key, fold, Phrase};
use super::ask_values::{build_similarity, build_trigrams, ValueIndex, ValueTarget};

/// Số giá trị tối đa giữ cho một cụm.
const VALUES_PER_PHRASE: usize = 5;

// Điểm của một khớp, từ 0 tới 1. Ba bậc cách nhau quá CLOSE_SCORE, nên khớp ở bậc trên luôn thắng khớp ở bậc dưới:
// chữ thường bằng nhau đúng nguyên văn, rồi bằng nhau sau khi bỏ dấu (khi một bên không dấu), rồi khớp gần (tối đa
// 0.7, kể cả khi similarity là 1 vì chỉ đảo thứ tự từ).
const SCORE_SAME_TEXT: f32 = 1.0;
const SCORE_SAME_FOLDED: f32 = 0.85;
const SCORE_FUZZY_MAX: f32 = 0.7;

/// Khớp gần có similarity từ ngần này là khớp chắc: cụm dài khớp gần như vậy được xét trước cụm ngắn nằm trong nó
/// khớp bằng nhau, để "southen europe" (0.722 với "Southern Europe") không bị "europe" (Market = Europe) giữ chỗ.
const STRONG_SIMILARITY: f32 = 0.7;

/// Một chuỗi (tên cột, bí danh, mô tả) ở dạng so được với cụm.
struct TextForm {
    key: String,
    folded: String,
    accented: bool,
}

fn build_form(text: &str) -> TextForm {
    let key = build_key(text);
    let folded = fold(text);
    TextForm {
        accented: key != folded,
        key,
        folded,
    }
}

/// Tên, bí danh, mô tả của một cột, viết sẵn ở dạng so được với cụm.
pub struct ColumnText {
    /// Tên và từng bí danh.
    names: Vec<TextForm>,
    description: TextForm,
}

pub fn build_column_text(column: &ColumnProfile) -> ColumnText {
    ColumnText {
        names: std::iter::once(&column.name)
            .chain(&column.aliases)
            .map(|n| build_form(n))
            .collect(),
        description: build_form(column.description.as_deref().unwrap_or("")),
    }
}

/// similarity của pg_trgm trên hai chuỗi đã qua build_key hoặc fold.
pub fn trigram_similarity(a: &str, b: &str) -> f32 {
    build_similarity(&build_trigrams(a), &build_trigrams(b))
}

/// Hai chuỗi cùng số từ giống nhau theo từng âm tiết (so âm tiết thứ i với âm tiết thứ i): âm tiết khác nhau chỉ được
/// sai một chữ (thêm, bớt, đổi một chữ, hoặc đảo hai chữ liền nhau). Âm tiết khác hẳn thường chính là chỗ phân biệt hai
/// giá trị anh em ("Bắc Trung Bộ" với "Nam Trung Bộ", "thời trang nữ" với "Thời trang nam"), nên không có ngoại lệ.
fn is_close_by_syllable(a: &str, b: &str) -> bool {
    a.split(' ')
        .zip(b.split(' '))
        .all(|(x, y)| build_edit_distance(x, y) <= 1)
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

/// Một phía của phép so bằng nhau: chữ so được (build_key), dạng bỏ dấu, có dấu hay không.
#[derive(Clone, Copy)]
struct Side<'a> {
    key: &'a str,
    folded: &'a str,
    accented: bool,
}

/// Điểm khi hai chuỗi bằng nhau: cả hai có dấu thì phải bằng đúng nguyên văn; một bên không dấu thì bằng sau bỏ dấu
/// (đúng nguyên văn vẫn được điểm cao nhất). text_only (từ ngắn của câu hỏi): chỉ nhận đúng nguyên văn.
fn build_equal_score(question: Side, target: Side, text_only: bool) -> Option<f32> {
    if question.key == target.key {
        Some(SCORE_SAME_TEXT)
    } else if question.accented && target.accented {
        None
    } else if !text_only && question.folded == target.folded {
        Some(SCORE_SAME_FOLDED)
    } else {
        None
    }
}

fn phrase_side(phrase: &Phrase) -> Side<'_> {
    Side {
        key: &phrase.text,
        folded: &phrase.folded,
        accented: phrase.accented,
    }
}

fn target_side(target: &ValueTarget) -> Side<'_> {
    Side {
        key: &target.key,
        folded: &target.folded,
        accented: target.accented,
    }
}

fn form_side(form: &TextForm) -> Side<'_> {
    Side {
        key: &form.key,
        folded: &form.folded,
        accented: form.accented,
    }
}

fn build_column_score(phrase: &Phrase, text: &ColumnText) -> Option<f32> {
    text.names
        .iter()
        .filter_map(|n| build_equal_score(phrase_side(phrase), form_side(n), phrase.text_only))
        .reduce(f32::max)
}

/// Một giá trị khớp một cụm, kèm điểm và similarity (1 với khớp bằng nhau).
pub struct ValueHit<'a> {
    pub target: &'a ValueTarget,
    pub score: f32,
    pub similarity: f32,
}

/// Điểm của khớp gần giữa cụm và giá trị cùng số từ; None khi không khớp theo luật ở đầu file.
fn build_fuzzy_score(
    phrase: &Phrase,
    trigrams: &[u64],
    target: &ValueTarget,
) -> Option<(f32, f32)> {
    let threshold = if phrase.len == 1 {
        VALUE_MIN_SIMILARITY
    } else {
        VALUE_MIN_SIMILARITY_WORDS
    };
    let folded_similarity = build_similarity(trigrams, &target.trigrams);
    if folded_similarity < threshold {
        return None;
    }
    // Cả hai có dấu: lấy số nhỏ hơn của so có dấu và so bỏ dấu, để so có dấu không bao giờ dễ hơn so bỏ dấu, và so
    // từng âm tiết trên chữ có dấu ("tây nguyen" với "Tây Nguyên": âm tiết "nguyen" sai một dấu).
    let (similarity, close) = if phrase.accented && target.accented {
        (
            folded_similarity.min(trigram_similarity(&phrase.text, &target.key)),
            is_close_by_syllable(&phrase.text, &target.key),
        )
    } else {
        (
            folded_similarity,
            is_close_by_syllable(&phrase.folded, &target.folded),
        )
    };
    (close && similarity >= threshold).then_some((SCORE_FUZZY_MAX * similarity, similarity))
}

/// Giá trị khớp trực tiếp một cụm: bằng nhau, hoặc khớp gần khi cụm được tìm gần (fuzzy). open: cột được tìm giá trị
/// (cột chiều không nhạy cảm, theo hồ sơ hiện tại).
fn build_direct_hits<'a>(
    phrase: &Phrase,
    index: &'a ValueIndex,
    open: &HashSet<Uuid>,
    fuzzy: bool,
) -> Vec<ValueHit<'a>> {
    let usable = |t: &ValueTarget| open.contains(&t.column_id) && (t.words != 1 || t.standalone);
    let mut hits: Vec<ValueHit> = index
        .equal_folded(&phrase.folded)
        .filter(|t| usable(t))
        .filter_map(|t| {
            let score = build_equal_score(phrase_side(phrase), target_side(t), phrase.text_only)?;
            Some(ValueHit {
                target: t,
                score,
                similarity: 1.0,
            })
        })
        .collect();
    if fuzzy && !phrase.exact_only {
        let trigrams = build_trigrams(&phrase.folded);
        for t in index.with_words(phrase.len) {
            // Chuỗi đã khớp bằng nhau thì thôi. Chuỗi bằng sau bỏ dấu mà khác dấu (cả hai có dấu, "tây nguyen" với
            // "Tây Nguyên") vẫn được khớp gần, theo luật từng âm tiết.
            let matched = hits.iter().any(|h| std::ptr::eq(h.target, t));
            if matched || !usable(t) {
                continue;
            }
            if let Some((score, similarity)) = build_fuzzy_score(phrase, &trigrams, t) {
                hits.push(ValueHit {
                    target: t,
                    score,
                    similarity,
                });
            }
        }
    }
    hits
}

/// Cụm là một đoạn từ liền nhau của tên hay bí danh ("region" trong Order Region), hoặc của mô tả với cụm từ hai từ
/// (một từ lẻ có trong quá nhiều mô tả). Khớp kiểu này chỉ giữ cột trong ngữ cảnh, không thành ứng viên của cụm: nó
/// sinh nhiều cột ngang điểm ("khách hàng" có trong bí danh của năm cột) và câu hỏi sẽ bị hỏi lại vô cớ.
pub fn is_partial_match(phrase: &Phrase, text: &ColumnText) -> bool {
    if phrase.exact_only {
        return false;
    }
    let inside = |form: &TextForm| {
        let (needle, haystack) = if phrase.accented && form.accented {
            (&phrase.text, &form.key)
        } else {
            (&phrase.folded, &form.folded)
        };
        format!(" {haystack} ").contains(&format!(" {needle} "))
    };
    text.names.iter().any(inside) || (phrase.len > 1 && inside(&text.description))
}

/// Một cụm mở đầu bằng tên hay bí danh của một cột, phần còn lại có thể là một giá trị của cột đó: "miền nam" là cột
/// "miền" với giá trị "Nam", "giới tính nữ" là cột "giới tính" với giá trị "Nữ". Đây là cách duy nhất một giá trị một
/// âm tiết tiếng Việt khớp được.
pub struct ColumnValuePair {
    /// Vị trí của cụm trong danh sách phrases.
    pub phrase: usize,
    /// Chỉ số cột trong columns.
    pub column: usize,
    tail: String,
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
            let head_side = Side {
                key: &head,
                folded: &head_folded,
                accented: head != head_folded,
            };
            for (column, text) in texts.iter().enumerate() {
                let is_column = text
                    .names
                    .iter()
                    .any(|n| build_equal_score(head_side, form_side(n), false).is_some());
                if is_column {
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

/// Giá trị khớp phần còn lại của một cặp cột và giá trị: chỉ khớp bằng nhau, trong đúng cột đó.
fn build_pair_hits<'a>(
    pair: &ColumnValuePair,
    column: &str,
    index: &'a ValueIndex,
    open: &HashSet<Uuid>,
) -> Vec<ValueHit<'a>> {
    let tail = Side {
        key: &pair.tail,
        folded: &pair.tail_folded,
        accented: pair.tail != pair.tail_folded,
    };
    index
        .equal_folded(&pair.tail_folded)
        .filter(|t| t.column_name == column && open.contains(&t.column_id))
        .filter_map(|t| {
            let score = build_equal_score(tail, target_side(t), false)?;
            Some(ValueHit {
                target: t,
                score,
                similarity: 1.0,
            })
        })
        .collect()
}

/// Một cụm cùng giá trị đã chấm điểm và cột ứng viên của nó.
pub struct ScoredPhrase<'a> {
    pub phrase: &'a Phrase,
    pub hits: Vec<ValueHit<'a>>,
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
    hits: &[ValueHit],
) -> Vec<(usize, f32)> {
    let mut best: HashMap<usize, f32> = HashMap::new();
    for (i, text) in texts.iter().enumerate() {
        if let Some(score) = build_column_score(phrase, text) {
            best.insert(i, score);
        }
    }
    for h in hits {
        let Some(i) = columns.iter().position(|c| c.name == h.target.column_name) else {
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

/// Mỗi cụm kèm giá trị đã chấm điểm và các cột ứng viên, theo thứ tự xét. fuzzy: chữ của các cụm được tìm khớp gần.
pub fn build_scored_phrases<'a>(
    phrases: &'a [Phrase],
    columns: &[ColumnProfile],
    texts: &[ColumnText],
    index: &'a ValueIndex,
    open: &HashSet<Uuid>,
    fuzzy: &HashSet<&str>,
) -> Vec<ScoredPhrase<'a>> {
    let pairs = build_column_value_pairs(phrases, texts);
    let mut scored: Vec<ScoredPhrase> = phrases
        .iter()
        .enumerate()
        .map(|(i, phrase)| {
            let mut hits =
                build_direct_hits(phrase, index, open, fuzzy.contains(phrase.text.as_str()));
            for pair in pairs.iter().filter(|p| p.phrase == i) {
                hits.extend(build_pair_hits(
                    pair,
                    &columns[pair.column].name,
                    index,
                    open,
                ));
            }
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
    // dài khớp gần kém hơn không được giữ chỗ của cụm ngắn khớp tốt hơn.
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
    hits: &[ValueHit],
    columns: &[&str],
) -> Vec<ValueMatch> {
    let mut kept: Vec<&ValueHit> = hits
        .iter()
        .filter(|h| columns.contains(&h.target.column_name.as_str()))
        .collect();
    kept.sort_by(|a, b| b.score.total_cmp(&a.score));
    let Some(best) = kept.first().map(|h| h.score) else {
        return Vec::new();
    };
    let mut values: Vec<ValueMatch> = Vec::new();
    for h in kept.into_iter().filter(|h| best - h.score < CLOSE_SCORE) {
        // Giá trị khớp qua nhiều bí danh chỉ ghi một lần, với khớp tốt nhất (đã xếp trước).
        let t = h.target;
        if values
            .iter()
            .any(|v| v.column == t.column_name && v.value == t.value)
        {
            continue;
        }
        values.push(ValueMatch {
            column: t.column_name.clone(),
            value: t.value.clone(),
            matched_text: t.matched_text.clone(),
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

    /// Cùng số với similarity() của pg_trgm đo trên Postgres 17.
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
        close("chau my", "chau au", 0.5);
        close("europ", "europe", 0.625);
        close("doanh thu", "viet nam", 0.0);
        close("từng châu", "châu âu", 0.417);
        close("tung chau", "chau au", 0.417);
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
        // Không ngoại lệ cho âm tiết khác hẳn: giá trị anh em không khớp nhau.
        assert!(!is_close_by_syllable("bắc trung bộ", "nam trung bộ"));
        assert!(!is_close_by_syllable("thoi trang nu", "thoi trang nam"));
        assert!(!is_close_by_syllable("hàng tiêu chuẩn", "giao tiêu chuẩn"));
    }
}
