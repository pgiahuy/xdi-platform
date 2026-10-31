// SPDX-License-Identifier: Apache-2.0
/* service/ask/context.rs - ngữ cảnh cho một câu hỏi: cột nào gửi cho mô hình, mỗi cột chiều kèm danh sách giá trị, và
gợi ý giá trị khớp chính xác, lấy từ hồ sơ cột của đúng phiên bản được hỏi (engine/store/columns.rs, do
service/profile.rs và #23 ghi). Prompt sinh SQL (#29) ghép AskContext vào prompt; mô hình tự ánh xạ chữ người dùng sang
giá trị trong danh sách ("miền Nam" thành South) và trả `{terms, sql}`, #33 kiểm `terms` với column_values. Endpoint hỏi
(#33) gọi build_ask_context, đổi `ambiguous` thành câu hỏi lại (`needs_choice`) và `picks` có cột khác thành giả định
(#35).

Không có dòng dữ liệu nào ở đây: chỉ tên cột, mô tả, bí danh và giá trị của cột chiều (column_values), cột nhạy cảm
không kèm giá trị nào. Rủi ro còn lại: bảng nhỏ có cột chữ ít giá trị như tên người, địa chỉ thì cột đó là cột chiều,
giá trị của nó vào prompt khi luật tên và dạng giá trị của lớp cấu trúc (service/profile.rs) không bắt được, cho
tới khi admin hay #23 đánh dấu (docs/PRIVACY.md).

Các bước:
- Tách câu hỏi thành cụm (service/ask/phrases.rs).
- Chọn cột: cụm bằng tên hay bí danh cột (service/ask/matching.rs). Cụm dài trước; cụm đè lên chữ của một cụm đã khớp
  thì bỏ qua ("doanh thu" khớp rồi thì không tìm "doanh" nữa). Một cột: dùng cột đó. Nhiều cột: điểm cao nhất hơn cột
  sau từ CLOSE_SCORE trở lên thì chọn nó và ghi lại để hiện giả định; gần nhau hơn thì câu hỏi mơ hồ, trừ khi người hỏi
  đã chọn. Giá trị không sinh mơ hồ: cùng một chữ có ở hai cột thì mô hình chọn theo ngữ cảnh của câu.
- Mọi cột của mọi bảng câu hỏi đọc luôn được gửi đủ metadata (tên, kiểu, vai trò, bí danh, mô tả, đơn vị, cách gộp)
  cho cả lượt phân loại lẫn lượt viết SQL (#230): mô hình phải biết mọi cột có thật, kể cả cột câu hỏi không gọi đúng
  tên. Chỉ khi tổng metadata vượt COLUMN_METADATA_MAX_CHARS (cỡ 600 cột) thì cột ngoài nhóm liên quan (is_related)
  mới rút về dòng gọn (brief: tên, kiểu, vai trò). Cột liên quan: cột khớp một cụm hay có gợi ý giá trị, cột lượt
  phân loại chọn (picked), cột thời gian, cột khóa nối bảng, cột dùng trong công thức chỉ số. Khớp cụm vẫn dùng cho
  gợi ý giá trị, câu hỏi lại và để chọn cột liên quan, không dùng để bỏ cột.
- Danh sách giá trị của từng cột chiều được gửi (build_column_values), và gợi ý khớp chính xác (build_value_hints). */

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::auth::WorkspaceId;
use crate::engine::store::{
    ColumnAggregation, DatasetVersion, MetricDefinition, ProfileColumn, ProfileRole, ProfileStatus,
};
use crate::service::ask::contract::ChosenOption;
use crate::service::ask::matching::{
    build_column_text, build_scored_phrases, build_value_hints, is_partial_match,
};
use crate::service::ask::phrases::{build_phrases, Phrase};
use crate::service::ask::values::{load_value_index, ColumnValues, ValueEntry, ValueIndex};
use crate::{error::AppError, state::AppState};

