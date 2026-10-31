// SPDX-License-Identifier: Apache-2.0
/* engine/domains.rs - gói khái niệm ngành: đọc các file .json trong data/domains/ mà build.rs nhúng vào binary, kiểm
theo schema, và các luật kiểm dùng chung cho file và cho khái niệm admin sửa. Engine chỉ biết hình dạng của một gói,
không biết nội dung từng ngành.

Ba tầng tách riêng: engine không biết ngành; gói mô tả khái niệm chung của mọi doanh nghiệp cùng ngành (doanh thu, tỷ
lệ giao trễ...) bằng lời, không ghi tên cột của dataset cụ thể; chỉ số của workspace (bảng metrics) mới quyết định
công thức trên cột thật.

File là dữ liệu khởi tạo (#229): service/domains/ chép gói và khái niệm còn thiếu vào bảng domain_packs,
domain_concepts của từng workspace (engine/store/domains.rs), admin sửa ở đó. Prompt sinh SQL và prompt form câu "tại
sao" (service/ask/prompt.rs) nhận DomainPackage dựng từ bảng, không đọc file. Schema của file ghi ở docs/DOMAINS.md. */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// build.rs sinh DOMAIN_PACKAGE_FILES: mỗi file data/domains/<id>.json một mục (tên file không đuôi, nội dung JSON).
include!(concat!(env!("OUT_DIR"), "/domain_packages.rs"));

/// Một gói khái niệm của một ngành, tương ứng một file data/domains/<id>.json, hoặc một dòng domain_packs kèm các
/// khái niệm đang bật (engine/store/domains.rs).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackage {
    /// Khóa của gói, phải trùng tên file (retail.json thì id là "retail"); dataset sẽ trỏ tới khóa này.
    pub id: String,
    /// Tên ngành hiện cho người dùng.
    pub name: String,
    /// Ngành này là gì, gói áp dụng cho kiểu dữ liệu nào.
    pub description: String,
    pub concepts: Vec<DomainConcept>,
}

/// Một khái niệm kinh doanh chung của ngành, ví dụ doanh thu hay tỷ lệ giao trễ.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainConcept {
    /// Khóa của khái niệm trong gói, tiếng Anh, không trùng nhau.
    pub id: String,
    /// Tên khái niệm hiện cho người dùng.
    pub name: String,
    /// Khái niệm nói về điều gì, khi nào dùng.
    pub description: String,
    /// Cách gọi khác của khái niệm, tiếng Việt và tiếng Anh, để nhận ra khái niệm trong câu hỏi. Hai khái niệm trong
    /// cùng một gói không được dùng chung một từ, kể cả khác dấu hay khác hoa thường (test kiểm); hai gói khác nhau
    /// được dùng chung từ, vì mỗi dataset chỉ gắn một gói.
    pub synonyms: Vec<String>,
    /// Hai đến ba cách tính phổ biến, viết bằng lời; công thức trên cột thật là của chỉ số trong workspace.
    pub calculations: Vec<Calculation>,
    /// Chiều hay dùng để phân rã khái niệm, ví dụ thời gian, khu vực, nhóm hàng.
    pub dimensions: Vec<String>,
    /// Luật kiểm giá trị có hợp lý không, ví dụ một tỷ lệ phải nằm trong 0 đến 1. Không có luật nào thì để mảng rỗng.
    pub sanity_checks: Vec<SanityCheck>,
}

// Calculation, SanityCheck, SanityCheckKind đi thẳng ra API của màn Khái niệm ngành (service/domains/contract.rs) và
// vào cột jsonb, nên có Serialize và ToSchema; chú thích /// là mô tả tiếng Anh trong đặc tả OpenAPI.
/// A common way to compute the concept, in words, not tied to the columns of a dataset.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    /// Short name of the method.
    pub name: String,
    /// What it computes and when to pick it over the others.
    pub description: String,
}

