// SPDX-License-Identifier: Apache-2.0
/* service/ingest.rs - nạp dữ liệu: đổi file người dùng gửi lên thành một phiên bản dataset. Lập hồ sơ cột (#22) và
nguồn database (sau này) là bước riêng. Xem docs/ARCHITECTURE.md mục 5 và 7.

Các bước, sắp để lỗi ở bước đầu không để lại gì:
1. Tầng api đã ghi file gửi lên vào thư mục tạm (Staging) và tính SHA-256 từng file.
2. Bộ file trùng một phiên bản đã có thì trả phiên bản đó, không làm gì thêm.
3. DuckDB đổi file sang Parquet trong thư mục tạm (spawn_blocking: DuckDB đồng bộ và tốn CPU).
4. Ghi dataset (nếu mới) và phiên bản vào Postgres.
5. Chuyển thư mục Parquet về chỗ cuối <DATA_DIR>/<workspace>/<dataset>/<thư mục phiên bản>, rồi mới đặt phiên bản
   hiện hành, để phiên bản hiện hành luôn có file.
Thư mục tạm xóa khi Staging ra khỏi phạm vi, kể cả khi lỗi; backend chết giữa chừng thì lần khởi động sau xóa
(clear_staging). Bước 4 và 5 chưa nằm trong một giao dịch: lỗi giữa hai bước có thể để lại dòng thừa trong Postgres
(#79).

Thao tác file (tạo, đổi tên, xóa thư mục) dùng std::fs đồng bộ: đều là thao tác ngắn trên metadata; riêng xóa thư mục
tạm có thể chạm file tới MAX_UPLOAD_MB, chấp nhận ở quy mô một bản cài. */

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::Member;
use crate::engine::analytics::{self, Column, FileFormat, InputFile};
use crate::engine::store::{Dataset, DatasetVersion, NewVersion, SourceKind};
use crate::{error::AppError, state::AppState};

/// Thư mục tạm cho một lần nạp, dưới <DATA_DIR>/staging/. Tự xóa khi ra khỏi phạm vi (Drop), nên lỗi giữa chừng
/// không để lại file. Cùng ổ đĩa với thư mục dữ liệu, nên bước cuối chỉ cần đổi tên thư mục, không chép.
pub struct Staging {
    dir: PathBuf,
}

impl Staging {
    pub fn new(data_dir: &Path) -> Result<Self, AppError> {
        let dir = data_dir.join("staging").join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(dir.join("upload"))
            .and_then(|_| std::fs::create_dir_all(dir.join("parquet")))
            .map_err(|e| {
                AppError::Internal(format!("create staging dir {}: {e}", dir.display()))
            })?;
        Ok(Self { dir })
    }

    /// Đường dẫn để tầng api ghi file thứ n gửi lên. Tên do máy chủ đặt, không dùng tên người dùng gửi.
    pub fn upload_path(&self, n: usize) -> PathBuf {
        self.dir.join("upload").join(format!("{n:04}"))
    }

    fn parquet_dir(&self) -> PathBuf {
        self.dir.join("parquet")
    }
}

