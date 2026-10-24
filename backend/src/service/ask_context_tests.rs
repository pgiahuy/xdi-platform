// SPDX-License-Identifier: Apache-2.0
/* service/ask_context_tests.rs - test của service/ask_context.rs: phần thuần (chọn cột, mơ hồ, bảng rộng) không cần
database; phần tìm giá trị chạy trên Postgres thật, bỏ qua khi thiếu DATABASE_URL như mọi test store. Bí danh, mô tả và
cờ nhạy cảm do #23 ghi, chưa có code ghi, nên test tự UPDATE bằng SQL. Dataset mẫu là hồ sơ viết sẵn của dataco trong
engine/llm/mock_answers.json (53 cột, bí danh tiếng Việt), cộng vài giá trị của DataCo thật. */

use uuid::Uuid;

use super::ask_context::*;
use super::ask_contract::ChosenOption;
use super::ask_phrases::build_phrases;
use crate::auth::WorkspaceId;
use crate::engine::store::test_support::{drop_workspace, exec, store, temp_workspace};
use crate::engine::store::{
    DatasetVersion, NewColumn, NewColumnValue, NewVersion, ProfileRole, SourceKind, Store, ValueHit,
};
use crate::error::AppError;
use crate::state::AppState;

fn profile(name: &str, role: &str, aliases: &[&str]) -> ColumnProfile {
    ColumnProfile {
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role: role.to_string(),
        time_format: None,
        description: None,
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
        sample_values: Vec::new(),
        is_sensitive: false,
        matched: false,
    }
}

fn hit(phrase: &str, column: &str, value: &str, matched_text: &str, similarity: f32) -> ValueHit {
    ValueHit {
        phrase: phrase.to_string(),
        column_name: column.to_string(),
        value: value.to_string(),
        matched_text: matched_text.to_string(),
        same_folded: similarity == 1.0,
        similarity,
    }
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

fn values(context: &AskContext) -> Vec<(&str, &str)> {
    context
        .values
        .iter()
        .map(|v| (v.column.as_str(), v.value.as_str()))
        .collect()
}

#[test]
fn close_columns_are_ambiguous_until_the_user_picks() {
    let columns = vec![
        profile("Market", "dimension", &["thị trường", "khu vực"]),
        profile("Order Region", "dimension", &["khu vực"]),
        profile("Sales", "measure", &[]),
    ];
    let phrases = build_phrases("Doanh thu theo khu vực");
    let context = build_context(&phrases, columns.clone(), &[], &[]);
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
        &[],
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
        let context = build_context(&phrases, columns.clone(), &[], &[wrong]);
        assert_eq!(context.ambiguous.len(), 1);
    }
}

/// Khớp có dấu đúng nguyên văn hơn khớp sau khi bỏ dấu đủ CLOSE_SCORE: chọn luôn, ghi thành giả định có cột khác.
#[test]
fn exact_accents_beat_folded_matches_without_asking() {
    let columns = vec![
        profile("Market", "dimension", &["khu vuc"]),
        profile("Order Region", "dimension", &["Khu vực"]),
    ];
    let context = build_context(&build_phrases("doanh thu theo khu vực"), columns, &[], &[]);
    assert!(context.ambiguous.is_empty());
    assert_eq!(context.picks.len(), 1);
    assert_eq!(context.picks[0].column, "Order Region");
    assert_eq!(context.picks[0].others, ["Market"]);
    assert!(!context.picks[0].chosen_by_user);
}

/// Giá trị khớp bằng nhau ở một cột và khớp gần ở cột khác: chọn cột khớp bằng nhau, chỉ giữ giá trị của cột đó.
#[test]
fn value_matches_in_two_columns_pick_the_better_one() {
    let columns = vec![
        profile("Market", "dimension", &[]),
        profile("Order Region", "dimension", &[]),
    ];
    let hits = [
        hit("europe", "Market", "Europe", "Europe", 1.0),
        hit(
            "europe",
            "Order Region",
            "Western Europe",
            "Western Europe",
            0.6,
        ),
    ];
    let context = build_context(&build_phrases("doanh thu europe"), columns, &hits, &[]);
    assert_eq!(values(&context), [("Market", "Europe")]);
    assert_eq!(context.values[0].score, 1.0);
    assert_eq!(context.picks[0].column, "Market");
    assert_eq!(context.picks[0].others, ["Order Region"]);
}

