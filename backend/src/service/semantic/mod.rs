// SPDX-License-Identifier: Apache-2.0
/* service/semantic/mod.rs - hồ sơ cột lớp ngữ nghĩa (#23): mô tả, bí danh cột, bí danh giá trị, đơn vị và cách gộp
mặc định (sum hay avg, #185) của cột số đo, câu hỏi gợi ý, đánh dấu cột nhạy cảm, và sửa vai trò cột trong phạm vi hẹp. Khác lớp cấu trúc (service/profile.rs, chạy trong request nạp),
lớp này gọi mô hình ngôn ngữ, nên mọi lỗi của nó không được chặn lần nạp.

Mỗi lượt gửi ba prompt song song (service/semantic/parts.rs, #184). Ba đường chạy, cả ba đi qua semaphore semantic_jobs
(SEMANTIC_JOBS lượt cùng lúc, mặc định 2, để nạp nhiều file liền không dồn request lên nhà cung cấp):
1. ingest.rs spawn sau khi trả 201 cho phiên bản mới; nạp lại đúng bộ file cũ mà phiên bản còn ở structure thì spawn
   lại (đường thử lại tường minh).
2. Việc nền lúc khởi động (profile_remaining_versions, main.rs gọi sau khi lớp cấu trúc xong) chạy lần lượt cho các
   phiên bản structure chưa thử với provider đang cấu hình, gồm cả phiên bản nạp trước khi có bước này.
3. Lỗi chia hai. Lỗi vĩnh viễn: câu trả lời không dùng được (JSON hỏng, không mục nào hợp lệ) và lỗi BadRequest của
   provider (Gemini chặn prompt vì nội dung, mock không có bảng); đánh dấu đã thử kèm tên provider
   (semantic_attempted_with) - cùng provider không gọi lại, đổi LLM_PROVIDER thì thử lại. Mọi lỗi khác là lỗi tạm
   (mô hình quá tải, timeout, mất mạng, thiếu cấu hình, Postgres lỗi), giữ structure và không đánh dấu gì, lần khởi
   động sau thử lại. Gemini trả Upstream cả khi câu trả lời bị cắt ở MAX_TOKENS, bị dừng vì SAFETY và với mã 400
   không phải khóa sai, nên các lỗi đó cũng được thử lại mỗi lần khởi động dù gọi lại có thể vẫn hỏng. Người dùng
   không nhận lỗi ở đâu cả.

Hai hàm thuần làm phần dễ sai: build_profile_prompt dựng prompt chỉ từ siêu dữ liệu (không bao giờ có giá trị của cột
text, key hay cột nhạy cảm; min, max chỉ của cột số thực và thời gian), build_profile_answer kiểm từng mục của câu trả
lời (mục sai bỏ riêng, mục đúng vẫn ghi; không còn mục nào hợp lệ thì coi như lỗi). Cờ is_sensitive đã được lớp cấu
trúc đặt lúc nạp (service/profile.rs), nên phiên bản dùng được là giá trị cột nhạy cảm đã an toàn; ở lớp này mô hình
chỉ đề xuất đánh dấu thêm (chỉ chiều bật). Ghi hồ sơ xuống Postgres ở engine/store/columns.rs
(apply_semantic_profile). Test ở service/semantic/tests.rs. */

mod parts;
pub mod schema;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::auth::WorkspaceId;
use crate::engine::llm::meter::{build_call_totals, CallLog};
use crate::engine::prompts::{self, fill};
use crate::engine::store::{
    ColumnAggregation, ColumnUnit, ColumnValue, Dataset, ProfileColumn, ProfileRole, ProfileStatus,
    SemanticColumnUpdate, SemanticValueUpdate,
};
use crate::service::ask::context;
use crate::service::ask::phrases::fold;
use crate::service::ask::values::STRING_MAX_BYTES;
use crate::service::profile::{is_integer_type, DIMENSION_MAX};
pub use crate::service::semantic::parts::ProfilePart;
use crate::service::semantic::parts::{ask_profile_parts, PartInput};
use crate::{error::AppError, state::AppState};

// Các giới hạn khi kiểm câu trả lời của mô hình (đếm theo ký tự, không phải byte). Mô tả và bí danh vào prompt của
// các bước sau (#28, #29), câu gợi ý hiện trên trang hỏi (#50); dài quá thì chiếm chỗ prompt và vỡ bố cục.
pub(crate) const DESCRIPTION_MAX_CHARS: usize = 300;
pub(crate) const ALIAS_MAX_CHARS: usize = 60;
pub(crate) const ALIASES_MAX: usize = 8;
pub(crate) const QUESTIONS_MAX: usize = 5;
pub(crate) const QUESTION_MAX_CHARS: usize = 200;

