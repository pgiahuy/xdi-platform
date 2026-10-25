// SPDX-License-Identifier: Apache-2.0
/* service/ask_context.rs - ngữ cảnh cho một câu hỏi: cột nào gửi cho mô hình, mỗi cột chiều kèm danh sách giá trị, và
gợi ý giá trị khớp chính xác, lấy từ hồ sơ cột của đúng phiên bản được hỏi (engine/store/columns.rs, do
service/profile.rs và #23 ghi). Prompt sinh SQL (#29) ghép AskContext vào prompt; mô hình tự ánh xạ chữ người dùng sang
giá trị trong danh sách ("miền Nam" thành South) và trả `{terms, sql}`, #33 kiểm `terms` với column_values. Endpoint hỏi
(#33) gọi build_ask_context, đổi `ambiguous` thành câu hỏi lại (`needs_choice`) và `picks` có cột khác thành giả định
(#35).

Không có dòng dữ liệu nào ở đây: chỉ tên cột, mô tả, bí danh và giá trị của cột chiều (column_values), cột nhạy cảm
không kèm giá trị nào. Rủi ro còn lại: bảng nhỏ có cột chữ ít giá trị như tên người, địa chỉ thì cột đó là cột chiều,
giá trị của nó vào prompt cho tới khi admin hay #23 đánh dấu nhạy cảm (docs/PRIVACY.md).

Các bước:
- Tách câu hỏi thành cụm (service/ask_phrases.rs).
- Chọn cột: cụm bằng tên hay bí danh cột (service/ask_match.rs). Cụm dài trước; cụm đè lên chữ của một cụm đã khớp
  thì bỏ qua ("doanh thu" khớp rồi thì không tìm "doanh" nữa). Một cột: dùng cột đó. Nhiều cột: điểm cao nhất hơn cột
  sau từ CLOSE_SCORE trở lên thì chọn nó và ghi lại để hiện giả định; gần nhau hơn thì câu hỏi mơ hồ, trừ khi người hỏi
  đã chọn. Giá trị không sinh mơ hồ: cùng một chữ có ở hai cột thì mô hình chọn theo ngữ cảnh của câu.
- Dataset tới FULL_SCHEMA_MAX_COLUMNS cột thì gửi cả schema; nhiều hơn thì cột khớp một cụm, cột có gợi ý giá trị và
  cột thời gian.
- Danh sách giá trị của từng cột chiều được gửi (build_column_values), và gợi ý khớp chính xác (build_value_hints). */

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::ask_contract::ChosenOption;
use super::ask_match::{
    build_column_text, build_scored_phrases, build_value_hints, is_partial_match,
};
use super::ask_phrases::{build_phrases, Phrase};
use super::ask_values::{load_value_index, ColumnValues, ValueEntry, ValueIndex};
use crate::auth::WorkspaceId;
use crate::engine::store::{DatasetVersion, ProfileColumn, ProfileRole, ProfileStatus};
use crate::{error::AppError, state::AppState};

/// Dataset tới ngần này cột thì gửi cả schema: DataCo có 53 cột, gửi hết vẫn gọn, và mô hình thấy cả những cột câu
/// hỏi không gọi đúng tên. Nhiều hơn thì chỉ gửi cột khớp một cụm, cộng cột thời gian.
pub const FULL_SCHEMA_MAX_COLUMNS: usize = 60;

/// Hai cột có điểm chênh dưới ngần này là gần nhau: câu hỏi mơ hồ. Chênh từ đây trở lên thì chọn cột điểm cao.
pub const CLOSE_SCORE: f32 = 0.1;

/// Cột chiều có tới ngần này giá trị thì gửi đủ danh sách, để mô hình ánh xạ được mọi cách gọi của người dùng sang
/// đúng giá trị. DataCo: Category Name có đúng 50 giá trị, Order Region 23, Market 5.
pub const COLUMN_VALUES_ALL_MAX: usize = 50;

