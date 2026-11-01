// SPDX-License-Identifier: Apache-2.0
/* service/samples.rs - dataset mẫu đóng gói sẵn: khi thiết lập lần đầu tạo xong workspace đầu tiên, có
SAMPLE_DATABASE_URL thì thêm mỗi database mẫu làm một nguồn và chụp các nhóm bảng của nó (service/samples_db.rs, luồng
chính, compose trỏ vào sales_demo và edu_demo của dịch vụ demo-db trong data/postgres-demo khi bật profile demo). Không
có, hay không database nào chụp được, thì nạp <SAMPLES_DIR>/dataco.csv thành dataset "DataCo"
(bảng dataco) qua đúng đường nạp file thường (service/ingest.rs, kể cả hồ sơ cột), rồi ghi các chỉ số trong
dataco.metrics.json vào bảng metrics. Cả hai nhánh gắn gói ngành ghi trong <SAMPLES_DIR>/domains.json cho dataset mẫu
(service/samples_domains.rs). Câu mẫu của mock viết cho cả hai (engine/llm/mock_answers.json). Bộ đánh giá câu
hỏi mẫu (service/eval.rs) nạp file qua cùng hàm upload_local_file.

Gắn vào thiết lập lần đầu (service/accounts.rs), không vào lúc khởi động: thiết lập chỉ thành công một lần cho mỗi bản
cài, nên không cần cột hay bảng đánh dấu "đã nạp mẫu"; khởi động lại không nạp thêm, và người dùng đã xóa dataset mẫu
thì nó không tự quay lại. Đổi lại, bản cài đã thiết lập từ trước không nhận dataset mẫu: nạp tay (docs/INSTALL.md mục 7).

Lỗi ở đây chỉ ghi log mức cảnh báo: thiếu dataset mẫu không được chặn việc tạo tài khoản quản trị. */

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::auth::{self, Member};
use crate::engine::store::{Dataset, DatasetVersion, NewMetric, User};
use crate::error::AppError;
use crate::service::ingest::{self, StagedFile, Staging, Upload, UploadResult};
use crate::service::metrics::check::{build_check_table, check_metric};
use crate::service::samples_db;
use crate::service::samples_domains::assign_sample_domain;
use crate::state::AppState;

/// Tên dataset mẫu. Tên bảng lấy từ tên dataset (dataco), đúng bảng mà mock_answers.json viết SQL cho.
pub const SAMPLE_NAME: &str = "DataCo";
const SAMPLE_FILE: &str = "dataco.csv";
const SAMPLE_METRICS: &str = "dataco.metrics.json";

/// Kết quả một lần nạp mẫu, để ghi log và để test kiểm từng nhánh.
#[derive(Debug, PartialEq, Eq)]
pub enum SeedOutcome {
    /// SEED_SAMPLE=false.
    Disabled,
    /// SAMPLES_DIR không có dataco.csv (ví dụ chạy binary ngoài repo).
    NoSampleFile,
    /// Workspace đã có dataset tên DataCo, hoặc đã có đúng file này: không nạp lặp.
    AlreadyPresent,
    /// Đã nạp, kèm số chỉ số ghi được.
    Seeded { metrics: u64 },
    /// Đã thêm database mẫu làm nguồn và chụp bảng: số nguồn (database) chụp được, số dataset, quan hệ, chỉ số.
    SeededDatabase {
        sources: usize,
        datasets: usize,
        relations: usize,
        metrics: u64,
    },
}

/// Gọi từ thiết lập lần đầu với tài khoản quản trị vừa tạo và slug workspace đầu tiên. Không trả lỗi: ghi log rồi thôi.
pub async fn seed_first_workspace(state: &AppState, user: &User, slug: &str) {
    let result = match auth::member_of(&state.store, user.clone(), slug).await {
        Ok(member) => seed_workspace(state, &member).await,
        Err(e) => Err(e),
    };
    match result {
        Ok(outcome) => tracing::info!(?outcome, slug, "sample dataset step finished"),
        Err(e) => tracing::warn!(
            error = ?e,
            slug,
            "cannot load the sample dataset; upload it on the Data page instead (docs/INSTALL.md, section 7)"
        ),
    }
}

/// Nạp dataset mẫu vào workspace của member (admin): các database mẫu nếu có SAMPLE_DATABASE_URL, không thì hay không
/// database nào chụp được thì file dataco.csv. Gọi lại trên cùng workspace không tạo thêm gì.
pub async fn seed_workspace(state: &AppState, member: &Member) -> Result<SeedOutcome, AppError> {
    if !state.config.seed_sample {
        return Ok(SeedOutcome::Disabled);
    }
    if let Some(outcome) = seed_databases(state, member).await {
        return Ok(outcome);
    }
    seed_csv(state, member).await
}

