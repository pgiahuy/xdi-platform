// SPDX-License-Identifier: Apache-2.0
/* engine/domains.rs - gói khái niệm ngành: đọc các file .json trong data/domains/ mà build.rs nhúng vào binary, kiểm
theo schema và cho tra cứu theo id. Engine chỉ biết hình dạng của một gói, không biết nội dung từng ngành.

Ba tầng tách riêng: engine không biết ngành; gói mô tả khái niệm chung của mọi doanh nghiệp cùng ngành (doanh thu, tỷ
lệ giao trễ...) bằng lời, không ghi tên cột của dataset cụ thể; chỉ số của workspace (bảng metrics) mới quyết định
công thức trên cột thật. Chưa có code gọi module này: chọn gói cho dataset lúc nạp (một dataset một gói) và đưa gói
vào prompt (#29) làm ở issue sau. Schema của file ghi ở docs/DOMAINS.md. */

use serde::Deserialize;

// build.rs sinh DOMAIN_PACKAGE_FILES: mỗi file data/domains/<id>.json một mục (tên file không đuôi, nội dung JSON).
include!(concat!(env!("OUT_DIR"), "/domain_packages.rs"));

/// Một gói khái niệm của một ngành, tương ứng một file data/domains/<id>.json.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainConcept {
    /// Khóa của khái niệm trong gói, tiếng Anh, không trùng nhau.
    pub id: String,
    /// Tên khái niệm hiện cho người dùng.
    pub name: String,
    /// Khái niệm nói về điều gì, khi nào dùng.
    pub description: String,
    /// Cách gọi khác của khái niệm, tiếng Việt và tiếng Anh, để nhận ra khái niệm trong câu hỏi. Hai khái niệm không
    /// được dùng chung một từ, kể cả khác dấu hay khác hoa thường (test kiểm).
    pub synonyms: Vec<String>,
    /// Hai đến ba cách tính phổ biến, viết bằng lời; công thức trên cột thật là của chỉ số trong workspace.
    pub calculations: Vec<Calculation>,
    /// Chiều hay dùng để phân rã khái niệm, ví dụ thời gian, khu vực, nhóm hàng.
    pub dimensions: Vec<String>,
    /// Luật kiểm giá trị có hợp lý không, ví dụ một tỷ lệ phải nằm trong 0 đến 1. Không có luật nào thì để mảng rỗng.
    pub sanity_checks: Vec<SanityCheck>,
}

/// Một cách tính phổ biến của khái niệm, bằng lời, không gắn với cột của dataset cụ thể.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    /// Tên cách tính, ví dụ "Tổng theo đơn".
    pub name: String,
    /// Cách tính làm gì và khi nào chọn nó thay các cách còn lại.
    pub description: String,
}

/// Một luật kiểm hợp lý của giá trị khái niệm. min và max chỉ có khi kind là value_range.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanityCheck {
    pub kind: SanityCheckKind,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SanityCheckKind {
    /// Giá trị phải nằm trong khoảng [min, max], ví dụ tỷ lệ trong 0 đến 1.
    ValueRange,
    /// Giá trị không được âm.
    NonNegative,
}

/// Mọi gói đã nhúng. Parse một lần và giữ lại: file nằm sẵn trong binary, không đọc lại được, nên parse mỗi lần gọi
/// chỉ tốn công. File hỏng thì panic với thông điệp nêu tên file; test của module này đọc mọi gói, nên gói sai schema
/// bị CI chặn trước khi tới máy chủ (cùng cách với engine/llm/mock.rs).
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

/// Tìm gói theo khóa (tên file, cũng là trường id trong file).
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    /// So từ đồng nghĩa sau chuẩn hóa: chữ thường, bỏ dấu, gộp khoảng trắng. "Doanh thu" và "doanh thu" hay "doanh
    /// thu" gõ không dấu là cùng một từ, hai khái niệm không được nhận cùng từ đó.
    fn normalize(text: &str) -> String {
        let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
        deunicode::deunicode(&joined).to_lowercase()
    }

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
    fn synonyms_do_not_repeat_within_or_across_concepts() {
        // Khóa là từ đã chuẩn hóa, giá trị là khái niệm đang giữ từ đó, để lời báo lỗi chỉ ra cả hai khái niệm.
        let mut seen: HashMap<String, String> = HashMap::new();
        for (file, package) in packages() {
            for concept in &package.concepts {
                for synonym in &concept.synonyms {
                    let key = normalize(synonym);
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
                    match check.kind {
                        SanityCheckKind::ValueRange => {
                            let (min, max) = (check.min, check.max);
                            assert!(
                                min.is_some_and(|min| max.is_some_and(|max| min <= max)),
                                "{file}/{}: value_range cần min và max, min không quá max",
                                concept.id
                            );
                        }
                        SanityCheckKind::NonNegative => {
                            assert!(
                                check.min.is_none() && check.max.is_none(),
                                "{file}/{}: non_negative không có min, max",
                                concept.id
                            );
                        }
                    }
                }
            }
        }
    }

    /// Trường lạ trong file gói phải bị từ chối (deny_unknown_fields), để gõ sai tên trường không lặng lẽ mất tác
    /// dụng.
    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"id": "x", "name": "x", "description": "x", "concepts": [], "extra": 1}"#;
        assert!(serde_json::from_str::<DomainPackage>(json).is_err());
    }
}
