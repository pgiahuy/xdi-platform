// SPDX-License-Identifier: Apache-2.0
/* service/semantic.rs - hồ sơ cột lớp ngữ nghĩa (#23): mô tả, bí danh cột, bí danh giá trị, câu hỏi gợi ý, và sửa vai
trò cột trong phạm vi hẹp. Khác lớp cấu trúc (service/profile.rs, chạy trong request nạp), lớp này gọi mô hình ngôn
ngữ một lần cho mỗi phiên bản mới, chạy nền bằng tokio::spawn sau khi route nạp trả 201 (ingest.rs): mô hình lỗi hay
trả lời hỏng thì phiên bản giữ profile_status = structure, người nạp không nhận lỗi. Phiên bản mới của cùng dataset
gọi lại mô hình, vì cột của phiên bản là dòng mới, chưa có mô tả.

Hai hàm thuần làm phần dễ sai: build_profile_prompt dựng prompt chỉ từ siêu dữ liệu (không bao giờ có giá trị của cột
text, key hay cột nhạy cảm), build_profile_answer kiểm từng mục của câu trả lời (mục sai bỏ riêng, mục đúng vẫn ghi;
không còn mục nào hợp lệ thì coi như lỗi). Ghi xuống Postgres ở engine/store/columns.rs (apply_semantic_profile). */

use std::collections::HashMap;

use uuid::Uuid;

use crate::auth::WorkspaceId;
use crate::engine::llm::{LlmRequest, LlmTask};
use crate::engine::store::{
    ColumnValue, Dataset, DatasetVersion, ProfileColumn, ProfileRole, SemanticColumnUpdate,
    SemanticValueUpdate,
};
use crate::service::profile::is_integer_type;
use crate::{error::AppError, state::AppState};

// Các giới hạn khi kiểm câu trả lời của mô hình (đếm theo ký tự, không phải byte). Mô tả và bí danh vào prompt của
// các bước sau (#28, #29), câu gợi ý hiện trên trang hỏi (#50); dài quá thì chiếm chỗ prompt và vỡ bố cục.
const DESCRIPTION_MAX_CHARS: usize = 300;
const ALIAS_MAX_CHARS: usize = 60;
const ALIASES_MAX: usize = 8;
const QUESTIONS_MAX: usize = 5;
const QUESTION_MAX_CHARS: usize = 200;

/// Câu trả lời của mô hình sau khi kiểm: tên cột và giá trị đã đổi ra id, mọi giới hạn đã áp.
pub struct SemanticProfile {
    pub columns: Vec<SemanticColumnUpdate>,
    pub values: Vec<SemanticValueUpdate>,
    pub suggested_questions: Vec<String>,
}

/// Lập hồ sơ ngữ nghĩa cho một phiên bản vừa nạp, chạy nền: lỗi (mô hình 502/503, mock không có bảng, JSON hỏng,
/// không mục nào hợp lệ) chỉ ghi log, phiên bản giữ profile_status = structure.
pub fn spawn_semantic_profile(
    state: AppState,
    ws: WorkspaceId,
    dataset: Dataset,
    version: DatasetVersion,
) {
    tokio::spawn(async move {
        let version_id = version.id;
        match apply_semantic_profile(&state, &ws, &dataset, &version).await {
            Ok(()) => tracing::info!(version_id = %version_id, "semantic profile applied"),
            Err(e) => tracing::warn!(version_id = %version_id, error = ?e,
                "semantic profile failed, keeping the structural profile"),
        }
    });
}