/// Trần tổng độ dài metadata cột (tính bằng metadata_chars, mọi bảng của câu hỏi) trước khi rút cột ngoài nhóm liên
/// quan về dòng gọn: cỡ 600 cột, mỗi cột khoảng 100 ký tự tên, mô tả và bí danh. DataCo 53 cột chưa tới 6 000 ký tự,
/// nên schema thường không bao giờ chạm trần; trần chỉ để một schema hàng nghìn cột không làm prompt quá lớn.
pub const COLUMN_METADATA_MAX_CHARS: usize = 60_000;

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
    /// Có cụm của câu hỏi khớp tên, bí danh, mô tả, hoặc có gợi ý giá trị của cột. Prompt quá dài thì cột không liên quan
    /// mất danh sách giá trị trước (#29, #230).
    pub matched: bool,
    /// Đơn vị của cột số đo (ColumnUnit::as_str), cho luật biểu đồ (ARCHITECTURE mục 6); None khi chưa biết.
    pub unit: Option<String>,
    /// Cách gộp mặc định của cột số đo (#185); prompt dặn mô hình dùng AVG với cột avg.
    pub aggregation: ColumnAggregation,
    /// Cột nằm trong một quan hệ khóa ngoại tới bảng khác của câu hỏi (#213). Luôn đủ chi tiết: mô hình cần cột khóa
    /// để viết JOIN.
    pub join_key: bool,
    /// Cột có trong biểu thức, điều kiện lọc hay cột thời gian của một chỉ số của bảng (apply_metric_columns). Luôn đủ
    /// chi tiết: mô hình tính chỉ số bằng đúng công thức đó.
    pub metric_column: bool,
    /// Lượt phân loại (ask/classify.rs) nói câu hỏi cần cột này.
    pub picked: bool,
    /// Chỉ in dạng gọn: tên, kiểu, vai trò, định dạng ngày; không bí danh, mô tả, đơn vị, cách gộp hay giá trị. Đặt khi
    /// tổng metadata vượt COLUMN_METADATA_MAX_CHARS và cột không liên quan (apply_metadata_cap, #230).
    pub brief: bool,
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
    build_ask_context_with_keys(state, ws, version, question, choices, &[], &[]).await
}

/// Như build_ask_context, và đánh dấu join_key cho các cột tên trong join_keys (cột khóa của quan hệ tới bảng liên
/// quan, #213), metric_column cho cột dùng trong chỉ số của dataset.
pub async fn build_ask_context_with_keys(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
    question: &str,
    choices: &[ChosenOption],
    join_keys: &[String],
    metrics: &[MetricDefinition],
) -> Result<AskContext, AppError> {
    check_profile_ready(version)?;
    let columns = state.store.list_columns(ws, version.id).await?;
    // Hồ sơ ghi cùng giao dịch với trạng thái structure, nên rỗng ở đây là phiên bản không thuộc workspace này.
    if columns.is_empty() {
        return Err(profile_not_ready());
    }
    let index = load_value_index(state, ws, version).await?;
    let mut profiles = build_profiles(columns);
    apply_join_keys(&mut profiles, join_keys);
    apply_metric_columns(&mut profiles, metrics);
    Ok(build_context(
        &build_phrases(question),
        profiles,
        &index,
        choices,
    ))
}

/// Đánh dấu join_key cho cột có tên trong join_keys.
pub(crate) fn apply_join_keys(columns: &mut [ColumnProfile], join_keys: &[String]) {
    for column in columns {
        column.join_key = join_keys.contains(&column.name);
    }
}

/// Đánh dấu metric_column cho cột được nhắc trong biểu thức, điều kiện lọc hay cột thời gian của một chỉ số.
pub(crate) fn apply_metric_columns(columns: &mut [ColumnProfile], metrics: &[MetricDefinition]) {
    for column in columns {
        column.metric_column = metrics.iter().any(|m| {
            m.time_column.as_deref() == Some(column.name.as_str())
                || mentions_column(&m.expression, &column.name)
                || m.filter
                    .as_deref()
                    .is_some_and(|f| mentions_column(f, &column.name))
        });
    }
}

