// SPDX-License-Identifier: Apache-2.0
/* service/ask_values.rs - giá trị và bí danh của các cột chiều của một phiên bản, giữ sẵn theo phiên bản trong AppState
(ValueCache): danh sách giá trị của từng cột để đưa vào ngữ cảnh (service/ask_context.rs), và bảng tra khớp chính xác
cho gợi ý (find_exact).

Tải từ Postgres một lần cho mỗi phiên bản, vì một câu hỏi cần mọi giá trị của các cột được gửi, và chuẩn hóa (NFC, dấu
thanh, bỏ dấu) hàng chục nghìn chuỗi ở mỗi câu hỏi tốn hàng trăm ms.

Cache không cần xóa khi dữ liệu đổi: nội dung một phiên bản không đổi, và khóa cache gồm cả profile_status, nên khi
lớp ngữ nghĩa (#23) ghi bí danh và chuyển phiên bản sang semantic thì cache tải lại. Sửa bí danh tại chỗ sau đó (màn
sửa hồ sơ, chưa có) cần thêm cách bỏ phiên bản khỏi cache; tới lúc đó bí danh mới chỉ có khi phiên bản bị đẩy khỏi
cache. Cờ nhạy cảm và vai trò thì không phụ thuộc cache: lúc dùng vẫn lọc theo hồ sơ vừa đọc (ask_context.rs), để một
cột vừa đánh dấu nhạy cảm không bao giờ lộ giá trị qua cache cũ. */

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;

use super::ask_phrases::{build_key, fold, Phrase, PHRASE_MAX_WORDS};
use crate::auth::WorkspaceId;
use crate::engine::store::{DatasetVersion, ValueRow};
use crate::{error::AppError, state::AppState};

/// Số phiên bản cache giữ tối đa.
pub const VALUE_CACHE_VERSIONS: usize = 32;

/// Tổng số chuỗi (giá trị và bí danh) cache giữ tối đa, mọi phiên bản cộng lại. Mỗi chuỗi dài tới STRING_MAX_BYTES; chuỗi
/// dài 2 tới 5 từ giữ thêm ba bản (chữ đã khớp, dạng so được, dạng bỏ dấu), nên trần bộ nhớ vào khoảng 200 000 x 4 x 200
/// byte, chừng 160 MB; dữ liệu thường ngắn hơn nhiều. Phiên bản một mình đã vượt trần thì không vào cache, mỗi câu hỏi tải
/// lại.
pub const VALUE_CACHE_STRINGS: usize = 200_000;

/// Chuỗi dài hơn ngần này byte không giữ: không vào prompt (một giá trị dài là văn bản tự do, không phải tên nhóm), và
/// cụm của câu hỏi (tối đa 5 từ, mỗi từ tối đa 40 ký tự) gần như không bao giờ bằng nó. Giá trị bị bỏ thì bí danh của
/// nó cũng bỏ, và danh sách giá trị của cột không còn đủ (ColumnProfile::values_complete).
pub const STRING_MAX_BYTES: usize = 200;

/// Thời gian tối đa tải giá trị của một phiên bản từ Postgres. Quá thì câu hỏi đi tiếp không có danh sách giá trị, và
/// lần hỏi sau thử tải lại.
const VALUE_LOAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Một giá trị của cột, kèm bí danh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueEntry {
    pub value: String,
    pub aliases: Vec<String>,
}

/// Các giá trị của một cột, giá trị nhiều dòng nhất trước.
#[derive(Debug, Clone)]
pub struct ColumnValues {
    pub column_id: Uuid,
    pub column_name: String,
    pub values: Vec<ValueEntry>,
    /// Có giá trị bị bỏ vì dài quá STRING_MAX_BYTES.
    pub dropped: bool,
}

/// Một chuỗi (giá trị hoặc bí danh) dài 2 tới PHRASE_MAX_WORDS từ, dạng so được với cụm của câu hỏi.
#[derive(Debug, Clone)]
struct ExactTarget {
    column: usize,
    value: usize,
    matched_text: String,
    key: String,
    accented: bool,
}

/// Các giá trị của một phiên bản, theo cột, cộng bảng tra khớp chính xác theo dạng bỏ dấu.
#[derive(Debug, Default)]
pub struct ValueIndex {
    pub columns: Vec<ColumnValues>,
    targets: Vec<ExactTarget>,
    by_folded: HashMap<String, Vec<usize>>,
    /// Số chuỗi đã giữ, giá trị và bí danh: đơn vị của VALUE_CACHE_STRINGS.
    pub strings: usize,
    /// Đã tải được giá trị của phiên bản. false với chỉ mục rỗng khi tải quá thời gian: khi đó không cột nào có danh
    /// sách đủ.
    pub loaded: bool,
}

/// Một giá trị khớp chính xác một cụm của câu hỏi.
pub struct ExactMatch<'a> {
    pub column: &'a ColumnValues,
    pub entry: &'a ValueEntry,
    /// Chữ đã khớp: chính giá trị hoặc bí danh.
    pub matched_text: &'a str,
}