// Trần phần giá trị trong prompt chia bằng hàm context::build_column_values của ask/context.rs: một cột có
// không quá 50 giá trị thì gửi hết, nhiều hơn thì 20 giá trị nhiều dòng nhất; tổng quá 400 thì hạ một trần chung cho
// mọi cột; giá trị dài quá STRING_MAX_BYTES byte không gửi. Không có trần này thì bảng khảo sát
// vài trăm cột, mỗi cột 200 giá trị dài tới 1000 byte, thổi prompt lên hàng chục MB: tốn token, vượt cửa sổ ngữ
// cảnh, mô hình trả lỗi và hồ sơ mất. Đo trên hồ sơ kiểu DataCo (test measure_dataco_like_prompt, 54 cột, 14 cột
// dimension có giá trị, cột 164 giá trị bị cắt còn 20): prompt khoảng 10 700 ký tự, trong đó phần giá trị khoảng
// 5 100.

/// Câu trả lời của mô hình sau khi kiểm: tên cột và giá trị đã đổi ra id, mọi giới hạn đã áp.
#[derive(Debug)]
pub struct SemanticProfile {
    pub columns: Vec<SemanticColumnUpdate>,
    pub values: Vec<SemanticValueUpdate>,
    /// Cột mô hình đề xuất đánh dấu nhạy cảm (chỉ chiều false -> true, có hiệu lực từ các lần sau).
    pub sensitive_columns: Vec<Uuid>,
    pub suggested_questions: Vec<String>,
}

/// Lỗi của một lượt lớp ngữ nghĩa, phân biệt để quyết định có thử lại lúc khởi động không.
#[derive(Debug)]
pub(crate) enum SemanticFailure {
    /// Lỗi tạm: mọi lỗi provider không phải BadRequest (quá tải, timeout, mất mạng, và cả MAX_TOKENS, SAFETY, mã 400
    /// của Gemini, đều là Upstream), máy chủ thiếu cấu hình (Unavailable), Postgres lỗi. Không đánh dấu gì: lần khởi
    /// động sau thử lại.
    Temporary(AppError),
    /// Câu trả lời không dùng được (JSON hỏng, không mục nào hợp lệ) hay provider trả BadRequest (Gemini chặn prompt
    /// vì nội dung, mock không có bảng): gọi lại y nguyên không khác, nên đánh dấu đã thử kèm tên provider; đổi
    /// LLM_PROVIDER thì thử lại.
    Permanent(AppError),
}

/// Ghi log và đánh dấu một lượt lỗi. Chỉ lỗi vĩnh viễn mới ghi semantic_attempted_with. BadRequest (mock không có
/// bảng, Gemini chặn prompt vì nội dung) không cần cảnh báo: gọi lại không khác gì.
async fn note_failure(
    state: &AppState,
    ws: &WorkspaceId,
    version_id: Uuid,
    failure: SemanticFailure,
) {
    let (e, permanent) = match failure {
        SemanticFailure::Temporary(e) => (e, false),
        SemanticFailure::Permanent(e) => (e, true),
    };
    if permanent {
        let provider = state.llm.name();
        if let Err(e) = state
            .store
            .mark_semantic_attempted(ws, version_id, &provider)
            .await
        {
            tracing::warn!(version_id = %version_id, error = ?e, "cannot mark semantic profile attempted");
        }
    }
    if matches!(e, AppError::BadRequest(_)) {
        tracing::info!(version_id = %version_id, error = ?e, "no semantic profile for this dataset");
    } else {
        tracing::warn!(version_id = %version_id, error = ?e,
            "semantic profile failed, keeping the structural profile");
    }
}

/// Chỉ gọi mô hình khi phiên bản còn ở structure: hai lượt xếp hàng chờ semaphore cho cùng một phiên bản (nạp lại
/// đúng bộ file hai lần liền, hay việc khởi động và nạp lại) thì lượt sau dừng ở đây, không tốn hai lời gọi mô hình.
/// skip_attempted (đường khởi động) bỏ cả phiên bản đã thử với provider đang cấu hình: danh sách ứng viên lấy một lần
/// lúc đầu, trong lúc đó một lượt nạp có thể đã thử và đánh dấu lỗi vĩnh viễn; không kiểm thì việc khởi động gọi mô
/// hình lần hai trong cùng lần chạy. Đường nạp không bỏ, vì nạp lại đúng bộ file là cách thử lại tường minh.
/// Phiên bản bị xóa trong lúc chờ (None) hay không đọc được status thì bỏ qua.
pub(crate) async fn still_structure(
    state: &AppState,
    ws: &WorkspaceId,
    version_id: Uuid,
    skip_attempted: bool,
) -> bool {
    match state.store.get_semantic_state(ws, version_id).await {
        Ok(Some((status, attempted_with))) => {
            let attempted = attempted_with.is_some_and(|p| p == state.llm.name());
            status == ProfileStatus::Structure.as_str() && !(skip_attempted && attempted)
        }
        Ok(None) => false,
        Err(e) => {
            tracing::warn!(version_id = %version_id, error = ?e, "cannot re-check profile status");
            false
        }
    }
}

