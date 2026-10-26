// SPDX-License-Identifier: Apache-2.0
/* service/ask_context_tests.rs - test của service/ask_context.rs: phần thuần (chọn cột, mơ hồ, bảng rộng, trần số giá
trị, gợi ý khớp chính xác) không cần database; phần đọc hồ sơ chạy trên Postgres thật, bỏ qua khi thiếu DATABASE_URL như
mọi test store. Bí danh, mô tả và cờ nhạy cảm do #23 ghi, chưa có code ghi, nên test tự UPDATE bằng SQL. Dataset mẫu là
hồ sơ viết sẵn của dataco trong engine/llm/mock_answers.json (53 cột, bí danh tiếng Việt), cộng giá trị của DataCo thật
cho các cột nhóm. Các câu đã sinh khớp nhầm ở những bản trước (review của #28) có trong test, mỗi câu kèm kết quả mong
đợi. */

use std::time::Duration;

use uuid::Uuid;

use super::ask_context::*;
use super::ask_contract::ChosenOption;
use super::ask_phrases::build_phrases;
use super::ask_values::ValueIndex;
use crate::auth::WorkspaceId;
use crate::engine::store::test_support::{drop_workspace, exec, store, temp_workspace};
use crate::engine::store::{
    DatasetVersion, NewColumn, NewColumnValue, NewVersion, ProfileRole, ProfileStatus, SourceKind,
    Store, ValueRow,
};
use crate::error::AppError;
use crate::state::AppState;

/// Id cột cố định theo tên, để hồ sơ và giá trị của test thuần trỏ cùng một cột.
fn column_id(name: &str) -> Uuid {
    Uuid::from_u128(
        name.bytes()
            .fold(7u128, |id, b| id.wrapping_mul(131).wrapping_add(b as u128)),
    )
}

fn profile(name: &str, role: &str, aliases: &[&str]) -> ColumnProfile {
    ColumnProfile {
        id: column_id(name),
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        time_format: None,
        description: None,
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
        is_sensitive: false,
        values: Vec::new(),
        values_complete: false,
        matched: false,
        unit: None,
    }
}

/// Chỉ mục giá trị từ các bộ (cột, giá trị, bí danh), theo thứ tự truyền vào.
fn index(rows: &[(&str, &str, &[&str])]) -> ValueIndex {
    let rows: Vec<ValueRow> = rows
        .iter()
        .map(|(column, value, aliases)| ValueRow {
            column_id: column_id(column),
            column_name: column.to_string(),
            value: value.to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        })
        .collect();
    ValueIndex::build(&rows)
}

fn chosen(choice_id: &str, option_id: &str) -> ChosenOption {
    ChosenOption {
        choice_id: choice_id.to_string(),
        option_id: option_id.to_string(),
    }
}

fn names(context: &AskContext) -> Vec<&str> {
    context.columns.iter().map(|c| c.name.as_str()).collect()
}

fn hints(context: &AskContext) -> Vec<(&str, &str)> {
    context
        .hints
        .iter()
        .map(|h| (h.column.as_str(), h.value.as_str()))
        .collect()
}

fn column<'a>(context: &'a AskContext, name: &str) -> &'a ColumnProfile {
    context
        .columns
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn column_values(context: &AskContext, name: &str) -> Vec<String> {
    column(context, name)
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect()
}

#[test]
fn close_columns_are_ambiguous_until_the_user_picks() {
    let columns = vec![
        profile("Market", "dimension", &["thị trường", "khu vực"]),
        profile("Order Region", "dimension", &["khu vực"]),
        profile("Sales", "measure", &[]),
    ];
    let empty = ValueIndex::default();
    let phrases = build_phrases("Doanh thu theo khu vực");
    let context = build_context(&phrases, columns.clone(), &empty, &[]);
    assert_eq!(
        context.ambiguous,
        [AmbiguousWord {
            choice_id: "column:khu vuc".to_string(),
            phrase: "khu vực".to_string(),
            columns: vec!["Market".to_string(), "Order Region".to_string()],
        }]
    );
    assert!(context.picks.is_empty());

    let picked = build_context(
        &phrases,
        columns.clone(),
        &empty,
        &[chosen("column:khu vuc", "Order Region")],
    );
    assert!(picked.ambiguous.is_empty());
    assert_eq!(
        picked.picks,
        [ColumnPick {
            choice_id: "column:khu vuc".to_string(),
            phrase: "khu vực".to_string(),
            column: "Order Region".to_string(),
            others: vec!["Market".to_string()],
            chosen_by_user: true,
        }]
    );

    // Lựa chọn trỏ tới cột không phải ứng viên, hay của cụm khác, thì bỏ qua: vẫn mơ hồ.
    for wrong in [
        chosen("column:khu vuc", "Sales"),
        chosen("column:thi truong", "Order Region"),
    ] {
        let context = build_context(&phrases, columns.clone(), &empty, &[wrong]);
        assert_eq!(context.ambiguous.len(), 1);
    }
}

