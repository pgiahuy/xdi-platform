// SPDX-License-Identifier: Apache-2.0
/* service/eval.rs - bộ đánh giá câu hỏi mẫu (#37): nạp dữ liệu mẫu vào workspace tạm, hỏi từng câu của file ca, so
kết quả với SQL đúng viết tay (câu đúng cũng đi qua guard và chạy trên DuckDB như câu mô hình sinh). Lệnh
`xdi-backend eval` ở cli.rs gọi hàm run_eval.

Hai nguồn dữ liệu: một file (dataco.csv, một dataset), hoặc các database mẫu (SAMPLE_DATABASE_URL, #230) chụp đúng
như thiết lập lần đầu (service/samples_db.rs), mỗi database một nguồn, mỗi bảng một dataset kèm quan hệ. Với database, mỗi ca ghi trường dataset
(bảng logic người hỏi chọn), và SQL đúng được đọc mọi bảng của workspace.

So bằng tập dòng kết quả, không so chữ SQL: nhiều câu SQL khác nhau cùng trả lời đúng một câu hỏi. Một ca có thể có
nhiều expected_sql (câu mơ hồ kiểu "doanh thu trước hay sau giảm giá"), khớp một là đúng. Luật so ở
service/eval_grade.rs (#202). Ba trạng thái (Verdict): đạt, trượt, cần xem lại; câu cần xem lại (mô hình không trả lời
được, đáp án không chạy, hệ thống hỏi lại) không vào tỷ lệ đúng. Câu loại why_* chạy như câu thường là trượt.

Đo bài học (#228): có file bài học (`--lessons`) thì cả bộ chạy hai lượt trên cùng workspace tạm, lượt đầu không bài
nào, rồi tạo và bật các bài đó qua đúng đường admin dùng (service/lessons/), lượt sau có bài học. Báo cáo là của lượt
có bài học, kèm tỷ lệ đúng của lượt không bài và các câu đổi kết quả giữa hai lượt. Mock không đọc prompt nên hai lượt
luôn như nhau; số đo có nghĩa khi chạy với mô hình thật.

Chạy lặp (#202, `--repeat`): cả bộ chạy nhiều lượt trên cùng workspace tạm, chỉ chụp và lập hồ sơ một lần. Báo cáo gộp
mọi lượt (mỗi dòng của cases ghi số lượt), kèm tỷ lệ đúng từng lượt và các câu đổi kết quả giữa các lượt. Bảng theo
loại câu và khoảng tin cậy ở service/eval_report.rs.

Câu phải từ chối (must_refuse, #202): không có đáp án; hệ thống trả rejected là đạt, trả lời ra bảng là trượt, lỗi 400
là cần xem lại. Câu tiếp nối (follow_up_of): câu gốc hỏi trong một cuộc trò chuyện thật
(service/conversation/), câu tiếp nối nhận khối "Hội thoại trước" của cuộc đó nhưng không ghi thêm vào cuộc, để các cách
hỏi khác của cùng câu tiếp nối không thấy nhau. Câu gốc phải đứng trước trong file và được trả lời trong cùng lượt. */

use std::collections::HashMap;

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::auth::{self, Member};
use crate::engine::guard::{build_guarded_sql, AllowedTable};
use crate::engine::query::run_query;
use crate::engine::store::Dataset;
use crate::error::AppError;
use crate::sample_database::SampleDatabase;
use crate::service;
use crate::service::ask::contract::{AskResponse, AskStatus, CellValue};
use crate::service::ask::fanout::build_links;
use crate::service::ask::result::build_result_table;
use crate::service::ask::{ask, AskHistory};
use crate::service::conversation;
use crate::service::conversation::history::{build_history_section, HISTORY_TURNS};
use crate::service::eval_grade::build_eval_verdict;
use crate::service::eval_report::{build_kind_rows, build_unstable, KindRow, RunSummary};
use crate::service::lessons::contract::LessonRequest;
use crate::service::samples::{upload_local_file, SAMPLE_NAME};
use crate::service::samples_db::seed_database;
use crate::service::workspaces;
use crate::state::AppState;