/// Lập hồ sơ ngữ nghĩa cho một phiên bản, chạy nền qua semaphore (xem đầu file). Lỗi chỉ ghi log và, khi lỗi vĩnh
/// viễn, đánh dấu đã thử kèm provider.
pub fn spawn_semantic_profile(
    state: AppState,
    ws: WorkspaceId,
    dataset: Dataset,
    version_id: Uuid,
    row_count: Option<i64>,
) {
    tokio::spawn(async move {
        // acquire không lỗi được trừ khi semaphore đóng, mà semaphore này không bao giờ đóng.
        let Ok(_permit) = state.semantic_jobs.acquire().await else {
            return;
        };
        if !still_structure(&state, &ws, version_id, false).await {
            return;
        }
        match apply_semantic_profile(&state, &ws, &dataset, version_id, row_count).await {
            Ok(()) => tracing::info!(version_id = %version_id, "semantic profile applied"),
            Err(f) => note_failure(&state, &ws, version_id, f).await,
        }
    });
}

/// Việc nền lúc khởi động: lần lượt lập hồ sơ ngữ nghĩa cho các phiên bản structure chưa thử với provider đang cấu
/// hình (gồm phiên bản nạp trước khi có bước này). Mỗi phiên bản lấy một permit của semantic_jobs như đường nạp, nên
/// trong lúc việc khởi động chạy, nạp mới không gọi mô hình song song với nó. Dataset hay phiên bản bị xóa trong lúc
/// chờ thì bỏ qua phiên bản đó.
pub async fn profile_remaining_versions(state: AppState) {
    let candidates = match crate::auth::pending_semantic_profiles(&state.store, &state.llm.name())
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = ?e, "cannot list dataset versions waiting for a semantic profile");
            return;
        }
    };
    if candidates.is_empty() {
        return;
    }
    tracing::info!(
        count = candidates.len(),
        "semantic profiling of earlier dataset versions"
    );
    for (ws, candidate) in candidates {
        let dataset = match state.store.get_dataset(&ws, candidate.dataset_id).await {
            Ok(Some(d)) => d,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!(version_id = %candidate.id, error = ?e, "cannot read dataset of a semantic candidate");
                continue;
            }
        };
        let Ok(_permit) = state.semantic_jobs.acquire().await else {
            return;
        };
        if !still_structure(&state, &ws, candidate.id, true).await {
            continue;
        }
        match apply_semantic_profile(&state, &ws, &dataset, candidate.id, candidate.row_count).await
        {
            Ok(()) => tracing::info!(version_id = %candidate.id, "semantic profile applied"),
            Err(f) => note_failure(&state, &ws, candidate.id, f).await,
        }
    }
}

/// Một lượt lớp ngữ nghĩa: đọc hồ sơ cấu trúc của phiên bản (cờ nhạy cảm đã được lớp cấu trúc đặt lúc nạp,
/// service/profile.rs), dựng prompt, gọi mô hình, kiểm rồi ghi trong một giao dịch. Lỗi trả về để người gọi ghi log
/// và đánh dấu đã thử; không bao giờ chuyển profile_status sang failed (khác lỗi lớp cấu trúc: lỗi ở đây là của mô
/// hình, thử lại lúc khởi động hay bằng cách nạp lại cùng bộ file).
pub async fn apply_semantic_profile(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    version_id: Uuid,
    row_count: Option<i64>,
) -> Result<(), SemanticFailure> {
    // Đo lượt lập hồ sơ như đo một câu hỏi (engine/llm/meter.rs, #184): một dòng log cuối lượt có tổng thời gian, số
    // lần gọi và số token.
    let log = CallLog::new();
    let metered = state.with_call_log(&log);
    let applied = profile_version(&metered, ws, dataset, version_id, row_count).await;
    let totals = build_call_totals(&log.calls());
    tracing::info!(
        version_id = %version_id,
        total_ms = log.elapsed_ms(),
        model_calls = totals.calls,
        model_ms = totals.model_ms,
        input_tokens = totals.input_tokens,
        output_tokens = totals.output_tokens,
        ok = applied.is_ok(),
        "semantic profile done"
    );
    applied
}