/// Một lượt lớp ngữ nghĩa: đọc hồ sơ cấu trúc của phiên bản, dựng prompt, gọi mô hình, kiểm rồi ghi trong một giao
/// dịch. Lỗi trả về để người gọi ghi log; không bao giờ chuyển profile_status sang failed (khác lỗi lớp cấu trúc:
/// lỗi ở đây là của mô hình, thử lại tay bằng cách nạp phiên bản mới).
pub async fn apply_semantic_profile(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    version: &DatasetVersion,
) -> Result<(), AppError> {
    let columns = state.store.list_columns(ws, version.id).await?;
    // Chỉ cột dimension không nhạy cảm có danh sách giá trị trong prompt, nên cũng chỉ đọc giá trị của các cột đó;
    // mô hình không thấy thì bí danh nó đề xuất cho giá trị khác cũng không qua được kiểm tra.
    let value_columns: Vec<Uuid> = columns
        .iter()
        .filter(|c| c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive)
        .map(|c| c.id)
        .collect();
    let values = state.store.list_column_values(ws, &value_columns).await?;
    let prompt = build_profile_prompt(
        &dataset.name,
        &dataset.logical_table,
        version.row_count,
        &columns,
        &values,
    );
    let request = LlmRequest {
        task: LlmTask::Profile,
        question: &dataset.logical_table,
        prompt: &prompt,
    };
    let text = state.llm.complete(&request).await?;
    let answer = build_profile_answer(&text, &columns, &values).ok_or_else(|| {
        AppError::Internal("semantic profile answer has no valid item".to_string())
    })?;
    let written = state
        .store
        .apply_semantic_profile(
            ws,
            version.id,
            &answer.columns,
            &answer.values,
            &answer.suggested_questions,
        )
        .await?;
    // Phiên bản không còn ở structure (ví dụ một lượt chạy song song đã ghi xong): không có gì để làm.
    if !written {
        tracing::info!(version_id = %version.id, "semantic profile skipped, version already left structure");
    }
    Ok(())
}

/// Prompt cho task Profile: luật và khuôn JSON trước, rồi mỗi cột một dòng siêu dữ liệu, cuối cùng danh sách giá trị
/// của các cột dimension không nhạy cảm. Chỉ siêu dữ liệu và giá trị cột dimension đi ra ngoài: min, max chỉ kèm cột
/// measure và time (cột chữ min, max có thể là một giá trị thật của người dùng); cột nhạy cảm chỉ gửi tên, kiểu, vai
/// trò và số đo, không gửi giá trị nào. Phần luật viết tiếng Anh (như prompt sinh SQL, #29); mô hình trả lời bằng
/// tiếng Việt theo luật.
pub fn build_profile_prompt(
    dataset_name: &str,
    logical_table: &str,
    row_count: Option<i64>,
    columns: &[ProfileColumn],
    values: &[ColumnValue],
) -> String {
    let mut prompt = format!(
        "You are documenting a dataset for a Vietnamese analytics tool. Reply with one JSON object and nothing else:\n\
         {{\"columns\": [{{\"name\", \"description\", \"aliases\"}}], \"values\": [{{\"column\", \"value\", \
         \"aliases\"}}], \"role_changes\": [{{\"name\", \"role\"}}], \"suggested_questions\": [...]}}\n\
         Rules:\n\
         - \"description\": one Vietnamese sentence on what the column means, 1 to {DESCRIPTION_MAX_CHARS} characters; \
         null when nothing useful to say.\n\
         - \"aliases\": up to {ALIASES_MAX} Vietnamese names users may use for that column or value, each 1 to \
         {ALIAS_MAX_CHARS} characters.\n\
         - Only the columns and values listed below exist; never invent new ones.\n\
         - \"role_changes\": propose a new role only in two cases: an integer \"measure\" column that is really an \
         identifier or a code (\"key\" when each value identifies one thing, like an order id; \"dimension\" when it \
         groups rows, like a year or a zip code), or a \"dimension\" column that is really free text (\"text\", like \
         an image URL). No other change is allowed; omit \"role_changes\" when nothing qualifies.\n\
         - \"suggested_questions\": up to {QUESTIONS_MAX} short Vietnamese questions this dataset can answer, each at \
         most {QUESTION_MAX_CHARS} characters.\n\
         Dataset \"{dataset_name}\", table \"{logical_table}\""
    );
    if let Some(rows) = row_count {
        prompt.push_str(&format!(", {rows} rows"));
    }
    prompt.push_str(".\nColumns (name | type | role | null % | distinct | min | max):\n");
    for c in columns {
        prompt.push_str(&column_line(c, row_count));
        prompt.push('\n');
    }
    if !values.is_empty() {
        prompt.push_str("Values of dimension columns (value (rows), ...):\n");
        let mut by_column: HashMap<Uuid, Vec<&ColumnValue>> = HashMap::new();
        for v in values {
            by_column.entry(v.column_id).or_default().push(v);
        }
        for c in columns {
            // Chốt riêng tư nằm ở đây chứ không trông chờ người gọi lọc trước: giá trị của cột text, key hay cột
            // nhạy cảm không bao giờ vào prompt, kể cả khi lỡ truyền vào.
            let printable = c.role == ProfileRole::Dimension.as_str() && !c.is_sensitive;
            let Some(list) = by_column.get(&c.id).filter(|_| printable) else {
                continue;
            };
            let items: Vec<String> = list
                .iter()
                .map(|v| match v.frequency {
                    Some(n) => format!("{} ({n})", v.value),
                    None => v.value.clone(),
                })
                .collect();
            prompt.push_str(&format!("{}: {}\n", c.name, items.join(", ")));
        }
    }
    prompt
}

