// SPDX-License-Identifier: Apache-2.0
/* service/ingest.rs - nạp dữ liệu: đổi file người dùng gửi lên thành một phiên bản dataset, kèm hồ sơ cột lớp cấu trúc
(service/profile.rs). Bảng của nguồn database đi cùng đường này: service/sources/ chép bảng ra một file (CSV của COPY
hay Parquet của DuckDB) rồi gọi upload với database_source và format. Xem docs/ARCHITECTURE.md mục 5 và 7. Phần riêng của file
Excel (chọn sheet, đổi sheet sang CSV) ở service/excel.rs.

Các bước, sắp để lỗi ở bước đầu không để lại gì:
1. Tầng api đã ghi file gửi lên vào thư mục tạm (Staging) và tính SHA-256 từng file.
2. Bộ file trùng một phiên bản đã có thì trả phiên bản đó, không làm gì thêm (không lập lại hồ sơ).
3. DuckDB đổi file sang Parquet trong thư mục tạm, rồi đo từng cột để lập hồ sơ (spawn_blocking: DuckDB đồng bộ và tốn
   CPU).
4. Ghi dataset (nếu mới), rồi phiên bản cùng hồ sơ cột trong một giao dịch (profile_status là structure ngay). Giao dịch
   lỗi thì xóa dataset vừa tạo, nên nạp lại cùng bộ file không vấp phiên bản trùng hash mà thiếu hồ sơ.
5. Chuyển thư mục Parquet về chỗ cuối <DATA_DIR>/<workspace>/<dataset>/<thư mục phiên bản>, rồi mới đặt phiên bản
   hiện hành, để phiên bản hiện hành luôn có file và có hồ sơ.
6. Trả 201 rồi lập hồ sơ lớp ngữ nghĩa nền (service/semantic/mod.rs): mô tả, bí danh, câu hỏi gợi ý.
Thư mục tạm xóa khi Staging ra khỏi phạm vi, kể cả khi lỗi; backend chết giữa chừng thì lần khởi động sau xóa
(clear_staging). Bước 4 và 5 chưa nằm trong một giao dịch: lỗi giữa hai bước có thể để lại dòng thừa trong Postgres
(#79).

Thao tác file (tạo, đổi tên, xóa thư mục) dùng std::fs đồng bộ: đều là thao tác ngắn trên metadata; riêng xóa thư mục
tạm có thể chạm file tới MAX_UPLOAD_MB, chấp nhận ở quy mô một bản cài. Lúc nạp, thư mục tạm chứa file gửi lên, Parquet
đang ghi, và với CSV không phải UTF-8 thêm bản UTF-8 của mọi file cần đổi mã cùng lúc (mỗi bản lớn tới ba lần file
gốc; bản của file nào xóa ngay khi file đó ghi xong Parquet; xem engine/csv_encoding.rs). */

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{Member, WorkspaceId};
use crate::engine::analytics::{self, Column, FileFormat, InputFile};
use crate::engine::store::{
    build_related, Dataset, DatasetVersion, NewColumn, NewVersion, ProfileStatus, SourceKind,
};
use crate::service::{excel, profile, semantic};
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
            .and_then(|_| std::fs::create_dir_all(dir.join("sheets")))
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

    /// Chỗ ghi CSV đổi từ các sheet Excel.
    fn sheets_dir(&self) -> PathBuf {
        self.dir.join("sheets")
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
    /// Kiểu DuckDB của từng cột, dạng (tên cột, kiểu), khi file là bản chụp bảng database (service/sources/). Rỗng với
    /// file người dùng nạp: kiểu do DuckDB đoán.
    pub column_types: Vec<(String, String)>,
}

pub struct Upload {
    /// Tên dataset mới; bỏ trống thì lấy tên file đầu tiên.
    pub name: Option<String>,
    /// Có thì thêm phiên bản cho dataset này, không thì tạo dataset mới.
    pub dataset_id: Option<String>,
    pub files: Vec<StagedFile>,
    /// Sheet cần nạp khi file là Excel; rỗng thì file phải chỉ có một sheet.
    pub sheets: Vec<String>,
    /// Có thì files là một file chép từ bảng của nguồn database này (service/sources/): dataset mới gắn vào nguồn
    /// đó thay vì một nguồn file riêng, phiên bản ghi source_kind postgres.
    pub database_source: Option<Uuid>,
    /// Định dạng backend đã biết chắc (bản chụp database: DatabaseCsv hay DatabaseParquet). None: đoán theo đuôi tên file
    /// người dùng gửi.
    pub format: Option<FileFormat>,
    /// Mô tả bảng và cột đọc từ database nguồn (COMMENT ON); rỗng với file nạp tay.
    pub descriptions: SourceDescriptions,
}