/// Cả hai bên có dấu thì so có dấu, đúng nguyên văn hơn sau bỏ dấu đủ CLOSE_SCORE nên chọn luôn và ghi thành giả định
/// có cột khác; một bên không dấu ("khu vuc", cột khu_vuc) thì so sau bỏ dấu. Gõ dạng tổ hợp cho cùng kết quả.
#[test]
fn column_names_compare_accents_when_both_sides_have_them() {
    let columns = vec![
        profile("Market", "dimension", &["khu vuc"]),
        profile("Order Region", "dimension", &["Khu vực"]),
    ];
    let empty = ValueIndex::default();
    for question in [
        "doanh thu theo khu vực",
        "doanh thu theo khu vu\u{31b}\u{323}c",
    ] {
        let context = build_context(&build_phrases(question), columns.clone(), &empty, &[]);
        assert!(context.ambiguous.is_empty(), "{question}");
        assert_eq!(context.picks.len(), 1);
        assert_eq!(context.picks[0].column, "Order Region");
        assert_eq!(context.picks[0].others, ["Market"]);
    }
    let context = build_context(
        &build_phrases("doanh thu theo khu vuc"),
        columns,
        &empty,
        &[],
    );
    assert_eq!(context.picks[0].column, "Market");
    assert_eq!(context.picks[0].others, ["Order Region"]);
    assert!(!context.picks[0].chosen_by_user);
    let columns = vec![profile("khu_vuc", "dimension", &[])];
    let context = build_context(
        &build_phrases("doanh thu theo khu vực"),
        columns,
        &empty,
        &[],
    );
    assert_eq!(context.picks[0].column, "khu_vuc");
}

#[test]
fn wide_tables_keep_matched_hinted_and_time_columns_only() {
    let mut columns: Vec<ColumnProfile> = (0..70)
        .map(|i| profile(&format!("Metric {i}"), "measure", &[]))
        .collect();
    columns.push(profile("order date (DateOrders)", "time", &[]));
    columns.push(profile("Category Name", "dimension", &["danh mục"]));
    columns.push(profile("Category Id", "key", &["mã danh mục"]));
    columns.push(profile("Market", "dimension", &[]));
    columns.push(profile("Order Region", "dimension", &[]));
    columns.push(profile("Customer Segment", "dimension", &[]));
    let values = index(&[
        ("Market", "Europe", &["châu Âu"]),
        ("Order Region", "Southern Europe", &[]),
        ("Customer Segment", "Home Office", &[]),
    ]);

    let context = build_context(
        &build_phrases("Doanh thu theo danh mục ở châu Âu, từng segment"),
        columns.clone(),
        &values,
        &[],
    );
    // Category Name: bí danh "danh mục". Market: gợi ý giá trị "châu âu". Customer Segment: "segment" là một từ của
    // tên. Category Id chỉ chứa "danh mục" trong bí danh dài hơn, mà "danh mục" đã khớp đúng Category Name.
    assert_eq!(
        names(&context),
        [
            "order date (DateOrders)",
            "Category Name",
            "Market",
            "Customer Segment"
        ]
    );
    assert!(context
        .columns
        .iter()
        .all(|c| c.matched || c.role == "time"));
    assert_eq!(hints(&context), [("Market", "Europe")]);
    assert_eq!(column_values(&context, "Market"), ["Europe"]);

    // Không cụm nào khớp: gửi cả schema thay vì chỉ cột thời gian.
    let none = build_context(&build_phrases("xin chào"), columns.clone(), &values, &[]);
    assert_eq!(none.columns.len(), columns.len());
}

#[test]
fn column_values_are_capped_per_column_and_in_total() {
    // Tới 50 giá trị thì gửi đủ, nhiều hơn thì 20 giá trị nhiều dòng nhất.
    assert_eq!(
        build_column_values(&[5, 23, 50, 51, 164]),
        [5, 23, 50, 20, 20]
    );
    // Tổng vượt 400: hạ trần chung, cột nhiều giá trị bị cắt trước, cột ít giá trị giữ đủ.
    let mut counts = vec![3, 4];
    counts.extend([50; 8]);
    counts.push(164);
    let sent = build_column_values(&counts);
    assert!(sent.iter().sum::<usize>() <= CONTEXT_VALUES_MAX);
    assert_eq!(&sent[..2], [3, 4]);
    assert_eq!(sent[2], 46);
    assert_eq!(sent[10], 20);
    assert_eq!(build_column_values(&[]), Vec::<usize>::new());
}

