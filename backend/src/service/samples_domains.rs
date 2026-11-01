// SPDX-License-Identifier: Apache-2.0
/* service/samples_domains.rs - gói ngành của dataset mẫu: thiết lập lần đầu tạo xong một dataset mẫu (database mẫu ở
service/samples_db.rs, hay DataCo từ file ở service/samples.rs) thì gắn gói ghi cho tên dataset đó trong
<SAMPLES_DIR>/domains.json, qua đúng hàm admin dùng ở màn Khái niệm ngành (service/domains/assign.rs).

File là một object JSON, khóa là tên dataset (tên database với database mẫu, "DataCo" với file mẫu), giá trị là khóa gói
trong data/domains/. Tên dataset và gói nằm trong file dữ liệu chứ không trong code, nên đổi database mẫu hay thêm gói
chỉ cần sửa file. Không có file, hay tên dataset không có trong file, thì dataset không có gói; admin chọn sau.

Lỗi ở đây chỉ ghi cảnh báo: thiếu gói không được làm hỏng việc nạp dataset mẫu. */

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;

use crate::auth::Member;
use crate::error::AppError;
use crate::service::domains::assign::assign;
use crate::state::AppState;

const DOMAINS_FILE: &str = "domains.json";

/// Gắn gói của dataset mẫu `name` (dataset `source_id`) nếu domains.json có ghi. Không trả lỗi: ghi log rồi thôi.
pub async fn assign_sample_domain(state: &AppState, member: &Member, source_id: Uuid, name: &str) {
    let path = state.config.samples_dir.join(DOMAINS_FILE);
    let key = match find_sample_domain(&path, name) {
        Ok(Some(key)) => key,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(error = ?e, path = %path.display(), "cannot read the sample domain packs");
            return;
        }
    };
    match assign(state, member, source_id, Some(&key)).await {
        Ok(()) => tracing::info!(dataset = name, domain = %key, "sample dataset domain pack set"),
        Err(e) => tracing::warn!(
            error = ?e,
            dataset = name,
            domain = %key,
            "cannot set the domain pack of the sample dataset; choose it on the domains page"
        ),
    }
}

/// Khóa gói ghi cho dataset `name` trong file. Ok(None) khi không có file hay file không ghi tên này; JSON hỏng là lỗi.
fn find_sample_domain(path: &Path, name: &str) -> Result<Option<String>, AppError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::Internal(format!("read {}: {e}", path.display()))),
    };
    let packs: HashMap<String, String> = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("parse {}: {e}", path.display())))?;
    Ok(packs.get(name).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_file() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/samples/domains.json")
    }

    #[test]
    fn repo_file_names_a_pack_for_every_sample_dataset() {
        let file = repo_file();
        let packs = crate::engine::domains::list_domain_packages();
        // Mỗi dataset mẫu của repo trỏ vào một gói có thật trong data/domains/.
        for name in [
            "sales_demo",
            "edu_demo",
            crate::service::samples::SAMPLE_NAME,
        ] {
            let key = find_sample_domain(&file, name).unwrap().expect(name);
            assert!(packs.iter().any(|p| p.id == key), "{name}: {key}");
        }
        assert_eq!(
            find_sample_domain(&file, "edu_demo").unwrap().as_deref(),
            Some("education")
        );
        assert_eq!(find_sample_domain(&file, "other").unwrap(), None);
    }

    #[test]
    fn missing_file_means_no_pack() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-domains.json");
        assert_eq!(find_sample_domain(&path, "sales_demo").unwrap(), None);
    }
}