/// Một câu đánh giá: câu hỏi, các câu SQL đúng viết tay, và có so thứ tự dòng không (top N, chuỗi thời gian thì có).
#[derive(Debug, Clone, Deserialize)]
pub struct EvalCase {
    pub id: String,
    /// Bảng logic của dataset để hỏi. Bỏ trống khi dữ liệu là một file (chỉ có một dataset).
    #[serde(default)]
    pub dataset: Option<String>,
    pub question: String,
    /// Các cách hỏi khác của cùng câu; chỉ hỏi khi chạy với --variants (service/eval_report.rs).
    #[serde(default)]
    pub variants: Vec<String>,
    pub expected_sql: Vec<String>,
    #[serde(default)]
    pub ordered: bool,
    /// Loại câu theo #202, ví dụ plain_simple, why_groups. Câu loại why_* phải được trả lời bằng đường giải thích.
    #[serde(default)]
    pub kind: Option<String>,
    /// Vị trí (từ 0) các cột của đáp án bắt buộc có trong kết quả; rỗng là mọi cột (service/eval_grade.rs).
    #[serde(default)]
    pub required_columns: Vec<usize>,
    /// Câu hệ thống phải từ chối (ghi dữ liệu, hỏi thứ dữ liệu không có); expected_sql rỗng.
    #[serde(default)]
    pub must_refuse: bool,
    /// Id câu gốc khi đây là câu tiếp nối trong cùng cuộc trò chuyện.
    #[serde(default)]
    pub follow_up_of: Option<String>,
}

impl EvalCase {
    fn is_why(&self) -> bool {
        self.kind.as_deref().is_some_and(|k| k.starts_with("why_"))
    }
}

/// Dữ liệu của một lần đánh giá.
pub enum EvalData<'a> {
    File(&'a Path),
    SampleDatabases(&'a [SampleDatabase]),
}

#[derive(Debug, Serialize)]
pub struct CaseOutcome {
    pub id: String,
    pub question: String,
    /// Loại câu của ca, để báo cáo theo loại.
    pub kind: Option<String>,
    /// Lượt chạy, từ 1; luôn là 1 khi không có --repeat.
    pub run: u32,
    /// verdict là pass. Giữ trường này để so hai lượt có và không có bài học như trước.
    pub ok: bool,
    pub verdict: Verdict,
    /// Ghi chú khi đạt (thừa cột, cột khác thứ tự); lý do khi trượt hay cần xem lại: trạng thái không phải answered,
    /// SQL sinh ra, chỗ kết quả lệch, lỗi của nhà cung cấp mô hình.
    pub detail: String,
}

/// Kết quả chấm một câu (#202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    /// Chưa chấm được: mô hình không trả lời (hết hạn mức, quá tải, thiếu khóa), đáp án không chạy, hay hệ thống hỏi lại
    /// để chọn cách hiểu. Không vào mẫu số của tỷ lệ đúng: một lần hết hạn mức không được kéo tỷ lệ xuống.
    Review,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub review: usize,
    /// passed / (passed + failed): câu cần xem lại không vào mẫu số.
    pub accuracy: f64,
    /// Tỷ lệ đúng theo loại câu, gộp mọi lượt.
    pub by_kind: Vec<KindRow>,
    /// Từng lượt và các câu đổi kết quả giữa các lượt; chỉ có khi chạy --repeat với hơn một lượt.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<RunSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unstable: Vec<String>,
    /// Câu đạt ở lần chạy trước (--previous) mà trượt ở lần này; cli.rs điền.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub regressions: Vec<String>,
    pub cases: Vec<CaseOutcome>,
    /// Chỉ có khi chạy với file bài học; vắng thì eval/result.json giữ đúng hình dạng cũ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lessons: Option<LessonsComparison>,
}

/// Một bài học trong file `--lessons`: chữ của bài, và có áp cho cả workspace không (mặc định chỉ dataset đang đánh giá).
#[derive(Debug, Deserialize)]
pub struct EvalLesson {
    pub text: String,
    #[serde(default)]
    pub workspace: bool,
}

/// So hai lượt: không bài học và có bài học.
#[derive(Debug, Serialize)]
pub struct LessonsComparison {
    pub lessons: usize,
    pub passed_without: usize,
    pub accuracy_without: f64,
    /// Id các câu đúng ở một lượt mà sai ở lượt kia.
    pub changed: Vec<String>,
}