async fn profile_version(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    version_id: Uuid,
    row_count: Option<i64>,
) -> Result<(), SemanticFailure> {
    let columns = state
        .store
        .list_columns(ws, version_id)
        .await
        .map_err(SemanticFailure::Temporary)?;
    // Chỉ cột dimension không nhạy cảm có danh sách giá trị trong prompt, nên cũng chỉ đọc giá trị của các cột đó;
    // mô hình không thấy thì bí danh nó đề xuất cho giá trị khác cũng không qua được kiểm tra.
    let value_columns: Vec<Uuid> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .map(|c| c.id)
        .collect();
    let values = state
        .store
        .list_column_values(ws, &value_columns)
        .await
        .map_err(SemanticFailure::Temporary)?;

    let input = PartInput {
        dataset,
        version_id,
        row_count,
        columns: &columns,
        values: &values,
    };
    let text = ask_profile_parts(state, &input).await?;
    let answer = build_profile_answer(&text, &columns, &values).map_err(|reasons| {
        // Vài lý do bỏ đầu tiên (tên trường, vị trí, lý do; không có giá trị dữ liệu) để biết mô hình trả sai chỗ
        // nào mà không phải bật log câu trả lời đầy đủ.
        tracing::warn!(version_id = %version_id, reasons = ?reasons,
            "semantic profile answer has no valid item");
        SemanticFailure::Permanent(AppError::Internal(
            "semantic profile answer has no valid item".to_string(),
        ))
    })?;
    let written = state
        .store
        .apply_semantic_profile(
            ws,
            version_id,
            &answer.columns,
            &answer.values,
            &answer.sensitive_columns,
            &answer.suggested_questions,
        )
        .await
        .map_err(SemanticFailure::Temporary)?;
    // Phiên bản không còn ở structure (ví dụ một lượt chạy song song đã ghi xong, hoặc phiên bản bị xóa): không có
    // gì để làm.
    if !written {
        tracing::info!(version_id = %version_id, "semantic profile skipped, version no longer at structure");
        return Ok(());
    }
    // Ghi lại mỗi đổi vai trò do mô hình đề xuất (tên cột, vai trò cũ, mới; không log giá trị): để sau này phân biệt
    // vai trò do luật cấu trúc chọn với vai trò do mô hình sửa.
    for update in &answer.columns {
        if let Some(role) = update.role {
            let old = columns
                .iter()
                .find(|c| c.id == update.column_id)
                .map(|c| c.name.as_str());
            tracing::info!(version_id = %version_id, column = ?old, new_role = role.as_str(),
                "column role changed by the semantic profile");
        }
    }
    Ok(())
}

// So tên và bí danh (trùng sau chuẩn hóa cũng tính là trùng) qua fold của service/ask/phrases.rs (bỏ dấu, chữ
// thường, tách từ), cùng một cách chuẩn hóa với khớp cụm ở ngữ cảnh hỏi: bí danh "Thị  Trường" trùng "thị trường".

/// Prompt của một phần hồ sơ (service/semantic/parts.rs): luật và khuôn JSON của phần đó trước, rồi mỗi cột một dòng siêu dữ liệu, cuối cùng danh sách giá trị
/// của các cột dimension không nhạy cảm. Chỉ siêu dữ liệu và giá trị cột dimension đi ra ngoài: min, max chỉ kèm cột
/// số thực và time (min, max của cột chữ hay mã số nguyên như mã đơn, số điện thoại là dữ liệu thật của người dùng);
/// cột nhạy cảm chỉ gửi tên, kiểu, vai trò và số đo, không gửi giá trị nào. Tên cột và giá trị in dạng chuỗi JSON
/// (xuống dòng, ngoặc kép được thoát) để dữ liệu không chèn được dòng giả vào prompt. Phần luật viết tiếng Anh (như
/// prompt sinh SQL, #29), chữ ở backend/prompts/semantic_profile_*.txt; mô hình trả lời bằng tiếng Việt theo luật.
pub fn build_profile_prompt(
    part: ProfilePart,
    dataset_name: &str,
    logical_table: &str,
    row_count: Option<i64>,
    columns: &[ProfileColumn],
    values: &[ColumnValue],
) -> String {
    let rows = match row_count {
        Some(rows) => format!(", {rows} rows"),
        None => String::new(),
    };
    let mut column_lines = String::new();
    for c in columns {
        column_lines.push_str(&column_line(c, row_count));
        column_lines.push('\n');
    }
    let rules = match part {
        ProfilePart::Columns => fill(
            &prompts::SEMANTIC_PROFILE_COLUMNS,
            &[
                ("description_max_chars", &DESCRIPTION_MAX_CHARS.to_string()),
                ("aliases_max", &ALIASES_MAX.to_string()),
                ("alias_max_chars", &ALIAS_MAX_CHARS.to_string()),
            ],
        ),
        ProfilePart::Values => fill(
            &prompts::SEMANTIC_PROFILE_VALUES,
            &[
                ("aliases_max", &ALIASES_MAX.to_string()),
                ("alias_max_chars", &ALIAS_MAX_CHARS.to_string()),
            ],
        ),
        ProfilePart::Questions => fill(
            &prompts::SEMANTIC_PROFILE_QUESTIONS,
            &[
                ("questions_max", &QUESTIONS_MAX.to_string()),
                ("question_max_chars", &QUESTION_MAX_CHARS.to_string()),
            ],
        ),
    };
    // Ba phần dùng chung một phần dữ liệu, nên chốt riêng tư (build_values_part, column_line) chỉ ở một chỗ.
    let data = fill(
        &prompts::SEMANTIC_PROFILE_DATA,
        &[
            ("dataset", &json_string(dataset_name)),
            ("table", &json_string(logical_table)),
            ("row_count", &rows),
            ("columns", &column_lines),
            ("values", &build_values_part(columns, values)),
        ],
    );
    format!("{rules}\n{data}")
}