/// Một dòng siêu dữ liệu của cột. null % và distinct lấy từ hồ sơ cấu trúc; min, max chỉ in với cột measure và time.
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
        c.name, c.data_type, c.role
    );
    let role = c.role.as_str();
    let with_range = role == ProfileRole::Measure.as_str() || role == ProfileRole::Time.as_str();
    if with_range {
        if let (Some(min), Some(max)) = (&c.min_value, &c.max_value) {
            line.push_str(&format!(" | {min} | {max}"));
        }
    }
    line
}

/// Đọc câu trả lời của mô hình thành SemanticProfile đã kiểm. None khi không đọc được JSON hoặc không còn mục nào
/// hợp lệ (người gọi coi như lỗi mô hình). Từng mục được kiểm riêng: cột không có thật, giá trị không có trong
/// column_values, chuỗi quá dài, vai trò không được phép thì bỏ mục đó, các mục khác vẫn ghi.
pub fn build_profile_answer(
    text: &str,
    columns: &[ProfileColumn],
    values: &[ColumnValue],
) -> Option<SemanticProfile> {
    let json: serde_json::Value = serde_json::from_str(strip_fence(text)).ok()?;
    let by_name: HashMap<&str, &ProfileColumn> =
        columns.iter().map(|c| (c.name.as_str(), c)).collect();

    // Mục cột và đổi vai trò cùng ghi vào bảng columns, nên gom theo id cột: đổi vai trò của một cột không có mục
    // riêng vẫn được ghi (mô tả null, bí danh rỗng là mặc định sẵn có).
    let mut column_updates: HashMap<Uuid, SemanticColumnUpdate> = HashMap::new();
    for item in json["columns"].as_array().into_iter().flatten() {
        let Some(column) = item["name"].as_str().and_then(|n| by_name.get(n)) else {
            continue;
        };
        column_updates.insert(
            column.id,
            SemanticColumnUpdate {
                column_id: column.id,
                description: valid_text(&item["description"], DESCRIPTION_MAX_CHARS),
                aliases: valid_aliases(&item["aliases"]),
                role: None,
            },
        );
    }
    for item in json["role_changes"].as_array().into_iter().flatten() {
        let (Some(name), Some(role)) = (item["name"].as_str(), item["role"].as_str()) else {
            continue;
        };
        let Some(column) = by_name.get(name) else {
            continue;
        };
        let Some(role) = allowed_role_change(column, role) else {
            continue;
        };
        column_updates
            .entry(column.id)
            .or_insert_with(|| SemanticColumnUpdate {
                column_id: column.id,
                description: None,
                aliases: Vec::new(),
                role: None,
            })
            .role = Some(role);
    }

    // Giá trị phải có thật trong column_values của đúng cột (đã đọc từ Postgres, tức là của cột dimension không
    // nhạy cảm): mô hình không thể đặt bí danh cho giá trị nó không thấy.
    let known: std::collections::HashSet<(Uuid, &str)> = values
        .iter()
        .map(|v| (v.column_id, v.value.as_str()))
        .collect();
    let mut value_updates = Vec::new();
    for item in json["values"].as_array().into_iter().flatten() {
        let (Some(column_name), Some(value)) = (item["column"].as_str(), item["value"].as_str())
        else {
            continue;
        };
        let Some(column) = by_name.get(column_name) else {
            continue;
        };
        if !known.contains(&(column.id, value)) {
            continue;
        }
        value_updates.push(SemanticValueUpdate {
            column_id: column.id,
            value: value.to_string(),
            aliases: valid_aliases(&item["aliases"]),
        });
    }

    let mut suggested_questions = Vec::new();
    for item in json["suggested_questions"].as_array().into_iter().flatten() {
        if suggested_questions.len() >= QUESTIONS_MAX {
            break;
        }
        if let Some(q) = item.as_str() {
            if (1..=QUESTION_MAX_CHARS).contains(&q.chars().count()) {
                suggested_questions.push(q.to_string());
            }
        }
    }

    if column_updates.is_empty() && value_updates.is_empty() && suggested_questions.is_empty() {
        return None;
    }
    // Giữ thứ tự cột của bảng để log và test đọc dễ.
    let columns_out: Vec<SemanticColumnUpdate> = columns
        .iter()
        .filter_map(|c| column_updates.remove(&c.id))
        .collect();
    Some(SemanticProfile {
        columns: columns_out,
        values: value_updates,
        suggested_questions,
    })
}

