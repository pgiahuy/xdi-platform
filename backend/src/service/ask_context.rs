// SPDX-License-Identifier: Apache-2.0
/* service/ask_context.rs - ngữ cảnh cho một câu hỏi: những cột và giá trị mà câu hỏi nhắc tới, lấy từ hồ sơ cột của
đúng phiên bản được hỏi (engine/store/columns.rs, do service/profile.rs và #23 ghi). Prompt sinh SQL (#29) ghép
AskContext vào prompt; endpoint hỏi (#33) gọi build_ask_context, đổi `ambiguous` thành câu hỏi lại (`needs_choice`) và
`picks` có cột khác thành giả định (#35). Không có dòng dữ liệu nào ở đây: chỉ tên cột, mô tả, bí danh và giá trị của
cột chiều (column_values), cột nhạy cảm thì không kèm giá trị nào. Rủi ro còn lại: bảng nhỏ có cột chữ ít giá trị như
tên người, địa chỉ thì cột đó là cột chiều, giá trị của nó vào ngữ cảnh cho tới khi admin hay #23 đánh dấu nhạy cảm.

Các bước, mỗi bước một hàm:
- Tách câu hỏi thành cụm (service/ask_phrases.rs).
- Lấy giá trị và bí danh đã chuẩn hóa của phiên bản (service/ask_values.rs, cache theo phiên bản).
- Từng cụm: so với tên, bí danh cột và với giá trị, ra các cột ứng viên kèm điểm (service/ask_match.rs). Cụm đè lên chữ của một cụm đã xét thì bỏ qua ("doanh thu" khớp rồi thì
  không tìm "doanh" nữa); thứ tự xét ở build_scored_phrases.
- Một cột ứng viên: dùng cột đó. Nhiều cột: điểm cao nhất hơn cột sau từ CLOSE_SCORE trở lên thì chọn nó và ghi lại
  để hiện giả định; gần nhau hơn thì câu hỏi mơ hồ, trừ khi người hỏi đã chọn.
- Dataset tới FULL_SCHEMA_MAX_COLUMNS cột thì gửi cả schema; nhiều hơn thì chỉ cột khớp một cụm và cột thời gian. */

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::ask_contract::ChosenOption;
use super::ask_match::{
    build_column_text, build_phrase_values, build_scored_phrases, is_partial_match,
};
use super::ask_phrases::{build_phrases, Phrase};
use super::ask_values::{load_value_index, ValueIndex};
use crate::auth::WorkspaceId;
use crate::engine::store::{
    ColumnValue, DatasetVersion, ProfileColumn, ProfileRole, ProfileStatus,
};
use crate::{error::AppError, state::AppState};

/// Dataset tới ngần này cột thì gửi cả schema: DataCo có 53 cột, gửi hết vẫn gọn, và mô hình thấy cả những cột câu
/// hỏi không gọi đúng tên. Nhiều hơn thì chỉ gửi cột khớp một cụm, cộng cột thời gian.
pub const FULL_SCHEMA_MAX_COLUMNS: usize = 60;

/// Ngưỡng khớp gần của giá trị một từ: similarity kiểu pg_trgm (ask_match.rs). "europ" với "Europe" được 0.625. Không
/// dùng word_similarity như đề xuất ban đầu: nó tìm cụm bên trong chuỗi dài, nên "châu Phi" với "châu Âu" được 0.556
/// và "europe" với "Southern Europe" được 1.0. similarity cũng chưa đủ khi một âm tiết so với giá trị hai âm tiết
/// ("châu" với "châu Âu" 0.714), nên khớp gần chỉ giữa chuỗi cùng số từ, và từng âm tiết phải gần nhau.
pub const VALUE_MIN_SIMILARITY: f32 = 0.5;

/// Ngưỡng khớp gần của giá trị nhiều từ. Thấp hơn VALUE_MIN_SIMILARITY vì điều kiện từng âm tiết (mỗi âm tiết khác
/// nhau chỉ sai một chữ) đã chặn các cặp khác hẳn một âm tiết ("châu Mỹ" với "châu Âu" được 0.5). Với các ca ở
/// ask_context_tests.rs, 0.4 không sinh khớp sai nào và nhận thêm lỗi đảo chữ như "mỹ latnih" (0.43 với "Mỹ Latinh");
/// bộ ca đó chưa phải mọi dữ liệu. Gõ sai một chữ "mien nma" với "miền Nam" được 0.5.
pub const VALUE_MIN_SIMILARITY_WORDS: f32 = 0.4;