/// Biểu thức SQL nhắc tới cột: tên trong nháy kép ("Order Item Total"), hoặc tên trơn đứng riêng như một từ (item_total,
/// không tính item_total_2). Đọc thô bằng chữ, không phân tích SQL: nhận nhầm một cột thì cột đó chỉ được in đủ chi
/// tiết hơn, không sai gì.
fn mentions_column(sql: &str, name: &str) -> bool {
    if sql.contains(&format!("\"{name}\"")) {
        return true;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    sql.match_indices(name).any(|(at, _)| {
        let before = sql[..at].chars().next_back();
        let after = sql[at + name.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

pub(crate) fn check_profile_ready(version: &DatasetVersion) -> Result<(), AppError> {
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

pub(crate) fn build_profiles(columns: Vec<ProfileColumn>) -> Vec<ColumnProfile> {
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
            unit: c.unit,
            // Chữ lạ (không thể có, vì CHECK của 0010_column_aggregation.sql) coi như mặc định sum.
            aggregation: ColumnAggregation::parse(&c.aggregation).unwrap_or(ColumnAggregation::Sum),
            join_key: false,
            metric_column: false,
            picked: false,
            brief: false,
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

    apply_user_columns(
        &mut context,
        phrases,
        &columns,
        choices,
        &resolved,
        &mut matched,
    );

    let mut sent = build_matched_columns(columns, &matched);
    apply_group_values(&mut [(&mut sent, index)]);
    context.columns = sent;
    context
}

/// Mọi cột, giữ thứ tự cột, đánh dấu cột khớp câu hỏi. Không bỏ cột nào (#230).
fn build_matched_columns(
    mut columns: Vec<ColumnProfile>,
    matched: &HashSet<usize>,
) -> Vec<ColumnProfile> {
    for (i, column) in columns.iter_mut().enumerate() {
        column.matched = matched.contains(&i);
    }
    columns
}

/// Cột liên quan tới câu hỏi: khớp câu hỏi hay có gợi ý giá trị, lượt phân loại chọn, cột thời gian (câu có kỳ nào cũng
/// cần), cột khóa nối bảng, cột dùng trong công thức chỉ số. Không bao giờ rút về dòng gọn.
pub(crate) fn is_related(column: &ColumnProfile) -> bool {
    column.matched
        || column.picked
        || column.role == ProfileRole::Time.as_str()
        || column.join_key
        || column.metric_column
}

/// Độ dài metadata của một cột như prompt in ra (tên, kiểu, vai trò, mô tả, bí danh, đơn vị), để so với
/// COLUMN_METADATA_MAX_CHARS. Đếm thô, không tính chữ khuôn của dòng.
pub fn metadata_chars(column: &ColumnProfile) -> usize {
    let text = |t: &str| t.chars().count();
    text(&column.name)
        + text(&column.data_type)
        + text(&column.role)
        + column.description.as_deref().map_or(0, text)
        + column.aliases.iter().map(|a| text(a) + 2).sum::<usize>()
        + column.unit.as_deref().map_or(0, text)
}

/// Đánh dấu brief cho mọi bảng của một câu hỏi (bảng đang chọn và các bảng liên quan, #213): tổng metadata vượt
/// COLUMN_METADATA_MAX_CHARS thì cột không liên quan rút về dòng gọn, không thì không cột nào gọn. Gọi lại sau khi
/// lượt phân loại chọn thêm cột thì tính lại từ đầu.
pub fn apply_metadata_cap(tables: &mut [&mut Vec<ColumnProfile>]) {
    let total: usize = tables
        .iter()
        .flat_map(|columns| columns.iter())
        .map(metadata_chars)
        .sum();
    let over = total > COLUMN_METADATA_MAX_CHARS;
    for columns in tables.iter_mut() {
        for column in columns.iter_mut() {
            column.brief = over && !is_related(column);
        }
    }
}

/// Đánh dấu picked cho cột lượt phân loại chọn (cặp bảng, cột; bảng là bảng logic). Gọi lại thì tính lại từ đầu.
pub fn apply_picked_columns(
    table: &str,
    columns: &mut [ColumnProfile],
    picked: &[(String, String)],
) {
    for column in columns {
        column.picked = picked.iter().any(|(t, c)| t == table && *c == column.name);
    }
}

/// Số giá trị gửi cho mỗi cột, theo thứ tự của counts (số giá trị có của từng cột): đủ khi tới COLUMN_VALUES_ALL_MAX,
/// COLUMN_VALUES_SOME khi nhiều hơn; tổng vượt CONTEXT_VALUES_MAX thì hạ một trần chung cho mọi cột, từ số lớn nhất
/// xuống, tới khi tổng vừa.
///
/// Đã đo trên DataCo thật (DataCoSupplyChainDataset.csv, 180 519 dòng, 53 cột), bằng test measure_dataco_values ở
/// ask/context_tests.rs: file đi qua đúng đường nạp (materialize, rồi lập hồ sơ của service/profile.rs), Customer Email
/// và Customer Password đánh dấu nhạy cảm, hỏi "Doanh thu theo thị trường", in mỗi cột có giá trị một dòng dạng
/// `"Cột": a | b | c` rồi đếm ký tự. Lệnh: `DATACO_CSV=<file> DATABASE_URL=... cargo test measure_dataco_values --
/// --ignored --nocapture`. Kết quả: 16 cột chiều gửi 224 giá trị, chưa tới trần 400; phần giá trị dài 4 052 ký tự, ước
/// chừng 1 000 token (4 ký tự một token, quy ước thô cho chữ Latin). Riêng dòng Product Image (20 đường dẫn ảnh) dài
/// 1 210 ký tự: cột chữ ít giá trị là cột chiều theo luật vai trò, nên admin hay #23 cần đổi vai trò cột như vậy sang
/// text.
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

/// Gắn danh sách giá trị vào các cột chiều không nhạy cảm, không gọn (brief), theo build_column_values. Mỗi phần tử của tables là
/// cột của một bảng cùng chỉ mục giá trị của phiên bản bảng đó; mọi bảng của một câu hỏi chung một trần
/// CONTEXT_VALUES_MAX (#213), nên thêm bảng liên quan không làm prompt dài gấp mấy lần. Gọi lại trên cột đã có giá trị
/// thì giá trị được tính lại từ đầu.
pub fn apply_group_values(tables: &mut [(&mut Vec<ColumnProfile>, &ValueIndex)]) {
    // (bảng, cột, giá trị của cột) của mọi cột có danh sách giá trị.
    let mut found: Vec<(usize, usize, &ColumnValues)> = Vec::new();
    for (t, (columns, index)) in tables.iter_mut().enumerate() {
        // Chép tham chiếu ra: found giữ giá trị mượn từ chỉ mục, không mượn từ tables, nên sửa tables ở dưới được.
        let index: &ValueIndex = index;
        for (i, column) in columns.iter_mut().enumerate() {
            column.values = Vec::new();
            column.values_complete = false;
            // Cột gọn không có danh sách giá trị, và không chiếm phần của trần CONTEXT_VALUES_MAX.
            if column.role != ProfileRole::Dimension.as_str() || column.is_sensitive || column.brief
            {
                continue;
            }
            match index.columns.iter().find(|v| v.column_id == column.id) {
                Some(values) => found.push((t, i, values)),
                // Cột chiều không có dòng nào trong column_values: danh sách rỗng là đủ, nếu giá trị đã tải được.
                None => column.values_complete = index.loaded,
            }
        }
    }
    let counts: Vec<usize> = found.iter().map(|(_, _, v)| v.values.len()).collect();
    for ((t, i, values), sent) in found.into_iter().zip(build_column_values(&counts)) {
        let column = &mut tables[t].0[i];
        column.values = values.values[..sent].to_vec();
        column.values_complete = sent == values.values.len() && !values.dropped;
    }
}

/// Lựa chọn cột người hỏi gửi cho một cụm mà bước chấm điểm ở trên không thấy cột nào khớp: câu hỏi lại của mô hình
/// (ví dụ "doanh thu tính bằng cột nào?" khi dataset chưa có chỉ số, ask/choices.rs) cho chọn cột theo cụm đó. Không
/// ghi thành pick thì lựa chọn không vào prompt, câu gửi lại bị hỏi lại y nguyên và kèm choice_not_applied. Chỉ nhận
/// khi cụm còn trong câu hỏi và cột có thật; còn lại bỏ qua như trước (choice_not_applied).
fn apply_user_columns(
    context: &mut AskContext,
    phrases: &[Phrase],
    columns: &[ColumnProfile],
    choices: &[ChosenOption],
    resolved: &HashMap<String, Vec<usize>>,
    matched: &mut HashSet<usize>,
) {
    for choice in choices {
        if resolved.contains_key(&choice.choice_id)
            || context
                .picks
                .iter()
                .any(|p| p.choice_id == choice.choice_id)
        {
            continue;
        }
        let Some(phrase) = phrases
            .iter()
            .find(|p| build_column_choice_id(p) == choice.choice_id)
        else {
            continue;
        };
        let Some(i) = columns.iter().position(|c| c.name == choice.option_id) else {
            continue;
        };
        context.picks.push(ColumnPick {
            choice_id: choice.choice_id.clone(),
            phrase: phrase.text.clone(),
            column: columns[i].name.clone(),
            others: Vec::new(),
            chosen_by_user: true,
        });
        matched.insert(i);
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
        // Người hỏi đã chọn một ứng viên (kể cả khi chỉ có một): ghi là lựa chọn của họ, để #33 không báo
        // choice_not_applied cho một lựa chọn đã áp dụng.
        Some(&(i, _)) => (i, true),
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
