// SPDX-License-Identifier: Apache-2.0
/* service/ask_values.rs - giá trị và bí danh của các cột chiều của một phiên bản, chuẩn hóa sẵn để so với câu hỏi
(service/ask_match.rs), và cache giữ chúng theo phiên bản trong AppState.

Vì sao so trong bộ nhớ thay vì trong Postgres: câu SQL trước đó bỏ dấu lại mọi giá trị ở mỗi câu hỏi (trên 20 000 giá
trị với 2 bí danh mỗi giá trị, riêng bước đó khoảng 240 ms, cả câu khoảng 470 ms khi máy rảnh, tới 1,5 đến 1,9 giây khi
máy bận), và LIMIT của nó cắt khớp gần không theo điểm. Giữ sẵn bản đã chuẩn hóa thì một câu hỏi chỉ còn tra bảng băm
cho khớp bằng nhau và so trigram với các giá trị cùng số từ cho khớp gần, không có LIMIT. Cách tính similarity là của
pg_trgm, viết lại ở Rust (build_similarity), cùng số với similarity() của Postgres.

Đã đo (bản release, máy phát triển 16 nhân, Postgres 17 của docker compose; 100 cột chiều, mỗi cột 200 giá trị 1 đến
3 âm tiết, 2 bí danh mỗi giá trị, tức 60 000 chuỗi; thời gian cả build_ask_context, 6 lần mỗi câu): lần hỏi đầu của một
phiên bản khoảng 310 đến 360 ms (tải 20 000 dòng khoảng 120 ms, dựng chỉ mục khoảng 180 ms); các lần sau, câu 40 từ
khoảng 63 ms, câu "doanh thu chau binh" khoảng 22 ms, câu có một từ 39 ký tự khoảng 17 ms. Bản debug chậm hơn khoảng 5
lần (dựng chỉ mục 1,2 giây, câu 40 từ khoảng 360 ms).

Cache không cần xóa khi dữ liệu đổi: nội dung một phiên bản không đổi, và khóa cache gồm cả profile_status, nên khi
lớp ngữ nghĩa (#23) ghi bí danh và chuyển phiên bản sang semantic thì cache tải lại. Sửa bí danh hay cờ nhạy cảm sau đó
(màn sửa hồ sơ, chưa có) phải gọi ValueCache::forget. Dù vậy, lúc so vẫn lọc theo cờ nhạy cảm hiện tại của cột
(ask_context.rs), để một cột vừa đánh dấu nhạy cảm không bao giờ lộ giá trị qua cache cũ. */

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;

use super::ask_phrases::{build_key, fold, has_accent};
use crate::auth::WorkspaceId;
use crate::engine::store::{DatasetVersion, ValueRow};
use crate::{error::AppError, state::AppState};

/// Số phiên bản cache giữ; thêm phiên bản thứ 33 thì bỏ phiên bản thêm vào lâu nhất. Một phiên bản 100 cột chiều mỗi
/// cột 200 giá trị, 2 bí danh mỗi giá trị, là 60 000 chuỗi, khoảng vài chục MB kể cả trigram.
pub const VALUE_CACHE_VERSIONS: usize = 32;

/// Thời gian tối đa tải giá trị của một phiên bản từ Postgres. Quá thì câu hỏi đi tiếp không có giá trị đã ánh xạ, và
/// lần hỏi sau thử tải lại.
const VALUE_LOAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Giá trị một âm tiết, không chữ số, ngắn hơn ngần này ký tự thì chỉ khớp qua "tên cột + giá trị" (xem standalone).
const STANDALONE_MIN_CHARS: usize = 4;

/// Một chuỗi so được với câu hỏi: một giá trị, hoặc một bí danh của nó.
#[derive(Debug, Clone)]
pub struct ValueTarget {
    pub column_id: Uuid,
    pub column_name: String,
    pub value: String,
    /// Chữ đã khớp: chính giá trị hoặc bí danh.
    pub matched_text: String,
    /// build_key của matched_text: chữ thường, NFC, dấu thanh đặt một chỗ.
    pub key: String,
    pub folded: String,
    pub words: usize,
    /// So có dấu được: chữ có dấu, hoặc là giá trị không dấu của một cột có giá trị viết có dấu. Cột có "Bắc", "Trung",
    /// "Nam" là cột viết có dấu, nên "Nam" của nó là Nam chứ không phải "năm" gõ thiếu dấu.
    pub accented: bool,
    /// Giá trị một từ đứng một mình khớp được, không cần tên cột đứng trước. Âm tiết tiếng Việt một mình ("Nam", "Nữ",
    /// "Trung") hay là một phần của tên dài hơn trong câu ("Việt Nam", "Quảng Nam", "Trung Quốc"), nên chỉ khớp qua
    /// "giới tính nam", "miền trung". Đứng một mình được: mã có chữ số ("B2B", "5G"), và từ không dấu từ
    /// STANDALONE_MIN_CHARS ký tự trong cột không viết có dấu ("Europe", "LATAM").
    pub standalone: bool,
    /// Trigram của folded, xếp tăng dần, không trùng (build_trigrams).
    pub trigrams: Vec<u64>,
}

