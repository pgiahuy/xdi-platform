// SPDX-License-Identifier: Apache-2.0
/* service/sample_questions.rs - câu gợi ý chọn tay cho các database mẫu (SAMPLE_DATABASE_URL, sales_demo và edu_demo).
File <SAMPLES_DIR>/suggested_questions.json là một object JSON, khóa là tên database mẫu, giá trị là danh sách câu. Các
câu đã hỏi thử với Gemini và đọc từng câu trả lời; câu mô hình gợi ý cho từng bảng thì không ai đọc trước, và bảng mẫu
là chỗ người dùng mới thử đầu tiên.

AppState::new đọc file một lần lúc khởi động (load_sample_questions), chỉ giữ tên là database mẫu đang cấu hình, nên
một file nạp tay hay database khác tình cờ cùng tên không nhận các câu này. Danh mục (service/catalog/mod.rs) gắn danh
sách vào DatasetSummary.suggested_questions của dataset database cùng tên; trang hỏi hiện danh sách này thay cho câu gợi
ý của từng bảng khi chưa ghim bảng nào. Không có file hay file hỏng thì không dataset nào có câu chọn tay: trang hỏi
dùng câu gợi ý của bảng như trước. */

use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;

const QUESTIONS_FILE: &str = "suggested_questions.json";

/// Câu chọn tay theo tên database mẫu. Lỗi chỉ ghi cảnh báo: thiếu câu gợi ý không được làm backend dừng.
pub fn load_sample_questions(config: &Config) -> HashMap<String, Vec<String>> {
    let path = config.samples_dir.join(QUESTIONS_FILE);
    let all = match read_questions(&path) {
        Ok(all) => all,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "cannot read the sample suggested questions");
            return HashMap::new();
        }
    };
    all.into_iter()
        .filter(|(name, _)| {
            config
                .sample_databases
                .iter()
                .any(|db| db.database == *name)
        })
        .collect()
}

/// Nội dung file; rỗng khi không có file. JSON hỏng là lỗi.
fn read_questions(path: &Path) -> Result<HashMap<String, Vec<String>>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(format!("read: {e}")),
    };
    serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_file() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../data/samples")
            .join(QUESTIONS_FILE)
    }

    /// File của repo có câu cho cả hai database mẫu; mỗi câu ngắn, không trùng.
    #[test]
    fn repo_file_has_questions_for_both_sample_databases() {
        let all = read_questions(&repo_file()).unwrap();
        for name in ["sales_demo", "edu_demo"] {
            let list = &all[name];
            assert_eq!(list.len(), 8, "{name}");
            let mut seen = std::collections::HashSet::new();
            for q in list {
                assert!(seen.insert(q), "{name}: {q}");
                assert!(
                    q.chars().count() <= crate::service::semantic::QUESTION_MAX_CHARS,
                    "{q}"
                );
            }
        }
    }

    /// Chỉ tên là database mẫu đang cấu hình được giữ.
    #[test]
    fn only_configured_sample_databases_keep_their_questions() {
        let mut config = Config::from_env();
        config.samples_dir = repo_file().parent().unwrap().to_path_buf();
        config.sample_databases = crate::sample_database::build_sample_database(
            "postgres://analyst:analyst@127.0.0.1:5440/edu_demo",
        )
        .unwrap()
        .into_iter()
        .collect();
        let kept = load_sample_questions(&config);
        assert_eq!(kept.keys().collect::<Vec<_>>(), ["edu_demo"]);
    }

    #[test]
    fn missing_file_means_no_questions() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-questions.json");
        assert!(read_questions(&path).unwrap().is_empty());
    }
}