/// Gợi ý chỉ là khớp chính xác của cụm từ hai âm tiết không có từ dừng; không chọn hộ khi hai cột cùng khớp, và không
/// có gợi ý cho các câu đã sinh khớp nhầm ở những bản trước.
#[test]
fn hints_are_exact_matches_of_multi_syllable_phrases() {
    let mut columns: Vec<ColumnProfile> = [
        "Market",
        "Order Region",
        "Shipping Mode",
        "region",
        "khu_vuc",
        "mien",
        "huong",
        "loai_khach",
        "gioi_tinh",
        "ghi_chu",
    ]
    .iter()
    .map(|name| profile(name, "dimension", &[]))
    .collect();
    let mut secret = profile("ho_ten", "dimension", &[]);
    secret.is_sensitive = true;
    columns.push(secret);
    let values = index(&[
        ("Market", "Europe", &[]),
        ("Order Region", "Southern Europe", &[]),
        ("Shipping Mode", "Standard Class", &[]),
        ("region", "South", &["miền Nam"]),
        ("khu_vuc", "Mien Nam", &[]),
        ("khu_vuc", "Ha Noi", &[]),
        ("mien", "Bac", &[]),
        ("mien", "Trung", &[]),
        ("huong", "Dong", &[]),
        ("loai_khach", "Khac", &[]),
        ("loai_khach", "Ca nhan", &[]),
        ("gioi_tinh", "Nam", &[]),
        ("gioi_tinh", "Nữ", &[]),
        ("ghi_chu", "Hòa Bình", &[]),
        ("ho_ten", "Nguyen Van A", &["khách quen"]),
    ]);
    let ask =
        |question: &str| build_context(&build_phrases(question), columns.clone(), &values, &[]);

    for (question, expected) in [
        (
            "doanh thu Southern Europe",
            ("Order Region", "Southern Europe"),
        ),
        (
            "số đơn standard class năm 2017",
            ("Shipping Mode", "Standard Class"),
        ),
        ("doanh thu Hà Nội", ("khu_vuc", "Ha Noi")),
        ("doanh thu Hoà Bình", ("ghi_chu", "Hòa Bình")),
    ] {
        assert_eq!(hints(&ask(question)), [expected], "{question}");
    }
    // Hai cột cùng khớp: hai gợi ý, không hỏi lại, không chọn hộ.
    let context = ask("doanh thu miền Nam");
    assert_eq!(
        hints(&context),
        [("region", "South"), ("khu_vuc", "Mien Nam")]
    );
    assert!(context.ambiguous.is_empty());
    for question in [
        "doanh thu từng miền năm 2017",
        "doanh thu theo miền năm 2017",
        "doanh thu mien nam",
        "doanh thu trung bình",
        "doanh thu trên 5 triệu đồng",
        "doanh thu khách lẻ",
        "doanh thu Việt Nam",
        "doanh thu Việt Nam năm 2017",
        "doanh thu Quảng Nam",
        "doanh thu Đông Nam Á",
        "doanh thu Europe",
        "doanh thu nữ",
        "doanh thu khách quen",
        "tổng số đơn theo tháng",
    ] {
        let context = ask(question);
        assert!(context.hints.is_empty(), "{question}: {:?}", context.hints);
        assert!(context.ambiguous.is_empty(), "{question}");
    }
    // Cột nhạy cảm: không có giá trị, không có gợi ý.
    let context = ask("doanh thu nguyen van a");
    assert!(context.hints.is_empty());
    assert!(column(&context, "ho_ten").values.is_empty());
    assert_eq!(column_values(&context, "gioi_tinh"), ["Nam", "Nữ"]);
    assert!(column(&context, "gioi_tinh").values_complete);
}

// ---- Postgres ----

