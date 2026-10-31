// SPDX-License-Identifier: Apache-2.0
/* service/samples_tests.rs - test nạp dataset mẫu (service/samples.rs, service/samples_db.rs) qua thiết lập lần đầu
thật: trên file mẫu của repo (data/samples), và trên database mẫu là chính database của test (chụp bảng, quan hệ, chỉ
số; không tới được thì nạp file). Mỗi test một database mới vì thiết lập cần "chưa có tài khoản nào"; bỏ qua khi không có
DATABASE_URL. */

use std::path::{Path, PathBuf};

use crate::auth::{self, Member};
use crate::config::{Config, Secret};
use crate::engine::excel_worker::ExcelWorker;
use crate::engine::store::test_support::{fetch_count, fresh_database};
use crate::engine::store::Store;
use crate::sample_database::SampleDatabase;
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

/// Khóa mã hóa nguồn cho test (32 byte base64), như api/sources_tests.rs.
const KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

/// Database mẫu là chính database của test: hai bảng nối bằng khóa ngoại trong schema shop, đọc bằng một tài khoản chỉ
/// đọc schema đó; chỉ số của bảng orders trong thư mục mẫu tự dựng.
async fn build_sample_database(
    db: &crate::engine::store::test_support::TestDb,
    samples: &Path,
) -> SampleDatabase {
    use crate::engine::store::test_support::{database_url, exec};
    for sql in [
        "CREATE SCHEMA shop",
        "CREATE TABLE shop.customers (customer_id integer PRIMARY KEY, segment text NOT NULL)",
        "CREATE TABLE shop.orders (order_id integer PRIMARY KEY, \
         customer_id integer NOT NULL REFERENCES shop.customers (customer_id), amount numeric NOT NULL)",
        "INSERT INTO shop.customers VALUES (1, 'Consumer'), (2, 'Corporate')",
        "INSERT INTO shop.orders VALUES (10, 1, 5), (11, 1, 7), (12, 2, 9)",
    ] {
        exec(&db.store, sql).await;
    }
    // Tài khoản chỉ đọc schema shop, như analyst của database mẫu: thiết lập chụp mọi bảng tài khoản đọc được, nên
    // tài khoản của test (chủ mọi bảng của ứng dụng) sẽ chụp cả bảng của xdi.
    let tag = &uuid::Uuid::new_v4().simple().to_string()[..12];
    let user = format!("xdi_sample_{tag}");
    for sql in [
        format!("CREATE ROLE {user} LOGIN PASSWORD 'sample-{tag}'"),
        format!("GRANT USAGE ON SCHEMA shop TO {user}"),
        format!("GRANT SELECT ON ALL TABLES IN SCHEMA shop TO {user}"),
    ] {
        exec(&db.store, &sql).await;
    }
    std::fs::write(
        samples.join("orders.metrics.json"),
        r#"[{"name": "total", "expression": "SUM(amount)"}]"#,
    )
    .unwrap();
    let mut sample = crate::sample_database::build_sample_database(&database_url().unwrap())
        .unwrap()
        .unwrap();
    sample.database = db.name().to_string();
    sample.user = user;
    sample.password = Secret::new(format!("sample-{tag}"));
    sample.sslmode = Some("disable".to_string());
    sample
}

fn with_sample_database(mut state: AppState, sample: SampleDatabase) -> AppState {
    let mut config = (*state.config).clone();
    config.source_secret_key = Secret::new(KEY);
    config.source_allowed_hosts = vec![sample.host.clone()];
    config.sample_database = Some(sample);
    state.config = std::sync::Arc::new(config);
    state
}

#[tokio::test]
async fn setup_snapshots_the_sample_database() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let samples = TempDir::new();
    let sample = build_sample_database(&db, &samples.0).await;
    let state = with_sample_database(
        build_state(db.store.clone(), &data.0, &samples.0, true),
        sample,
    );
    let member = setup(&state).await;

    // Mỗi bảng một dataset, không có DataCo; quan hệ orders -> customers được ghi; chỉ số của orders được ghi.
    let datasets = state.store.list_datasets(&member.workspace).await.unwrap();
    let mut tables: Vec<&str> = datasets.iter().map(|d| d.logical_table.as_str()).collect();
    tables.sort();
    assert_eq!(tables, ["customers", "orders"]);
    let relations = state
        .store
        .list_relations(&member.workspace, None)
        .await
        .unwrap();
    assert_eq!(relations.len(), 1);
    let orders = datasets
        .iter()
        .find(|d| d.logical_table == "orders")
        .unwrap();
    let metrics = state
        .store
        .list_metrics(&member.workspace, orders.id)
        .await
        .unwrap();
    assert_eq!(
        metrics.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["total"]
    );
    assert_eq!(
        state
            .store
            .list_sources(&member.workspace)
            .await
            .unwrap()
            .len(),
        1
    );

    // Gọi lại: nguồn đã có, không thêm gì.
    assert_eq!(
        seed_workspace(&state, &member).await.unwrap(),
        SeedOutcome::AlreadyPresent
    );
    assert_eq!(
        state
            .store
            .list_datasets(&member.workspace)
            .await
            .unwrap()
            .len(),
        2
    );
    db.drop().await;
}

/// Database mẫu không tới được: thiết lập vẫn xong và workspace đầu tiên có DataCo từ file.
#[tokio::test]
async fn unreachable_sample_database_falls_back_to_the_csv() {
    let Some(db) = fresh_database().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let data = TempDir::new();
    let sample = crate::sample_database::build_sample_database(
        "postgres://analyst:analyst@127.0.0.1:1/company_demo",
    )
    .unwrap()
    .unwrap();
    let state = with_sample_database(
        build_state(db.store.clone(), &data.0, &repo_samples(), true),
        sample,
    );
    let member = setup(&state).await;

    let datasets = state.store.list_datasets(&member.workspace).await.unwrap();
    assert_eq!(
        datasets.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        [SAMPLE_NAME]
    );
    db.drop().await;
}
