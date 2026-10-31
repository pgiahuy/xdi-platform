// SPDX-License-Identifier: Apache-2.0
/* service/domains/check.rs - kiểm một khái niệm admin gửi lên trước khi lưu, như kiểm chỉ số (service/metrics/mod.rs):
bỏ khoảng trắng hai đầu, độ dài, ký tự điều khiển, tên không trùng trong gói, từ đồng nghĩa không trùng giữa hai khái
niệm của gói (cùng luật với test của file gói, engine/domains.rs), luật kiểm nhất quán. Sai thì lỗi 400 nêu đúng
trường (AppError::InvalidField), lời báo tiếng Việt cho admin.

Khái niệm vào prompt của mọi câu hỏi trên dataset gắn gói, mỗi khái niệm một dòng "- tên (từ, từ): mô tả"
(service/ask/prompt.rs). Vì vậy xuống dòng bị từ chối (chèn được dòng giả vào prompt), tên và từ đồng nghĩa không có
ngoặc tròn, từ đồng nghĩa không có dấu phẩy: các dấu đó làm dòng đọc ra sai ranh giới. Luật nhẹ hơn test của file ở một
chỗ: file cần hai đến ba cách tính và có chiều phân rã, admin được để trống vì chưa code nào ngoài màn này đọc hai
trường đó. */

use std::collections::HashMap;

use uuid::Uuid;

use crate::engine::domains::{build_synonym_key, check_sanity_check, Calculation, SanityCheck};
use crate::engine::store::{DomainConceptRecord, NewDomainConcept};
use crate::error::AppError;
use crate::service::ask::phrases::build_key;
use crate::service::domains::contract::ConceptRequest;

// Trần, đếm theo ký tự. Cao hơn khái niệm dài nhất của các file gói hiện có; test bên dưới kiểm mọi khái niệm của
// file đều qua, để admin mở rồi lưu lại một khái niệm có sẵn không bị báo lỗi.
const NAME_MAX_CHARS: usize = 80;
const DESCRIPTION_MAX_CHARS: usize = 500;
const WORD_MAX_CHARS: usize = 60;
const SYNONYMS_MAX: usize = 20;
const CALCULATIONS_MAX: usize = 3;
const CALCULATION_DESCRIPTION_MAX_CHARS: usize = 300;
const DIMENSIONS_MAX: usize = 10;
const SANITY_CHECKS_MAX: usize = 5;

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::InvalidField {
        field: field.to_string(),
        message: message.into(),
    }
}

/// Kiểm và làm sạch request. others là mọi khái niệm của cùng gói; editing là id của khái niệm đang sửa (bỏ nó ra
/// khi so trùng), None khi thêm mới.
pub fn build_new_concept(
    request: ConceptRequest,
    others: &[DomainConceptRecord],
    editing: Option<Uuid>,
) -> Result<NewDomainConcept, AppError> {
    let others: Vec<&DomainConceptRecord> =
        others.iter().filter(|c| Some(c.id) != editing).collect();
    let name = clean_text(&request.name, "name", "Tên", NAME_MAX_CHARS)?;
    check_no_brackets(&name, "name", "Tên")?;
    let key = build_key(&name);
    if others.iter().any(|c| build_key(&c.name) == key) {
        return Err(invalid(
            "name",
            "Gói đã có khái niệm tên này; sửa khái niệm đó hoặc đặt tên khác",
        ));
    }
    Ok(NewDomainConcept {
        name,
        description: clean_text(
            &request.description,
            "description",
            "Mô tả",
            DESCRIPTION_MAX_CHARS,
        )?,
        synonyms: build_synonyms(&request.synonyms, &others)?,
        calculations: build_calculations(&request.calculations)?,
        dimensions: build_dimensions(&request.dimensions)?,
        sanity_checks: check_sanity_checks(request.sanity_checks)?,
        enabled: request.enabled,
    })
}

/// Chuỗi đã bỏ khoảng trắng hai đầu; rỗng, quá dài hay có ký tự điều khiển (xuống dòng, tab) thì báo lỗi.
fn clean_text(text: &str, field: &str, label: &str, max_chars: usize) -> Result<String, AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(invalid(field, format!("{label} không được để trống")));
    }
    if trimmed.chars().count() > max_chars {
        return Err(invalid(
            field,
            format!("{label} dài tối đa {max_chars} ký tự"),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid(
            field,
            format!("{label} có ký tự không in được, ví dụ xuống dòng"),
        ));
    }
    Ok(trimmed.to_string())
}