/// Phần giá trị của cột dimension không nhạy cảm, mỗi cột một dòng; chuỗi rỗng khi không có giá trị nào.
fn build_values_part(columns: &[ProfileColumn], values: &[ColumnValue]) -> String {
    let mut part = String::new();
    if !values.is_empty() {
        part.push_str("Values of dimension columns (value (rows), ...):\n");
        let mut by_column: HashMap<Uuid, Vec<&ColumnValue>> = HashMap::new();
        for v in values {
            by_column.entry(v.column_id).or_default().push(v);
        }
        // Chốt riêng tư nằm ở đây chứ không trông chờ người gọi lọc trước: giá trị của cột text, key hay cột nhạy
        // cảm không bao giờ vào prompt, kể cả khi lỡ truyền vào. Giá trị quá dài bỏ trước khi đếm.
        let printable: Vec<(&ProfileColumn, Vec<&ColumnValue>)> = columns
            .iter()
            .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
            .filter_map(|c| {
                let list: Vec<&ColumnValue> = by_column
                    .get(&c.id)?
                    .iter()
                    .filter(|v| v.value.len() <= STRING_MAX_BYTES)
                    .copied()
                    .collect();
                (!list.is_empty()).then_some((c, list))
            })
            .collect();
        // Chia trần bằng context::build_column_values: đủ khi tới COLUMN_VALUES_ALL_MAX, COLUMN_VALUES_SOME khi
        // nhiều hơn; tổng vượt CONTEXT_VALUES_MAX thì hạ một trần chung cho mọi cột, không bỏ hẳn cột đứng sau.
        let counts: Vec<usize> = printable.iter().map(|(_, list)| list.len()).collect();
        let caps = context::build_column_values(&counts);
        for ((c, list), cap) in printable.iter().zip(caps) {
            // list đã xếp giá trị nhiều dòng nhất trước (list_column_values).
            let items: Vec<String> = list
                .iter()
                .take(cap)
                .map(|v| match v.frequency {
                    Some(n) => format!("{} ({n})", json_string(&v.value)),
                    None => json_string(&v.value),
                })
                .collect();
            if !items.is_empty() {
                part.push_str(&format!("{}: {}\n", json_string(&c.name), items.join(", ")));
            }
        }
    }
    part
}

/// In một chuỗi dữ liệu vào prompt dạng chuỗi JSON (có ngoặc kép, ký tự đặc biệt đã thoát): dữ liệu có xuống dòng
/// hay ngoặc kép cũng không chèn được dòng giả.
fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "null".to_string())
}

/// Một dòng siêu dữ liệu của cột. null % và distinct lấy từ hồ sơ cấu trúc; min, max chỉ in với cột time và cột
/// measure không phải số nguyên (số nguyên lặp lại thường là mã: mã đơn, mã bưu điện, số điện thoại).
fn column_line(c: &ProfileColumn, row_count: Option<i64>) -> String {
    let null_pct = match (c.null_count, row_count) {
        (Some(nulls), Some(rows)) if rows > 0 => format!("{}%", nulls * 100 / rows),
        _ => "?".to_string(),
    };
    let distinct = c
        .distinct_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".to_string());
    let mut line = format!(
        "{} | {} | {} | null {null_pct} | {distinct} distinct",
        json_string(&c.name),
        c.data_type,
        c.role
    );
    let with_range = !c.is_sensitive
        && (c.role == ProfileRole::Time.as_str()
            || (c.role == ProfileRole::Measure.as_str() && !is_integer_type(&c.data_type)));
    if with_range {
        if let (Some(min), Some(max)) = (&c.min_value, &c.max_value) {
            line.push_str(&format!(" | {min} | {max}"));
        }
    }
    line
}

/// Số lý do bỏ mục giữ lại để ghi log khi cả câu trả lời bị bỏ: đủ thấy mô hình sai kiểu gì, không làm dòng log dài
/// theo số cột của bảng.
const REJECTIONS_MAX: usize = 5;

/// Ghi một lý do bỏ mục, dạng "columns[2]: unknown column name". Chỉ tên trường, vị trí và lý do, không bao giờ có
/// giá trị dữ liệu hay chữ mô hình viết.
fn note_rejection(reasons: &mut Vec<String>, field: &str, index: usize, reason: &str) {
    if reasons.len() < REJECTIONS_MAX {
        reasons.push(format!("{field}[{index}]: {reason}"));
    }
}

/// Danh sách ở khóa field của câu trả lời. Khóa có mà không phải danh sách thì ghi lý do, vì đó là kiểu sai khuôn
/// làm mất cả nhóm mục một lúc.
fn answer_list<'a>(
    json: &'a serde_json::Value,
    field: &str,
    reasons: &mut Vec<String>,
) -> &'a [serde_json::Value] {
    match &json[field] {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => &[],
        _ => {
            if reasons.len() < REJECTIONS_MAX {
                reasons.push(format!("{field}: not a list"));
            }
            &[]
        }
    }
}