/// Xóa thư mục tạm còn sót từ lần chạy trước (backend chết giữa một lần nạp). Gọi lúc khởi động, khi chưa có lần
/// nạp nào đang chạy; giả định mỗi DATA_DIR chỉ một backend dùng.
pub fn clear_staging(data_dir: &Path) {
    let dir = data_dir.join("staging");
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => tracing::info!(dir = %dir.display(), "cleared leftover upload staging"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(dir = %dir.display(), error = %e, "cannot clear upload staging"),
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Một file đã ghi vào Staging.
pub struct StagedFile {
    pub path: PathBuf,
    /// Tên người dùng gửi, dùng để đoán định dạng, ghi vào cột _source_file và thông báo lỗi.
    pub name: String,
    /// SHA-256 dạng hex.
    pub sha256: String,
}

pub struct Upload {
    /// Tên dataset mới; bỏ trống thì lấy tên file đầu tiên.
    pub name: Option<String>,
    /// Có thì thêm phiên bản cho dataset này, không thì tạo dataset mới.
    pub dataset_id: Option<String>,
    pub files: Vec<StagedFile>,
}

#[derive(Serialize)]
pub struct UploadResult {
    /// false: bộ file trùng một phiên bản đã có, không tạo gì mới.
    pub created: bool,
    pub dataset: Dataset,
    pub version: DatasetVersion,
    /// Cột của bảng vừa tạo; rỗng khi created = false (hồ sơ cột đầy đủ ở #22).
    pub columns: Vec<Column>,
}

pub async fn list_datasets(state: &AppState, member: &Member) -> Result<Vec<Dataset>, AppError> {
    state.store.list_datasets(&member.workspace).await
}

/// Dataset của chính workspace này. Id của workspace khác trả 404 như id không tồn tại: store lọc theo WorkspaceId.
pub async fn dataset_detail(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<Dataset, AppError> {
    let id = parse_dataset_id(id)?;
    state
        .store
        .get_dataset(&member.workspace, id)
        .await?
        .ok_or_else(dataset_not_found)
}

pub async fn upload(
    state: &AppState,
    member: &Member,
    staging: Staging,
    upload: Upload,
) -> Result<UploadResult, AppError> {
    let ws = &member.workspace;
    let format = common_format(&upload.files)?;
    reject_repeated_files(&upload.files)?;
    let content_hash = content_hash(&upload.files);

    // Bước 2: chống trùng. Có dataset_id thì chỉ so trong dataset đó; không thì so cả workspace.
    let existing_dataset = match &upload.dataset_id {
        Some(id) => Some(
            state
                .store
                .get_dataset(ws, parse_dataset_id(id)?)
                .await?
                .ok_or_else(dataset_not_found)?,
        ),
        None => None,
    };
    let duplicate = match &existing_dataset {
        Some(d) => {
            state
                .store
                .find_version_by_hash(ws, d.id, &content_hash)
                .await?
        }
        None => {
            state
                .store
                .find_version_by_hash_in_workspace(ws, &content_hash)
                .await?
        }
    };
    if let Some(version) = duplicate {
        let dataset = state
            .store
            .get_dataset(ws, version.dataset_id)
            .await?
            .ok_or_else(dataset_not_found)?;
        return Ok(UploadResult {
            created: false,
            dataset,
            version,
            columns: Vec::new(),
        });
    }

    // Bước 3: Parquet trong thư mục tạm.
    let inputs: Vec<InputFile> = upload
        .files
        .iter()
        .map(|f| InputFile {
            path: f.path.clone(),
            name: f.name.clone(),
            format,
        })
        .collect();
    let parquet_dir = staging.parquet_dir();
    let materialized =
        tokio::task::spawn_blocking(move || analytics::materialize(&inputs, &parquet_dir))
            .await
            .map_err(|e| AppError::Internal(format!("materialize task: {e}")))??;

    // Bước 4: dataset (nếu mới) và phiên bản. Đường dẫn lưu trong database là tương đối với DATA_DIR, để đổi chỗ thư
    // mục dữ liệu không phải sửa database.
    let dataset = match existing_dataset {
        Some(d) => d,
        None => {
            let name = dataset_name(&upload)?;
            let logical_table = free_logical_table(state, member, &name).await?;
            let source = state.store.create_local_source(ws).await?;
            state
                .store
                .create_dataset(ws, source, &name, &logical_table, Some(member.user.id))
                .await?
        }
    };
    let relative = format!("{}/{}/{}", ws.as_uuid(), dataset.id, Uuid::new_v4());
    let new_version = NewVersion {
        source_kind: source_kind(format),
        file_hash: &content_hash,
        parquet_path: &relative,
        row_count: materialized.row_count,
    };
    let mut attempts = 0;
    let version = loop {
        match state
            .store
            .create_version(ws, dataset.id, &new_version)
            .await
        {
            Ok(version) => break version,
            Err(e) => {
                // Cùng bộ file nạp song song vào cùng dataset: bên ghi sau vấp UNIQUE (dataset_id, file_hash). Có phiên
                // bản cùng nội dung rồi thì trả nó, như nạp lại tuần tự.
                if let Some(version) = state
                    .store
                    .find_version_by_hash(ws, dataset.id, &content_hash)
                    .await?
                {
                    return Ok(UploadResult {
                        created: false,
                        dataset,
                        version,
                        columns: Vec::new(),
                    });
                }
                // Khác nội dung nạp song song: hai lần cùng tính ra một số phiên bản (MAX + 1) và một bên vấp UNIQUE
                // (dataset_id, version). Thử lại thì số kế tiếp đã khác. Lỗi khác cũng chỉ thử lại vài lần rồi báo.
                attempts += 1;
                if attempts >= 5 {
                    return Err(e);
                }
            }
        }
    };

    // Bước 5: đưa Parquet về chỗ cuối rồi mới đặt phiên bản hiện hành, để phiên bản hiện hành luôn có file.
    let final_dir = state.config.data_dir.join(&relative);
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create {}: {e}", parent.display())))?;
    }
    std::fs::rename(staging.parquet_dir(), &final_dir)
        .map_err(|e| AppError::Internal(format!("move parquet to {}: {e}", final_dir.display())))?;
    // Workspace hoặc dataset bị xóa đúng lúc đang nạp: bước trên đã tạo lại thư mục sau khi lệnh xóa dọn xong. Không
    // dọn ở đây thì file nằm lại trên đĩa mà không còn dòng nào trong database trỏ tới.
    if let Err(e) = state
        .store
        .set_current_version(ws, dataset.id, version.id)
        .await
    {
        remove_version_files(&state.config.data_dir, &relative);
        return Err(e);
    }
    let dataset = state
        .store
        .get_dataset(ws, dataset.id)
        .await?
        .ok_or_else(dataset_not_found)?;
    tracing::info!(dataset_id = %dataset.id, version = version.version, rows = materialized.row_count, "dataset version created");

    Ok(UploadResult {
        created: true,
        dataset,
        version,
        columns: materialized.columns,
    })
}

/// Xóa thư mục Parquet của một phiên bản, rồi các thư mục cha (dataset, workspace) nếu chúng đã trống. remove_dir chỉ
/// xóa thư mục trống, nên file của phiên bản khác không bị đụng tới.
fn remove_version_files(data_dir: &Path, relative: &str) {
    let mut dir = data_dir.join(relative);
    if let Err(e) = std::fs::remove_dir_all(&dir) {
        tracing::warn!(dir = %dir.display(), error = %e, "could not remove files of a failed upload");
        return;
    }
    // Đi ngược lên: thư mục dataset, rồi workspace. Dừng ở thư mục đầu tiên còn file, hoặc khi tới DATA_DIR.
    while dir.pop() && dir != data_dir {
        if std::fs::remove_dir(&dir).is_err() {
            break;
        }
    }
}

/// Cùng một file chọn hai lần (cùng nội dung) làm số dòng nhân đôi; hai file khác thư mục mà trùng tên làm cột
/// _source_file không phân biệt được. Cả hai trường hợp đều báo để người dùng chọn lại.
fn reject_repeated_files(files: &[StagedFile]) -> Result<(), AppError> {
    for (i, f) in files.iter().enumerate() {
        for earlier in &files[..i] {
            if earlier.sha256 == f.sha256 {
                return Err(AppError::BadRequest(format!(
                    "File {} và {} có nội dung giống hệt nhau; mỗi file chỉ chọn một lần",
                    earlier.name, f.name
                )));
            }
            if earlier.name == f.name {
                return Err(AppError::BadRequest(format!(
                    "Có hai file cùng tên {}; đổi tên một file để phân biệt dòng đến từ file nào",
                    f.name
                )));
            }
        }
    }
    Ok(())
}

/// Mọi file một lần nạp cùng định dạng: phiên bản ghi một source_kind, và gộp CSV với Parquet hiếm khi là ý người dùng.
fn common_format(files: &[StagedFile]) -> Result<FileFormat, AppError> {
    let Some(first) = files.first() else {
        return Err(AppError::BadRequest("Chưa chọn file nào".to_string()));
    };
    let format_of = |f: &StagedFile| {
        FileFormat::from_file_name(&f.name).ok_or_else(|| {
            AppError::BadRequest(format!(
                "File {} không phải CSV hay Parquet (đuôi .csv hoặc .parquet)",
                f.name
            ))
        })
    };
    let format = format_of(first)?;
    for f in &files[1..] {
        if format_of(f)? != format {
            return Err(AppError::BadRequest(
                "Các file nạp cùng lúc phải cùng định dạng: toàn CSV hoặc toàn Parquet".to_string(),
            ));
        }
    }
    Ok(format)
}

/// Băm của cả bộ file: băm danh sách băm từng file đã sắp xếp, nên thứ tự chọn file không làm đổi kết quả.
fn content_hash(files: &[StagedFile]) -> String {
    let mut hashes: Vec<&str> = files.iter().map(|f| f.sha256.as_str()).collect();
    hashes.sort_unstable();
    Sha256::digest(hashes.join("\n").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn dataset_name(upload: &Upload) -> Result<String, AppError> {
    let from_field = upload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    match from_field {
        Some(n) if n.chars().count() > 100 => Err(AppError::BadRequest(
            "Tên dataset dài tối đa 100 ký tự".to_string(),
        )),
        Some(n) => Ok(n.to_string()),
        // Người dùng không đặt tên: lấy tên file bỏ đuôi ("ban_hang_thang1.csv" thành "ban_hang_thang1"), cắt còn 100
        // ký tự thay vì báo lỗi về cái tên họ không hề chọn.
        None => {
            let first = &upload.files[0].name;
            let stem = first
                .rsplit_once('.')
                .map_or(first.as_str(), |(stem, _)| stem)
                .trim();
            // Tên file chỉ có đuôi (".csv"): database không nhận tên rỗng.
            if stem.is_empty() {
                return Ok("Dataset".to_string());
            }
            Ok(stem.chars().take(100).collect())
        }
    }
}

/// Tên bảng mô hình ngôn ngữ dùng trong SQL: "Bán hàng 2024" thành "ban_hang_2024", trùng thì thêm _2, _3.
async fn free_logical_table(
    state: &AppState,
    member: &Member,
    name: &str,
) -> Result<String, AppError> {
    let base = logical_table_from_name(name);
    let mut candidate = base.clone();
    let mut n = 2;
    while state
        .store
        .logical_table_taken(&member.workspace, &candidate)
        .await?
    {
        candidate = format!("{base}_{n}");
        n += 1;
    }
    Ok(candidate)
}

/// Định danh SQL hợp lệ ([a-z_][a-z0-9_]*), khớp ràng buộc của cột datasets.logical_table.
fn logical_table_from_name(name: &str) -> String {
    let ascii = deunicode::deunicode(name).to_lowercase();
    let joined = ascii
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let short: String = joined.chars().take(48).collect();
    let short = short.trim_end_matches('_');
    match short.chars().next() {
        None => "dataset".to_string(),
        Some(c) if c.is_ascii_digit() => format!("t_{short}"),
        Some(_) => short.to_string(),
    }
}

fn source_kind(format: FileFormat) -> SourceKind {
    match format {
        FileFormat::Csv => SourceKind::Csv,
        FileFormat::Parquet => SourceKind::Parquet,
    }
}

fn parse_dataset_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| dataset_not_found())
}

fn dataset_not_found() -> AppError {
    AppError::NotFound("Không có dataset này".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dọn phiên bản lỗi: xóa thư mục cha khi chúng trống, giữ thư mục còn phiên bản khác và chính DATA_DIR.
    #[test]
    fn remove_version_files_keeps_other_versions() {
        let data = std::env::temp_dir().join(format!("xdi-ingest-{}", Uuid::new_v4()));
        for version in ["ws/ds/v1", "ws/ds/v2"] {
            std::fs::create_dir_all(data.join(version)).unwrap();
            std::fs::write(data.join(version).join("part.parquet"), b"x").unwrap();
        }
        remove_version_files(&data, "ws/ds/v1");
        assert!(!data.join("ws/ds/v1").exists());
        assert!(data.join("ws/ds/v2/part.parquet").exists());
        remove_version_files(&data, "ws/ds/v2");
        assert!(!data.join("ws").exists());
        assert!(data.is_dir());
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn logical_table_names() {
        assert_eq!(logical_table_from_name("Bán hàng 2024"), "ban_hang_2024");
        assert_eq!(logical_table_from_name("2024 sales"), "t_2024_sales");
        assert_eq!(logical_table_from_name("???"), "dataset");
    }

    #[test]
    fn content_hash_ignores_file_order() {
        let file = |h: &str| StagedFile {
            path: PathBuf::new(),
            name: "x.csv".to_string(),
            sha256: h.to_string(),
        };
        assert_eq!(
            content_hash(&[file("a"), file("b")]),
            content_hash(&[file("b"), file("a")])
        );
        assert_ne!(
            content_hash(&[file("a")]),
            content_hash(&[file("a"), file("b")])
        );
    }
}