impl ValueIndex {
    /// rows theo cột, trong mỗi cột giá trị nhiều dòng nhất trước (Store::list_value_rows).
    pub fn build(rows: &[ValueRow]) -> ValueIndex {
        let mut index = ValueIndex {
            loaded: true,
            ..ValueIndex::default()
        };
        for row in rows {
            let column = match index.columns.last() {
                Some(last) if last.column_id == row.column_id => index.columns.len() - 1,
                _ => {
                    index.columns.push(ColumnValues {
                        column_id: row.column_id,
                        column_name: row.column_name.clone(),
                        values: Vec::new(),
                        dropped: false,
                    });
                    index.columns.len() - 1
                }
            };
            if row.value.len() > STRING_MAX_BYTES {
                index.columns[column].dropped = true;
                continue;
            }
            let aliases: Vec<String> = row
                .aliases
                .iter()
                .filter(|a| a.len() <= STRING_MAX_BYTES)
                .cloned()
                .collect();
            let value = index.columns[column].values.len();
            for text in std::iter::once(&row.value).chain(&aliases) {
                index.add_target(column, value, text);
            }
            index.strings += 1 + aliases.len();
            index.columns[column].values.push(ValueEntry {
                value: row.value.clone(),
                aliases,
            });
        }
        index
    }

    /// Thêm một chuỗi vào bảng tra nếu nó dài 2 tới PHRASE_MAX_WORDS từ: chỉ cụm như vậy được tìm gợi ý.
    fn add_target(&mut self, column: usize, value: usize, text: &str) {
        let key = build_key(text);
        let words = key.split(' ').filter(|w| !w.is_empty()).count();
        if !(2..=PHRASE_MAX_WORDS).contains(&words) {
            return;
        }
        let folded = fold(text);
        let i = self.targets.len();
        self.by_folded.entry(folded.clone()).or_default().push(i);
        self.targets.push(ExactTarget {
            column,
            value,
            matched_text: text.to_string(),
            accented: key != folded,
            key,
        });
    }

    /// Giá trị khớp chính xác một cụm: cả hai có dấu thì phải bằng nhau đúng dấu (dấu thanh đã đặt một chỗ); một bên
    /// không dấu thì bằng nhau sau bỏ dấu ("Hà Nội" với "Ha Noi"), vì bên không dấu không mang thông tin dấu để phân
    /// biệt.
    pub fn find_exact<'a>(&'a self, phrase: &Phrase) -> Vec<ExactMatch<'a>> {
        self.by_folded
            .get(&phrase.folded)
            .into_iter()
            .flatten()
            .map(|&i| &self.targets[i])
            .filter(|t| !(phrase.accented && t.accented) || t.key == phrase.key)
            .map(|t| {
                let column = &self.columns[t.column];
                ExactMatch {
                    column,
                    entry: &column.values[t.value],
                    matched_text: &t.matched_text,
                }
            })
            .collect()
    }
}

/// Khóa cache: workspace, phiên bản, profile_status.
type CacheKey = (Uuid, Uuid, String);

/// Cache ValueIndex theo phiên bản, tối đa VALUE_CACHE_VERSIONS phiên bản và VALUE_CACHE_STRINGS chuỗi; đầy thì bỏ
/// phiên bản thêm vào lâu nhất.
pub struct ValueCache {
    // Mutex chuẩn là đủ: chỉ giữ khóa trong lúc tìm hay thêm một phần tử, không qua await.
    entries: Mutex<VecDeque<(CacheKey, Arc<ValueIndex>)>>,
    max_versions: usize,
    max_strings: usize,
}

impl ValueCache {
    pub fn new() -> Self {
        Self::with_limits(VALUE_CACHE_VERSIONS, VALUE_CACHE_STRINGS)
    }