/// Bỏ khối rào markdown (```json ... ```) mô hình hay bọc quanh JSON dù luật đã cấm.
fn strip_fence(text: &str) -> &str {
    let text = text.trim();
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    // Dòng đầu sau ``` là tên ngôn ngữ (json) hoặc rỗng; bỏ cả dòng đó.
    let body = rest.split_once('\n').map_or(rest, |(_, body)| body);
    body.trim().strip_suffix("```").unwrap_or(body).trim()
}

/// Chuỗi hợp lệ trong khoảng 1 tới max_chars ký tự; null, sai kiểu hay quá dài thành None (mặc định).
fn valid_text(value: &serde_json::Value, max_chars: usize) -> Option<String> {
    let text = value.as_str()?;
    (1..=max_chars)
        .contains(&text.chars().count())
        .then(|| text.to_string())
}

/// Bí danh hợp lệ: chuỗi 1 tới ALIAS_MAX_CHARS ký tự, bỏ trùng, tối đa ALIASES_MAX cái. Mục sai bỏ riêng.
fn valid_aliases(value: &serde_json::Value) -> Vec<String> {
    let mut aliases = Vec::new();
    for item in value.as_array().into_iter().flatten() {
        let Some(alias) = item.as_str() else { continue };
        if !(1..=ALIAS_MAX_CHARS).contains(&alias.chars().count())
            || aliases.iter().any(|a| a == alias)
        {
            continue;
        }
        aliases.push(alias.to_string());
        if aliases.len() >= ALIASES_MAX {
            break;
        }
    }
    aliases
}