/// Đọc câu trả lời của mô hình thành SemanticProfile đã kiểm. Err khi không đọc được JSON hoặc không còn mục nào
/// hợp lệ (người gọi coi như lỗi mô hình), kèm vài lý do bỏ đầu tiên để ghi log. Từng mục được kiểm riêng: cột không
/// có thật, giá trị không có trong column_values, chuỗi rỗng sau trim hay có ký tự điều khiển (một chuỗi như vậy làm
/// Postgres từ chối cả giao dịch), chuỗi quá dài, bí danh trùng tên khác, vai trò không được phép thì bỏ mục đó, các
/// mục khác vẫn ghi.
pub fn build_profile_answer(
    text: &str,
    columns: &[ProfileColumn],
    values: &[ColumnValue],
) -> Result<SemanticProfile, Vec<String>> {
    let json = parse_answer_json(text)
        .ok_or_else(|| vec!["the answer is not a JSON object".to_string()])?;
    let mut reasons: Vec<String> = Vec::new();
    let by_name: HashMap<&str, &ProfileColumn> =
        columns.iter().map(|c| (c.name.as_str(), c)).collect();
    // Tên mọi cột, đã chuẩn hóa: bí danh của cột không được trùng tên một cột (kể cả tên chính nó), nếu không câu
    // hỏi có chữ đó sẽ khớp nhầm cột.
    let column_names: HashSet<String> = columns.iter().map(|c| fold(&c.name)).collect();

    // Mục cột và đổi vai trò cùng ghi vào bảng columns, nên gom theo id cột: đổi vai trò của một cột không có mục
    // riêng vẫn được ghi (mô tả null, bí danh rỗng là mặc định sẵn có). Mục cột không có gì hợp lệ (mô tả null, bí
    // danh rỗng) thì bỏ hẳn: ghi NULL không đổi gì mà làm câu trả lời toàn mục sai vẫn thành "có mục hợp lệ".
    let mut column_updates: HashMap<Uuid, SemanticColumnUpdate> = HashMap::new();
    for (i, item) in answer_list(&json, "columns", &mut reasons)
        .iter()
        .enumerate()
    {
        let Some(name) = item["name"].as_str() else {
            note_rejection(
                &mut reasons,
                "columns",
                i,
                "name is missing or not a string",
            );
            continue;
        };
        let Some(column) = by_name.get(name) else {
            note_rejection(&mut reasons, "columns", i, "unknown column name");
            continue;
        };
        let description = clean_text_opt(&item["description"], DESCRIPTION_MAX_CHARS);
        let aliases = valid_aliases(&item["aliases"], &column_names);
        let unit = valid_unit(column, &item["unit"]);
        let aggregation = valid_aggregation(column, &item["aggregation"]);
        if description.is_none() && aliases.is_empty() && unit.is_none() && aggregation.is_none() {
            note_rejection(
                &mut reasons,
                "columns",
                i,
                "no valid description, alias, unit or aggregation",
            );
            continue;
        }
        column_updates.insert(
            column.id,
            SemanticColumnUpdate {
                column_id: column.id,
                description,
                aliases,
                role: None,
                unit,
                aggregation,
            },
        );
    }
    for (i, item) in answer_list(&json, "role_changes", &mut reasons)
        .iter()
        .enumerate()
    {
        let (Some(name), Some(role)) = (item["name"].as_str(), item["role"].as_str()) else {
            note_rejection(&mut reasons, "role_changes", i, "name or role is missing");
            continue;
        };
        let Some(column) = by_name.get(name) else {
            note_rejection(&mut reasons, "role_changes", i, "unknown column name");
            continue;
        };
        let Some(role) = allowed_role_change(column, role) else {
            note_rejection(&mut reasons, "role_changes", i, "role change not allowed");
            continue;
        };
        column_updates
            .entry(column.id)
            .or_insert_with(|| SemanticColumnUpdate {
                column_id: column.id,
                description: None,
                aliases: Vec::new(),
                role: None,
                unit: None,
                aggregation: None,
            })
            .role = Some(role);
    }
    let mut sensitive_columns: Vec<Uuid> = Vec::new();
    for (i, item) in answer_list(&json, "sensitive_columns", &mut reasons)
        .iter()
        .enumerate()
    {
        match item.as_str().and_then(|name| by_name.get(name)) {
            Some(column) => sensitive_columns.push(column.id),
            None => note_rejection(&mut reasons, "sensitive_columns", i, "unknown column name"),
        }
    }

    // Giá trị phải có thật trong column_values của đúng cột (danh sách đã đọc chỉ gồm cột dimension không nhạy cảm):
    // mô hình không thể đặt bí danh cho giá trị nó không thấy.
    let known: HashSet<(Uuid, &str)> = values
        .iter()
        .map(|v| (v.column_id, v.value.as_str()))
        .collect();
    let mut values_by_column: HashMap<Uuid, HashSet<String>> = HashMap::new();
    for v in values {
        values_by_column
            .entry(v.column_id)
            .or_default()
            .insert(fold(&v.value));
    }
    let mut value_updates = Vec::new();
    // Bí danh đã nhận của từng cột, để một bí danh không đứng cho hai giá trị cùng cột ("châu Âu" không nhận cho cả
    // Western Europe và Southern Europe).
    let mut accepted_by_column: HashMap<Uuid, HashSet<String>> = HashMap::new();
    for (i, item) in answer_list(&json, "values", &mut reasons)
        .iter()
        .enumerate()
    {
        let (Some(column_name), Some(value)) = (item["column"].as_str(), item["value"].as_str())
        else {
            note_rejection(&mut reasons, "values", i, "column or value is missing");
            continue;
        };
        let Some(column) = by_name.get(column_name) else {
            note_rejection(&mut reasons, "values", i, "unknown column name");
            continue;
        };
        if !known.contains(&(column.id, value)) {
            note_rejection(
                &mut reasons,
                "values",
                i,
                "value not in the column's value list",
            );
            continue;
        }
        // Bí danh của giá trị không được trùng (sau chuẩn hóa) một giá trị khác cùng cột hay bí danh đã nhận của cột
        // đó: "Europe" không nhận "LATAM".
        let mut forbidden = values_by_column
            .get(&column.id)
            .cloned()
            .unwrap_or_default();
        forbidden.extend(
            accepted_by_column
                .get(&column.id)
                .cloned()
                .unwrap_or_default(),
        );
        let aliases = valid_aliases(&item["aliases"], &forbidden);
        // Mục giá trị không có bí danh hợp lệ thì bỏ hẳn: không có gì để ghi.
        if aliases.is_empty() {
            note_rejection(&mut reasons, "values", i, "no valid alias");
            continue;
        }
        accepted_by_column
            .entry(column.id)
            .or_default()
            .extend(aliases.iter().map(|a| fold(a)));
        value_updates.push(SemanticValueUpdate {
            column_id: column.id,
            value: value.to_string(),
            aliases,
        });
    }

    let mut suggested_questions = Vec::new();
    let mut seen_questions = HashSet::new();
    for (i, item) in answer_list(&json, "suggested_questions", &mut reasons)
        .iter()
        .enumerate()
    {
        if suggested_questions.len() >= QUESTIONS_MAX {
            break;
        }
        // Câu còn chỗ trống của khuôn trong prompt ("<measure>") là câu mô hình chưa điền xong: bấm vào thì hỏi vô
        // nghĩa, nên bỏ như câu không hợp lệ.
        let Some(q) = item
            .as_str()
            .and_then(|q| clean_text(q, QUESTION_MAX_CHARS))
            .filter(|q| !q.contains('<') && !q.contains('>'))
        else {
            note_rejection(
                &mut reasons,
                "suggested_questions",
                i,
                "not a valid question",
            );
            continue;
        };
        if seen_questions.insert(fold(&q)) {
            suggested_questions.push(q);
        }
    }

    if column_updates.is_empty()
        && value_updates.is_empty()
        && suggested_questions.is_empty()
        && sensitive_columns.is_empty()
    {
        if reasons.is_empty() {
            reasons.push("the answer has no item".to_string());
        }
        return Err(reasons);
    }
    // Giữ thứ tự cột của bảng để log và test đọc dễ.
    let columns_out: Vec<SemanticColumnUpdate> = columns
        .iter()
        .filter_map(|c| column_updates.remove(&c.id))
        .collect();
    Ok(SemanticProfile {
        columns: columns_out,
        values: value_updates,
        sensitive_columns,
        suggested_questions,
    })
}