fn new_column(position: i32, name: &str, role: ProfileRole, values: &[&str]) -> NewColumn {
    NewColumn {
        position,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role,
        is_sensitive: false,
        null_count: 0,
        distinct_count: values.len() as i64,
        min_value: None,
        max_value: None,
        time_format: None,
        // Tần suất giảm dần theo thứ tự truyền vào, để thứ tự giá trị biết trước.
        values: values
            .iter()
            .enumerate()
            .map(|(i, v)| NewColumnValue {
                value: v.to_string(),
                frequency: 1000 - i as i64,
            })
            .collect(),
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn text_array(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| quote(i)).collect();
    format!("ARRAY[{}]::text[]", quoted.join(", "))
}

async fn set_column(store: &Store, version: Uuid, column: &str, sets: &str) {
    exec(
        store,
        &format!(
            "UPDATE columns SET {sets} WHERE version_id = '{version}' AND name = {}",
            quote(column)
        ),
    )
    .await;
}

async fn set_value_aliases(
    store: &Store,
    version: Uuid,
    column: &str,
    value: &str,
    aliases: &[&str],
) {
    exec(
        store,
        &format!(
            "UPDATE column_values SET aliases = {} WHERE value = {} AND column_id =
                 (SELECT id FROM columns WHERE version_id = '{version}' AND name = {})",
            text_array(aliases),
            quote(value),
            quote(column)
        ),
    )
    .await;
}

async fn new_version(
    store: &Store,
    ws: &WorkspaceId,
    dataset: Uuid,
    hash: &str,
    columns: &[NewColumn],
) -> DatasetVersion {
    let new = NewVersion {
        source_kind: SourceKind::Csv,
        file_hash: hash,
        parquet_path: "x",
        row_count: 10,
    };
    store
        .create_version(ws, dataset, &new, columns)
        .await
        .unwrap()
}

fn app_state(store: &Store) -> AppState {
    AppState::new(crate::config::Config::from_env(), store.clone())
}

async fn ask(
    state: &AppState,
    ws: &WorkspaceId,
    version: &DatasetVersion,
    question: &str,
) -> AskContext {
    build_ask_context(state, ws, version, question, &[])
        .await
        .unwrap()
}

/// Bảng nhỏ có cột miền (bí danh tiếng Việt), cột nước và một cột nhạy cảm có giá trị và bí danh.
async fn small_version(
    store: &Store,
    ws: &WorkspaceId,
    dataset: Uuid,
    hash: &str,
    south: &str,
) -> DatasetVersion {
    let columns = [
        new_column(
            0,
            "region",
            ProfileRole::Dimension,
            &[south, "North", "Central"],
        ),
        new_column(
            1,
            "country",
            ProfileRole::Dimension,
            &["Việt Nam", "Thái Lan"],
        ),
        new_column(2, "amount", ProfileRole::Measure, &["1", "2"]),
        new_column(3, "order_date", ProfileRole::Time, &[]),
        new_column(
            4,
            "customer_name",
            ProfileRole::Dimension,
            &["Miền Nam", "An"],
        ),
    ];
    let v = new_version(store, ws, dataset, hash, &columns).await;
    set_value_aliases(store, v.id, "region", south, &["miền Nam"]).await;
    set_value_aliases(store, v.id, "region", "North", &["miền Bắc"]).await;
    set_value_aliases(store, v.id, "customer_name", "An", &["khách quen"]).await;
    set_column(store, v.id, "customer_name", "is_sensitive = true").await;
    v
}

#[tokio::test]
async fn context_reads_one_version_of_one_workspace() {
    let Some(store) = store().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let state = app_state(&store);
    let ws = temp_workspace(&store).await;
    let src = store.create_local_source(&ws).await.unwrap();
    let ds = store
        .create_dataset(&ws, src, "Sales", "sales", None)
        .await
        .unwrap();
    let v1 = small_version(&store, &ws, ds.id, "h1", "South").await;

    // Danh sách giá trị của cột chiều, nhiều dòng nhất trước, kèm bí danh; cột số và cột nhạy cảm không có.
    let context = ask(&state, &ws, &v1, "doanh thu theo miền").await;
    assert_eq!(
        names(&context),
        ["region", "country", "amount", "order_date", "customer_name"]
    );
    assert_eq!(
        column_values(&context, "region"),
        ["South", "North", "Central"]
    );
    assert_eq!(column(&context, "region").values[0].aliases, ["miền Nam"]);
    assert!(column(&context, "region").values_complete);
    assert!(column(&context, "amount").values.is_empty());
    let sensitive = column(&context, "customer_name");
    assert!(sensitive.is_sensitive && sensitive.values.is_empty());
    assert!(context.hints.is_empty());

    // Gợi ý khớp chính xác: có dấu khớp bí danh; cột nhạy cảm (giá trị "Miền Nam", bí danh "khách quen") không có.
    assert_eq!(
        hints(&ask(&state, &ws, &v1, "Doanh thu Miền Nam").await),
        [("region", "South")]
    );
    for question in [
        "doanh thu khách quen",
        "doanh thu từng miền năm 2017",
        "doanh thu năm 2017",
    ] {
        let context = ask(&state, &ws, &v1, question).await;
        assert!(context.hints.is_empty(), "{question}");
    }

    // Câu toàn dấu câu, câu rất dài, câu không khớp gì, một từ rất dài: vẫn có cả schema, không lỗi.
    let huge = format!("doanh thu miền nam {}", "x".repeat(100_000));
    for question in [
        "?!...",
        &"doanh thu miền nam ".repeat(300),
        "xin chào",
        &huge,
    ] {
        let context = ask(&state, &ws, &v1, question).await;
        assert_eq!(context.columns.len(), 5);
    }

    // Phiên bản khác của cùng dataset có giá trị khác: mỗi phiên bản chỉ thấy giá trị của nó.
    let v2 = small_version(&store, &ws, ds.id, "h2", "Southeast").await;
    let context = ask(&state, &ws, &v2, "miền nam").await;
    assert_eq!(column_values(&context, "region")[0], "Southeast");
    assert_eq!(hints(&context), [("region", "Southeast")]);
    let context = ask(&state, &ws, &v1, "miền nam").await;
    assert_eq!(column_values(&context, "region")[0], "South");

    // Workspace khác ra rỗng, kể cả khi biết đúng id phiên bản.
    let other = temp_workspace(&store).await;
    let rows = |ws| store.list_value_rows(ws, v1.id, Duration::from_secs(2));
    assert!(rows(&other).await.unwrap().unwrap().is_empty());
    assert!(!rows(&ws).await.unwrap().unwrap().is_empty());
    assert!(matches!(
        build_ask_context(&state, &other, &v1, "miền nam", &[]).await,
        Err(AppError::BadRequest(_))
    ));

    // Phiên bản chưa lập hồ sơ, và phiên bản lập hồ sơ lỗi.
    let pending = new_version(&store, &ws, ds.id, "h3", &[]).await;
    let err = build_ask_context(&state, &ws, &pending, "miền nam", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::BadRequest(m) if m.contains("chưa lập hồ sơ xong")),
        "{err:?}"
    );
    let mut failed = new_version(&store, &ws, ds.id, "h4", &[]).await;
    store
        .set_profile_status(&ws, failed.id, ProfileStatus::Failed)
        .await
        .unwrap();
    failed.profile_status = ProfileStatus::Failed.as_str().to_string();
    let err = build_ask_context(&state, &ws, &failed, "miền nam", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::BadRequest(m) if m.contains("Không lập được hồ sơ")),
        "{err:?}"
    );

    drop_workspace(&store, ws).await;
    drop_workspace(&store, other).await;
}