/// Mô tả tổ chức đã ghi sẵn trong database nguồn (COMMENT ON TABLE, COMMENT ON COLUMN), đã làm sạch ở
/// service/sources/comments.rs. Mô tả cột thành mô tả trong hồ sơ cấu trúc với nguồn database; mô tả bảng ghi trên
/// phiên bản.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceDescriptions {
    pub table: Option<String>,
    /// (tên cột, mô tả), chỉ cột có comment.
    pub columns: Vec<(String, String)>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct UploadResult {
    // false: bộ file trùng một phiên bản đã có, không tạo gì mới.
    /// False when the files match an existing version: nothing new was created.
    pub created: bool,
    pub dataset: Dataset,
    pub version: DatasetVersion,
    // Cột của bảng vừa tạo; rỗng khi created = false. Hồ sơ cột đầy đủ (vai trò, số đo) nằm ở bảng columns, đọc qua route
    // tổng quan (service/overview/mod.rs).
    /// Columns of the new table; empty when `created` is false.
    pub columns: Vec<Column>,
}

pub async fn list_datasets(state: &AppState, member: &Member) -> Result<Vec<Dataset>, AppError> {
    let mut datasets = state.store.list_datasets(&member.workspace).await?;
    // Một câu cho cả workspace, rồi chia theo dataset: không hỏi Postgres một lần cho mỗi dataset.
    let relations = state.store.list_relations(&member.workspace, None).await?;
    for dataset in &mut datasets {
        apply_semantic_state(state, dataset);
        dataset.relations = build_related(dataset.id, &relations);
    }
    Ok(datasets)
}

/// Mọi dataset trả qua API đi qua đây: store không biết provider đang chạy, nên semantic_unavailable chỉ đúng sau
/// bước này. Thiếu bước này ở một đường trả thì trang hỏi lại hiện "đang lập" mãi với dataset mock không có hồ sơ.
fn apply_semantic_state(state: &AppState, dataset: &mut Dataset) {
    dataset.apply_semantic_unavailable(&state.llm.name());
}

/// Dataset của chính workspace này. Id của workspace khác trả 404 như id không tồn tại: store lọc theo WorkspaceId.
pub async fn dataset_detail(
    state: &AppState,
    member: &Member,
    id: &str,
) -> Result<Dataset, AppError> {
    let id = parse_dataset_id(id)?;
    let mut dataset = state
        .store
        .get_dataset(&member.workspace, id)
        .await?
        .ok_or_else(dataset_not_found)?;
    apply_semantic_state(state, &mut dataset);
    let relations = state
        .store
        .list_relations(&member.workspace, Some(id))
        .await?;
    dataset.relations = build_related(id, &relations);
    Ok(dataset)
}

/// Xóa hẳn một dataset (chỉ admin): dòng database, gồm cả lịch sử câu hỏi, rồi file Parquet của mọi phiên bản. File xóa sau commit; lỡ xóa
/// file lỗi thì chỉ còn file mồ côi trên đĩa (dọn được bằng tay), không phải dữ liệu lệch giữa hai nơi.
pub async fn delete_dataset(state: &AppState, member: &Member, id: &str) -> Result<(), AppError> {
    member.require_admin()?;
    let id = parse_dataset_id(id)?;
    // get_dataset trước để id không có hoặc thuộc workspace khác nhận 404, chứ không xóa lặng lẽ như không có gì.
    state
        .store
        .get_dataset(&member.workspace, id)
        .await?
        .ok_or_else(dataset_not_found)?;
    let paths = state.store.delete_dataset(&member.workspace, id).await?;
    for path in paths {
        remove_version_files(&state.config.data_dir, &path);
    }
    Ok(())
}