fn check_no_brackets(text: &str, field: &str, label: &str) -> Result<(), AppError> {
    if text.contains(['(', ')']) {
        return Err(invalid(
            field,
            format!("{label} không được có ngoặc tròn: \"{text}\""),
        ));
    }
    Ok(())
}

/// Từ đồng nghĩa: ít nhất một, không trùng nhau trong khái niệm và không trùng từ của khái niệm khác trong gói (so
/// sau khi chữ thường, bỏ dấu).
fn build_synonyms(
    synonyms: &[String],
    others: &[&DomainConceptRecord],
) -> Result<Vec<String>, AppError> {
    const FIELD: &str = "synonyms";
    if synonyms.is_empty() {
        return Err(invalid(FIELD, "Cần ít nhất một từ đồng nghĩa"));
    }
    if synonyms.len() > SYNONYMS_MAX {
        return Err(invalid(
            FIELD,
            format!("Tối đa {SYNONYMS_MAX} từ đồng nghĩa"),
        ));
    }
    // Khóa là từ đã chuẩn hóa, giá trị là tên khái niệm đang giữ từ đó, để lời báo nêu được khái niệm kia.
    let mut taken: HashMap<String, &str> = HashMap::new();
    for concept in others {
        for synonym in &concept.synonyms {
            taken.insert(build_synonym_key(synonym), &concept.name);
        }
    }
    let mut cleaned = Vec::new();
    let mut seen = Vec::new();
    for synonym in synonyms {
        let word = clean_text(synonym, FIELD, "Từ đồng nghĩa", WORD_MAX_CHARS)?;
        check_no_brackets(&word, FIELD, "Từ đồng nghĩa")?;
        if word.contains(',') {
            return Err(invalid(
                FIELD,
                format!("Từ đồng nghĩa không được có dấu phẩy: \"{word}\"; tách thành hai từ"),
            ));
        }
        let key = build_synonym_key(&word);
        if let Some(owner) = taken.get(&key) {
            return Err(invalid(
                FIELD,
                format!("Từ \"{word}\" đã thuộc khái niệm {owner}"),
            ));
        }
        if seen.contains(&key) {
            return Err(invalid(FIELD, format!("Từ \"{word}\" ghi hai lần")));
        }
        seen.push(key);
        cleaned.push(word);
    }
    Ok(cleaned)
}

fn build_calculations(calculations: &[Calculation]) -> Result<Vec<Calculation>, AppError> {
    const FIELD: &str = "calculations";
    if calculations.len() > CALCULATIONS_MAX {
        return Err(invalid(
            FIELD,
            format!("Tối đa {CALCULATIONS_MAX} cách tính"),
        ));
    }
    calculations
        .iter()
        .map(|c| {
            Ok(Calculation {
                name: clean_text(&c.name, FIELD, "Tên cách tính", NAME_MAX_CHARS)?,
                description: clean_text(
                    &c.description,
                    FIELD,
                    "Mô tả cách tính",
                    CALCULATION_DESCRIPTION_MAX_CHARS,
                )?,
            })
        })
        .collect()
}

fn build_dimensions(dimensions: &[String]) -> Result<Vec<String>, AppError> {
    const FIELD: &str = "dimensions";
    if dimensions.len() > DIMENSIONS_MAX {
        return Err(invalid(
            FIELD,
            format!("Tối đa {DIMENSIONS_MAX} chiều phân rã"),
        ));
    }
    dimensions
        .iter()
        .map(|d| clean_text(d, FIELD, "Chiều phân rã", WORD_MAX_CHARS))
        .collect()
}

