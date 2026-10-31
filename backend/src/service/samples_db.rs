// SPDX-License-Identifier: Apache-2.0
/* service/samples_db.rs - dataset mẫu chụp từ một database mẫu (một chuỗi của SAMPLE_DATABASE_URL, compose trỏ vào
sales_demo và edu_demo của dịch vụ demo-db trong data/postgres-demo): thêm database đó làm nguồn Postgres của workspace
đầu tiên, rồi chụp các nhóm bảng của nó ghi trong <SAMPLES_DIR>/sample_database.json, mỗi bảng một dataset kèm quan hệ
khóa ngoại, đúng như admin làm ở trang Dữ liệu (service/sources/). Bảng nào có <SAMPLES_DIR>/<tên bảng>.metrics.json thì
ghi thêm chỉ số mẫu.

service/samples.rs gọi seed_database cho từng database mẫu; lỗi ở đây thì bên đó ghi log, và khi không database nào
chụp được thì nạp dataco.csv thay.
Bộ đánh giá trên database (service/eval.rs) gọi cùng hàm, nên `make eval` kiểm đúng đường mà bản cài mới đi qua. */

use std::path::Path;

use serde::Deserialize;

use crate::auth::Member;
use crate::error::AppError;
use crate::sample_database::SampleDatabase;
use crate::service::samples::{keep_runnable, load_metrics};
use crate::service::sources::{self, CreateSourceRequest, SnapshotRequest};
use crate::state::AppState;

/// File trong SAMPLES_DIR liệt kê các nhóm bảng cần chụp.
pub const PLAN_FILE: &str = "sample_database.json";

/// Một lần chụp: database, bảng bắt đầu và các bảng chụp kèm, như request POST /sources/{id}/snapshot.
#[derive(Debug, Deserialize)]
pub struct PlannedSnapshot {
    /// Tên database mà lần chụp này dành cho (sales_demo, edu_demo). Không ghi thì áp cho mọi database mẫu, như file
    /// kế hoạch khi chỉ có một database.
    #[serde(default)]
    pub database: Option<String>,
    pub table: String,
    #[serde(default)]
    pub tables: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Plan {
    snapshots: Vec<PlannedSnapshot>,
}

/// Kết quả: số dataset, số quan hệ và số chỉ số đã ghi.
#[derive(Debug, PartialEq, Eq)]
pub struct DatabaseSeed {
    pub datasets: usize,
    pub relations: usize,
    pub metrics: u64,
}

/// Thêm nguồn và chụp mọi nhóm bảng của kế hoạch. None khi workspace đã có nguồn trỏ vào đúng database này (gọi lại
/// không tạo thêm gì).
pub async fn seed_database(
    state: &AppState,
    member: &Member,
    db: &SampleDatabase,
) -> Result<Option<DatabaseSeed>, AppError> {
    let plan = load_plan(&state.config.samples_dir.join(PLAN_FILE))?;
    let planned = build_planned_for(&plan, &db.database);
    // Kiểm trước khi thêm nguồn: database không có trong kế hoạch (ví dụ company_demo của bản cũ) thì không để lại một
    // nguồn không bảng nào.
    if planned.is_empty() {
        return Err(AppError::Internal(format!(
            "{PLAN_FILE} has no snapshot for database {}",
            db.database
        )));
    }
    let existing = sources::list(state, member).await?;
    if existing
        .iter()
        .any(|s| s.host == db.host && s.port == db.port && s.database == db.database)
    {
        return Ok(None);
    }
    let source = sources::create(state, member, build_request(db)).await?;
    let mut seed = DatabaseSeed {
        datasets: 0,
        relations: 0,
        metrics: 0,
    };
    for planned in planned {
        let request = SnapshotRequest {
            table: Some(planned.table.clone()),
            tables: planned.tables.clone(),
            name: None,
            dataset_id: None,
        };
        let done = sources::snapshot(state, member, source.id, request).await?;
        seed.datasets += done.datasets.len();
        seed.relations += done.relations.len();
        for result in &done.datasets {
            seed.metrics += seed_metrics(state, member, result).await?;
        }
    }
    Ok(Some(seed))
}

/// Các lần chụp dành cho một database: ghi đúng tên database đó, hay không ghi database.
fn build_planned_for<'a>(plan: &'a Plan, database: &str) -> Vec<&'a PlannedSnapshot> {
    plan.snapshots
        .iter()
        .filter(|s| s.database.as_deref().is_none_or(|d| d == database))
        .collect()
}

fn build_request(db: &SampleDatabase) -> CreateSourceRequest {
    CreateSourceRequest {
        host: db.host.clone(),
        port: Some(db.port),
        database: db.database.clone(),
        user: db.user.clone(),
        password: db.password.expose().to_string(),
        sslmode: db.sslmode.clone(),
        ssl_root_cert: None,
    }
}

fn load_plan(path: &Path) -> Result<Plan, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Chỉ số mẫu của một dataset vừa chụp, từ <SAMPLES_DIR>/<bảng logic>.metrics.json nếu có. Chỉ số không chạy được
/// trên bảng thì bỏ và ghi log, như dataset mẫu từ file.
async fn seed_metrics(
    state: &AppState,
    member: &Member,
    result: &crate::service::ingest::UploadResult,
) -> Result<u64, AppError> {
    let file = format!("{}.metrics.json", result.dataset.logical_table);
    let path = state.config.samples_dir.join(file);
    if !path.is_file() {
        return Ok(0);
    }
    let metrics = load_metrics(&path)?;
    let runnable = keep_runnable(state, member, &result.dataset, &result.version, metrics).await?;
    state
        .store
        .create_sample_metrics(&member.workspace, result.dataset.id, &runnable)
        .await
}