pub async fn upload(
    state: &AppState,
    member: &Member,
    staging: Staging,
    upload: Upload,
) -> Result<UploadResult, AppError> {
    let ws = &member.workspace;
    // Bản chụp bảng database có tên postgres://..., không có đuôi file để đoán định dạng; service/sources/ ghi rõ định
    // dạng (CSV của COPY hay Parquet của DuckDB) trong upload.format.
    let format = match upload.format {
        Some(format) => format,
        None => common_format(&upload.files)?,
    };
    reject_repeated_files(&upload.files)?;
    // Chọn sheet trước khi tính hash: cùng một file Excel mà khác sheet là hai bộ dữ liệu khác nhau.
    let sheets = if format == FileFormat::Xlsx {
        excel::choose_sheets(state, &upload).await?
    } else if !upload.sheets.is_empty() {
        return Err(AppError::BadRequest(
            "Trường sheet chỉ dùng khi nạp file Excel".to_string(),
        ));
    } else {
        Vec::new()
    };
    let content_hash = content_hash(&upload.files, &sheets);

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
        let mut dataset = state
            .store
            .get_dataset(ws, version.dataset_id)
            .await?
            .ok_or_else(dataset_not_found)?;
        // Nạp lại đúng bộ file cũ là đường thử lại tường minh của lớp ngữ nghĩa: phiên bản còn ở structure (lần
        // trước mô hình lỗi) thì gọi lại một lần nữa.
        respawn_semantic_profile(state, ws, &dataset, &version);
        apply_semantic_state(state, &mut dataset);
        return Ok(UploadResult {
            created: false,
            dataset,
            version,
            columns: Vec::new(),
        });
    }

    // Bước 3: Parquet trong thư mục tạm. File Excel: mỗi sheet đã chọn đổi sang CSV trước (trong tiến trình con, xem
    // service/excel.rs), rồi đi như CSV.
    let inputs: Vec<InputFile> = match format {
        FileFormat::Xlsx => {
            excel::sheet_inputs(state, &upload.files[0], &sheets, &staging.sheets_dir()).await?
        }
        _ => upload
            .files
            .iter()
            .map(|f| InputFile {
                path: f.path.clone(),
                name: f.name.clone(),
                format,
                column_types: f.column_types.clone(),
            })
            .collect(),
    };
    let parquet_dir = staging.parquet_dir();
    let materialized =
        tokio::task::spawn_blocking(move || analytics::materialize(&inputs, &parquet_dir))
            .await
            .map_err(|e| AppError::Internal(format!("materialize task: {e}")))??;
    // Đo cột ngay trên Parquet trong thư mục tạm, trước khi ghi gì vào Postgres: lỗi ở đây không để lại dòng nào.
    let previous_formats = match &existing_dataset {
        Some(d) => profile::previous_time_formats(state, ws, d.id).await?,
        None => HashMap::new(),
    };
    let mut columns = profile::build_profile(
        staging.parquet_dir(),
        materialized.row_count,
        previous_formats,
    )
    .await?;
    apply_source_descriptions(&mut columns, &upload.descriptions);

    // Bước 4: dataset (nếu mới) và phiên bản. Đường dẫn lưu trong database là tương đối với DATA_DIR, để đổi chỗ thư
    // mục dữ liệu không phải sửa database.
    let is_new_dataset = existing_dataset.is_none();
    let mut dataset = match existing_dataset {
        Some(d) => d,
        None => {
            let name = dataset_name(&upload)?;
            let logical_table = free_logical_table(state, member, &name).await?;
            let source = match upload.database_source {
                Some(id) => id,
                None => state.store.create_local_source(ws).await?,
            };
            state
                .store
                .create_dataset(ws, source, &name, &logical_table, Some(member.user.id))
                .await?
        }
    };
    let relative = format!("{}/{}/{}", ws.as_uuid(), dataset.id, Uuid::new_v4());
    let new_version = NewVersion {
        source_kind: match upload.database_source {
            Some(_) => SourceKind::Postgres,
            None => source_kind(format),
        },
        file_hash: &content_hash,
        parquet_path: &relative,
        row_count: materialized.row_count,
        description: upload.descriptions.table.as_deref(),
    };
    let created = create_version_retrying(state, ws, dataset.id, &new_version, &columns).await;
    // Phiên bản và hồ sơ không ghi được thì dataset vừa tạo cho lần nạp này không có phiên bản nào: xóa nó, để danh
    // sách dataset không có dòng rỗng và nạp lại cùng file tạo lại từ đầu. File Parquet còn trong thư mục tạm, tự xóa.
    if created.is_err() && is_new_dataset {
        if let Err(e) = state
            .store
            .delete_dataset_without_versions(ws, dataset.id)
            .await
        {
            tracing::warn!(dataset_id = %dataset.id, error = ?e, "cannot remove dataset of a failed upload");
        }
    }
    let version = match created? {
        Created::New(version) => version,
        Created::Duplicate(version) => {
            // Như nhánh trùng ở bước 2: phiên bản còn ở structure thì nạp lại là đường thử lại của lớp ngữ nghĩa.
            respawn_semantic_profile(state, ws, &dataset, &version);
            apply_semantic_state(state, &mut dataset);
            return Ok(UploadResult {
                created: false,
                dataset,
                version,
                columns: Vec::new(),
            });
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
    // Workspace hoặc dataset bị xóa đúng lúc đang nạp (NotFound): bước trên đã tạo lại thư mục sau khi lệnh xóa dọn
    // xong. Không dọn thì file nằm lại trên đĩa mà không còn dòng nào trong database trỏ tới. Lỗi khác (mất kết nối
    // database) thì giữ file: UPDATE có thể đã chạy xong mà chỉ mất phản hồi, xóa file lúc đó là xóa phiên bản hiện hành.
    match state
        .store
        .set_current_version(ws, dataset.id, version.id)
        .await
    {
        Ok(()) => {}
        Err(AppError::NotFound(message)) => {
            remove_version_files(&state.config.data_dir, &relative);
            return Err(AppError::NotFound(message));
        }
        Err(e) => return Err(e),
    }
    let mut dataset = state
        .store
        .get_dataset(ws, dataset.id)
        .await?
        .ok_or_else(dataset_not_found)?;
    apply_semantic_state(state, &mut dataset);
    tracing::info!(dataset_id = %dataset.id, version = version.version, rows = materialized.row_count, "dataset version created");

    // Lớp ngữ nghĩa (#23) chạy nền sau khi trả 201: gọi mô hình có thể chậm hoặc lỗi, không để chặn hay làm hỏng
    // lần nạp. Giao diện theo dõi qua current_profile_status và suggested_questions của dataset.
    semantic::spawn_semantic_profile(
        state.clone(),
        *ws,
        dataset.clone(),
        version.id,
        version.row_count,
    );

    Ok(UploadResult {
        created: true,
        dataset,
        version,
        columns: materialized.columns,
    })
}

/// Gắn mô tả cột đọc từ database nguồn vào hồ sơ cấu trúc, theo tên cột: file CSV của bản chụp lấy tên cột đúng như
/// Postgres trả, nên tên khớp nhau. Cột không có comment giữ None, lớp ngữ nghĩa viết mô tả cho nó sau.
fn apply_source_descriptions(columns: &mut [NewColumn], descriptions: &SourceDescriptions) {
    for column in columns.iter_mut() {
        if let Some((_, text)) = descriptions
            .columns
            .iter()
            .find(|(name, _)| *name == column.name)
        {
            column.description = Some(text.clone());
        }
    }
}

/// Nạp lại đúng bộ file cũ: phiên bản trùng còn ở structure (lần trước mô hình lỗi) thì gọi lại lớp ngữ nghĩa một
/// lần nữa; đã semantic hay failed thì thôi.
fn respawn_semantic_profile(
    state: &AppState,
    ws: &WorkspaceId,
    dataset: &Dataset,
    version: &DatasetVersion,
) {
    if version.profile_status == ProfileStatus::Structure.as_str() {
        semantic::spawn_semantic_profile(
            state.clone(),
            *ws,
            dataset.clone(),
            version.id,
            version.row_count,
        );
    }
}

/// Kết quả create_version_retrying: phiên bản mới, hoặc phiên bản cùng nội dung mà một lần nạp song song vừa tạo.
enum Created {
    New(DatasetVersion),
    Duplicate(DatasetVersion),
}

/// Tạo phiên bản kèm hồ sơ cột (một giao dịch, xem Store::create_version), thử lại khi vấp lần nạp song song.
async fn create_version_retrying(
    state: &AppState,
    ws: &WorkspaceId,
    dataset_id: Uuid,
    new_version: &NewVersion<'_>,
    columns: &[NewColumn],
) -> Result<Created, AppError> {
    let mut attempts = 0;
    loop {
        match state
            .store
            .create_version(ws, dataset_id, new_version, columns)
            .await
        {
            Ok(version) => return Ok(Created::New(version)),
            Err(e) => {
                // Cùng bộ file nạp song song vào cùng dataset: bên ghi sau vấp UNIQUE (dataset_id, file_hash). Có phiên
                // bản cùng nội dung rồi thì trả nó, như nạp lại tuần tự.
                if let Some(version) = state
                    .store
                    .find_version_by_hash(ws, dataset_id, new_version.file_hash)
                    .await?
                {
                    return Ok(Created::Duplicate(version));
                }
                // Khác nội dung nạp song song: hai lần cùng tính ra một số phiên bản (MAX + 1) và một bên vấp UNIQUE
                // (dataset_id, version). Thử lại thì số kế tiếp đã khác. Lỗi khác cũng chỉ thử lại vài lần rồi báo.
                attempts += 1;
                if attempts >= 5 {
                    return Err(e);
                }
            }
        }
    }
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
                "File {} không phải CSV, Parquet hay Excel (đuôi .csv, .parquet hoặc .xlsx)",
                f.name
            ))
        })
    };
    let format = format_of(first)?;
    for f in &files[1..] {
        if format_of(f)? != format {
            return Err(AppError::BadRequest(
                "Các file nạp cùng lúc phải cùng định dạng: toàn CSV hoặc toàn Parquet; file Excel nạp riêng".to_string(),
            ));
        }
    }
    Ok(format)
}