// Một luật kiểm hợp lý của giá trị khái niệm. min và max chỉ có khi kind là value_range, và cần ít nhất một đầu
// (ví dụ biên lợi nhuận chỉ chặn cận trên: {"kind": "value_range", "max": 1.0}); check_sanity_check kiểm điều này.
/// A plausibility rule for values of the concept. `min` and `max` only with `value_range`, at least one of them.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SanityCheck {
    pub kind: SanityCheckKind,
    // skip_serializing_if: cột jsonb giữ đúng dạng của file ({"kind": "non_negative"}), không thêm "min": null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// `value_range`: the value lies in [min, max], one end may be missing. `non_negative`: the value is not negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SanityCheckKind {
    ValueRange,
    NonNegative,
}

/// Mọi gói đã nhúng. Parse một lần và giữ lại: file nằm sẵn trong binary, không đọc lại được, nên parse mỗi lần gọi
/// chỉ tốn công. File hỏng thì panic với thông điệp nêu tên file; test của module này đọc mọi gói và CI chạy test cả
/// khi PR chỉ đổi data/domains (bộ lọc của .github/workflows/ci.yml), nên gói sai schema bị chặn trước khi tới máy
/// chủ (cùng cách với engine/llm/mock.rs).
pub fn list_domain_packages() -> &'static [DomainPackage] {
    static PACKAGES: std::sync::OnceLock<Vec<DomainPackage>> = std::sync::OnceLock::new();
    PACKAGES.get_or_init(|| {
        DOMAIN_PACKAGE_FILES
            .iter()
            .map(|(file, json)| {
                parse_package(file, json)
                    .unwrap_or_else(|err| panic!("invalid domain package {file}.json: {err}"))
            })
            .collect()
    })
}

/// Tìm gói theo khóa (tên file, cũng là trường id trong file). Chỉ test dùng: lúc chạy, prompt đọc gói từ bảng của
/// workspace (service/domains/), không đọc file.
#[cfg(test)]
pub fn find_domain_package(id: &str) -> Option<&'static DomainPackage> {
    list_domain_packages()
        .iter()
        .find(|package| package.id == id)
}

/// Đọc một file gói. Khóa trong file phải trùng tên file, để tra cứu theo tên file và theo id cho cùng một kết quả.
fn parse_package(file: &str, json: &str) -> Result<DomainPackage, String> {
    let package: DomainPackage =
        serde_json::from_str(json).map_err(|err| format!("invalid JSON: {err}"))?;
    if package.id != file {
        return Err(format!("id {:?} does not match the file name", package.id));
    }
    Ok(package)
}

/// Dạng so của một từ đồng nghĩa: chữ thường, bỏ dấu, gộp khoảng trắng. "Doanh thu" và "doanh thu" hay "doanh thu"
/// gõ không dấu là cùng một từ, hai khái niệm trong một gói không được nhận cùng từ đó. Test của file gói và kiểm khái
/// niệm admin sửa (service/domains/check.rs) dùng chung hàm này.
pub fn build_synonym_key(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    deunicode::deunicode(&joined).to_lowercase()
}