/// Một cột của hồ sơ dataco trong mock_answers.json.
#[derive(serde::Deserialize)]
struct SampleColumn {
    name: String,
    description: String,
    aliases: Vec<String>,
}

#[derive(serde::Deserialize)]
struct SampleValue {
    column: String,
    value: String,
    aliases: Vec<String>,
}

#[derive(serde::Deserialize)]
struct SampleProfile {
    columns: Vec<SampleColumn>,
    values: Vec<SampleValue>,
}

fn dataco_profile() -> SampleProfile {
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../engine/llm/mock_answers.json")).unwrap();
    let answer = entries
        .into_iter()
        .find(|e| e["task"] == "profile" && e["question"] == "dataco")
        .expect("dataco profile in mock_answers.json")["answer"]
        .clone();
    serde_json::from_value(answer).unwrap()
}

/// 23 khu vực của DataCo thật, nhiều dòng nhất trước.
const DATACO_REGIONS: [&str; 23] = [
    "Central America",
    "Western Europe",
    "South America",
    "Oceania",
    "Northern Europe",
    "Southeast Asia",
    "Southern Europe",
    "Caribbean",
    "West of USA ",
    "South Asia",
    "Eastern Asia",
    "East of USA",
    "West Asia",
    "US Center ",
    "South of  USA ",
    "Eastern Europe",
    "West Africa",
    "North Africa",
    "East Africa",
    "Central Africa",
    "Southern Africa",
    "Canada",
    "Central Asia",
];

/// Giá trị của các cột chiều: Market, Delivery Status, Shipping Mode, Customer Segment lấy từ hồ sơ mẫu (đủ như DataCo
/// thật); Order Region đủ 23 khu vực thật; Category Name 50 giá trị và Order Country 164 giá trị như DataCo thật (tên
/// sinh ra, chỉ số lượng là thật).
fn dataco_values(column: &str, profile: &SampleProfile) -> Vec<String> {
    let mut list: Vec<String> = profile
        .values
        .iter()
        .filter(|v| v.column == column)
        .map(|v| v.value.clone())
        .collect();
    let generated = |prefix: &str, n: usize| -> Vec<String> {
        (1..=n).map(|i| format!("{prefix} {i:03}")).collect()
    };
    match column {
        "Order Region" => list.extend(DATACO_REGIONS.iter().map(|r| r.to_string())),
        "Category Name" => list.extend(generated("Category", 50)),
        "Order Country" => list.extend(generated("Country", 164)),
        "Customer Email" | "Customer Password" => list.push("XXXXXXXXX".to_string()),
        _ => {}
    }
    list
}

fn dataco_role(name: &str, values: &[String]) -> ProfileRole {
    if name.ends_with("(DateOrders)") {
        ProfileRole::Time
    } else if !values.is_empty() {
        ProfileRole::Dimension
    } else {
        ProfileRole::Measure
    }
}

