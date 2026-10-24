// SPDX-License-Identifier: Apache-2.0
/* service/ask_match.rs - chấm điểm khớp giữa một cụm của câu hỏi (service/ask_phrases.rs) với tên, bí danh, mô tả của
cột và với giá trị Postgres đã tìm (Store::find_values). service/ask_context.rs dùng điểm này để chọn cột, hỏi lại
khi mơ hồ và chọn giá trị đưa vào ngữ cảnh. Hàm thuần, không đọc database. */

use std::collections::HashMap;

use super::ask_context::{ColumnProfile, ValueMatch, CLOSE_SCORE};
use super::ask_phrases::{build_words, fold, Phrase};
use crate::engine::store::ValueHit;

/// Số giá trị tối đa giữ cho một cụm ("miền" khớp gần cả miền Bắc, miền Nam, miền Trung).
const VALUES_PER_PHRASE: usize = 5;

// Điểm của một khớp, từ 0 tới 1. Ba bậc cách nhau quá CLOSE_SCORE, nên khớp ở bậc trên luôn thắng khớp ở bậc dưới:
// chữ thường còn dấu bằng nhau, rồi bằng nhau sau khi bỏ dấu, rồi khớp gần (tối đa 0.7, kể cả khi similarity là 1 vì
// chỉ đảo thứ tự từ).
const SCORE_SAME_TEXT: f32 = 1.0;
const SCORE_SAME_FOLDED: f32 = 0.85;
const SCORE_FUZZY_MAX: f32 = 0.7;

/// Tên, bí danh, mô tả của một cột, viết sẵn ở hai dạng để so với cụm.
pub struct ColumnText {
    /// Tên và từng bí danh, chữ thường còn dấu, các từ cách nhau một dấu cách.
    names: Vec<String>,
    /// Như names, đã bỏ dấu.
    folded_names: Vec<String>,
    folded_description: String,
}

pub fn build_column_text(column: &ColumnProfile) -> ColumnText {
    let names: Vec<String> = std::iter::once(&column.name)
        .chain(&column.aliases)
        .map(|n| build_words(n).join(" "))
        .collect();
    ColumnText {
        folded_names: names.iter().map(|n| fold(n)).collect(),
        names,
        folded_description: fold(column.description.as_deref().unwrap_or("")),
    }
}

pub struct ScoredHit<'a> {
    hit: &'a ValueHit,
    score: f32,
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

    /// Khớp tốt nhất là bằng nhau (có dấu hoặc sau khi bỏ dấu), không phải khớp gần.
    pub fn is_exact(&self) -> bool {
        self.best() >= SCORE_SAME_FOLDED
    }
}

fn build_value_score(phrase: &Phrase, hit: &ValueHit) -> f32 {
    if build_words(&hit.matched_text).join(" ") == phrase.text {
        SCORE_SAME_TEXT
    } else if hit.same_folded {
        SCORE_SAME_FOLDED
    } else {
        SCORE_FUZZY_MAX * hit.similarity
    }
}

/// Điểm của cột khi cụm bằng tên hoặc một bí danh của nó; None khi không bằng.
fn build_column_score(phrase: &Phrase, text: &ColumnText) -> Option<f32> {
    if text.names.contains(&phrase.text) {
        Some(SCORE_SAME_TEXT)
    } else if text.folded_names.contains(&phrase.folded) {
        Some(SCORE_SAME_FOLDED)
    } else {
        None
    }
}

/// Cụm là một đoạn từ liền nhau của tên hay bí danh ("region" trong Order Region), hoặc của mô tả với cụm từ hai từ
/// (một từ lẻ có trong quá nhiều mô tả). Khớp kiểu này chỉ giữ cột trong ngữ cảnh, không thành ứng viên của cụm: nó
/// sinh nhiều cột ngang điểm ("khách hàng" có trong bí danh của năm cột) và câu hỏi sẽ bị hỏi lại vô cớ.
pub fn is_partial_match(phrase: &Phrase, text: &ColumnText) -> bool {
    if phrase.exact_only {
        return false;
    }
    let inside = |haystack: &str| format!(" {haystack} ").contains(&format!(" {} ", phrase.folded));
    text.folded_names.iter().any(|n| inside(n))
        || (phrase.len > 1 && inside(&text.folded_description))
}

/// Cột ứng viên của một cụm và điểm tốt nhất của mỗi cột, qua tên, bí danh hoặc giá trị; điểm cao trước, bằng điểm
/// thì theo thứ tự cột. Phần tử là chỉ số trong columns.
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
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    candidates
}

/// Mỗi cụm kèm giá trị đã tìm cho nó và các cột ứng viên, theo thứ tự xét.
pub fn build_scored_phrases<'a>(
    phrases: &'a [Phrase],
    columns: &[ColumnProfile],
    texts: &[ColumnText],
    hits: &'a [ValueHit],
) -> Vec<ScoredPhrase<'a>> {
    let mut scored: Vec<ScoredPhrase> = phrases
        .iter()
        .map(|phrase| {
            let hits: Vec<ScoredHit> = hits
                .iter()
                .filter(|h| h.phrase == phrase.text)
                .map(|h| ScoredHit {
                    hit: h,
                    score: build_value_score(phrase, h),
                })
                .collect();
            let candidates = build_candidates(phrase, columns, texts, &hits);
            ScoredPhrase {
                phrase,
                hits,
                candidates,
            }
        })
        .collect();
    // Cụm khớp bằng nhau trước cụm chỉ khớp gần; trong mỗi nhóm, cụm dài trước, rồi điểm cao trước. Xếp theo độ dài
    // trước thì một cụm dài khớp gần sẽ giữ chỗ của cụm ngắn hơn khớp đúng ("thu châu âu" khớp gần "châu Âu" và chặn
    // mất "doanh thu"); xếp theo điểm trước thì "miền" (một từ, khớp gần cả miền Bắc) đi trước "mien nma" (gõ sai).
    scored.sort_by(|a, b| {
        b.is_exact()
            .cmp(&a.is_exact())
            .then(b.phrase.len.cmp(&a.phrase.len))
            .then(b.best().total_cmp(&a.best()))
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
