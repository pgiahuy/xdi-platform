// SPDX-License-Identifier: Apache-2.0
/* service/profile.rs - hồ sơ cột lớp cấu trúc của một phiên bản dataset: số đo của từng cột (engine/profile.rs) cộng
vai trò cột (build_column_role) cộng cờ nhạy cảm theo luật (is_sensitive_column), ghi vào columns và column_values
(engine/store/columns.rs). Code tính hết, không gọi mô hình ngôn ngữ, nên chạy như nhau với LLM_PROVIDER=mock. Guard
(#31, #33), ngữ cảnh prompt (#28) và câu "tại sao" (#38, #39) đọc hồ sơ này; mô tả, bí danh là lớp ngữ nghĩa (#23).

Ba chỗ gọi: service/ingest.rs lập hồ sơ cho phiên bản vừa nạp, trước khi trả 201; main.rs gọi profile_pending_versions
lúc khởi động cho phiên bản nạp trước khi có bước này; main.rs cũng gọi mark_sensitive_columns_at_start (đồng bộ,
trước khi nhận request) để chạy lại luật nhạy cảm cho phiên bản nạp trước khi luật tồn tại. Luật vai trò ghi lại ở
docs/DATA.md. */

use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use crate::auth::WorkspaceId;
use crate::engine::analytics::SOURCE_COLUMN;
use crate::engine::duckdb_limits::DuckdbLimits;
use crate::engine::profile::{self, ColumnStats, ValueCount};
use crate::engine::store::{NewColumn, NewColumnValue, PendingVersion, ProfileRole, ProfileStatus};
use crate::service::ask::phrases::fold;
use crate::{error::AppError, state::AppState};

/// Cột chữ có không quá ngần này giá trị khác nhau là chiều (dimension), nhiều hơn là chữ tự do (text). Cũng là ngưỡng
/// ghi column_values cho mọi cột. 200 để DataCo có Order Country (khoảng 160 nước) là chiều, Order City (hàng nghìn
/// thành phố) là text: danh sách giá trị đi vào prompt và vào kiểm form "tại sao", dài hơn thì tốn chỗ mà ít khi dùng.
pub const DIMENSION_MAX: i64 = 200;

/// Giá trị dài hơn ngần này byte thì cột không ghi column_values. Ràng buộc UNIQUE (column_id, value) là một chỉ mục
/// btree, mà Postgres từ chối dòng chỉ mục dài quá khoảng 2700 byte: không chặn thì một cột ít giá trị mà giá trị dài
/// (mô tả sản phẩm trong bảng nhỏ) làm hỏng cả lần nạp. Bỏ cả danh sách của cột chứ không bỏ riêng giá trị dài, để
/// danh sách đã ghi luôn đủ.
const VALUE_MAX_BYTES: usize = 1000;

/// Tên cột dài hơn ngần này byte thì từ chối lần nạp. Tên cột nằm trong chỉ mục UNIQUE (version_id, name), cùng giới
/// hạn khoảng 2700 byte như trên. 1000 byte vẫn nhận tiêu đề dài như câu hỏi của Google Forms (nạp được từ trước khi có
/// hồ sơ cột), mà còn cách xa trần đó; lỗi báo trước khi ghi gì.
const COLUMN_NAME_MAX_BYTES: usize = 1000;

/// Vai trò của một cột theo số đo của nó, thử từng luật theo thứ tự, luật đầu tiên khớp thắng. Hàm thuần, không đọc
/// tên cột (trừ cột hệ thống _source_file): số nguyên lặp lại như Order Id thành measure, sửa ở lớp ngữ nghĩa (#23).
pub fn build_column_role(stats: &ColumnStats, row_count: i64) -> ProfileRole {
    let data_type = stats.data_type.as_str();
    // Không có giá trị nào khác null thì không dùng được vào đâu.
    if stats.distinct_count == 0 || stats.name == SOURCE_COLUMN {
        return ProfileRole::Ignore;
    }
    if is_time_type(data_type) || stats.time_format.is_some() {
        return ProfileRole::Time;
    }
    // Bảng một hai dòng thì cột số nào cũng mỗi dòng một giá trị và có tối đa 2 giá trị: hai luật key và cờ 0/1 dưới
    // đây không nói lên điều gì, nên số ở bảng như vậy là measure.
    let enough_rows = row_count > 2;
    // Mỗi dòng một giá trị khác nhau, không ô trống: định danh dòng, cộng hay nhóm theo đều vô nghĩa.
    if enough_rows
        && is_integer_type(data_type)
        && stats.null_count == 0
        && stats.distinct_count == row_count
    {
        return ProfileRole::Key;
    }
    if is_number_type(data_type) {
        // Cột cờ 0/1 (DataCo: Late_delivery_risk) là nhóm có hay không, không phải số để cộng.
        return if enough_rows && stats.distinct_count <= 2 {
            ProfileRole::Dimension
        } else {
            ProfileRole::Measure
        };
    }
    // Cột chiều luôn có danh sách giá trị trong column_values (các phần sau kiểm giá trị lọc theo danh sách này), nên
    // cột ít giá trị mà danh sách không ghi được là text.
    if (data_type == "BOOLEAN" || data_type == "VARCHAR")
        && stats.distinct_count <= DIMENSION_MAX
        && can_store_values(&stats.values)
    {
        return ProfileRole::Dimension;
    }
    ProfileRole::Text
}