/// Thêm từng database mẫu làm một nguồn và chụp bảng của nó. None khi không có database mẫu, hay không database nào
/// chụp được (không chạy, chưa nạp, không có khóa mã hóa): khi đó workspace đầu tiên vẫn nhận dataco.csv để hỏi ngay.
/// Một database lỗi trong khi database khác chụp được thì chỉ ghi cảnh báo; bảng đã chụp trước lúc lỗi (nếu có) vẫn
/// giữ.
async fn seed_databases(state: &AppState, member: &Member) -> Option<SeedOutcome> {
    let databases = &state.config.sample_databases;
    if databases.is_empty() {
        return None;
    }
    let (mut sources, mut datasets, mut relations, mut metrics) = (0, 0, 0, 0);
    let mut already_present = false;
    for db in databases {
        match samples_db::seed_database(state, member, db).await {
            Ok(Some(seed)) => {
                sources += 1;
                datasets += seed.datasets;
                relations += seed.relations;
                metrics += seed.metrics;
            }
            Ok(None) => already_present = true,
            Err(e) => tracing::warn!(
                error = ?e,
                host = %db.host,
                database = %db.database,
                "cannot snapshot the sample database (data/postgres-demo/README.md)"
            ),
        }
    }
    if sources > 0 {
        return Some(SeedOutcome::SeededDatabase {
            sources,
            datasets,
            relations,
            metrics,
        });
    }
    if already_present {
        return Some(SeedOutcome::AlreadyPresent);
    }
    tracing::warn!("no sample database could be snapshotted, loading dataco.csv instead");
    None
}

/// Nạp dataco.csv và chỉ số của nó.
async fn seed_csv(state: &AppState, member: &Member) -> Result<SeedOutcome, AppError> {
    let dir = &state.config.samples_dir;
    let csv = dir.join(SAMPLE_FILE);
    if !csv.is_file() {
        tracing::info!(path = %csv.display(), "no sample file, skipping the sample dataset");
        return Ok(SeedOutcome::NoSampleFile);
    }
    let datasets = state.store.list_datasets(&member.workspace).await?;
    if datasets.iter().any(|d| d.name == SAMPLE_NAME) {
        return Ok(SeedOutcome::AlreadyPresent);
    }

    let uploaded = upload_local_file(state, member, &csv, SAMPLE_NAME).await?;
    // Cùng file đã nạp dưới tên khác: ingest trả phiên bản cũ, không ghi chỉ số vào dataset của người dùng.
    if !uploaded.created {
        return Ok(SeedOutcome::AlreadyPresent);
    }
    assign_sample_domain(state, member, uploaded.dataset.parent_id, SAMPLE_NAME).await;
    let metrics = load_metrics(&dir.join(SAMPLE_METRICS))?;
    let runnable =
        keep_runnable(state, member, &uploaded.dataset, &uploaded.version, metrics).await?;
    let written = state
        .store
        .create_sample_metrics(&member.workspace, uploaded.dataset.id, &runnable)
        .await?;
    Ok(SeedOutcome::Seeded { metrics: written })
}

/// Nạp một file trên đĩa máy chủ như khi người dùng gửi file lên: chép vào thư mục tạm, tính SHA-256, rồi gọi
/// ingest::upload (Parquet, hồ sơ cột, hồ sơ ngữ nghĩa chạy nền).
pub async fn upload_local_file(
    state: &AppState,
    member: &Member,
    path: &Path,
    dataset_name: &str,
) -> Result<UploadResult, AppError> {
    let staging = Staging::new(&state.config.data_dir)?;
    let staged = staging.upload_path(0);
    tokio::fs::copy(path, &staged)
        .await
        .map_err(|e| AppError::Internal(format!("copy {}: {e}", path.display())))?;
    let bytes = tokio::fs::read(&staged)
        .await
        .map_err(|e| AppError::Internal(format!("read {}: {e}", staged.display())))?;
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    // Tên file quyết định cách đọc (đuôi .csv) và ghi vào cột _source_file.
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| SAMPLE_FILE.to_string());
    let upload = Upload {
        name: Some(dataset_name.to_string()),
        dataset_id: None,
        files: vec![StagedFile {
            path: staged,
            name,
            sha256,
            column_types: Vec::new(),
        }],
        sheets: Vec::new(),
        database_source: None,
        format: None,
        descriptions: Default::default(),
    };
    ingest::upload(state, member, staging, upload).await
}

/// Đọc file chỉ số mẫu. Không có file thì không có chỉ số (dataset vẫn nạp); JSON hỏng là lỗi.
pub(super) fn load_metrics(path: &Path) -> Result<Vec<NewMetric>, AppError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %path.display(), "no sample metrics file, the sample dataset has no metrics");
            return Ok(Vec::new());
        }
        Err(e) => return Err(AppError::Internal(format!("read {}: {e}", path.display()))),
    };
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Chỉ giữ chỉ số chạy được trên dataset vừa nạp. Chỉ số vào thẳng prompt như công thức đã duyệt, nên một biểu thức
/// sai cột (file mẫu sửa tay, hay dataco.csv bị thay bằng bản khác cột) sẽ kéo mô hình viết SQL lỗi mãi; bỏ nó và ghi
/// log thì tốt hơn.
pub(super) async fn keep_runnable(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
    version: &DatasetVersion,
    metrics: Vec<NewMetric>,
) -> Result<Vec<NewMetric>, AppError> {
    let table = build_check_table(state, &member.workspace, dataset, version).await?;
    let mut kept = Vec::new();
    for metric in metrics {
        match check_metric(state, &table, &metric).await {
            Ok(()) => kept.push(metric),
            Err(why) => tracing::warn!(metric = %metric.name, %why, "sample metric skipped"),
        }
    }
    Ok(kept)
}