/// Luật kiểm có nhất quán không: non_negative không kèm min, max; value_range có ít nhất một đầu và min không quá
/// max. Trả lời báo tiếng Việt, vì service/domains/check.rs đưa thẳng cho admin.
pub fn check_sanity_check(check: &SanityCheck) -> Result<(), String> {
    match check.kind {
        SanityCheckKind::ValueRange => {
            if check.min.is_none() && check.max.is_none() {
                return Err("Khoảng giá trị cần ít nhất một đầu min hoặc max".to_string());
            }
            if let (Some(min), Some(max)) = (check.min, check.max) {
                if min > max {
                    return Err("Khoảng giá trị có min lớn hơn max".to_string());
                }
            }
        }
        SanityCheckKind::NonNegative => {
            if check.min.is_some() || check.max.is_some() {
                return Err("Luật không âm không kèm min, max".to_string());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    fn packages() -> Vec<(&'static str, DomainPackage)> {
        DOMAIN_PACKAGE_FILES
            .iter()
            .map(|(file, json)| (*file, parse_package(file, json).unwrap()))
            .collect()
    }

    #[test]
    fn every_package_file_matches_schema_and_file_name() {
        let packages = packages();
        assert!(!packages.is_empty());
        for (file, package) in &packages {
            assert_eq!(&package.id, file, "id phải trùng tên file");
            assert!(!package.name.is_empty(), "{file}: thiếu tên ngành");
            assert!(
                !package.concepts.is_empty(),
                "{file}: không có khái niệm nào"
            );
        }
        assert!(find_domain_package("retail").is_some());
        assert!(find_domain_package("khong-co-nganh-nay").is_none());
    }

    #[test]
    fn package_and_concept_ids_are_unique() {
        let mut package_ids = HashSet::new();
        for (file, package) in packages() {
            assert!(
                package_ids.insert(package.id.clone()),
                "trùng id gói: {}",
                package.id
            );
            let mut concept_ids = HashSet::new();
            for concept in &package.concepts {
                assert!(
                    concept_ids.insert(&concept.id),
                    "{file}: trùng id khái niệm: {}",
                    concept.id
                );
            }
        }
    }

    #[test]
    fn each_concept_has_two_to_three_calculations_and_synonyms() {
        for (file, package) in packages() {
            for concept in &package.concepts {
                assert!(
                    (2..=3).contains(&concept.calculations.len()),
                    "{file}/{}: cần 2 đến 3 cách tính, có {}",
                    concept.id,
                    concept.calculations.len()
                );
                assert!(
                    !concept.synonyms.is_empty(),
                    "{file}/{}: thiếu từ đồng nghĩa",
                    concept.id
                );
                assert!(
                    !concept.dimensions.is_empty(),
                    "{file}/{}: thiếu chiều phân rã",
                    concept.id
                );
            }
        }
    }

    #[test]
    fn synonyms_do_not_repeat_within_a_package() {
        for (file, package) in packages() {
            // Khóa là từ đã chuẩn hóa, giá trị là khái niệm đang giữ từ đó, để lời báo lỗi chỉ ra cả hai khái niệm.
            // seen đặt trong vòng lặp gói: mỗi dataset chỉ gắn một gói, nên hai gói khác nhau được dùng chung một từ.
            let mut seen: HashMap<String, String> = HashMap::new();
            for concept in &package.concepts {
                for synonym in &concept.synonyms {
                    let key = build_synonym_key(synonym);
                    assert!(
                        !key.is_empty(),
                        "{file}/{}: từ đồng nghĩa rỗng hoặc toàn khoảng trắng",
                        concept.id
                    );
                    if let Some(owner) = seen.get(&key) {
                        panic!(
                            "{file}: từ đồng nghĩa {synonym:?} thuộc cả {} và {}",
                            owner, concept.id
                        );
                    }
                    seen.insert(key, concept.id.clone());
                }
            }
        }
    }

    #[test]
    fn sanity_checks_are_consistent() {
        for (file, package) in packages() {
            for concept in &package.concepts {
                for check in &concept.sanity_checks {
                    if let Err(problem) = check_sanity_check(check) {
                        panic!("{file}/{}: {problem}", concept.id);
                    }
                }
            }
        }
        let range = |min, max| SanityCheck {
            kind: SanityCheckKind::ValueRange,
            min,
            max,
        };
        assert!(check_sanity_check(&range(None, None)).is_err());
        assert!(check_sanity_check(&range(Some(1.0), Some(0.0))).is_err());
        assert!(check_sanity_check(&range(None, Some(1.0))).is_ok());
        let non_negative_with_max = SanityCheck {
            kind: SanityCheckKind::NonNegative,
            min: None,
            max: Some(1.0),
        };
        assert!(check_sanity_check(&non_negative_with_max).is_err());
    }

    /// Trường lạ trong file gói phải bị từ chối (deny_unknown_fields), để gõ sai tên trường không lặng lẽ mất tác
    /// dụng.
    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"id": "x", "name": "x", "description": "x", "concepts": [], "extra": 1}"#;
        assert!(serde_json::from_str::<DomainPackage>(json).is_err());
        let concept = r#"{"id": "x", "name": "x", "description": "x", "synonyms": [], "calculations": [],
            "dimensions": [], "sanity_checks": [], "extra": 1}"#;
        assert!(serde_json::from_str::<DomainConcept>(concept).is_err());
        let check = r#"{"kind": "non_negative", "extra": 1}"#;
        assert!(serde_json::from_str::<SanityCheck>(check).is_err());
    }
}