/// Hồ sơ chờ ghi của mọi cột, giữ thứ tự cột của bảng. Cờ nhạy cảm được đặt ngay tại đây (is_sensitive_column),
/// trong cùng giao dịch ghi hồ sơ, trước khi phiên bản dùng được: giá trị của cột nhạy cảm không bao giờ rời máy
/// chủ, kể cả khi lớp ngữ nghĩa (#23) chưa chạy hay lỗi mãi.
pub fn build_new_columns(stats: Vec<ColumnStats>, row_count: i64) -> Vec<NewColumn> {
    stats
        .into_iter()
        .enumerate()
        .map(|(i, s)| NewColumn {
            // Số cột của một bảng không tới giới hạn i32 (DuckDB và Parquet giới hạn thấp hơn nhiều).
            position: i as i32,
            role: build_column_role(&s, row_count),
            is_sensitive: is_sensitive_column(&s.name, &s.values),
            name: s.name,
            data_type: s.data_type,
            null_count: s.null_count,
            distinct_count: s.distinct_count,
            min_value: s.min_value.map(remove_nul),
            max_value: s.max_value.map(remove_nul),
            time_format: s.time_format,
            description: None,
            values: build_new_values(s.values),
        })
        .collect()
}

/// Cụm tên cột nhạy cảm, so theo dạng đã chuẩn hóa (bỏ dấu, chữ thường, tách từ): email, mật khẩu, số điện
/// thoại, số giấy tờ, địa chỉ, họ tên. Một cụm khớp khi các từ của nó đứng liền nhau trong tên cột (tách theo
/// khoảng trắng, gạch, camelCase và ranh giới chữ - số), nên "tel" không bắt nhầm "Hotel" và không có cụm "mail" đứng
/// một mình (để "Mailing Region" không thành nhạy cảm). Riêng "dien thoai" (SENSITIVE_NAME_PREFIX) chỉ khớp khi đứng đầu tên cột ("Điện
/// thoại", "Điện thoại liên hệ"), để "Dòng điện thoại", "Loại điện thoại" vẫn là chiều thường. Luật cố ý đơn giản và
/// nghiêng về đánh dấu nhầm còn hơn bỏ sót: đánh dấu nhầm chỉ làm prompt thiếu một danh sách giá trị.
const SENSITIVE_NAME_PHRASES: &[&str] = &[
    "email",
    "e mail",
    "emails",
    "password",
    "passwd",
    "mat khau",
    "phone",
    "phone number",
    "phones",
    "telephone",
    "telephones",
    "mobile",
    "tel",
    "sdt",
    "so dien thoai",
    "so dt",
    "cmnd",
    "cccd",
    "passport",
    "ho chieu",
    "dia chi",
    "address",
    "addresses",
    "street",
    "ho va ten",
    "ho ten",
    "ten khach hang",
    "ten kh",
    "ten khach",
    "customer name",
    "full name",
    "fullname",
    "first name",
    "firstname",
    "fname",
    "last name",
    "lastname",
    "lname",
];

/// Tên cột bắt đầu bằng cụm này là nhạy cảm (xem SENSITIVE_NAME_PHRASES). Không có "dt": trong bảng tiếng Việt
/// "DT" hay là doanh thu.
const SENSITIVE_NAME_PREFIX: &str = "dien thoai";