#[test]
fn wide_tables_keep_matched_and_time_columns_only() {
    let mut columns: Vec<ColumnProfile> = (0..70)
        .map(|i| profile(&format!("Metric {i}"), "measure", &[]))
        .collect();
    columns.push(profile("order date (DateOrders)", "time", &[]));
    columns.push(profile("Category Name", "dimension", &["danh mục"]));
    columns.push(profile("Category Id", "key", &["mã danh mục"]));
    columns.push(profile("Market", "dimension", &[]));
    columns.push(profile("Order Region", "dimension", &[]));
    let hits = [hit("châu âu", "Market", "Europe", "châu Âu", 1.0)];

    let context = build_context(
        &build_phrases("Doanh thu theo danh mục ở châu Âu, từng region"),
        columns.clone(),
        &hits,
        &[],
    );
    // Category Id chỉ chứa "danh mục" trong bí danh dài hơn, mà "danh mục" đã khớp đúng Category Name, nên không giữ.
    // "region" không bằng tên cột nào, nhưng là một từ của Order Region, nên cột vẫn được giữ.
    assert_eq!(
        names(&context),
        [
            "order date (DateOrders)",
            "Category Name",
            "Market",
            "Order Region"
        ]
    );
    assert!(context
        .columns
        .iter()
        .all(|c| c.matched || c.role == "time"));
    assert_eq!(values(&context), [("Market", "Europe")]);
    // Chỉ một cột ứng viên mỗi cụm: không có giả định nào cần hiện.
    assert!(context.picks.iter().all(|p| p.others.is_empty()));

    // Không cụm nào khớp: gửi cả schema thay vì chỉ cột thời gian.
    let none = build_context(&build_phrases("xin chào"), columns.clone(), &[], &[]);
    assert_eq!(none.columns.len(), columns.len());
}

// ---- Postgres ----