/// Đọc file ca đánh giá. Lỗi đọc hay JSON hỏng là lỗi hạ tầng (mã thoát 2 ở cli), không phải câu trả lời sai.
pub fn load_cases(path: &Path) -> Result<Vec<EvalCase>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Đọc file bài học: mảng JSON [{"text": "...", "workspace": false}]. Lỗi là lỗi hạ tầng như load_cases.
pub fn load_lessons(path: &Path) -> Result<Vec<EvalLesson>, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))
}

/// Chạy cả bộ: tạo workspace tạm, nạp dữ liệu mẫu (file hay database mẫu), chờ hồ sơ ngữ nghĩa, hỏi từng câu rồi xóa
/// workspace tạm. lessons rỗng là repeat lượt (một lượt như trước khi repeat là 1); có bài thì hai lượt (đầu file),
/// cli.rs không cho dùng chung với repeat.
pub async fn run_eval(
    state: &AppState,
    data: EvalData<'_>,
    cases: &[EvalCase],
    lessons: &[EvalLesson],
    repeat: u32,
) -> Result<EvalReport, AppError> {
    let (member, datasets) = match data {
        EvalData::File(path) => {
            let (member, id) = prepare(state, path).await?;
            let dataset = state
                .store
                .get_dataset(
                    &member.workspace,
                    parse_uuid(&id).map_err(AppError::Internal)?,
                )
                .await?
                .ok_or_else(|| AppError::Internal("eval dataset vanished".to_string()))?;
            (member, vec![dataset])
        }
        EvalData::SampleDatabases(dbs) => prepare_database(state, dbs).await?,
    };
    let outcome = if lessons.is_empty() {
        run_repeats(state, &member, &datasets, cases, repeat).await
    } else {
        run_rounds(state, &member, &datasets, cases, lessons).await
    };
    let _ = workspaces::delete(state, &member).await;
    outcome
}

/// repeat lượt trên cùng workspace, gộp kết quả. Một lượt thì báo cáo như trước, không có runs và unstable.
async fn run_repeats(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    cases: &[EvalCase],
    repeat: u32,
) -> Result<EvalReport, AppError> {
    let mut all = Vec::new();
    let mut runs = Vec::new();
    for run in 1..=repeat.max(1) {
        let outcomes = run_cases(state, member, datasets, cases, run).await?;
        runs.push(RunSummary {
            passed: count(&outcomes, Verdict::Pass),
            failed: count(&outcomes, Verdict::Fail),
            review: count(&outcomes, Verdict::Review),
            accuracy: build_accuracy(&outcomes),
        });
        all.extend(outcomes);
    }
    let mut report = build_report(all, None);
    if runs.len() > 1 {
        report.unstable = build_unstable(&report.cases);
        report.runs = runs;
    }
    Ok(report)
}

/// Hai lượt, không và có bài học, trên các dataset của workspace tạm (một với file, mọi bảng đã chụp với database mẫu).
async fn run_rounds(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    cases: &[EvalCase],
    lessons: &[EvalLesson],
) -> Result<EvalReport, AppError> {
    let without = run_cases(state, member, datasets, cases, 1).await?;
    add_lessons(state, member, datasets, lessons).await?;
    let with = run_cases(state, member, datasets, cases, 1).await?;
    let changed = with
        .iter()
        .zip(&without)
        .filter(|(a, b)| a.verdict != b.verdict)
        .map(|(a, _)| a.id.clone())
        .collect();
    let comparison = LessonsComparison {
        lessons: lessons.len(),
        passed_without: count(&without, Verdict::Pass),
        accuracy_without: build_accuracy(&without),
        changed,
    };
    Ok(build_report(with, Some(comparison)))
}

fn build_report(cases: Vec<CaseOutcome>, lessons: Option<LessonsComparison>) -> EvalReport {
    EvalReport {
        total: cases.len(),
        passed: count(&cases, Verdict::Pass),
        failed: count(&cases, Verdict::Fail),
        review: count(&cases, Verdict::Review),
        accuracy: build_accuracy(&cases),
        by_kind: build_kind_rows(&cases),
        runs: Vec::new(),
        unstable: Vec::new(),
        regressions: Vec::new(),
        cases,
        lessons,
    }
}

fn count(cases: &[CaseOutcome], verdict: Verdict) -> usize {
    cases.iter().filter(|c| c.verdict == verdict).count()
}

fn build_accuracy(cases: &[CaseOutcome]) -> f64 {
    let passed = count(cases, Verdict::Pass);
    passed as f64 / (passed + count(cases, Verdict::Fail)).max(1) as f64
}