/// Số cụm được tìm khớp gần tối đa, cụm ngắn trước. Mỗi cụm khớp gần phải so với mọi giá trị cùng số từ của phiên
/// bản; câu 40 từ có tới 117 cụm như vậy. Cụm vượt trần vẫn được tìm khớp bằng nhau (tra bảng băm, rẻ).
const FUZZY_PHRASES_MAX: usize = 40;

/// Hai cột có điểm chênh dưới ngần này là gần nhau: câu hỏi mơ hồ. Chênh từ đây trở lên thì chọn cột điểm cao.
pub const CLOSE_SCORE: f32 = 0.1;

/// Số giá trị mẫu tối đa của một cột chiều đưa vào ngữ cảnh, giá trị nhiều dòng nhất trước. Đủ để mô hình biết cách
/// viết giá trị (hoa thường, viết tắt); danh sách đủ nằm trong column_values.
const SAMPLE_VALUES_MAX: i64 = 10;

/// Một cột như prompt cần.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnProfile {
    pub id: Uuid,
    pub name: String,
    pub data_type: String,
    /// Vai trò trong hồ sơ: dimension, measure, time, key, text, ignore.
    pub role: String,
    /// Định dạng strptime của cột ngày dạng chữ; SQL phải đọc cột qua strptime với định dạng này.
    pub time_format: Option<String>,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    /// Giá trị nhiều dòng nhất trước; chỉ có ở cột chiều không nhạy cảm.
    pub sample_values: Vec<String>,
    pub is_sensitive: bool,
    /// Có cụm của câu hỏi khớp tên, bí danh, mô tả hoặc một giá trị của cột. Prompt quá dài thì bỏ cột không khớp
    /// trước (#29).
    pub matched: bool,
}

/// Một giá trị câu hỏi nhắc tới, như "chau au" là Market = Europe.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueMatch {
    pub column: String,
    pub value: String,
    /// Chữ đã khớp: chính giá trị hoặc bí danh của nó ("châu Âu").
    pub matched_text: String,
    /// Cụm của câu hỏi, chữ thường còn dấu.
    pub phrase: String,
    pub score: f32,
}

/// Một cụm khớp nhiều cột với điểm gần nhau mà người hỏi chưa chọn. #33 đổi thành `Choice` với id `choice_id`, mỗi
/// cột một lựa chọn, id lựa chọn là tên cột.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbiguousWord {
    pub choice_id: String,
    pub phrase: String,
    /// Các cột gần nhau, điểm cao trước.
    pub columns: Vec<String>,
}

/// Cột đã dùng cho một cụm. `others` có cột thì đây là giả định người dùng đổi được (#35 hiện thẻ, cùng `choice_id`
/// như AmbiguousWord); rỗng thì chỉ một cột khớp.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnPick {
    pub choice_id: String,
    pub phrase: String,
    pub column: String,
    /// Các cột ứng viên khác, điểm cao trước.
    pub others: Vec<String>,
    /// Người hỏi đã chọn cột này (gửi trong `choices`), không phải chọn theo điểm.
    pub chosen_by_user: bool,
}

/// Lựa chọn người hỏi gửi mà không có trong `picks` với `chosen_by_user` (cột không còn là ứng viên, cụm không còn
/// trong câu) là lựa chọn không áp dụng; #33 báo cảnh báo `choice_not_applied` cho chúng.
#[derive(Debug, Clone, PartialEq)]
pub struct AskContext {
    /// Theo thứ tự cột của bảng.
    pub columns: Vec<ColumnProfile>,
    /// Theo thứ tự cụm được xét, trong một cụm điểm cao trước; mỗi giá trị một lần.
    pub values: Vec<ValueMatch>,
    pub ambiguous: Vec<AmbiguousWord>,
    pub picks: Vec<ColumnPick>,
}

/// Id lựa chọn cột của một cụm: `column:` cộng cụm đã bỏ dấu, ổn định cho cùng câu hỏi trên cùng phiên bản
/// (ARCHITECTURE mục 8).
pub fn build_column_choice_id(phrase: &Phrase) -> String {
    format!("column:{}", phrase.folded)
}