/// Các chuỗi so được của một phiên bản, cộng bảng tra theo dạng bỏ dấu và theo số từ.
#[derive(Debug, Default)]
pub struct ValueIndex {
    pub targets: Vec<ValueTarget>,
    by_folded: HashMap<String, Vec<usize>>,
    by_words: HashMap<usize, Vec<usize>>,
}

impl ValueIndex {
    pub fn build(rows: &[ValueRow]) -> ValueIndex {
        // Cột viết có dấu: có ít nhất một giá trị (không tính bí danh) có dấu. Bí danh tiếng Việt của cột giá trị tiếng
        // Anh ("châu Âu" của Europe) không làm cột thành viết có dấu.
        let mut accented_columns: HashMap<Uuid, bool> = HashMap::new();
        for row in rows {
            *accented_columns.entry(row.column_id).or_default() |= has_accent(&row.value);
        }
        let mut index = ValueIndex::default();
        for row in rows {
            let column_accented = accented_columns[&row.column_id];
            for text in std::iter::once(&row.value).chain(&row.aliases) {
                let target = build_target(row, text, column_accented);
                if target.words == 0 {
                    continue;
                }
                let i = index.targets.len();
                index
                    .by_folded
                    .entry(target.folded.clone())
                    .or_default()
                    .push(i);
                index.by_words.entry(target.words).or_default().push(i);
                index.targets.push(target);
            }
        }
        index
    }

    /// Các chuỗi bằng nhau sau bỏ dấu.
    pub fn equal_folded<'a>(&'a self, folded: &str) -> impl Iterator<Item = &'a ValueTarget> + 'a {
        self.by_folded
            .get(folded)
            .into_iter()
            .flatten()
            .map(|&i| &self.targets[i])
    }

    /// Các chuỗi có đúng số từ.
    pub fn with_words(&self, words: usize) -> impl Iterator<Item = &ValueTarget> + '_ {
        self.by_words
            .get(&words)
            .into_iter()
            .flatten()
            .map(|&i| &self.targets[i])
    }
}

fn build_target(row: &ValueRow, text: &str, column_accented: bool) -> ValueTarget {
    let key = build_key(text);
    let folded = fold(text);
    let words = key.split(' ').filter(|w| !w.is_empty()).count();
    let text_accented = key != folded;
    let standalone = words != 1
        || key.chars().any(|c| c.is_ascii_digit())
        || (!column_accented && !text_accented && folded.chars().count() >= STANDALONE_MIN_CHARS);
    ValueTarget {
        column_id: row.column_id,
        column_name: row.column_name.clone(),
        value: row.value.clone(),
        matched_text: text.to_string(),
        trigrams: build_trigrams(&folded),
        accented: text_accented || column_accented,
        standalone,
        key,
        folded,
        words,
    }
}

/// Trigram theo cách của pg_trgm, trên chuỗi đã qua build_key hoặc fold: mỗi từ thêm hai dấu cách đầu, một dấu cách
/// cuối, lấy mọi đoạn 3 ký tự. Mỗi đoạn gói thành một số u64 (mỗi ký tự Unicode vừa 21 bit), xếp tăng dần, bỏ trùng.
pub fn build_trigrams(text: &str) -> Vec<u64> {
    let mut trigrams = Vec::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let padded: Vec<char> = "  ".chars().chain(word.chars()).chain([' ']).collect();
        for w in padded.windows(3) {
            trigrams.push(((w[0] as u64) << 42) | ((w[1] as u64) << 21) | (w[2] as u64));
        }
    }
    trigrams.sort_unstable();
    trigrams.dedup();
    trigrams
}