/// Cột nhạy cảm theo luật: tên khớp một cụm trong SENSITIVE_NAME_PHRASES (theo từ) hay bắt đầu bằng
/// SENSITIVE_NAME_PREFIX, hoặc phần lớn giá trị của cột trông như email hay số điện thoại. Chỉ cột ít giá trị có danh
/// sách giá trị trong bộ nhớ, nên luật giá trị chỉ xét được các cột đó; đó cũng là các cột có giá trị đi vào prompt.
/// Mô hình (#23) đề xuất đánh dấu thêm sau, chỉ chiều bật.
pub(crate) fn is_sensitive_column(name: &str, values: &[impl AsRef<str>]) -> bool {
    let tokens = name_tokens(name);
    if starts_with_phrase(&tokens, SENSITIVE_NAME_PREFIX)
        || SENSITIVE_NAME_PHRASES
            .iter()
            .any(|phrase| contains_phrase(&tokens, phrase))
    {
        return true;
    }
    !values.is_empty()
        && values.iter().filter(|v| looks_personal(v.as_ref())).count() * 2 > values.len()
}

/// Tên cột thành dãy từ: tách ranh giới camelCase ("CustomerName" thành "Customer Name") và ranh giới chữ - số
/// ("Address1" thành "Address 1"), rồi fold (bỏ dấu, chữ thường, tách theo ký tự không phải chữ hay số) và tách từ.
fn name_tokens(name: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(name.len() + 8);
    let mut prev: Option<char> = None;
    for c in name.chars() {
        if let Some(p) = prev {
            let camel = p.is_lowercase() && c.is_uppercase();
            let letter_digit = (p.is_alphabetic() && c.is_ascii_digit())
                || (p.is_ascii_digit() && c.is_alphabetic());
            if camel || letter_digit {
                spaced.push(' ');
            }
        }
        prev = Some(c);
        spaced.push(c);
    }
    fold(&spaced)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Cụm (dãy từ) có đứng liền nhau trong dãy từ của tên cột không.
fn contains_phrase(tokens: &[String], phrase: &str) -> bool {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    tokens
        .windows(words.len())
        .any(|w| w.iter().zip(&words).all(|(a, b)| a == b))
}

/// Cụm có đứng ngay đầu dãy từ của tên cột không.
fn starts_with_phrase(tokens: &[String], phrase: &str) -> bool {
    let len = phrase.split_whitespace().count();
    tokens.len() >= len && contains_phrase(&tokens[..len], phrase)
}

/// Giá trị trông như dữ liệu cá nhân: có dạng email (một @ và dấu chấm sau nó) hay số điện thoại
/// (looks_like_phone).
fn looks_personal(value: &str) -> bool {
    if let Some((_, domain)) = value.split_once('@') {
        if domain.contains('.') {
            return true;
        }
    }
    looks_like_phone(value)
}

/// Số điện thoại, bỏ qua khoảng trắng, chấm, gạch, ngoặc: dấu + rồi 10 tới 12 chữ số (dạng quốc tế "+84 912 345 678",
/// "+1 415 555 0100", "+84 (0) 912 345 678"); 9 tới 11 chữ số bắt đầu bằng 0 (di động "0912 345 678", số bàn
/// "(028) 3823 4567"); hoặc 11 chữ số bắt đầu bằng 84 ("84912345678"). Ngày (8 chữ số) và mã số không có 0 hay 84 ở
/// đầu thì không khớp; mã đệm số 0 ("0012345678") khớp, chấp nhận vì đánh dấu nhầm chỉ mất danh sách giá trị. Số lưu
/// kiểu số nguyên mất số 0 đầu ("912345678") nên không khớp, trừ dạng 84 ở đầu.
fn looks_like_phone(value: &str) -> bool {
    // Số thập phân và số âm ("0.039999999", "-0.8") không phải số điện thoại: nếu bỏ dấu chấm như bên dưới thì
    // "0.039999999" thành mười chữ số bắt đầu bằng 0, và cột tỷ lệ kiểu DOUBLE bị đánh dấu nhạy cảm nhầm.
    let trimmed = value.trim();
    if trimmed.starts_with('-') || (trimmed.contains('.') && trimmed.parse::<f64>().is_ok()) {
        return false;
    }
    let d: String = value
        .chars()
        .filter(|c| !matches!(c, ' ' | '.' | '-' | '(' | ')'))
        .collect();
    let digits = d.chars().filter(|c| c.is_ascii_digit()).count();
    if let Some(rest) = d.strip_prefix('+') {
        return (10..=12).contains(&digits) && rest.chars().all(|c| c.is_ascii_digit());
    }
    if !(9..=11).contains(&digits) || !d.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    d.starts_with('0') || (digits == 11 && d.starts_with("84"))
}

/// Danh sách giá trị ghi được vào column_values: không giá trị nào dài quá VALUE_MAX_BYTES, không giá trị nào có ký tự
/// NUL. Postgres không lưu được NUL trong text; bỏ NUL đi thì "a\0" và "a" thành hai dòng trùng nhau, nên cột như vậy
/// không có danh sách, như cột có giá trị dài.
fn can_store_values(values: &[ValueCount]) -> bool {
    values
        .iter()
        .all(|v| v.value.len() <= VALUE_MAX_BYTES && !v.value.contains('\0'))
}

fn build_new_values(values: Vec<ValueCount>) -> Vec<NewColumnValue> {
    if !can_store_values(&values) {
        return Vec::new();
    }
    values
        .into_iter()
        .map(|v| NewColumnValue {
            value: v.value,
            frequency: v.frequency,
        })
        .collect()
}

/// min, max chỉ để xem, nên bỏ ký tự NUL (Postgres không lưu được) thay vì bỏ cả giá trị.
fn remove_nul(value: String) -> String {
    value.replace('\0', "")
}

/// Tên cột Postgres không lưu được thì báo người dùng sửa file, trước khi ghi gì.
fn check_column_names(stats: &[ColumnStats]) -> Result<(), AppError> {
    for s in stats {
        if s.name.contains('\0') {
            return Err(AppError::BadRequest(format!(
                "Tên cột \"{}\" có ký tự NUL (mã 0); sửa dòng tiêu đề rồi nạp lại",
                remove_nul(s.name.clone())
            )));
        }
        if s.name.len() > COLUMN_NAME_MAX_BYTES {
            let start: String = s.name.chars().take(40).collect();
            return Err(AppError::BadRequest(format!(
                "Tên cột bắt đầu bằng \"{start}\" dài quá {COLUMN_NAME_MAX_BYTES} byte; đổi tên cột cho ngắn hơn rồi nạp lại"
            )));
        }
    }
    Ok(())
}

/// Đo các cột trong thư mục Parquet của một phiên bản và chọn vai trò. DuckDB chạy trong spawn_blocking: đồng bộ và tốn
/// CPU, chạy thẳng trong hàm async sẽ chặn các request khác. previous_formats: xem previous_time_formats.
pub async fn build_profile(
    parquet_dir: PathBuf,
    row_count: i64,
    previous_formats: HashMap<String, String>,
    limits: DuckdbLimits,
) -> Result<Vec<NewColumn>, AppError> {
    // limits chuyển hẳn vào luồng của spawn_blocking (move), như parquet_dir.
    let stats = tokio::task::spawn_blocking(move || {
        profile::build_column_stats(&parquet_dir, DIMENSION_MAX, &previous_formats, &limits)
    })
    .await
    .map_err(|e| AppError::Internal(format!("profile task: {e}")))??;
    check_column_names(&stats)?;
    Ok(build_new_columns(stats, row_count))
}

/// Tên cột và time_format của phiên bản hiện hành của dataset, để phiên bản mới giữ cách đọc ngày đã chọn lần trước
/// (engine/profile.rs, choose_time_format). Rỗng khi dataset chưa có phiên bản hiện hành.
pub async fn previous_time_formats(
    state: &AppState,
    ws: &WorkspaceId,
    dataset_id: Uuid,
) -> Result<HashMap<String, String>, AppError> {
    let Some(current) = state.store.get_current_version(ws, dataset_id).await? else {
        return Ok(HashMap::new());
    };
    let columns = state.store.list_columns(ws, current.id).await?;
    Ok(columns
        .into_iter()
        .filter_map(|c| Some((c.name, c.time_format?)))
        .collect())
}

/// Lỗi khi lập hồ sơ cho một phiên bản cũ: lỗi do chính phiên bản (lần sau vẫn vậy) hay lỗi có thể tự hết.
enum Failure {
    Permanent(AppError),
    Temporary(AppError),
}

/// Lập hồ sơ cho các phiên bản nạp trước khi có bước hồ sơ (profile_status còn pending). main.rs lấy danh sách trước
/// khi nhận request rồi chạy hàm này nền. Từng phiên bản một: mỗi lần đo DuckDB đã dùng mọi nhân CPU, chạy song song
/// không nhanh hơn mà tốn thêm RAM. Lỗi một phiên bản chỉ ghi log, backend vẫn phục vụ bình thường: lỗi do dữ liệu
/// (Parquet hỏng, tên cột không lưu được) đặt failed để lần khởi động sau không thử lại mãi; lỗi có thể tự hết (thiếu
/// thư mục vì DATA_DIR hay volume chưa đúng, Postgres lỗi) giữ pending để lần khởi động sau thử lại.
pub async fn profile_pending_versions(
    state: AppState,
    pending: Vec<(WorkspaceId, PendingVersion)>,
) {
    if pending.is_empty() {
        return;
    }
    tracing::info!(
        count = pending.len(),
        "profiling dataset versions created before column profiles"
    );
    for (ws, version) in pending {
        match profile_version(&state, &ws, &version).await {
            Ok(()) => tracing::info!(version_id = %version.id, "dataset version profiled"),
            Err(Failure::Temporary(e)) => {
                tracing::warn!(version_id = %version.id, error = ?e, "cannot profile dataset version, will retry at next start")
            }
            Err(Failure::Permanent(e)) => {
                tracing::warn!(version_id = %version.id, error = ?e, "cannot profile dataset version, marking it failed");
                if let Err(e) = state
                    .store
                    .set_profile_status(&ws, version.id, ProfileStatus::Failed)
                    .await
                {
                    tracing::warn!(version_id = %version.id, error = ?e, "cannot mark dataset version failed");
                }
            }
        }
    }
}

/// Chạy lại luật đánh dấu nhạy cảm (is_sensitive_column) cho các phiên bản đã có hồ sơ từ trước khi luật nằm ở lớp
/// cấu trúc: chỉ bật cờ, không tắt (cờ do mô hình hay admin đặt thì giữ). Không có dấu "đã quét", nên mỗi lần khởi
/// động quét lại: rẻ (hai câu SELECT mỗi phiên bản) và lần sau thường không còn gì để ghi. main.rs gọi đồng bộ
/// trước khi server nhận request, để giá trị cột nhạy cảm của phiên bản cũ không lọt vào ngữ cảnh hỏi (#28) ngay từ
/// đầu phiên chạy. Lỗi một phiên bản chỉ ghi log, không chặn phiên bản sau.
pub async fn mark_sensitive_columns_at_start(state: &AppState) {
    let candidates = match crate::auth::versions_for_sensitive_scan(&state.store).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = ?e, "cannot list dataset versions for the sensitive-column scan");
            return;
        }
    };
    for (ws, version) in candidates {
        let result: Result<usize, AppError> = async {
            let columns = state.store.list_columns(&ws, version.id).await?;
            // Luật tên không cần giá trị, luật giá trị xét mọi cột có column_values, không chỉ dimension. Cột số
            // điện thoại lưu BIGINT đã mất số 0 đầu nên luật giá trị chỉ bắt dạng 84 ở đầu (looks_like_phone); cột số
            // nguyên thường là measure hay key, mà chỉ giá trị của cột dimension vào prompt.
            let open_ids: Vec<Uuid> = columns
                .iter()
                .filter(|c| !c.is_sensitive)
                .map(|c| c.id)
                .collect();
            let values = state.store.list_column_values(&ws, &open_ids).await?;
            let detected: Vec<Uuid> = columns
                .iter()
                .filter(|c| !c.is_sensitive)
                .filter(|c| {
                    let column_values: Vec<&str> = values
                        .iter()
                        .filter(|v| v.column_id == c.id)
                        .map(|v| v.value.as_str())
                        .collect();
                    is_sensitive_column(&c.name, &column_values)
                })
                .map(|c| c.id)
                .collect();
            if detected.is_empty() {
                return Ok(0);
            }
            state
                .store
                .mark_columns_sensitive(&ws, version.id, &detected)
                .await?;
            Ok(detected.len())
        }
        .await;
        match result {
            Ok(0) => {}
            Ok(n) => tracing::info!(version_id = %version.id, count = n,
                "marked sensitive columns of an earlier dataset version"),
            Err(e) => tracing::warn!(version_id = %version.id, error = ?e,
                "cannot scan a dataset version for sensitive columns"),
        }
    }
}