/// Ngữ cảnh của câu hỏi trên một phiên bản dataset, đọc hồ sơ của workspace người hỏi. Lỗi BadRequest khi phiên bản
/// chưa có hồ sơ; Postgres lỗi là Internal.
pub async fn build_ask_context(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
    question: &str,
    choices: &[ChosenOption],
) -> Result<AskContext, AppError> {
    check_profile_ready(version)?;
    let columns = state.store.list_columns(ws, version.id).await?;
    // Hồ sơ ghi cùng giao dịch với trạng thái structure, nên rỗng ở đây là phiên bản không thuộc workspace này.
    if columns.is_empty() {
        return Err(profile_not_ready());
    }
    let sample_ids: Vec<_> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .map(|c| c.id)
        .collect();
    let samples = state
        .store
        .list_sample_values(ws, &sample_ids, SAMPLE_VALUES_MAX)
        .await?;
    let profiles = build_profiles(columns, &samples);
    let index = load_value_index(state, ws, version).await?;
    Ok(build_context(
        &build_phrases(question),
        profiles,
        &index,
        choices,
    ))
}

fn check_profile_ready(version: &DatasetVersion) -> Result<(), AppError> {
    if version.profile_status == ProfileStatus::Failed.as_str() {
        return Err(AppError::BadRequest(
            "Không lập được hồ sơ cột cho phiên bản này của dataset; nạp lại file".to_string(),
        ));
    }
    if version.profile_status == ProfileStatus::Pending.as_str() {
        return Err(profile_not_ready());
    }
    Ok(())
}

fn profile_not_ready() -> AppError {
    AppError::BadRequest("Dataset chưa lập hồ sơ xong, thử lại sau ít phút".to_string())
}

/// Chữ của các cụm được tìm khớp gần: cụm không exact_only, cụm ngắn trước, tới FUZZY_PHRASES_MAX chữ khác nhau.
pub fn build_fuzzy_phrases(phrases: &[Phrase]) -> HashSet<&str> {
    let mut candidates: Vec<&Phrase> = phrases.iter().filter(|p| !p.exact_only).collect();
    candidates.sort_by_key(|p| (p.len, p.start));
    let mut fuzzy = HashSet::new();
    for phrase in candidates {
        if fuzzy.len() == FUZZY_PHRASES_MAX {
            break;
        }
        fuzzy.insert(phrase.text.as_str());
    }
    fuzzy
}

/// Hồ sơ từng cột kèm giá trị mẫu. samples chỉ có giá trị của cột chiều không nhạy cảm (list_sample_values), theo
/// cột, giá trị nhiều dòng nhất trước.
fn build_profiles(columns: Vec<ProfileColumn>, samples: &[ColumnValue]) -> Vec<ColumnProfile> {
    let mut by_column: HashMap<_, Vec<String>> = HashMap::new();
    for v in samples {
        by_column
            .entry(v.column_id)
            .or_default()
            .push(v.value.clone());
    }
    columns
        .into_iter()
        .map(|c| ColumnProfile {
            id: c.id,
            sample_values: if c.is_sensitive {
                Vec::new()
            } else {
                by_column.remove(&c.id).unwrap_or_default()
            },
            name: c.name,
            data_type: c.data_type,
            role: c.role,
            time_format: c.time_format,
            description: c.description,
            aliases: c.aliases,
            is_sensitive: c.is_sensitive,
            matched: false,
        })
        .collect()
}

/// Phần thuần của build_ask_context: chọn cột và giá trị từ các cụm của câu hỏi, hồ sơ cột, giá trị của phiên bản và
/// lựa chọn của người hỏi. Chỉ tìm giá trị ở cột chiều không nhạy cảm theo columns (hồ sơ vừa đọc), kể cả khi index
/// lấy từ cache cũ hơn.
pub fn build_context(
    phrases: &[Phrase],
    columns: Vec<ColumnProfile>,
    index: &ValueIndex,
    choices: &[ChosenOption],
) -> AskContext {
    let texts: Vec<_> = columns.iter().map(build_column_text).collect();
    let open: HashSet<Uuid> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .map(|c| c.id)
        .collect();
    let fuzzy = build_fuzzy_phrases(phrases);
    let mut context = AskContext {
        columns: Vec::new(),
        values: Vec::new(),
        ambiguous: Vec::new(),
        picks: Vec::new(),
    };
    // Cột khớp một cụm, theo chỉ số trong columns.
    let mut matched = HashSet::new();
    // Vị trí từ đã thuộc một cụm khớp: cụm nằm đè lên đó không xét nữa.
    let mut used: Vec<&Phrase> = Vec::new();
    let is_free =
        |used: &[&Phrase], phrase: &Phrase| !used.iter().any(|u| phrase.overlaps(u.start, u.len));

    let scored = build_scored_phrases(phrases, &columns, &texts, index, &open, &fuzzy);

    // Cột đã chọn cho mỗi id lựa chọn: cụm lặp lại trong câu hỏi dùng lại, không ghi mơ hồ hay giả định hai lần.
    let mut resolved: HashMap<String, Vec<usize>> = HashMap::new();
    for item in scored.iter().filter(|s| !s.candidates.is_empty()) {
        if !is_free(&used, item.phrase) {
            continue;
        }
        used.push(item.phrase);
        let choice_id = build_column_choice_id(item.phrase);
        let chosen = match resolved.get(&choice_id) {
            Some(chosen) => chosen.clone(),
            None => {
                let chosen = apply_choice(
                    &mut context,
                    item.phrase,
                    &columns,
                    &item.candidates,
                    choices,
                );
                resolved.insert(choice_id, chosen.clone());
                chosen
            }
        };
        matched.extend(chosen.iter().copied());
        let chosen_names: Vec<&str> = chosen.iter().map(|&i| columns[i].name.as_str()).collect();
        for value in build_phrase_values(item.phrase, &item.hits, &chosen_names) {
            apply_value(&mut context.values, value);
        }
    }
    // Cụm không khớp chắc cột nào: vẫn giữ cột có tên, bí danh hay mô tả chứa cụm, để mô hình thấy chúng.
    for item in scored.iter().filter(|s| s.candidates.is_empty()) {
        if !is_free(&used, item.phrase) {
            continue;
        }
        for (i, text) in texts.iter().enumerate() {
            if is_partial_match(item.phrase, text) {
                matched.insert(i);
            }
        }
    }

    context.columns = build_sent_columns(columns, &matched);
    context
}