async fn dataco_version(store: &Store, ws: &WorkspaceId) -> DatasetVersion {
    let profile = dataco_profile();
    let src = store.create_local_source(ws).await.unwrap();
    let ds = store
        .create_dataset(ws, src, "DataCo", "dataco", None)
        .await
        .unwrap();
    let columns: Vec<NewColumn> = profile
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let values = dataco_values(&c.name, &profile);
            let refs: Vec<&str> = values.iter().map(String::as_str).collect();
            new_column(i as i32, &c.name, dataco_role(&c.name, &values), &refs)
        })
        .collect();
    let v = new_version(store, ws, ds.id, "dataco", &columns).await;
    for c in &profile.columns {
        let aliases: Vec<&str> = c.aliases.iter().map(String::as_str).collect();
        let sets = format!(
            "description = {}, aliases = {}",
            quote(&c.description),
            text_array(&aliases)
        );
        set_column(store, v.id, &c.name, &sets).await;
    }
    for value in &profile.values {
        let aliases: Vec<&str> = value.aliases.iter().map(String::as_str).collect();
        set_value_aliases(store, v.id, &value.column, &value.value, &aliases).await;
    }
    for sensitive in ["Customer Email", "Customer Password"] {
        set_column(store, v.id, sensitive, "is_sensitive = true").await;
    }
    v
}

#[tokio::test]
async fn dataco_sample_sends_group_values_and_exact_hints() {
    let Some(store) = store().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let state = app_state(&store);
    let ws = temp_workspace(&store).await;
    let v = dataco_version(&store, &ws).await;

    // 53 cột, dưới ngưỡng: cả schema; cột nhóm có đủ giá trị, cột 164 nước gửi 20 giá trị nhiều dòng nhất.
    let context = ask(&state, &ws, &v, "Doanh thu theo thị trường").await;
    assert_eq!(context.columns.len(), 53);
    for (name, count) in [
        ("Market", 5),
        ("Order Region", 23),
        ("Customer Segment", 3),
        ("Shipping Mode", 4),
        ("Delivery Status", 4),
        ("Category Name", 50),
    ] {
        let c = column(&context, name);
        assert_eq!(c.values.len(), count, "{name}");
        assert!(c.values_complete, "{name}");
    }
    let market = column(&context, "Market");
    assert_eq!(market.values[0].value, "Europe");
    assert_eq!(market.values[0].aliases, ["châu Âu"]);
    let country = column(&context, "Order Country");
    assert_eq!(country.values.len(), 20);
    assert!(!country.values_complete);
    for name in ["Customer Email", "Customer Password"] {
        assert!(column(&context, name).values.is_empty(), "{name}");
    }
    let sent: usize = context.columns.iter().map(|c| c.values.len()).sum();
    assert!(sent <= CONTEXT_VALUES_MAX);

    let column_of = |context: &AskContext, phrase: &str| {
        context
            .picks
            .iter()
            .find(|p| p.phrase == phrase)
            .map(|p| p.column.clone())
    };
    let context = ask(
        &state,
        &ws,
        &v,
        "5 danh mục có doanh thu cao nhất ở châu Âu",
    )
    .await;
    assert_eq!(
        column_of(&context, "danh mục").as_deref(),
        Some("Category Name")
    );
    assert_eq!(
        column_of(&context, "doanh thu").as_deref(),
        Some("Order Item Total")
    );
    assert_eq!(hints(&context), [("Market", "Europe")]);
    assert!(context.ambiguous.is_empty());

    // Khớp chính xác ra gợi ý đúng.
    for (question, expected) in [
        (
            "doanh thu Southern Europe năm 2017",
            ("Order Region", "Southern Europe"),
        ),
        (
            "số đơn Standard Class theo tháng",
            ("Shipping Mode", "Standard Class"),
        ),
        ("doanh thu chau au", ("Market", "Europe")),
    ] {
        assert_eq!(
            hints(&ask(&state, &ws, &v, question).await),
            [expected],
            "{question}"
        );
    }
    // Không có ánh xạ nào do luật tự đoán.
    for question in [
        "doanh thu từng miền năm 2017",
        "doanh thu trung bình",
        "doanh thu trên 5 triệu đồng",
        "doanh thu khách lẻ",
        "doanh thu Việt Nam",
        "doanh thu Quảng Nam",
        "doanh thu châu Á",
        "số đơn giao hàng tiêu chuẩn",
    ] {
        let context = ask(&state, &ws, &v, question).await;
        assert!(context.hints.is_empty(), "{question}: {:?}", context.hints);
    }

    // "khu vực" chỉ là bí danh của Order Region; thêm vào Market thì mơ hồ, chọn rồi thì hết.
    let context = ask(&state, &ws, &v, "Doanh thu theo khu vực").await;
    assert_eq!(
        column_of(&context, "khu vực").as_deref(),
        Some("Order Region")
    );
    set_column(
        &store,
        v.id,
        "Market",
        "aliases = ARRAY['thị trường', 'châu lục', 'khu vực']",
    )
    .await;
    let context = ask(&state, &ws, &v, "Doanh thu theo khu vực").await;
    assert_eq!(context.ambiguous.len(), 1);
    assert_eq!(context.ambiguous[0].columns, ["Market", "Order Region"]);
    let choice = chosen(&context.ambiguous[0].choice_id, "Order Region");
    let context = build_ask_context(&state, &ws, &v, "Doanh thu theo khu vực", &[choice])
        .await
        .unwrap();
    assert!(context.ambiguous.is_empty());
    assert_eq!(
        column_of(&context, "khu vực").as_deref(),
        Some("Order Region")
    );

    drop_workspace(&store, ws).await;
}