/// Tạo rồi bật từng bài, như admin làm ở màn Bài học; người chạy eval là admin của workspace tạm. Bài không áp cho cả
/// workspace thì gắn vào từng dataset đang đánh giá: với database mẫu là mỗi bảng đã chụp một bản, vì câu nào cũng có
/// thể hỏi trên bảng nào.
async fn add_lessons(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    lessons: &[EvalLesson],
) -> Result<(), AppError> {
    for lesson in lessons {
        let targets: Vec<Option<uuid::Uuid>> = if lesson.workspace {
            vec![None]
        } else {
            datasets.iter().map(|d| Some(d.id)).collect()
        };
        for dataset_id in targets {
            let request = LessonRequest {
                text: lesson.text.clone(),
                dataset_id,
                error_id: None,
            };
            let created = service::lessons::create_lesson(state, member, request).await?;
            service::lessons::set_enabled(state, member, &created.id.to_string(), true).await?;
        }
    }
    Ok(())
}

/// Tài khoản và workspace tạm cho lần đánh giá, với dataset mẫu đã nạp và hồ sơ ngữ nghĩa xong (mock thì có ngay).
/// Trả id của dataset để hỏi. Workspace bị xóa sau khi chạy xong (run_eval), kéo theo user rác trong bảng users:
/// chấp nhận được vì eval nhắm vào database thử, không phải bản cài thật.
pub(crate) async fn prepare(state: &AppState, data: &Path) -> Result<(Member, String), AppError> {
    let email = format!("eval-{}@example.com", uuid::Uuid::new_v4());
    let user = state.store.create_user(&email, "Eval", "unused").await?;
    let membership = workspaces::create(state, &user, "Eval").await?;
    let member = auth::member_of(&state.store, user, &membership.slug).await?;

    let result = upload_local_file(state, &member, data, SAMPLE_NAME).await?;
    wait_semantic(state, &member).await?;
    Ok((member, result.dataset.id.to_string()))
}

/// Như prepare, nhưng dữ liệu là các database mẫu chụp qua đúng hàm của thiết lập lần đầu, mỗi database một nguồn.
/// Trả mọi dataset đã chụp.
async fn prepare_database(
    state: &AppState,
    dbs: &[SampleDatabase],
) -> Result<(Member, Vec<Dataset>), AppError> {
    let email = format!("eval-{}@example.com", uuid::Uuid::new_v4());
    let user = state.store.create_user(&email, "Eval", "unused").await?;
    let membership = workspaces::create(state, &user, "Eval").await?;
    let member = auth::member_of(&state.store, user, &membership.slug).await?;
    // Khác thiết lập lần đầu: một database lỗi là cả lần đánh giá lỗi, để không chấm trên một nửa dữ liệu mà không
    // biết.
    for db in dbs {
        let seeded = match seed_database(state, &member, db).await {
            Ok(seeded) => seeded,
            Err(e) => {
                let _ = workspaces::delete(state, &member).await;
                return Err(e);
            }
        };
        if seeded.is_none() {
            let _ = workspaces::delete(state, &member).await;
            return Err(AppError::Internal(format!(
                "eval workspace already has the sample source {}",
                db.database
            )));
        }
    }
    // Lỗi sau khi đã tạo workspace tạm thì xóa nó luôn, như khi chụp lỗi ở trên: run_eval chỉ xóa khi bước này xong.
    let ready = match wait_semantic(state, &member).await {
        Ok(()) => state.store.list_datasets(&member.workspace).await,
        Err(e) => Err(e),
    };
    match ready {
        Ok(datasets) => Ok((member, datasets)),
        Err(e) => {
            let _ = workspaces::delete(state, &member).await;
            Err(e)
        }
    }
}

/// Thời gian tối đa không có dataset nào xong hồ sơ ngữ nghĩa thêm, trước khi báo lỗi: đủ cho ba lần chờ hết hạn mức
/// 60 giây của service/eval_pace.rs cộng các lần gọi.
const SEMANTIC_STALL: Duration = Duration::from_secs(240);