/// Thêm một giá trị vào ngữ cảnh nếu chưa có: một giá trị khớp nhiều cụm (câu hỏi nhắc lại) chỉ ghi một lần.
fn apply_value(values: &mut Vec<ValueMatch>, value: ValueMatch) {
    let seen = values
        .iter()
        .any(|v| v.column == value.column && v.value == value.value);
    if !seen {
        values.push(value);
    }
}

/// Cột gửi cho prompt, giữ thứ tự cột. Tới FULL_SCHEMA_MAX_COLUMNS cột, hoặc không cụm nào khớp cột nào, thì gửi cả
/// schema: lọc khi không có gì để lọc theo sẽ chỉ còn cột thời gian, mô hình không viết được SQL nào.
fn build_sent_columns(
    mut columns: Vec<ColumnProfile>,
    matched: &HashSet<usize>,
) -> Vec<ColumnProfile> {
    let send_all = columns.len() <= FULL_SCHEMA_MAX_COLUMNS || matched.is_empty();
    for (i, column) in columns.iter_mut().enumerate() {
        column.matched = matched.contains(&i);
    }
    columns
        .into_iter()
        .filter(|c| send_all || c.matched || c.role == "time")
        .collect()
}

/// Chọn cột cho một cụm có ứng viên và ghi vào context (picks hoặc ambiguous). Trả các cột dùng cho cụm: một cột khi
/// đã chọn được, các cột gần nhau khi còn mơ hồ.
fn apply_choice(
    context: &mut AskContext,
    phrase: &Phrase,
    columns: &[ColumnProfile],
    candidates: &[(usize, f32)],
    choices: &[ChosenOption],
) -> Vec<usize> {
    let choice_id = build_column_choice_id(phrase);
    let name = |i: usize| columns[i].name.clone();
    let user_pick = choices
        .iter()
        .filter(|c| c.choice_id == choice_id)
        .find_map(|c| {
            candidates
                .iter()
                .find(|(i, _)| columns[*i].name == c.option_id)
        });
    let top = candidates[0];
    let close: Vec<usize> = candidates
        .iter()
        .filter(|(_, score)| top.1 - score < CLOSE_SCORE)
        .map(|(i, _)| *i)
        .collect();

    let (column, chosen_by_user) = match user_pick {
        // Lựa chọn chỉ có nghĩa khi có hơn một cột; một cột thì chọn gì cũng là cột đó.
        Some(&(i, _)) if candidates.len() > 1 => (i, true),
        _ if close.len() > 1 => {
            context.ambiguous.push(AmbiguousWord {
                choice_id,
                phrase: phrase.text.clone(),
                columns: close.iter().map(|&i| name(i)).collect(),
            });
            return close;
        }
        _ => (top.0, false),
    };
    context.picks.push(ColumnPick {
        choice_id,
        phrase: phrase.text.clone(),
        column: name(column),
        others: candidates
            .iter()
            .filter(|(i, _)| *i != column)
            .map(|(i, _)| name(*i))
            .collect(),
        chosen_by_user,
    });
    vec![column]
}