/// Câu hỏi sau dùng giá trị đã cache; cột vừa đánh dấu nhạy cảm không lộ giá trị qua cache cũ.
#[tokio::test]
async fn cached_values_respect_current_sensitivity() {
    let Some(store) = store().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let state = app_state(&store);
    let ws = temp_workspace(&store).await;
    let src = store.create_local_source(&ws).await.unwrap();
    let ds = store
        .create_dataset(&ws, src, "VN", "vn", None)
        .await
        .unwrap();
    let columns = [new_column(
        0,
        "tinh_thanh",
        ProfileRole::Dimension,
        &["Ha Noi", "Da Nang"],
    )];
    let v = new_version(&store, &ws, ds.id, "vn", &columns).await;
    assert_eq!(
        hints(&ask(&state, &ws, &v, "doanh thu Hà Nội").await),
        [("tinh_thanh", "Ha Noi")]
    );
    assert_eq!(state.value_cache.len(), 1);
    // Xóa thẳng trong database mà không qua cache: câu sau vẫn thấy giá trị cũ, tức là đã đọc từ cache.
    exec(
        &store,
        &format!(
            "DELETE FROM column_values WHERE value = 'Da Nang' AND workspace_id = '{}'",
            ws.as_uuid()
        ),
    )
    .await;
    let context = ask(&state, &ws, &v, "doanh thu Đà Nẵng").await;
    assert_eq!(hints(&context), [("tinh_thanh", "Da Nang")]);
    assert_eq!(state.value_cache.len(), 1);
    set_column(&store, v.id, "tinh_thanh", "is_sensitive = true").await;
    let context = ask(&state, &ws, &v, "doanh thu Hà Nội").await;
    assert!(context.hints.is_empty());
    assert!(column(&context, "tinh_thanh").values.is_empty());

    drop_workspace(&store, ws).await;
}

