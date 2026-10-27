// SPDX-License-Identifier: Apache-2.0
/* service/samples_tests.rs - test nạp dataset mẫu (service/samples.rs) qua thiết lập lần đầu thật, trên file mẫu của
repo (data/samples). Mỗi test một database mới vì thiết lập cần "chưa có tài khoản nào"; bỏ qua khi không có
DATABASE_URL. */

use std::path::{Path, PathBuf};

use crate::auth::{self, Member};
use crate::config::Config;
use crate::engine::excel_worker::ExcelWorker;
use crate::engine::store::test_support::{fetch_count, fresh_database};
use crate::engine::store::Store;
use crate::service::accounts::{self, SetupInput};
use crate::service::samples::{seed_workspace, SeedOutcome, SAMPLE_NAME};
use crate::state::AppState;

const CODE: &str = "ABCD-EFGH-JKMN";
const SAMPLE_METRICS_SQL: &str =
    "SELECT count(*) FROM metrics WHERE origin = 'sample' AND status = 'confirmed'";

/// Thư mục tạm cho một test (dữ liệu nạp, hay bộ file mẫu tự dựng), tự xóa khi ra khỏi phạm vi.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("xdi-samples-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo_samples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/samples")
}

fn build_state(store: Store, data: &Path, samples: &Path, seed: bool) -> AppState {
    let mut config = Config::from_env();
    config.data_dir = data.to_path_buf();
    config.samples_dir = samples.to_path_buf();
    config.seed_sample = seed;
    config.llm_provider = "mock".to_string();
    config.excel_worker = ExcelWorker::InProcess;
    let state = AppState::new(config, store);
    *state.setup_code.lock().unwrap() = Some(CODE.to_string());
    state
}

/// Đi thiết lập lần đầu như người dùng, trả admin của workspace đầu tiên.
async fn setup(state: &AppState) -> Member {
    let input = SetupInput {
        setup_code: CODE.to_string(),
        email: "lan@example.com".to_string(),
        display_name: "Lan".to_string(),
        password: "a long password".to_string(),
        workspace_name: "Cửa hàng".to_string(),
    };
    let (user, _) = accounts::setup(state, input).await.expect("setup");
    auth::member_of(&state.store, user, "cua-hang")
        .await
        .unwrap()
}

fn sample_metric_count() -> usize {
    let text = std::fs::read_to_string(repo_samples().join("dataco.metrics.json")).unwrap();
    serde_json::from_str::<Vec<serde_json::Value>>(&text)
        .unwrap()
        .len()
}

#[tokio::test]
async fn setup_seeds_dataco_and_metrics_once() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let state = build_state(db.store.clone(), &data.0, &repo_samples(), true);
    let member = setup(&state).await;

    let datasets = state.store.list_datasets(&member.workspace).await.unwrap();
    assert_eq!(datasets.len(), 1);
    assert_eq!(datasets[0].name, SAMPLE_NAME);
    assert_eq!(datasets[0].logical_table, "dataco");
    // Hồ sơ cấu trúc xong ngay trong lần nạp; lớp ngữ nghĩa chạy nền nên có thể đã lên semantic.
    assert!(matches!(
        datasets[0].current_profile_status.as_deref(),
        Some("structure" | "semantic")
    ));
    // Mọi chỉ số trong file mẫu đều qua guard và chạy được trên dữ liệu mẫu, nên đều được ghi.
    let metrics = state
        .store
        .list_metrics(&member.workspace, datasets[0].id)
        .await
        .unwrap();
    assert_eq!(metrics.len(), sample_metric_count());
    assert!(metrics.iter().all(|m| m.confirmed));
    assert_eq!(
        fetch_count(&db.store, SAMPLE_METRICS_SQL).await,
        sample_metric_count() as i64
    );

    // Gọi lại trên cùng workspace: không thêm dataset hay chỉ số.
    let again = seed_workspace(&state, &member).await.unwrap();
    assert_eq!(again, SeedOutcome::AlreadyPresent);
    assert_eq!(
        state
            .store
            .list_datasets(&member.workspace)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fetch_count(&db.store, SAMPLE_METRICS_SQL).await,
        sample_metric_count() as i64
    );
    db.drop().await;
}

#[tokio::test]
async fn seed_sample_false_seeds_nothing() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let state = build_state(db.store.clone(), &data.0, &repo_samples(), false);
    let member = setup(&state).await;

    assert!(state
        .store
        .list_datasets(&member.workspace)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(fetch_count(&db.store, SAMPLE_METRICS_SQL).await, 0);
    db.drop().await;
}

#[tokio::test]
async fn missing_sample_file_is_skipped_without_error() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let empty = TempDir::new();
    let state = build_state(db.store.clone(), &data.0, &empty.0, true);
    // Thiết lập vẫn thành công khi không có file mẫu.
    let member = setup(&state).await;

    assert!(state
        .store
        .list_datasets(&member.workspace)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        seed_workspace(&state, &member).await.unwrap(),
        SeedOutcome::NoSampleFile
    );
    db.drop().await;
}

#[tokio::test]
async fn metric_that_does_not_run_is_skipped() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let samples = TempDir::new();
    std::fs::copy(
        repo_samples().join("dataco.csv"),
        samples.0.join("dataco.csv"),
    )
    .unwrap();
    // Một chỉ số đúng, một chỉ số trên cột không có, một chỉ số có cột thời gian không có.
    std::fs::write(
        samples.0.join("dataco.metrics.json"),
        r#"[
            {"name": "revenue", "expression": "SUM(\"Sales\")"},
            {"name": "broken", "expression": "SUM(\"No Such Column\")"},
            {"name": "bad time", "expression": "SUM(\"Sales\")", "time_column": "No Such Date"}
        ]"#,
    )
    .unwrap();
    let state = build_state(db.store.clone(), &data.0, &samples.0, true);
    let member = setup(&state).await;

    let datasets = state.store.list_datasets(&member.workspace).await.unwrap();
    let metrics = state
        .store
        .list_metrics(&member.workspace, datasets[0].id)
        .await
        .unwrap();
    assert_eq!(
        metrics.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["revenue"]
    );
    db.drop().await;
}