fn new_column(position: i32, name: &str, role: ProfileRole, values: &[&str]) -> NewColumn {
    NewColumn {
        position,
        name: name.to_string(),
        data_type: "VARCHAR".to_string(),
        role,
        null_count: 0,
        distinct_count: values.len() as i64,
        min_value: None,
        max_value: None,
        time_format: None,
        // Tần suất giảm dần theo thứ tự truyền vào, để thứ tự giá trị mẫu biết trước.
        values: values
            .iter()
            .enumerate()
            .map(|(i, v)| NewColumnValue {
                value: v.to_string(),
                frequency: 100 - i as i64,
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

/// Bảng nhỏ có cột miền (bí danh tiếng Việt), cột nước có "Việt Nam" và một cột nhạy cảm chứa đúng chữ "Miền Nam".
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
        new_column(2, "amount", ProfileRole::Measure, &[]),
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
    set_value_aliases(store, v.id, "region", "Central", &["miền Trung"]).await;
    set_column(store, v.id, "customer_name", "is_sensitive = true").await;
    v
}

#[tokio::test]
async fn vietnamese_spellings_map_to_the_value_and_time_words_do_not() {
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

    for question in [
        "doanh thu mien nam",
        "Doanh thu Miền Nam",
        "DOANH THU MIỀN NAM",
        "doanh thu mien nma",
    ] {
        let context = ask(&state, &ws, &v1, question).await;
        assert_eq!(values(&context), [("region", "South")], "{question}");
        assert_eq!(context.values[0].matched_text, "miền Nam", "{question}");
        assert!(context.ambiguous.is_empty(), "{question}");
    }
    let accented = ask(&state, &ws, &v1, "Doanh thu Miền Nam").await;
    let plain = ask(&state, &ws, &v1, "doanh thu mien nam").await;
    let typo = ask(&state, &ws, &v1, "doanh thu mien nma").await;
    assert!(accented.values[0].score > plain.values[0].score);
    assert!(plain.values[0].score > typo.values[0].score);

    // "năm" bỏ dấu là "nam", nhưng không bao giờ thành South hay Việt Nam.
    for question in [
        "doanh thu năm 2017",
        "doanh thu nam 2017",
        "Doanh thu tháng 5 năm 2017 theo quý",
    ] {
        let context = ask(&state, &ws, &v1, question).await;
        assert!(
            context.values.is_empty(),
            "{question}: {:?}",
            context.values
        );
        assert!(context.ambiguous.is_empty() && context.picks.is_empty());
    }
    let context = ask(&state, &ws, &v1, "Doanh thu Việt Nam năm 2017").await;
    assert_eq!(values(&context), [("country", "Việt Nam")]);

    // Cột nhạy cảm: có trong schema, không giá trị mẫu, không tìm giá trị (giá trị "Miền Nam" của nó không khớp).
    let context = ask(&state, &ws, &v1, "Miền Nam").await;
    assert_eq!(values(&context), [("region", "South")]);
    assert_eq!(
        names(&context),
        ["region", "country", "amount", "order_date", "customer_name"]
    );
    let sensitive = &context.columns[4];
    assert!(sensitive.is_sensitive && sensitive.sample_values.is_empty());
    assert_eq!(
        context.columns[0].sample_values,
        ["South", "North", "Central"]
    );

    // Câu toàn dấu câu, câu rất dài, câu không khớp gì: vẫn có cả schema, không lỗi.
    for question in ["?!...", &"doanh thu miền nam ".repeat(300), "xin chào"] {
        let context = ask(&state, &ws, &v1, question).await;
        assert_eq!(context.columns.len(), 5);
    }

    // Phiên bản khác của cùng dataset có giá trị khác: mỗi phiên bản chỉ thấy giá trị của nó.
    let v2 = small_version(&store, &ws, ds.id, "h2", "Southeast").await;
    assert_eq!(
        values(&ask(&state, &ws, &v2, "miền nam").await),
        [("region", "Southeast")]
    );
    assert_eq!(
        values(&ask(&state, &ws, &v1, "miền nam").await),
        [("region", "South")]
    );

    // Workspace khác ra rỗng, kể cả khi biết đúng id phiên bản.
    let other = temp_workspace(&store).await;
    let phrases = vec!["miền nam".to_string()];
    assert!(store
        .find_values(&other, v1.id, &phrases, &phrases, VALUE_MIN_SIMILARITY)
        .await
        .unwrap()
        .is_empty());
    assert!(!store
        .find_values(&ws, v1.id, &phrases, &[], VALUE_MIN_SIMILARITY)
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        build_ask_context(&state, &other, &v1, "miền nam", &[]).await,
        Err(AppError::BadRequest(_))
    ));

    // Phiên bản chưa lập hồ sơ.
    let pending = new_version(&store, &ws, ds.id, "h3", &[]).await;
    let err = build_ask_context(&state, &ws, &pending, "miền nam", &[])
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::BadRequest(m) if m.contains("chưa lập hồ sơ xong")),
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

/// Giá trị của DataCo thật cho vài cột chiều; Market và các cột có bí danh giá trị lấy từ hồ sơ mẫu.
fn dataco_values(column: &str, profile: &SampleProfile) -> Vec<String> {
    let extra: &[&str] = match column {
        "Order Region" => &[
            "Western Europe",
            "Central America",
            "South America",
            "Northern Europe",
            "Southern Europe",
            "Oceania",
            "Southeast Asia",
            "Caribbean",
            "West of USA ",
            "South Asia",
            "Eastern Asia",
            "East of USA",
            "US Center ",
            "Eastern Europe",
            "South of  USA ",
            "West Africa",
        ],
        "Order Country" => &[
            "Estados Unidos",
            "Francia",
            "México",
            "Alemania",
            "Vietnam",
            "Japón",
        ],
        "Category Name" => &[
            "Cleats",
            "Men's Footwear",
            "Women's Apparel",
            "Fishing",
            "Water Sports",
        ],
        "Customer Email" => &["XXXXXXXXX"],
        "Customer Password" => &["XXXXXXXXX"],
        _ => &[],
    };
    let mut list: Vec<String> = profile
        .values
        .iter()
        .filter(|v| v.column == column)
        .map(|v| v.value.clone())
        .collect();
    list.extend(extra.iter().map(|v| v.to_string()));
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
    set_value_aliases(store, v.id, "Order Country", "Vietnam", &["Việt Nam"]).await;
    for sensitive in ["Customer Email", "Customer Password"] {
        set_column(store, v.id, sensitive, "is_sensitive = true").await;
    }
    v
}

#[tokio::test]
async fn dataco_sample_maps_aliases_and_columns() {
    let Some(store) = store().await else {
        eprintln!("skip: DATABASE_URL not set");
        return;
    };
    let state = app_state(&store);
    let ws = temp_workspace(&store).await;
    let v = dataco_version(&store, &ws).await;

    // 53 cột, dưới ngưỡng: cả schema, theo thứ tự cột.
    let context = ask(
        &state,
        &ws,
        &v,
        "5 danh mục có doanh thu cao nhất ở châu Âu",
    )
    .await;
    assert_eq!(context.columns.len(), 53);
    assert_eq!(context.columns[0].name, "Type");
    assert_eq!(values(&context), [("Market", "Europe")]);
    assert_eq!(context.values[0].matched_text, "châu Âu");
    let column_of = |context: &AskContext, phrase: &str| {
        context
            .picks
            .iter()
            .find(|p| p.phrase == phrase)
            .map(|p| p.column.clone())
    };
    // "danh mục" là bí danh của Category Name, và chỉ nằm trong bí danh dài hơn của Category Id: chọn Category Name,
    // không hỏi lại.
    assert_eq!(
        column_of(&context, "danh mục").as_deref(),
        Some("Category Name")
    );
    assert_eq!(
        column_of(&context, "doanh thu").as_deref(),
        Some("Order Item Total")
    );
    assert!(context.ambiguous.is_empty());

    // Không dấu, và qua bí danh giá trị.
    let context = ask(&state, &ws, &v, "doanh thu chau au theo danh muc").await;
    assert_eq!(values(&context), [("Market", "Europe")]);
    assert_eq!(
        column_of(&context, "danh muc").as_deref(),
        Some("Category Name")
    );
    // Gõ sai, khớp gần; "châu Phi" không bị hiểu thành châu Âu.
    assert_eq!(
        values(&ask(&state, &ws, &v, "doanh thu Mỹ Latin").await),
        [("Market", "LATAM")]
    );
    assert_eq!(
        values(&ask(&state, &ws, &v, "doanh thu châu Phi").await),
        [("Market", "Africa")]
    );
    // "năm" không thành Vietnam (bí danh Việt Nam), "nam" không thành South Asia hay South of USA.
    let context = ask(&state, &ws, &v, "Doanh thu năm 2017 theo tháng").await;
    assert!(context.values.is_empty(), "{:?}", context.values);
    assert_eq!(
        values(&ask(&state, &ws, &v, "doanh thu Việt Nam").await),
        [("Order Country", "Vietnam")]
    );

    // Cột nhạy cảm không có giá trị mẫu, giá trị của nó không được tìm.
    let context = ask(&state, &ws, &v, "khách XXXXXXXXX").await;
    assert!(context.values.is_empty(), "{:?}", context.values);
    for c in context.columns.iter().filter(|c| c.is_sensitive) {
        assert!(c.sample_values.is_empty(), "{}", c.name);
    }
    let market = context.columns.iter().find(|c| c.name == "Market").unwrap();
    assert_eq!(market.sample_values[0], "Europe");

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