async fn profile_version(
    state: &AppState,
    ws: &WorkspaceId,
    version: &PendingVersion,
) -> Result<(), Failure> {
    // Phiên bản nạp từ file luôn có cả hai; nguồn database (sau này) thì không, và cần cách lập hồ sơ khác.
    let (Some(relative), Some(row_count)) = (&version.parquet_path, version.row_count) else {
        return Err(Failure::Permanent(AppError::Internal(
            "version has no parquet path or row count".to_string(),
        )));
    };
    let dir = state.config.data_dir.join(relative);
    if !dir.is_dir() {
        return Err(Failure::Temporary(AppError::Internal(format!(
            "parquet directory {} not found",
            dir.display()
        ))));
    }
    // Phiên bản cũ đều chưa có hồ sơ, nên không có định dạng ngày nào của phiên bản trước để giữ.
    let columns = build_profile(dir, row_count, HashMap::new(), state.config.duckdb_limits())
        .await
        .map_err(Failure::Permanent)?;
    // false: đã có hồ sơ từ lúc liệt kê tới giờ; không có gì phải làm.
    state
        .store
        .insert_profile(ws, version.id, &columns)
        .await
        .map_err(Failure::Temporary)?;
    Ok(())
}

pub(crate) fn is_time_type(data_type: &str) -> bool {
    // TIMESTAMP, TIMESTAMP WITH TIME ZONE, TIMESTAMP_S, TIMESTAMP_MS, TIMESTAMP_NS đều bắt đầu bằng TIMESTAMP.
    data_type == "DATE" || data_type.starts_with("TIMESTAMP")
}