/// Hồ sơ ngữ nghĩa chạy nền; hỏi khi chưa xong thì ngữ cảnh thiếu bí danh và câu gợi ý. Chờ mọi dataset của workspace.
/// Đếm thời gian từ lần cuối có dataset xong, không đếm từ đầu: có giãn cách lần gọi (--pause-ms, #202) thì 12 bảng của
/// database mẫu mất vài phút, trần cố định sẽ báo lỗi dù hồ sơ vẫn đang xong dần.
async fn wait_semantic(state: &AppState, member: &Member) -> Result<(), AppError> {
    let mut done = 0;
    let mut last_progress = std::time::Instant::now();
    loop {
        let datasets = state.store.list_datasets(&member.workspace).await?;
        let now_done = datasets
            .iter()
            .filter(|d| d.current_profile_status.as_deref() == Some("semantic"))
            .count();
        if now_done == datasets.len() {
            return Ok(());
        }
        if now_done > done {
            done = now_done;
            last_progress = std::time::Instant::now();
        }
        if last_progress.elapsed() > SEMANTIC_STALL {
            return Err(AppError::Internal(format!(
                "hồ sơ ngữ nghĩa của dữ liệu mẫu đứng ở {done}/{} dataset suốt {} giây",
                datasets.len(),
                SEMANTIC_STALL.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn run_cases(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    cases: &[EvalCase],
    run: u32,
) -> Result<Vec<CaseOutcome>, AppError> {
    let mut outcomes = Vec::new();
    // Cuộc trò chuyện của từng câu đã trả lời trong lượt này, để câu tiếp nối đọc lại.
    let mut conversations = HashMap::new();
    for case in cases {
        outcomes.push(run_case(state, member, datasets, case, run, &mut conversations).await);
    }
    Ok(outcomes)
}

/// Dataset để hỏi: theo trường dataset của ca, hay dataset duy nhất khi dữ liệu là một file.
fn pick_dataset<'a>(datasets: &'a [Dataset], case: &EvalCase) -> Result<&'a Dataset, String> {
    match &case.dataset {
        Some(table) => datasets
            .iter()
            .find(|d| &d.logical_table == table)
            .ok_or_else(|| format!("không có dataset với bảng {table}")),
        None if datasets.len() == 1 => Ok(&datasets[0]),
        None => Err("ca thiếu trường dataset trong khi có nhiều dataset".to_string()),
    }
}

async fn run_case(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    case: &EvalCase,
    run: u32,
    conversations: &mut HashMap<String, uuid::Uuid>,
) -> CaseOutcome {
    let (verdict, detail) = grade_case(state, member, datasets, case, conversations).await;
    CaseOutcome {
        id: case.id.clone(),
        question: case.question.clone(),
        kind: case.kind.clone(),
        run,
        ok: verdict == Verdict::Pass,
        verdict,
        detail,
    }
}

async fn grade_case(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    case: &EvalCase,
    conversations: &mut HashMap<String, uuid::Uuid>,
) -> (Verdict, String) {
    // Ca trỏ tới bảng không có, mọi expected_sql đều hỏng, hay câu gốc của câu tiếp nối chưa được trả lời: lỗi của bộ
    // câu hay của câu trước, không phải mô hình trả lời sai câu này.
    let dataset_id = match pick_dataset(datasets, case) {
        Ok(d) => d.id.to_string(),
        Err(why) => return (Verdict::Review, why),
    };
    let expected = if case.must_refuse {
        Vec::new()
    } else {
        match expected_tables(state, member, datasets, case).await {
            Ok(t) => t,
            Err(why) => return (Verdict::Review, why),
        }
    };
    let history = match build_case_history(state, member, case, conversations).await {
        Ok(h) => h,
        Err(why) => return (Verdict::Review, why),
    };
    // Câu không tiếp nối hỏi qua cuộc trò chuyện mới, như người dùng hỏi câu đầu, để câu tiếp nối sau có lượt để đọc.
    let asked = match &history {
        Some(history) => ask(state, member, &dataset_id, &case.question, &[], history).await,
        None => conversation::ask(state, member, &dataset_id, &case.question, &[], None).await,
    };
    let response = match asked {
        Ok(r) => r,
        Err(error) if case.must_refuse => return build_refuse_error_verdict(&error),
        Err(error) => return build_ask_error_verdict(&error),
    };
    if history.is_none() && response.status == AskStatus::Answered {
        conversations.insert(case.id.clone(), response.conversation_id);
    }
    if case.must_refuse {
        build_refuse_verdict(&response)
    } else {
        build_answer_verdict(&response, case, &expected)
    }
}

/// Khối "Hội thoại trước" cho câu tiếp nối, đọc từ cuộc của câu gốc như service/conversation/ đọc khi người dùng hỏi
/// tiếp. None với câu không tiếp nối.
async fn build_case_history(
    state: &AppState,
    member: &Member,
    case: &EvalCase,
    conversations: &HashMap<String, uuid::Uuid>,
) -> Result<Option<AskHistory>, String> {
    let Some(parent) = &case.follow_up_of else {
        return Ok(None);
    };
    let Some(&conversation) = conversations.get(parent) else {
        return Err(format!(
            "câu gốc {parent} chưa được trả lời trong lượt này (phải đứng trước trong file)"
        ));
    };
    let turns = state
        .store
        .list_turns(&member.workspace, conversation, HISTORY_TURNS)
        .await
        .map_err(|e| format!("đọc lượt của câu gốc {parent}: {e:?}"))?;
    Ok(Some(AskHistory {
        section: build_history_section(&turns),
        questions: turns.iter().map(|turn| turn.question.clone()).collect(),
    }))
}

/// Chấm câu phải từ chối: rejected là đạt, trả lời ra bảng là trượt. Failed cũng là trượt: hệ thống đã thử trả lời
/// mà không được, người hỏi không nhận được lời nói rõ vì sao câu này không hỏi được.
pub fn build_refuse_verdict(response: &AskResponse) -> (Verdict, String) {
    let message = response.message.as_deref().unwrap_or_default();
    match response.status {
        AskStatus::Rejected => (Verdict::Pass, format!("đã từ chối: {message}")),
        AskStatus::NeedsChoice => (
            Verdict::Review,
            format!("hệ thống hỏi lại thay vì từ chối: {message}"),
        ),
        AskStatus::Answered => (
            Verdict::Fail,
            format!(
                "trả lời thay vì từ chối; sql sinh ra: {}",
                response.sql_generated.as_deref().unwrap_or_default()
            ),
        ),
        AskStatus::Failed => (
            Verdict::Fail,
            format!("thất bại thay vì từ chối: {message}"),
        ),
    }
}

/// ask trả lỗi với câu phải từ chối. BadRequest có thể là mô hình từ chối vì nội dung câu hỏi (engine/llm/mod.rs),
/// nhưng cũng có thể là câu không gửi được (mock không có câu này, dataset chưa có bảng): người đọc kết quả quyết định,
/// không tự tính đạt. Lỗi khác chấm như câu thường.
pub fn build_refuse_error_verdict(error: &AppError) -> (Verdict, String) {
    match error {
        AppError::BadRequest(m) => (
            Verdict::Review,
            format!("lỗi 400, có thể là mô hình từ chối vì nội dung: {m}"),
        ),
        e => build_ask_error_verdict(e),
    }
}

/// ask trả lỗi. Lỗi của nhà cung cấp mô hình (hết hạn mức sau mọi lần chờ ở service/eval_pace.rs, quá tải, thiếu khóa)
/// chưa nói gì về câu trả lời, nên là cần xem lại; lỗi khác là trượt.
pub fn build_ask_error_verdict(error: &AppError) -> (Verdict, String) {
    match error {
        AppError::Upstream(m) | AppError::Unavailable(m) => {
            (Verdict::Review, format!("mô hình không trả lời được: {m}"))
        }
        e => (Verdict::Fail, format!("lỗi hệ thống khi hỏi: {e:?}")),
    }
}

/// Chấm câu trả lời với các bảng đáp án (mỗi expected_sql chạy được một bảng, khớp một là đạt), luật so ở
/// service/eval_grade.rs.
pub fn build_answer_verdict(
    response: &AskResponse,
    case: &EvalCase,
    expected: &[Vec<Vec<Option<CellValue>>>],
) -> (Verdict, String) {
    let sql = response.sql_generated.as_deref().unwrap_or_default();
    let message = response.message.as_deref().unwrap_or_default();
    match response.status {
        AskStatus::Answered => {}
        // Hỏi lại để chọn cách hiểu không sai với câu mơ hồ; người đọc kết quả quyết định câu đó có mơ hồ không.
        AskStatus::NeedsChoice => {
            return (
                Verdict::Review,
                format!("hệ thống hỏi lại để chọn cách hiểu: {message}"),
            )
        }
        status => {
            return (
                Verdict::Fail,
                format!("status {status:?} thay vì answered; message: {message}"),
            )
        }
    }
    // Bảng khớp mà không có phần giải thích thì câu đã chạy như câu thường (form sai hai lần, explain/plain.rs): người
    // hỏi "vì sao" không nhận được nguyên nhân.
    if case.is_why() && response.explanation.is_none() {
        return (
            Verdict::Fail,
            format!("câu tìm nguyên nhân chạy như câu thường; sql sinh ra: {sql}"),
        );
    }
    let Some(result) = &response.result else {
        return (Verdict::Fail, "answered nhưng không có result".to_string());
    };
    let mut first_error = None;
    for table in expected {
        match build_eval_verdict(&result.rows, table, case.ordered, &case.required_columns) {
            Ok(note) => return (Verdict::Pass, note),
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    (
        Verdict::Fail,
        format!("{}; sql sinh ra: {sql}", first_error.unwrap_or_default()),
    )
}

/// Chạy các câu SQL đúng viết tay qua guard và DuckDB, giữ bảng của mọi câu chạy được (khớp một là đạt). Cả expected lẫn câu mô hình sinh đều
/// qua cùng một guard và cùng một engine nên cách so không thiên vị cách viết SQL. SQL đúng được đọc mọi dataset của
/// workspace tạm (với database mẫu là mọi bảng đã chụp), bản hiện tại của từng cái.
async fn expected_tables(
    state: &AppState,
    member: &Member,
    datasets: &[Dataset],
    case: &EvalCase,
) -> Result<Vec<Vec<Vec<Option<CellValue>>>>, String> {
    let mut tables = Vec::new();
    for dataset in datasets {
        tables.push(build_allowed_table(state, member, dataset).await?);
    }
    // Cùng luật nối bảng như câu mô hình viết (#256): quan hệ đã ghi lúc chụp database mẫu, theo tên bảng logic.
    let relations = state
        .store
        .list_relations(&member.workspace, None)
        .await
        .map_err(|e| format!("đọc quan hệ giữa các dataset: {e:?}"))?;
    let links = build_links(&relations, |id| {
        let dataset = datasets.iter().find(|d| d.id == id)?;
        Some(dataset.logical_table.clone())
    });
    let mut tables_out = Vec::new();
    let mut last_error = String::new();
    for sql in &case.expected_sql {
        let guarded = match build_guarded_sql(sql, &tables, &links, state.config.ask_max_rows) {
            Ok(g) => g,
            Err(e) => {
                last_error = format!("expected_sql không qua được guard: {e:?}");
                continue;
            }
        };
        let runnable = guarded.runnable;
        let timeout = state.config.query_timeout;
        let output = tokio::task::spawn_blocking(move || run_query(&runnable, 100_000, timeout))
            .await
            .map_err(|e| format!("{e:?}"))?;
        match output {
            Ok(o) => tables_out.push(build_result_table(o, state.config.ask_max_rows).rows),
            Err(e) => last_error = format!("expected_sql chạy lỗi: {e:?}"),
        }
    }
    if tables_out.is_empty() {
        return Err(format!("không expected_sql nào chạy được: {last_error}"));
    }
    Ok(tables_out)
}

/// Bảng guard cho phép của một dataset: tên bảng logic, cột và thư mục Parquet của phiên bản hiện tại.
async fn build_allowed_table(
    state: &AppState,
    member: &Member,
    dataset: &Dataset,
) -> Result<AllowedTable, String> {
    let version = state
        .store
        .get_current_version(&member.workspace, dataset.id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .ok_or("dataset chưa có phiên bản")?;
    let columns = state
        .store
        .list_columns(&member.workspace, version.id)
        .await
        .map_err(|e| format!("{e:?}"))?
        .iter()
        .map(|c| c.name.clone())
        .collect();
    Ok(AllowedTable {
        name: dataset.logical_table.clone(),
        columns,
        parquet_dir: state.config.data_dir.join(
            version
                .parquet_path
                .ok_or("phiên bản không có file parquet")?,
        ),
        row_filters: Vec::new(),
    })
}

fn parse_uuid(text: &str) -> Result<uuid::Uuid, String> {
    uuid::Uuid::parse_str(text).map_err(|e| e.to_string())
}