/// Tải giá trị quá thời gian thì trả None, không lỗi: câu hỏi vẫn đi tiếp, không có danh sách giá trị.
#[tokio::test]
async fn value_loading_times_out_to_no_values() {
    let Some(store) = store().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let ws = temp_workspace(&store).await;
    let src = store.create_local_source(&ws).await.unwrap();
    let ds = store
        .create_dataset(&ws, src, "Wide", "wide", None)
        .await
        .unwrap();
    let values: Vec<String> = (0..200).map(|i| format!("value number {i}")).collect();
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    let columns: Vec<NewColumn> = (0..100)
        .map(|i| new_column(i, &format!("c{i}"), ProfileRole::Dimension, &refs))
        .collect();
    let v = new_version(&store, &ws, ds.id, "wide", &columns).await;
    let rows = store
        .list_value_rows(&ws, v.id, Duration::from_millis(1))
        .await
        .unwrap();
    assert!(rows.is_none());
    let rows = store
        .list_value_rows(&ws, v.id, Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(rows.unwrap().len(), 100 * 200);

    drop_workspace(&store, ws).await;
}

/// Người hỏi chọn cột cho một cụm chỉ có một ứng viên: vẫn ghi là lựa chọn của họ.
#[test]
fn a_choice_for_a_single_candidate_counts_as_applied() {
    let columns = vec![profile("Order Region", "dimension", &["khu vực"])];
    let context = build_context(
        &build_phrases("Doanh thu theo khu vực"),
        columns,
        &ValueIndex::default(),
        &[chosen("column:khu vuc", "Order Region")],
    );
    assert_eq!(context.picks.len(), 1);
    assert_eq!(context.picks[0].column, "Order Region");
    assert!(context.picks[0].chosen_by_user);
}

/// Cột nhiều giá trị chỉ gửi 20 giá trị nhiều dòng nhất, nhưng gợi ý vẫn có cho giá trị nằm ngoài 20 giá trị đó.
#[test]
fn hints_reach_values_beyond_the_sample_of_a_large_column() {
    let columns = vec![profile("Order Country", "dimension", &[])];
    let names: Vec<String> = (1..=60).map(|i| format!("Country {i:03}")).collect();
    let mut rows: Vec<(&str, &str, &[&str])> = names
        .iter()
        .map(|n| ("Order Country", n.as_str(), &[][..]))
        .collect();
    rows.push(("Order Country", "Puerto Rico", &[]));
    let context = build_context(
        &build_phrases("doanh thu Puerto Rico"),
        columns,
        &index(&rows),
        &[],
    );
    let country = column(&context, "Order Country");
    assert_eq!(country.values.len(), 20);
    assert!(!country.values_complete);
    assert!(!column_values(&context, "Order Country").contains(&"Puerto Rico".to_string()));
    assert_eq!(hints(&context), [("Order Country", "Puerto Rico")]);
}

/// values_complete: false khi một giá trị bị bỏ vì quá dài; true với cột chiều không có giá trị nào, trừ khi giá trị
/// chưa tải được (tải quá thời gian).
#[test]
fn values_complete_follows_dropped_values_and_loading() {
    let columns = vec![
        profile("note", "dimension", &[]),
        profile("empty", "dimension", &[]),
        profile("region", "dimension", &[]),
    ];
    let long = "x".repeat(300);
    let values = index(&[
        ("note", "short", &[]),
        ("note", long.as_str(), &[]),
        ("region", "South", &[]),
    ]);
    let context = build_context(&build_phrases("xin chào"), columns.clone(), &values, &[]);
    assert_eq!(column_values(&context, "note"), ["short"]);
    assert!(!column(&context, "note").values_complete);
    assert!(column(&context, "empty").values_complete);
    assert!(column(&context, "region").values_complete);
    let not_loaded = build_context(
        &build_phrases("xin chào"),
        columns,
        &ValueIndex::default(),
        &[],
    );
    assert!(not_loaded.columns.iter().all(|c| !c.values_complete));
}

/// Đo phần giá trị gửi cho mô hình trên một file DataCo thật, qua đúng đường nạp (materialize, lập hồ sơ). Không chạy
/// mặc định vì file không có trong repo (data/README.md). Chạy:
/// `DATACO_CSV=<đường dẫn DataCoSupplyChainDataset.csv> DATABASE_URL=... cargo test measure_dataco_values -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn measure_dataco_values() {
    use crate::engine::analytics::{materialize, FileFormat, InputFile};
    let (Some(store), Ok(csv)) = (store().await, std::env::var("DATACO_CSV")) else {
        panic!("set DATABASE_URL and DATACO_CSV");
    };
    let dir = std::env::temp_dir().join(format!("xdi-measure-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = InputFile {
        path: csv.into(),
        name: "DataCoSupplyChainDataset.csv".to_string(),
        format: FileFormat::Csv,
        timestamp_columns: Vec::new(),
    };
    let materialized = materialize(&[input], &dir).unwrap();
    let columns =
        super::profile::build_profile(dir.clone(), materialized.row_count, Default::default())
            .await
            .unwrap();
    let state = app_state(&store);
    let ws = temp_workspace(&store).await;
    let src = store.create_local_source(&ws).await.unwrap();
    let ds = store
        .create_dataset(&ws, src, "DataCo", "dataco", None)
        .await
        .unwrap();
    let v = new_version(&store, &ws, ds.id, "dataco-real", &columns).await;
    for name in ["Customer Email", "Customer Password"] {
        set_column(&store, v.id, name, "is_sensitive = true").await;
    }
    let context = ask(&state, &ws, &v, "Doanh thu theo thị trường").await;
    let mut block = String::new();
    for c in context.columns.iter().filter(|c| !c.values.is_empty()) {
        let values: Vec<&str> = c.values.iter().map(|v| v.value.as_str()).collect();
        block.push_str(&format!("\"{}\": {}\n", c.name, values.join(" | ")));
    }
    println!("{block}");
    let image = block
        .lines()
        .find(|l| l.starts_with("\"Product Image\""))
        .map_or(0, |l| l.chars().count());
    println!(
        "rows={} columns_with_values={} values={} chars={} product_image_chars={}",
        materialized.row_count,
        context
            .columns
            .iter()
            .filter(|c| !c.values.is_empty())
            .count(),
        context
            .columns
            .iter()
            .map(|c| c.values.len())
            .sum::<usize>(),
        block.chars().count(),
        image
    );
    drop_workspace(&store, ws).await;
    std::fs::remove_dir_all(&dir).unwrap();
}