/// Đọc câu trả lời của mô hình thành JSON; None khi không đọc được.
pub(crate) fn parse_answer_json(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str(strip_fence(text))
        .ok()
        // Mô hình thêm lời dẫn trước hoặc sau khối JSON: lấy từ { đầu tới } cuối.
        .or_else(|| {
            let start = text.find('{')?;
            let end = text.rfind('}')?;
            serde_json::from_str(text.get(start..=end)?).ok()
        })
}

/// Bỏ khối rào markdown (```json ... ```) mô hình hay bọc quanh JSON dù luật đã cấm; chịu cả chữ thừa trước khối
/// ("Here is the JSON:") nhờ bước dự phòng lấy từ { tới } ở build_profile_answer.
fn strip_fence(text: &str) -> &str {
    let text = text.trim();
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    // Dòng đầu sau ``` là tên ngôn ngữ (json) hoặc rỗng; bỏ cả dòng đó.
    let body = rest.split_once('\n').map_or(rest, |(_, body)| body);
    body.trim().strip_suffix("```").unwrap_or(body).trim()
}

/// Chuỗi từ mô hình sau khi làm sạch: trim, rỗng sau trim hay dài quá max_chars thì bỏ; chuỗi có ký tự điều khiển
/// (NUL, xuống dòng, tab) hay ký tự điều hướng hai chiều (U+202A..U+202E, U+2066..U+2069) thì bỏ. Postgres từ chối
/// NUL trong text và jsonb, nên không lọc thì một mục hỏng làm rollback cả hồ sơ; xuống dòng trong mô tả, bí danh hay
/// câu gợi ý còn vỡ cả prompt của các bước sau và giao diện.
fn clean_text(text: &str, max_chars: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars {
        return None;
    }
    let bad = trimmed.chars().any(|c| {
        c.is_control()
            || ('\u{200b}'..='\u{200f}').contains(&c) // zero-width, LRM/RLM
            || ('\u{2028}'..='\u{2029}').contains(&c) // line/paragraph separator
            || ('\u{202a}'..='\u{202e}').contains(&c) // điều hướng hai chiều
            || ('\u{2060}'..='\u{2064}').contains(&c) // word joiner, dấu nối vô hình
            || ('\u{2066}'..='\u{2069}').contains(&c)
            || c == '\u{61c}' || c == '\u{feff}'
    });
    if bad {
        return None;
    }
    Some(trimmed.to_string())
}