/// Băm của cả bộ file: băm danh sách băm từng file đã sắp xếp, nên thứ tự chọn file không làm đổi kết quả.
/// Tăng khi đổi cách engine/excel.rs đổi sheet thành dữ liệu (xem content_hash).
const EXCEL_READER_VERSION: u32 = 1;

fn content_hash(files: &[StagedFile], sheets: &[String]) -> String {
    let mut parts: Vec<String> = files.iter().map(|f| f.sha256.clone()).collect();
    parts.sort_unstable();
    // Sheet đã chọn của file Excel, kèm phiên bản cách đọc Excel: đổi cách đọc (ví dụ cách ghi ô ngày giờ) thì tăng
    // EXCEL_READER_VERSION, để nạp lại file cũ ra phiên bản mới thay vì bị coi là trùng. Không có sheet (CSV, Parquet)
    // thì hash giữ như cũ, khớp các phiên bản đã nạp.
    if !sheets.is_empty() {
        let mut sheet_parts: Vec<String> = sheets.iter().map(|s| format!("sheet:{s}")).collect();
        sheet_parts.sort_unstable();
        parts.extend(sheet_parts);
        parts.push(format!("excel-reader:{EXCEL_READER_VERSION}"));
    }
    Sha256::digest(parts.join("\n").as_bytes())
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
        FileFormat::Xlsx => SourceKind::Xlsx,
        FileFormat::DatabaseCsv | FileFormat::DatabaseParquet => SourceKind::Postgres,
    }
}

/// Id dataset trong đường dẫn; sai dạng UUID thì 404 như dataset không có. ask/mod.rs dùng chung.
pub(super) fn parse_dataset_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id.trim()).map_err(|_| dataset_not_found())
}

pub(super) fn dataset_not_found() -> AppError {
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
            column_types: Vec::new(),
        };
        let sheets = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(
            content_hash(&[file("a"), file("b")], &[]),
            content_hash(&[file("b"), file("a")], &[])
        );
        assert_ne!(
            content_hash(&[file("a")], &[]),
            content_hash(&[file("a"), file("b")], &[])
        );
        // Cùng file Excel: thứ tự chọn sheet không quan trọng, chọn sheet khác thì khác.
        assert_eq!(
            content_hash(&[file("a")], &sheets(&["S1", "S2"])),
            content_hash(&[file("a")], &sheets(&["S2", "S1"]))
        );
        assert_ne!(
            content_hash(&[file("a")], &sheets(&["S1"])),
            content_hash(&[file("a")], &sheets(&["S1", "S2"]))
        );
    }
}