/// Cột chiều nhiều giá trị hơn COLUMN_VALUES_ALL_MAX thì chỉ gửi ngần này giá trị nhiều dòng nhất, đủ để mô hình biết
/// cách viết giá trị (ngôn ngữ, hoa thường, viết tắt). DataCo: Order Country có 164 nước.
pub const COLUMN_VALUES_SOME: usize = 20;

/// Tổng số giá trị (không tính bí danh) gửi trong một ngữ cảnh. Vượt thì hạ một trần chung cho mọi cột tới khi tổng
/// vừa: cột nhiều giá trị bị cắt trước, cột ít giá trị (thường là cột người dùng hay nhắc tên giá trị: thị trường,
/// phân khúc) giữ đủ. Số đo trên DataCo thật ở chú thích của build_column_values.
pub const CONTEXT_VALUES_MAX: usize = 400;

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
    pub is_sensitive: bool,
    /// Giá trị của cột chiều không nhạy cảm, nhiều dòng nhất trước, kèm bí danh; rỗng với cột khác.
    pub values: Vec<ValueEntry>,
    /// values là mọi giá trị của cột: mô hình chỉ được dùng giá trị trong danh sách. false thì danh sách chỉ là mẫu.
    pub values_complete: bool,
    /// Có cụm của câu hỏi khớp tên, bí danh, mô tả, hoặc có gợi ý giá trị của cột. Prompt quá dài thì bỏ cột không khớp
    /// trước (#29).
    pub matched: bool,
}

/// Một cụm của câu hỏi bằng chính xác một giá trị hay bí danh ("southern europe" là Order Region = Southern Europe).
/// Chỉ là gợi ý cho prompt: mô hình vẫn quyết định cụm nghĩa là gì.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueHint {
    /// Cụm như người hỏi gõ (NFC, chữ thường).
    pub phrase: String,
    pub column: String,
    pub value: String,
    /// Chữ đã khớp: chính giá trị hoặc bí danh của nó.
    pub matched_text: String,
}

/// Một cụm khớp tên hay bí danh của nhiều cột với điểm gần nhau mà người hỏi chưa chọn. #33 đổi thành `Choice` với id
/// `choice_id`, mỗi cột một lựa chọn, id lựa chọn là tên cột.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbiguousWord {
    pub choice_id: String,
    /// Cụm như người hỏi gõ.
    pub phrase: String,
    /// Các cột gần nhau, điểm cao trước.
    pub columns: Vec<String>,
}