/// Đổi vai trò chỉ trong phạm vi hẹp đã chốt: cột số nguyên đang là measure thành key (mã định danh lặp lại như
/// Order Id, mã bưu điện) hoặc dimension (năm, tháng dạng số); cột dimension thành text (đường dẫn ảnh, mô tả dài).
/// Mọi chiều đổi khác (ví dụ measure thành time) bị bỏ: luật cấu trúc đã xét kiểu và số đo, mô hình chỉ sửa được
/// những ca số đo không nói lên được.
fn allowed_role_change(column: &ProfileColumn, target: &str) -> Option<ProfileRole> {
    let measure = ProfileRole::Measure.as_str();
    let dimension = ProfileRole::Dimension.as_str();
    match (column.role.as_str(), target) {
        (r, "key") if r == measure && is_integer_type(&column.data_type) => Some(ProfileRole::Key),
        (r, "dimension") if r == measure && is_integer_type(&column.data_type) => {
            Some(ProfileRole::Dimension)
        }
        (r, "text") if r == dimension => Some(ProfileRole::Text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::engine::llm::LlmProvider;
    use crate::engine::store::test_support::{drop_workspace, store, temp_workspace};
    use crate::engine::store::{NewColumn, NewColumnValue, NewVersion, SourceKind};

    fn column(id: Uuid, name: &str, data_type: &str, role: ProfileRole) -> ProfileColumn {
        ProfileColumn {
            id,
            position: 0,
            name: name.to_string(),
            data_type: data_type.to_string(),
            role: role.as_str().to_string(),
            description: None,
            aliases: Vec::new(),
            is_sensitive: false,
            null_count: Some(0),
            distinct_count: Some(3),
            min_value: None,
            max_value: None,
            time_format: None,
        }
    }

    fn value(column_id: Uuid, text: &str, frequency: i64) -> ColumnValue {
        ColumnValue {
            column_id,
            value: text.to_string(),
            aliases: Vec::new(),
            frequency: Some(frequency),
        }
    }

    /// Prompt chỉ chứa siêu dữ liệu và giá trị của cột dimension không nhạy cảm: giá trị của cột text, key và cột
    /// nhạy cảm không được lọt vào, kể cả khi lỡ truyền vào. Min, max chỉ kèm cột measure và time.
    #[test]
    fn prompt_has_only_metadata_and_dimension_values() {
        let market = column(Uuid::new_v4(), "Market", "VARCHAR", ProfileRole::Dimension);
        let total = {
            let mut c = column(
                Uuid::new_v4(),
                "Order Item Total",
                "DOUBLE",
                ProfileRole::Measure,
            );
            c.min_value = Some("1.5".to_string());
            c.max_value = Some("999.0".to_string());
            c
        };
        let note = column(Uuid::new_v4(), "Ghi chú", "VARCHAR", ProfileRole::Text);
        let order_id = column(Uuid::new_v4(), "Order Id", "BIGINT", ProfileRole::Key);
        let mut secret = column(
            Uuid::new_v4(),
            "Mã bí mật",
            "VARCHAR",
            ProfileRole::Dimension,
        );
        secret.is_sensitive = true;
        let columns = [market.clone(), total, note, order_id, secret];
        // Giá trị của cả năm cột, kể cả những cột không bao giờ được gửi.
        let values = vec![
            value(market.id, "Europe", 5),
            value(market.id, "LATAM", 3),
            value(columns[2].id, "giao trước 5 giờ", 1),
            value(columns[3].id, "882139", 1),
            value(columns[4].id, "hunter2", 1),
        ];
        let prompt = build_profile_prompt("DataCo", "dataco", Some(100), &columns, &values);

        assert!(prompt.contains("Market | VARCHAR | dimension"), "{prompt}");
        assert!(prompt.contains("Europe (5), LATAM (3)"), "{prompt}");
        assert!(prompt.contains("1.5 | 999.0"), "{prompt}");
        for leaked in ["giao trước 5 giờ", "882139", "hunter2"] {
            assert!(
                !prompt.contains(leaked),
                "prompt leaked {leaked:?}: {prompt}"
            );
        }
        // Cột text không có min, max dù hồ sơ cấu trúc có đo.
        let note_line = prompt.lines().find(|l| l.starts_with("Ghi chú")).unwrap();
        assert_eq!(note_line, "Ghi chú | VARCHAR | text | null 0% | 3 distinct");
    }

    /// Mỗi mục của câu trả lời được kiểm riêng: mục sai bỏ, mục đúng ghi; câu bọc ```json vẫn đọc được.
    #[test]
    fn answers_are_checked_item_by_item() {
        let market = column(Uuid::new_v4(), "Market", "VARCHAR", ProfileRole::Dimension);
        let order_id = column(Uuid::new_v4(), "Order Id", "BIGINT", ProfileRole::Measure);
        let sales = column(Uuid::new_v4(), "Sales", "DOUBLE", ProfileRole::Measure);
        let image = column(
            Uuid::new_v4(),
            "Product Image",
            "VARCHAR",
            ProfileRole::Dimension,
        );
        let columns = [market.clone(), order_id.clone(), sales, image];
        let values = vec![value(market.id, "Europe", 5)];
        let long_alias = "x".repeat(500);
        let answer = format!(
            r#"```json
            {{"columns": [
                {{"name": "Market", "description": "Thị trường", "aliases": ["thị trường", "{long_alias}"]}},
                {{"name": "Không có", "description": "x", "aliases": []}},
                {{"name": "Sales", "description": "{}"}}
            ],
            "values": [
                {{"column": "Market", "value": "Europe", "aliases": ["châu Âu"]}},
                {{"column": "Market", "value": "Châu Âu", "aliases": ["x"]}},
                {{"column": "Không có", "value": "Europe", "aliases": ["x"]}}
            ],
            "role_changes": [
                {{"name": "Order Id", "role": "key"}},
                {{"name": "Product Image", "role": "text"}},
                {{"name": "Sales", "role": "key"}},
                {{"name": "Market", "role": "key"}}
            ],
            "suggested_questions": [{}]}}
            ```"#,
            "m".repeat(DESCRIPTION_MAX_CHARS + 1),
            (0..12)
                .map(|i| format!("\"Câu {i}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let profile = build_profile_answer(&answer, &columns, &values).unwrap();

        // Cột không có thật bị bỏ; mô tả quá dài thành null nhưng cột vẫn ghi; bí danh 500 ký tự bị bỏ riêng.
        // Bốn cột được ghi: Market và Sales (mục cột), Order Id và Product Image (chỉ có đổi vai trò).
        assert_eq!(profile.columns.len(), 4, "{:?}", profile.columns);
        let market_update = &profile.columns[0];
        assert_eq!(market_update.description.as_deref(), Some("Thị trường"));
        assert_eq!(market_update.aliases, ["thị trường"]);
        let sales_update = profile
            .columns
            .iter()
            .find(|c| c.column_id == columns[2].id)
            .unwrap();
        assert_eq!(sales_update.description, None);

        // Giá trị không có trong column_values, hoặc thuộc cột không có, đều bị bỏ.
        assert_eq!(profile.values.len(), 1);
        assert_eq!(profile.values[0].aliases, ["châu Âu"]);

        // Chỉ hai đổi vai trò được phép: measure số nguyên -> key, dimension -> text.
        let role_of = |id: Uuid| {
            profile
                .columns
                .iter()
                .find(|c| c.column_id == id)
                .unwrap()
                .role
        };
        assert_eq!(role_of(order_id.id), Some(ProfileRole::Key));
        assert_eq!(role_of(columns[3].id), Some(ProfileRole::Text));
        assert_eq!(role_of(market.id), None);

        // 12 câu gợi ý cắt còn 5.
        assert_eq!(profile.suggested_questions.len(), QUESTIONS_MAX);
        assert_eq!(profile.suggested_questions[0], "Câu 0");
    }

    #[test]
    fn unusable_answers_are_rejected_as_a_whole() {
        let market = column(Uuid::new_v4(), "Market", "VARCHAR", ProfileRole::Dimension);
        let columns = std::slice::from_ref(&market);
        let values = vec![value(market.id, "Europe", 5)];
        // JSON hỏng, không phải JSON, và JSON đúng nhưng không mục nào hợp lệ.
        assert!(build_profile_answer("{not json", columns, &values).is_none());
        assert!(build_profile_answer("xin lỗi, tôi không biết", columns, &values).is_none());
        let only_invalid =
            r#"{"columns": [{"name": "Không có"}], "values": [], "suggested_questions": ["", 42]}"#;
        assert!(build_profile_answer(only_invalid, columns, &values).is_none());
    }

    #[test]
    fn role_changes_stay_within_the_narrow_range() {
        let int_measure = column(Uuid::new_v4(), "Order Id", "BIGINT", ProfileRole::Measure);
        assert_eq!(
            allowed_role_change(&int_measure, "key"),
            Some(ProfileRole::Key)
        );
        assert_eq!(
            allowed_role_change(&int_measure, "dimension"),
            Some(ProfileRole::Dimension)
        );
        assert_eq!(allowed_role_change(&int_measure, "time"), None);
        // Measure không phải số nguyên (tiền) không đổi được.
        let money = column(Uuid::new_v4(), "Sales", "DOUBLE", ProfileRole::Measure);
        assert_eq!(allowed_role_change(&money, "key"), None);
        // Dimension thành text được; chiều ngược lại và mọi chiều khác không.
        let dim = column(
            Uuid::new_v4(),
            "Product Image",
            "VARCHAR",
            ProfileRole::Dimension,
        );
        assert_eq!(allowed_role_change(&dim, "text"), Some(ProfileRole::Text));
        assert_eq!(allowed_role_change(&dim, "measure"), None);
        let text = column(Uuid::new_v4(), "Ghi chú", "VARCHAR", ProfileRole::Text);
        assert_eq!(allowed_role_change(&text, "dimension"), None);
        let key = column(Uuid::new_v4(), "Id", "BIGINT", ProfileRole::Key);
        assert_eq!(allowed_role_change(&key, "measure"), None);
    }

    fn new_column(
        position: i32,
        name: &str,
        data_type: &str,
        role: ProfileRole,
        values: &[(&str, i64)],
    ) -> NewColumn {
        NewColumn {
            position,
            name: name.to_string(),
            data_type: data_type.to_string(),
            role,
            null_count: 0,
            distinct_count: values.len() as i64,
            min_value: None,
            max_value: None,
            time_format: None,
            values: values
                .iter()
                .map(|(value, frequency)| NewColumnValue {
                    value: value.to_string(),
                    frequency: *frequency,
                })
                .collect(),
        }
    }

    async fn dataco(
        store: &crate::engine::store::Store,
        ws: &WorkspaceId,
    ) -> (Dataset, DatasetVersion) {
        let src = store.create_local_source(ws).await.unwrap();
        let ds = store
            .create_dataset(ws, src, "DataCo", "dataco", None)
            .await
            .unwrap();
        let columns = [
            new_column(
                0,
                "Market",
                "VARCHAR",
                ProfileRole::Dimension,
                &[
                    ("Europe", 5),
                    ("LATAM", 3),
                    ("Pacific Asia", 2),
                    ("USCA", 1),
                    ("Africa", 1),
                ],
            ),
            new_column(1, "Order Id", "BIGINT", ProfileRole::Measure, &[]),
            new_column(
                2,
                "Product Image",
                "VARCHAR",
                ProfileRole::Dimension,
                &[("img/1.jpg", 4)],
            ),
            new_column(3, "Order Item Total", "DOUBLE", ProfileRole::Measure, &[]),
        ];
        let version = NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: "h1",
            parquet_path: "x",
            row_count: 12,
        };
        let v = store
            .create_version(ws, ds.id, &version, &columns)
            .await
            .unwrap();
        (ds, v)
    }

    /// Mock trên dataco: mô tả, bí danh cột và giá trị, đổi vai trò trong phạm vi hẹp, 5 câu gợi ý, status semantic.
    /// Gọi lại lần hai (lượt song song chậm) không ghi đè.
    #[tokio::test]
    async fn mock_dataco_profile_is_applied() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let (ds, v) = dataco(&store, &ws).await;
        // Config mặc định dùng provider mock (LLM_PROVIDER=mock).
        let state = AppState::new(crate::config::Config::from_env(), store.clone());
        apply_semantic_profile(&state, &ws, &ds, &v).await.unwrap();

        let columns = store.list_columns(&ws, v.id).await.unwrap();
        let by_name = |name: &str| columns.iter().find(|c| c.name == name).unwrap();
        let market = by_name("Market");
        assert_eq!(
            market.description.as_deref(),
            Some("Thị trường nơi giao đơn hàng")
        );
        assert_eq!(market.aliases, ["thị trường", "châu lục"]);
        // Số nguyên lặp lại thành key; cột chiều là đường dẫn ảnh thành text.
        assert_eq!(by_name("Order Id").role, "key");
        assert_eq!(by_name("Product Image").role, "text");
        assert_eq!(by_name("Order Item Total").role, "measure");

        let market_values = store.list_column_values(&ws, &[market.id]).await.unwrap();
        let europe = market_values.iter().find(|v| v.value == "Europe").unwrap();
        assert_eq!(europe.aliases, ["châu Âu"]);
        // Giá trị khác của Market cũng có bí danh (mock ghi đủ 5 giá trị).
        let latam = market_values.iter().find(|v| v.value == "LATAM").unwrap();
        assert_eq!(latam.aliases, ["Mỹ Latinh"]);

        let v = store
            .find_version_by_hash(&ws, ds.id, "h1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.profile_status, "semantic");
        store.set_current_version(&ws, ds.id, v.id).await.unwrap();
        let ds = store.get_dataset(&ws, ds.id).await.unwrap().unwrap();
        assert_eq!(ds.suggested_questions.len(), 5);
        assert_eq!(ds.suggested_questions[0], "Doanh thu theo thị trường");
        // Danh sách dataset cũng có câu gợi ý của phiên bản hiện hành.
        let list = store.list_datasets(&ws).await.unwrap();
        assert_eq!(list[0].suggested_questions.len(), 5);

        // Lượt thứ hai: phiên bản đã semantic, store từ chối ghi, dữ liệu không đổi.
        apply_semantic_profile(&state, &ws, &ds, &v).await.unwrap();
        let columns = store.list_columns(&ws, v.id).await.unwrap();
        assert_eq!(
            columns[0].description.as_deref(),
            Some("Thị trường nơi giao đơn hàng")
        );

        drop_workspace(&store, ws).await;
    }

    /// Provider luôn lỗi.
    struct FailingLlm;

    #[async_trait::async_trait]
    impl LlmProvider for FailingLlm {
        fn name(&self) -> String {
            "failing".to_string()
        }

        async fn complete(&self, _request: &LlmRequest<'_>) -> Result<String, AppError> {
            Err(AppError::Upstream("model is down".to_string()))
        }
    }

    /// Mô hình lỗi: apply báo lỗi để người gọi ghi log, phiên bản giữ structure, không mô tả nào được ghi.
    #[tokio::test]
    async fn llm_failure_keeps_the_structural_profile() {
        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let (ds, v) = dataco(&store, &ws).await;
        let mut state = AppState::new(crate::config::Config::from_env(), store.clone());
        state.llm = Arc::new(FailingLlm);

        let err = apply_semantic_profile(&state, &ws, &ds, &v)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)), "{err:?}");
        let v = store
            .find_version_by_hash(&ws, ds.id, "h1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.profile_status, "structure");
        let columns = store.list_columns(&ws, v.id).await.unwrap();
        assert!(columns.iter().all(|c| c.description.is_none()));
        let ds = store.get_dataset(&ws, ds.id).await.unwrap().unwrap();
        assert!(ds.suggested_questions.is_empty());

        drop_workspace(&store, ws).await;
    }
}