    fn with_limits(max_versions: usize, max_strings: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            max_versions,
            max_strings,
        }
    }

    fn get(&self, key: &CacheKey) -> Option<Arc<ValueIndex>> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, index)| index.clone())
    }

    fn insert(&self, key: CacheKey, index: Arc<ValueIndex>) {
        if index.strings > self.max_strings {
            return;
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        // Hai câu hỏi cùng lúc trên một phiên bản chưa có trong cache cùng tải; giữ một bản.
        entries.retain(|(k, _)| k != &key);
        let total = |entries: &VecDeque<(CacheKey, Arc<ValueIndex>)>| -> usize {
            entries.iter().map(|(_, i)| i.strings).sum()
        };
        while entries.len() >= self.max_versions
            || total(&entries) + index.strings > self.max_strings
        {
            entries.pop_front();
        }
        entries.push_back((key, index));
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

impl Default for ValueCache {
    fn default() -> Self {
        Self::new()
    }
}

/// ValueIndex của một phiên bản, lấy từ cache hoặc tải từ Postgres. Tải quá thời gian thì trả chỉ mục rỗng và không
/// lưu vào cache.
pub async fn load_value_index(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
) -> Result<Arc<ValueIndex>, AppError> {
    let key = (ws.as_uuid(), version.id, version.profile_status.clone());
    if let Some(index) = state.value_cache.get(&key) {
        return Ok(index);
    }
    let Some(rows) = state
        .store
        .list_value_rows(ws, version.id, VALUE_LOAD_TIMEOUT)
        .await?
    else {
        return Ok(Arc::new(ValueIndex::default()));
    };
    // Chuẩn hóa hàng chục nghìn chuỗi tốn CPU; chạy trong spawn_blocking để không chặn các request khác.
    let index = tokio::task::spawn_blocking(move || ValueIndex::build(&rows))
        .await
        .map_err(|e| AppError::Internal(format!("value index task: {e}")))?;
    let index = Arc::new(index);
    state.value_cache.insert(key, index.clone());
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::super::ask_phrases::build_phrases;
    use super::*;

    fn row(column: u128, value: &str, aliases: &[&str]) -> ValueRow {
        ValueRow {
            column_id: Uuid::from_u128(column),
            column_name: format!("c{column}"),
            value: value.to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        }
    }

    fn exact(index: &ValueIndex, question: &str, phrase: &str) -> Vec<String> {
        let phrases = build_phrases(question);
        let phrase = phrases.iter().find(|p| p.text == phrase).unwrap();
        index
            .find_exact(phrase)
            .iter()
            .map(|m| m.entry.value.clone())
            .collect()
    }

    #[test]
    fn values_stay_in_order_and_long_strings_are_dropped() {
        let long = "x".repeat(STRING_MAX_BYTES + 1);
        let index = ValueIndex::build(&[
            row(1, "Europe", &["châu Âu"]),
            row(1, "LATAM", &[]),
            row(2, &long, &["dài"]),
            row(2, "Ngắn", &[&long]),
        ]);
        assert_eq!(index.columns.len(), 2);
        assert_eq!(index.columns[0].values[0].value, "Europe");
        assert_eq!(index.columns[0].values[0].aliases, ["châu Âu"]);
        assert!(!index.columns[0].dropped);
        assert_eq!(index.columns[1].values.len(), 1);
        assert!(index.columns[1].values[0].aliases.is_empty());
        assert!(index.columns[1].dropped);
        assert_eq!(index.strings, 4);
    }

    #[test]
    fn exact_matches_compare_accents_only_when_both_sides_have_them() {
        let index = ValueIndex::build(&[
            row(1, "Ha Noi", &[]),
            row(1, "Hòa Bình", &[]),
            row(2, "South", &["miền Nam"]),
            row(3, "Europe", &[]),
        ]);
        assert_eq!(exact(&index, "doanh thu Hà Nội", "hà nội"), ["Ha Noi"]);
        assert_eq!(
            exact(&index, "doanh thu Hoà Bình", "hoà bình"),
            ["Hòa Bình"]
        );
        assert_eq!(exact(&index, "doanh thu miền Nam", "miền nam"), ["South"]);
        // Hai bên có dấu, khác dấu: không khớp.
        assert!(exact(&index, "doanh thu từng miền năm 2017", "miền năm").is_empty());
        // Giá trị một từ không vào bảng tra.
        assert!(exact(&index, "doanh thu Europe", "europe").is_empty());
    }

    #[test]
    fn cache_keeps_the_newest_versions_within_limits() {
        let cache = ValueCache::with_limits(3, 10);
        let key = |i: u128| (Uuid::nil(), Uuid::from_u128(i), "structure".to_string());
        let sized = |n: usize| {
            Arc::new(ValueIndex {
                strings: n,
                ..ValueIndex::default()
            })
        };
        for i in 0..5 {
            cache.insert(key(i), sized(2));
        }
        // Trần số phiên bản: còn 3 phiên bản mới nhất.
        assert_eq!(cache.len(), 3);
        assert!(cache.get(&key(1)).is_none() && cache.get(&key(4)).is_some());
        // Trần số chuỗi: thêm 7 chuỗi thì bỏ phiên bản cũ tới khi tổng không quá 10.
        cache.insert(key(5), sized(7));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key(3)).is_none() && cache.get(&key(4)).is_some());
        // Một phiên bản vượt trần thì không vào cache, không đẩy phiên bản khác ra.
        cache.insert(key(6), sized(11));
        assert!(cache.get(&key(6)).is_none());
        assert_eq!(cache.len(), 2);
        // Khóa gồm cả workspace và profile_status.
        let other_ws = (
            Uuid::from_u128(9),
            Uuid::from_u128(4),
            "structure".to_string(),
        );
        assert!(cache.get(&other_ws).is_none());
        let semantic = (Uuid::nil(), Uuid::from_u128(4), "semantic".to_string());
        assert!(cache.get(&semantic).is_none());
    }
}