/// similarity của pg_trgm: số trigram chung chia số trigram của cả hai. Hai danh sách đã xếp và bỏ trùng.
pub fn build_similarity(left: &[u64], right: &[u64]) -> f32 {
    let (mut i, mut j, mut common) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let total = left.len() + right.len() - common;
    if total == 0 {
        return 0.0;
    }
    common as f32 / total as f32
}

/// Khóa cache: workspace, phiên bản, profile_status.
type CacheKey = (Uuid, Uuid, String);

/// Cache ValueIndex theo phiên bản, tối đa VALUE_CACHE_VERSIONS phiên bản, bỏ phiên bản thêm vào lâu nhất.
pub struct ValueCache {
    // Mutex chuẩn là đủ: chỉ giữ khóa trong lúc tìm hay thêm một phần tử, không qua await.
    entries: Mutex<VecDeque<(CacheKey, Arc<ValueIndex>)>>,
}

impl ValueCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
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
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        // Hai câu hỏi cùng lúc trên một phiên bản chưa có trong cache cùng tải; giữ một bản.
        entries.retain(|(k, _)| k != &key);
        if entries.len() == VALUE_CACHE_VERSIONS {
            entries.pop_front();
        }
        entries.push_back((key, index));
    }

    /// Bỏ mọi bản của một phiên bản, khi hồ sơ của nó được sửa tại chỗ (màn sửa hồ sơ, chưa có).
    pub fn forget(&self, version_id: Uuid) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|((_, version, _), _)| *version != version_id);
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
    use super::*;

    fn row(column: &str, value: &str, aliases: &[&str]) -> ValueRow {
        ValueRow {
            column_id: Uuid::from_u128(
                column
                    .bytes()
                    .fold(0u128, |id, b| id.wrapping_mul(131).wrapping_add(b as u128)),
            ),
            column_name: column.to_string(),
            value: value.to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn single_syllables_need_their_column_name() {
        let index = ValueIndex::build(&[
            row("gender", "Nam", &[]),
            row("gender", "Nữ", &[]),
            row("market", "Europe", &["châu Âu"]),
            row("market", "USCA", &[]),
            row("segmentx", "B2B", &[]),
            row("areaxxxx", "Mien Bac", &[]),
            row("regionxxxx", "Nam", &[]),
        ]);
        let get = |column: &str, text: &str| {
            index
                .targets
                .iter()
                .find(|t| t.column_name == column && t.matched_text == text)
                .unwrap()
        };
        // Cột viết có dấu: "Nam" so có dấu và không đứng một mình.
        assert!(get("gender", "Nam").accented && !get("gender", "Nam").standalone);
        // Cột không dấu: "Nam" (3 chữ) vẫn không đứng một mình, "Europe", "USCA" thì được.
        assert!(!get("regionxxxx", "Nam").standalone && !get("regionxxxx", "Nam").accented);
        assert!(get("market", "Europe").standalone && get("market", "USCA").standalone);
        // Bí danh có dấu không làm cột Market thành cột viết có dấu.
        assert!(!get("market", "Europe").accented && get("market", "châu Âu").accented);
        assert!(get("segmentx", "B2B").standalone);
        assert!(get("areaxxxx", "Mien Bac").standalone && !get("areaxxxx", "Mien Bac").accented);
        assert_eq!(index.equal_folded("nam").count(), 2);
        assert_eq!(index.with_words(2).count(), 2);
    }

    #[test]
    fn cache_keeps_the_newest_versions() {
        let cache = ValueCache::new();
        let key = |i: u128| (Uuid::nil(), Uuid::from_u128(i), "structure".to_string());
        for i in 0..(VALUE_CACHE_VERSIONS as u128 + 3) {
            cache.insert(key(i), Arc::new(ValueIndex::default()));
        }
        assert_eq!(cache.len(), VALUE_CACHE_VERSIONS);
        assert!(cache.get(&key(0)).is_none());
        assert!(cache.get(&key(VALUE_CACHE_VERSIONS as u128 + 2)).is_some());
        cache.forget(Uuid::from_u128(5));
        assert!(cache.get(&key(5)).is_none());
        // Khóa gồm cả workspace và profile_status.
        let other_ws = (
            Uuid::from_u128(9),
            Uuid::from_u128(10),
            "structure".to_string(),
        );
        assert!(cache.get(&other_ws).is_none());
        let semantic = (Uuid::nil(), Uuid::from_u128(10), "semantic".to_string());
        assert!(cache.get(&semantic).is_none());
    }
}