pub(crate) fn is_integer_type(data_type: &str) -> bool {
    matches!(
        data_type,
        "TINYINT"
            | "SMALLINT"
            | "INTEGER"
            | "BIGINT"
            | "HUGEINT"
            | "UTINYINT"
            | "USMALLINT"
            | "UINTEGER"
            | "UBIGINT"
            | "UHUGEINT"
    )
}

pub(crate) fn is_number_type(data_type: &str) -> bool {
    // DuckDB ghi số thập phân cố định kèm độ chính xác, ví dụ DECIMAL(18,3).
    is_integer_type(data_type)
        || matches!(data_type, "FLOAT" | "DOUBLE")
        || data_type.starts_with("DECIMAL")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(name: &str, data_type: &str, null_count: i64, distinct_count: i64) -> ColumnStats {
        ColumnStats {
            name: name.to_string(),
            data_type: data_type.to_string(),
            null_count,
            distinct_count,
            min_value: None,
            max_value: None,
            time_format: None,
            values: Vec::new(),
        }
    }

    #[test]
    fn roles_follow_the_rules_in_order() {
        let rows = 1000;
        let role = |s: ColumnStats| build_column_role(&s, rows);

        assert_eq!(role(stats("note", "VARCHAR", rows, 0)), ProfileRole::Ignore);
        assert_eq!(
            role(stats(SOURCE_COLUMN, "VARCHAR", 0, 1)),
            ProfileRole::Ignore
        );

        assert_eq!(role(stats("day", "DATE", 0, 300)), ProfileRole::Time);
        assert_eq!(
            role(stats("at", "TIMESTAMP WITH TIME ZONE", 0, 900)),
            ProfileRole::Time
        );
        let mut text_date = stats("order date (DateOrders)", "VARCHAR", 0, 900);
        text_date.time_format = Some("%m/%d/%Y %H:%M".to_string());
        assert_eq!(role(text_date), ProfileRole::Time);

        assert_eq!(
            role(stats("Order Item Id", "BIGINT", 0, rows)),
            ProfileRole::Key
        );
        // Có ô trống, hoặc lặp lại: không phải khóa. Số nguyên lặp lại như Order Id thành measure.
        assert_eq!(
            role(stats("id", "BIGINT", 1, rows - 1)),
            ProfileRole::Measure
        );
        assert_eq!(
            role(stats("Order Id", "BIGINT", 0, 400)),
            ProfileRole::Measure
        );
        // Số thực mỗi dòng một giá trị vẫn là độ đo.
        assert_eq!(
            role(stats("Sales", "DOUBLE", 0, rows)),
            ProfileRole::Measure
        );
        assert_eq!(
            role(stats("price", "DECIMAL(18,3)", 0, 50)),
            ProfileRole::Measure
        );

        assert_eq!(
            role(stats("Late_delivery_risk", "BIGINT", 0, 2)),
            ProfileRole::Dimension
        );
        assert_eq!(role(stats("flag", "DOUBLE", 0, 1)), ProfileRole::Dimension);

        assert_eq!(
            role(stats("is_late", "BOOLEAN", 0, 2)),
            ProfileRole::Dimension
        );
        assert_eq!(
            role(stats("Market", "VARCHAR", 0, 3)),
            ProfileRole::Dimension
        );
        assert_eq!(
            role(stats("Order Country", "VARCHAR", 0, DIMENSION_MAX)),
            ProfileRole::Dimension
        );
        assert_eq!(
            role(stats("Order City", "VARCHAR", 0, DIMENSION_MAX + 1)),
            ProfileRole::Text
        );
        assert_eq!(
            role(stats("payload", "STRUCT(a BIGINT)", 0, 3)),
            ProfileRole::Text
        );
    }

    #[test]
    fn new_columns_keep_order_and_values() {
        let mut market = stats("Market", "VARCHAR", 0, 2);
        market.values = vec![
            ValueCount {
                value: "Europe".to_string(),
                frequency: 7,
            },
            ValueCount {
                value: "LATAM".to_string(),
                frequency: 3,
            },
        ];
        let mut long = stats("Description", "VARCHAR", 0, 2);
        long.values = vec![
            ValueCount {
                value: "short".to_string(),
                frequency: 1,
            },
            ValueCount {
                value: "x".repeat(VALUE_MAX_BYTES + 1),
                frequency: 1,
            },
        ];
        let mut nul = stats("Code", "VARCHAR", 0, 2);
        nul.values = vec![
            ValueCount {
                value: "a\0b".to_string(),
                frequency: 1,
            },
            ValueCount {
                value: "ab".to_string(),
                frequency: 1,
            },
        ];
        nul.min_value = Some("a\0b".to_string());
        let columns =
            build_new_columns(vec![market, stats("Sales", "DOUBLE", 0, 10), long, nul], 10);
        assert_eq!(columns[0].position, 0);
        assert_eq!(columns[0].role, ProfileRole::Dimension);
        assert_eq!(columns[0].values[1].value, "LATAM");
        assert_eq!(columns[0].values[1].frequency, 3);
        assert_eq!(
            (columns[1].position, columns[1].role),
            (1, ProfileRole::Measure)
        );
        // Một giá trị quá dài: cả cột không có danh sách giá trị, nên là text dù ít giá trị.
        assert!(columns[2].values.is_empty());
        assert_eq!(columns[2].role, ProfileRole::Text);
        // Giá trị có NUL: như trên; min, max bỏ ký tự NUL.
        assert!(columns[3].values.is_empty());
        assert_eq!(columns[3].role, ProfileRole::Text);
        assert_eq!(columns[3].min_value.as_deref(), Some("ab"));
    }

    /// Bảng một hai dòng: số nào cũng mỗi dòng một giá trị, tối đa 2 giá trị; không thành key hay cờ mà là measure.
    #[test]
    fn numbers_in_tiny_tables_are_measures() {
        for rows in [1, 2] {
            assert_eq!(
                build_column_role(&stats("id", "BIGINT", 0, rows), rows),
                ProfileRole::Measure
            );
            assert_eq!(
                build_column_role(&stats("amount", "DOUBLE", 0, rows), rows),
                ProfileRole::Measure
            );
            assert_eq!(
                build_column_role(&stats("region", "VARCHAR", 0, rows), rows),
                ProfileRole::Dimension
            );
        }
        assert_eq!(
            build_column_role(&stats("id", "BIGINT", 0, 3), 3),
            ProfileRole::Key
        );
        assert_eq!(
            build_column_role(&stats("flag", "BIGINT", 0, 2), 3),
            ProfileRole::Dimension
        );
    }

    #[test]
    fn column_names_postgres_cannot_store_are_user_errors() {
        let long = "x".repeat(COLUMN_NAME_MAX_BYTES + 1);
        let err =
            check_column_names(&[stats("ok", "VARCHAR", 0, 1), stats(&long, "VARCHAR", 0, 1)])
                .unwrap_err();
        assert!(
            matches!(&err, AppError::BadRequest(m) if m.contains("1000 byte")),
            "{err:?}"
        );
        assert!(matches!(
            check_column_names(&[stats("a\0b", "VARCHAR", 0, 1)]),
            Err(AppError::BadRequest(_))
        ));
        let at_limit = "x".repeat(COLUMN_NAME_MAX_BYTES);
        assert!(check_column_names(&[stats(&at_limit, "VARCHAR", 0, 1)]).is_ok());
    }

    /// Việc nền lúc khởi động: phiên bản cũ có Parquet thì được lập hồ sơ; phiên bản mất thư mục giữ pending (lần sau
    /// thử lại), phiên bản có Parquet hỏng thành failed; cả hai không chặn phiên bản sau.
    #[tokio::test]
    async fn pending_versions_are_profiled_at_start() {
        use crate::engine::analytics::{materialize, FileFormat, InputFile};
        use crate::engine::store::test_support::{drop_workspace, store, temp_workspace};
        use crate::engine::store::{NewVersion, SourceKind};

        let Some(store) = store().await else {
            eprintln!("skip: DATABASE_URL not set");
            return;
        };
        let ws = temp_workspace(&store).await;
        let data = std::env::temp_dir().join(format!("xdi-profile-start-{}", uuid::Uuid::new_v4()));
        let relative = "ws/ds/v1";
        std::fs::create_dir_all(data.join(relative)).unwrap();
        let csv = data.join("in.csv");
        std::fs::write(
            &csv,
            "Market,Sales,Order Item Id\nEurope,1.5,1\nLATAM,2.5,2\nEurope,3.0,3\n",
        )
        .unwrap();
        let input = InputFile {
            path: csv,
            name: "in.csv".to_string(),
            format: FileFormat::Csv,
            column_types: Vec::new(),
        };
        let materialized = materialize(
            &[input],
            &data.join(relative),
            &crate::engine::duckdb_limits::test_limits(),
        )
        .unwrap();

        let src = store.create_local_source(&ws, "Test").await.unwrap();
        let ds = store
            .create_dataset(&ws, src, "Sales", "sales", None)
            .await
            .unwrap();
        let version = |hash, path| NewVersion {
            source_kind: SourceKind::Csv,
            file_hash: hash,
            parquet_path: path,
            row_count: materialized.row_count,
            description: None,
        };
        let missing = store
            .create_version(&ws, ds.id, &version("h0", "ws/ds/gone"), &[])
            .await
            .unwrap();
        let broken_dir = "ws/ds/broken";
        std::fs::create_dir_all(data.join(broken_dir)).unwrap();
        std::fs::write(
            data.join(broken_dir).join("part-0001.parquet"),
            b"not parquet",
        )
        .unwrap();
        let broken = store
            .create_version(&ws, ds.id, &version("h2", broken_dir), &[])
            .await
            .unwrap();
        let good = store
            .create_version(&ws, ds.id, &version("h1", relative), &[])
            .await
            .unwrap();

        // Test khác chạy song song cũng có phiên bản pending; chỉ lấy của workspace này.
        let pending: Vec<_> = crate::auth::pending_profiles(&store)
            .await
            .unwrap()
            .into_iter()
            .filter(|(w, _)| *w == ws)
            .collect();
        assert_eq!(pending.len(), 3);
        let mut config = crate::config::Config::from_env();
        config.data_dir = data.clone();
        let state = AppState::new(config, store.clone());
        profile_pending_versions(state, pending).await;

        let columns = store.list_columns(&ws, good.id).await.unwrap();
        let roles: Vec<(&str, &str)> = columns
            .iter()
            .map(|c| (c.name.as_str(), c.role.as_str()))
            .collect();
        assert_eq!(
            roles,
            [
                ("Market", "dimension"),
                ("Sales", "measure"),
                ("Order Item Id", "key"),
                ("_source_file", "ignore")
            ]
        );
        let status = |hash| {
            let store = store.clone();
            async move {
                store
                    .find_version_by_hash(&ws, ds.id, hash)
                    .await
                    .unwrap()
                    .unwrap()
                    .profile_status
            }
        };
        assert_eq!(status("h1").await, "structure");
        assert_eq!(status("h0").await, "pending");
        assert_eq!(status("h2").await, "failed");
        assert!(store.list_columns(&ws, broken.id).await.unwrap().is_empty());
        assert!(store
            .list_columns(&ws, missing.id)
            .await
            .unwrap()
            .is_empty());

        drop_workspace(&store, ws).await;
        std::fs::remove_dir_all(&data).unwrap();
    }
}
