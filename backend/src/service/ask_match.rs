// SPDX-License-Identifier: Apache-2.0
/* service/ask_match.rs - so các cụm của câu hỏi (service/ask_phrases.rs) với tên, bí danh, mô tả của cột để chọn cột,
và tìm gợi ý giá trị khớp chính xác (service/ask_values.rs). service/ask_context.rs dùng kết quả này. Hàm thuần, không
đọc database.

Chỉ có khớp bằng nhau: dấu thanh đặt một chỗ cố định ở hai phía; cả hai có dấu thì phải bằng đúng dấu, một bên không
dấu thì bằng sau bỏ dấu ("khu vực" với cột khu_vuc). Ánh xạ chữ người dùng sang giá trị là việc của mô hình (#29). */

use std::collections::HashSet;

use uuid::Uuid;

use super::ask_context::{ColumnProfile, ValueHint};
use super::ask_phrases::{build_key, fold, Phrase};
use super::ask_values::ValueIndex;

// Điểm của tên hay bí danh cột bằng một cụm: đúng nguyên văn (cả dấu), hoặc bằng sau bỏ dấu khi một bên không dấu. Hai
// bậc cách nhau quá CLOSE_SCORE (ask_context.rs), nên khớp đúng nguyên văn luôn thắng mà không hỏi lại.
const SCORE_SAME_TEXT: f32 = 1.0;
const SCORE_SAME_FOLDED: f32 = 0.85;

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

/// Điểm của cột khi cụm bằng tên hoặc một bí danh của nó; None khi không bằng.
fn build_column_score(phrase: &Phrase, text: &ColumnText) -> Option<f32> {
    text.names
        .iter()
        .filter_map(|n| {
            if n.key == phrase.key {
                Some(SCORE_SAME_TEXT)
            } else if !(phrase.accented && n.accented) && n.folded == phrase.folded {
                Some(SCORE_SAME_FOLDED)
            } else {
                None
            }
        })
        .reduce(f32::max)
}

/// Cụm là một đoạn từ liền nhau của tên hay bí danh ("region" trong Order Region), hoặc của mô tả với cụm từ hai từ
/// (một từ lẻ có trong quá nhiều mô tả). Khớp kiểu này chỉ giữ cột trong ngữ cảnh, không thành ứng viên của cụm: nó
/// sinh nhiều cột ngang điểm ("khách hàng" có trong bí danh của năm cột) và câu hỏi sẽ bị hỏi lại vô cớ. Cụm có từ dừng
/// không xét, vì "theo", "của" có trong mô tả nào cũng được.
pub fn is_partial_match(phrase: &Phrase, text: &ColumnText) -> bool {
    if phrase.has_stop {
        return false;
    }
    let inside = |form: &TextForm| {
        let (needle, haystack) = if phrase.accented && form.accented {
            (&phrase.key, &form.key)
        } else {
            (&phrase.folded, &form.folded)
        };
        format!(" {haystack} ").contains(&format!(" {needle} "))
    };
    text.names.iter().any(inside) || (phrase.len > 1 && inside(&text.description))
}

/// Một cụm cùng các cột ứng viên của nó (cột có tên hay bí danh bằng cụm), điểm cao trước; phần tử là chỉ số trong
/// columns.
pub struct ScoredPhrase<'a> {
    pub phrase: &'a Phrase,
    pub candidates: Vec<(usize, f32)>,
}

/// Mỗi cụm kèm cột ứng viên, theo thứ tự xét: cụm dài trước ("trạng thái giao hàng" trước "giao hàng"), cùng độ dài
/// thì điểm cao trước, rồi theo thứ tự trong câu.
pub fn build_scored_phrases<'a>(
    phrases: &'a [Phrase],
    texts: &[ColumnText],
) -> Vec<ScoredPhrase<'a>> {
    let mut scored: Vec<ScoredPhrase> = phrases
        .iter()
        .map(|phrase| {
            let mut candidates: Vec<(usize, f32)> = texts
                .iter()
                .enumerate()
                .filter_map(|(i, text)| Some((i, build_column_score(phrase, text)?)))
                .collect();
            candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            ScoredPhrase { phrase, candidates }
        })
        .collect();
    let best = |s: &ScoredPhrase| s.candidates.first().map_or(0.0, |c| c.1);
    scored.sort_by(|a, b| {
        b.phrase
            .len
            .cmp(&a.phrase.len)
            .then(best(b).total_cmp(&best(a)))
            .then(a.phrase.start.cmp(&b.phrase.start))
    });
    scored
}

/// Gợi ý giá trị: cụm từ hai âm tiết, không từ dừng (Phrase::is_hint_phrase), bằng chính xác một giá trị hay bí danh
/// của cột trong open (cột chiều không nhạy cảm theo hồ sơ hiện tại). Cụm dài trước; cụm đè lên chữ của một cụm đã có
/// gợi ý thì bỏ ("châu á thái bình dương" có gợi ý thì không xét "thái bình dương" nữa). Không chọn hộ giữa các cột: cụm
/// bằng giá trị ở hai cột thì có hai gợi ý, mô hình quyết định.
///
/// Rủi ro đã biết: khớp chính xác hai âm tiết vẫn gợi ý nhầm khi giá trị trùng một cụm từ thường (giá trị "Trung Bình"
/// với câu "phí vận chuyển trung bình"). Gợi ý chỉ là gợi ý: mô hình quyết định, #33 kiểm `terms` với column_values.
pub fn build_value_hints(
    phrases: &[Phrase],
    index: &ValueIndex,
    open: &HashSet<Uuid>,
) -> Vec<ValueHint> {
    let mut hints: Vec<ValueHint> = Vec::new();
    let mut used: Vec<&Phrase> = Vec::new();
    // build_phrases trả cụm dài trước, cùng độ dài theo thứ tự trong câu.
    for phrase in phrases.iter().filter(|p| p.is_hint_phrase()) {
        if used.iter().any(|u| u.overlaps(phrase)) {
            continue;
        }
        let mut found = false;
        for m in index.find_exact(phrase) {
            if !open.contains(&m.column.column_id) {
                continue;
            }
            found = true;
            let seen = hints
                .iter()
                .any(|h| h.column == m.column.column_name && h.value == m.entry.value);
            if !seen {
                hints.push(ValueHint {
                    phrase: phrase.text.clone(),
                    column: m.column.column_name.clone(),
                    value: m.entry.value.clone(),
                    matched_text: m.matched_text.to_string(),
                });
            }
        }
        if found {
            used.push(phrase);
        }
    }
    hints
}