fn check_sanity_checks(checks: Vec<SanityCheck>) -> Result<Vec<SanityCheck>, AppError> {
    const FIELD: &str = "sanity_checks";
    if checks.len() > SANITY_CHECKS_MAX {
        return Err(invalid(
            FIELD,
            format!("Tối đa {SANITY_CHECKS_MAX} luật kiểm"),
        ));
    }
    for check in &checks {
        check_sanity_check(check).map_err(|message| invalid(FIELD, message))?;
    }
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use sqlx::types::Json;

    use super::*;
    use crate::engine::domains::list_domain_packages;

    fn record(name: &str, synonyms: &[&str]) -> DomainConceptRecord {
        DomainConceptRecord {
            id: Uuid::new_v4(),
            pack_id: Uuid::nil(),
            key: None,
            name: name.to_string(),
            description: "x".to_string(),
            synonyms: synonyms.iter().map(|s| s.to_string()).collect(),
            calculations: Json(Vec::new()),
            dimensions: Vec::new(),
            sanity_checks: Json(Vec::new()),
            enabled: true,
            updated_at: None,
        }
    }

    fn request(name: &str, synonyms: &[&str]) -> ConceptRequest {
        ConceptRequest {
            name: name.to_string(),
            description: "Số sinh viên phân biệt".to_string(),
            synonyms: synonyms.iter().map(|s| s.to_string()).collect(),
            calculations: Vec::new(),
            dimensions: Vec::new(),
            sanity_checks: Vec::new(),
            enabled: true,
        }
    }

    fn field_of(result: Result<NewDomainConcept, AppError>) -> String {
        match result {
            Err(AppError::InvalidField { field, .. }) => field,
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    /// Admin mở một khái niệm có sẵn của file rồi lưu lại không đổi gì thì phải qua.
    #[test]
    fn every_file_concept_passes_the_check() {
        for package in list_domain_packages() {
            for concept in &package.concepts {
                let request = ConceptRequest {
                    name: concept.name.clone(),
                    description: concept.description.clone(),
                    synonyms: concept.synonyms.clone(),
                    calculations: concept.calculations.clone(),
                    dimensions: concept.dimensions.clone(),
                    sanity_checks: concept.sanity_checks.clone(),
                    enabled: true,
                };
                if let Err(err) = build_new_concept(request, &[], None) {
                    panic!("{}/{}: {err:?}", package.id, concept.id);
                }
            }
        }
    }

    #[test]
    fn duplicates_in_the_pack_are_refused() {
        let others = [record("Số sinh viên", &["sinh viên", "students"])];
        let same_name = build_new_concept(request("số SINH viên", &["học viên"]), &others, None);
        assert_eq!(field_of(same_name), "name");
        let taken = build_new_concept(request("Người học", &["Sinh Viên"]), &others, None);
        assert_eq!(field_of(taken), "synonyms");
        let twice = build_new_concept(
            request("Người học", &["học viên", "hoc vien"]),
            &others,
            None,
        );
        assert_eq!(field_of(twice), "synonyms");
        // Sửa chính khái niệm đó thì không tính là trùng với chính nó.
        let edit = build_new_concept(
            request("Số sinh viên", &["sinh viên"]),
            &others,
            Some(others[0].id),
        );
        assert!(edit.is_ok(), "{edit:?}");
    }

    #[test]
    fn text_that_would_break_the_prompt_line_is_refused() {
        let cases = [
            (request("Tỷ lệ\nđạt", &["pass rate"]), "name"),
            (request("Tỷ lệ (đạt)", &["pass rate"]), "name"),
            (request("Tỷ lệ đạt", &["đạt, giỏi"]), "synonyms"),
            (request("Tỷ lệ đạt", &[]), "synonyms"),
            (request("  ", &["pass rate"]), "name"),
        ];
        for (request, field) in cases {
            assert_eq!(field_of(build_new_concept(request, &[], None)), field);
        }
        let mut long = request("Tỷ lệ đạt", &["pass rate"]);
        long.description = "x".repeat(DESCRIPTION_MAX_CHARS + 1);
        assert_eq!(field_of(build_new_concept(long, &[], None)), "description");
    }

    #[test]
    fn values_are_trimmed() {
        let mut padded = request("  Tỷ lệ đạt ", &[" pass rate "]);
        padded.dimensions = vec![" học phần ".to_string()];
        let concept = build_new_concept(padded, &[], None).unwrap();
        assert_eq!(concept.name, "Tỷ lệ đạt");
        assert_eq!(concept.synonyms, ["pass rate"]);
        assert_eq!(concept.dimensions, ["học phần"]);
    }
}