/// Cột đã dùng cho một cụm. `others` có cột thì đây là giả định người dùng đổi được (#35 hiện thẻ, cùng `choice_id`
/// như AmbiguousWord); rỗng thì chỉ một cột khớp.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnPick {
    pub choice_id: String,
    /// Cụm như người hỏi gõ.
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
    /// Cụm dài trước, mỗi giá trị một lần.
    pub hints: Vec<ValueHint>,
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
    let index = load_value_index(state, ws, version).await?;
    Ok(build_context(
        &build_phrases(question),
        build_profiles(columns),
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

fn build_profiles(columns: Vec<ProfileColumn>) -> Vec<ColumnProfile> {
    columns
        .into_iter()
        .map(|c| ColumnProfile {
            id: c.id,
            name: c.name,
            data_type: c.data_type,
            role: c.role,
            time_format: c.time_format,
            description: c.description,
            aliases: c.aliases,
            is_sensitive: c.is_sensitive,
            values: Vec::new(),
            values_complete: false,
            matched: false,
        })
        .collect()
}

/// Phần thuần của build_ask_context: chọn cột, gắn danh sách giá trị và gợi ý từ các cụm của câu hỏi, hồ sơ cột, giá
/// trị của phiên bản và lựa chọn của người hỏi. Chỉ dùng giá trị của cột chiều không nhạy cảm theo columns (hồ sơ vừa
/// đọc), kể cả khi index lấy từ cache cũ hơn.
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
    let mut context = AskContext {
        columns: Vec::new(),
        hints: build_value_hints(phrases, index, &open),
        ambiguous: Vec::new(),
        picks: Vec::new(),
    };
    // Cột khớp một cụm hoặc có gợi ý giá trị, theo chỉ số trong columns.
    let mut matched: HashSet<usize> = context
        .hints
        .iter()
        .filter_map(|h| columns.iter().position(|c| c.name == h.column))
        .collect();
    // Cụm đã khớp cột: cụm nằm đè lên đó không xét nữa.
    let mut used: Vec<&Phrase> = Vec::new();
    let scored = build_scored_phrases(phrases, &texts);
    // Cột đã chọn cho mỗi id lựa chọn: cụm lặp lại trong câu hỏi dùng lại, không ghi mơ hồ hay giả định hai lần.
    let mut resolved: HashMap<String, Vec<usize>> = HashMap::new();
    for item in scored.iter().filter(|s| !s.candidates.is_empty()) {
        if used.iter().any(|u| u.overlaps(item.phrase)) {
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
        matched.extend(chosen);
    }
    // Cụm không khớp chắc cột nào: vẫn giữ cột có tên, bí danh hay mô tả chứa cụm, để mô hình thấy chúng.
    for item in scored.iter().filter(|s| s.candidates.is_empty()) {
        if used.iter().any(|u| u.overlaps(item.phrase)) {
            continue;
        }
        for (i, text) in texts.iter().enumerate() {
            if is_partial_match(item.phrase, text) {
                matched.insert(i);
            }
        }
    }

    let mut sent = build_sent_columns(columns, &matched);
    apply_column_values(&mut sent, index, &open);
    context.columns = sent;
    context
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

/// Số giá trị gửi cho mỗi cột, theo thứ tự của counts (số giá trị có của từng cột): đủ khi tới COLUMN_VALUES_ALL_MAX,
/// COLUMN_VALUES_SOME khi nhiều hơn; tổng vượt CONTEXT_VALUES_MAX thì hạ một trần chung cho mọi cột, từ số lớn nhất
/// xuống, tới khi tổng vừa.
///
/// Đã đo trên DataCo thật (180 519 dòng, 53 cột; hồ sơ dựng theo luật vai trò của docs/DATA.md, Customer Email và
/// Customer Password đánh dấu nhạy cảm): 16 cột chiều gửi 224 giá trị, chưa tới trần 400. In mỗi cột một dòng dạng
/// `"Cột": a | b | c`, phần giá trị dài 4 070 ký tự, ước chừng 1 000 token (4 ký tự một token, quy ước thô cho chữ
/// Latin). Khoảng 1 600 ký tự trong đó là 20 đường dẫn ảnh của Product Image: cột chữ ít giá trị là cột chiều theo luật
/// vai trò, nên admin hay #23 cần đổi vai trò cột như vậy sang text.
pub fn build_column_values(counts: &[usize]) -> Vec<usize> {
    let wanted: Vec<usize> = counts
        .iter()
        .map(|&n| {
            if n <= COLUMN_VALUES_ALL_MAX {
                n
            } else {
                COLUMN_VALUES_SOME
            }
        })
        .collect();
    let total = |cap: usize| -> usize { wanted.iter().map(|&w| w.min(cap)).sum() };
    let mut cap = wanted.iter().copied().max().unwrap_or(0);
    while total(cap) > CONTEXT_VALUES_MAX {
        cap -= 1;
    }
    wanted.into_iter().map(|w| w.min(cap)).collect()
}

/// Gắn danh sách giá trị vào các cột chiều không nhạy cảm sẽ gửi (open), theo build_column_values.
fn apply_column_values(columns: &mut [ColumnProfile], index: &ValueIndex, open: &HashSet<Uuid>) {
    let found: Vec<(usize, &ColumnValues)> = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| open.contains(&c.id))
        .filter_map(|(i, c)| Some((i, index.columns.iter().find(|v| v.column_id == c.id)?)))
        .collect();
    let counts: Vec<usize> = found.iter().map(|(_, v)| v.values.len()).collect();
    for ((i, values), sent) in found.into_iter().zip(build_column_values(&counts)) {
        columns[i].values = values.values[..sent].to_vec();
        columns[i].values_complete = sent == values.values.len() && !values.dropped;
    }
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