/// Như clean_text cho trường JSON có thể null hay sai kiểu: thành None (mặc định), không loại cả mục.
fn clean_text_opt(value: &serde_json::Value, max_chars: usize) -> Option<String> {
    clean_text(value.as_str()?, max_chars)
}

/// Bí danh hợp lệ: qua clean_text (ALIAS_MAX_CHARS), không trùng (sau chuẩn hóa fold) với tên bị cấm (tên cột khác,
/// giá trị khác cùng cột, tên hay giá trị gốc) và không trùng bí danh đã nhận của cùng mục; tối đa ALIASES_MAX cái.
/// Mục sai bỏ riêng.
fn valid_aliases(value: &serde_json::Value, forbidden: &HashSet<String>) -> Vec<String> {
    let mut aliases = Vec::new();
    let mut accepted = HashSet::new();
    for item in value.as_array().into_iter().flatten() {
        let Some(alias) = item.as_str().and_then(|a| clean_text(a, ALIAS_MAX_CHARS)) else {
            continue;
        };
        let folded = fold(&alias);
        if forbidden.contains(&folded) || !accepted.insert(folded) {
            continue;
        }
        aliases.push(alias);
        if aliases.len() >= ALIASES_MAX {
            break;
        }
    }
    aliases
}

/// Đổi vai trò chỉ trong phạm vi hẹp đã chốt: cột số nguyên đang là measure thành key (mã định danh lặp lại như
/// Order Id) hoặc dimension (năm, tháng dạng số), và cột dimension thành text (đường dẫn ảnh, mô tả dài). Measure
/// thành dimension còn cần không quá DIMENSION_MAX giá trị khác nhau: khi đó column_values đã có danh sách của cột
/// (số nguyên luôn ngắn), còn nhiều hơn thì cột dimension mới sẽ không có danh sách, phá bất biến "cột chiều luôn có
/// danh sách giá trị" của lớp cấu trúc. Mọi chiều đổi khác (ví dụ measure thành time) bị bỏ: luật cấu trúc đã xét
/// kiểu và số đo, mô hình chỉ sửa được những ca số đo không nói lên được.
/// Đơn vị chỉ nhận cho cột measure của hồ sơ cấu trúc và chỉ giá trị trong danh sách ColumnUnit. Sai thì bỏ riêng
/// đơn vị, mô tả và bí danh của cùng mục vẫn giữ: đơn vị chỉ dùng để chọn biểu đồ, thiếu nó thì biểu đồ tách riêng
/// từng số đo chứ không sai số liệu.
fn valid_unit(column: &ProfileColumn, value: &serde_json::Value) -> Option<ColumnUnit> {
    if column.role != ProfileRole::Measure.as_str() {
        return None;
    }
    value.as_str().and_then(ColumnUnit::parse)
}

/// Cách gộp chỉ nhận cho cột measure của hồ sơ cấu trúc, và chỉ sum hay avg. Sai hay thiếu thì giữ mặc định sum của
/// migration, như mọi bước trước #185: cộng nhầm một cột mô tả từng dòng ra số vô nghĩa nhưng vẫn là số đúng của phép
/// cộng, còn admin sửa được ở màn Chỉ số.
fn valid_aggregation(
    column: &ProfileColumn,
    value: &serde_json::Value,
) -> Option<ColumnAggregation> {
    if column.role != ProfileRole::Measure.as_str() {
        return None;
    }
    value.as_str().and_then(ColumnAggregation::parse)
}

pub(crate) fn allowed_role_change(column: &ProfileColumn, target: &str) -> Option<ProfileRole> {
    let measure = ProfileRole::Measure.as_str();
    let dimension = ProfileRole::Dimension.as_str();
    match (column.role.as_str(), target) {
        (r, "key") if r == measure && is_integer_type(&column.data_type) => Some(ProfileRole::Key),
        (r, "dimension")
            if r == measure
                && is_integer_type(&column.data_type)
                && column.distinct_count.is_some_and(|n| n <= DIMENSION_MAX) =>
        {
            Some(ProfileRole::Dimension)
        }
        (r, "text") if r == dimension => Some(ProfileRole::Text),
        _ => None,
    }
}
