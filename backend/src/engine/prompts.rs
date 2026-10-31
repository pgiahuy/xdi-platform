// SPDX-License-Identifier: Apache-2.0
/* engine/prompts.rs - chữ cố định của các prompt gửi mô hình ngôn ngữ (luật, khuôn câu) nằm trong file ở
backend/prompts/, mỗi prompt một file; module này nhúng các file đó vào binary và điền chỗ trống. Người gọi: luật của
guard (engine/guard/prompt_rules.rs), prompt sinh SQL và sửa SQL (service/ask/prompt.rs, ask/repair.rs), form câu "tại
sao" và sửa form (ask/why_prompt.rs, explain/form.rs), đoạn trả lời (ask/summary.rs), lời của phần giải thích
(insight/narrate.rs), phân loại câu hỏi (ask/classify.rs), mô hình chấm lời diễn giải (explain/judge.rs), hồ sơ ngữ
nghĩa (semantic/mod.rs).
Phần dữ liệu (dòng cột, danh sách giá trị, JSON, lượt hỏi trước) vẫn do các file đó dựng rồi điền vào chỗ trống.

Đặt ở engine vì cả guard (engine) lẫn service cần; service gọi xuống engine, không ngược lại.

File nhúng bằng include_str! lúc biên dịch, không đọc lúc chạy: binary cài bằng make install hay trong image Docker
không kèm thư mục backend/prompts/, và prompt không đổi được khi máy chủ đang chạy.

Chỗ trống viết `{{tên}}`, tên gồm chữ thường và gạch dưới. Không dùng một ngoặc như `{tên}` vì prompt hồ sơ ngữ nghĩa
có mẫu JSON (`{"columns": ...}`) viết ngoặc nhọn đơn. File thiếu giá trị cho một chỗ trống, giá trị truyền vào mà file
không dùng, `{{` không mở một chỗ trống hợp lệ, hay chỗ trống viết nhầm một ngoặc (`{question}`) đều làm fill panic:
tên chỗ trống và tên giá trị đều cố định trong mã, nên lỗi này luôn lộ ra ở test so mẫu (service/testdata/) chứ không
chờ tới lúc chạy. Giá trị điền vào không bị quét lại, nên dữ liệu có chữ `{{...}}` vẫn đi nguyên văn. */

/// Một file chữ trong backend/prompts/. name chỉ để lời báo lỗi nói đúng file.
pub struct PromptFile {
    pub name: &'static str,
    pub text: &'static str,
}

// Đường dẫn của include_str! tính từ file này (backend/src/engine/), nên ../../prompts là backend/prompts.

/// Luật của prompt sinh SQL (service/ask/prompt.rs).
pub const SQL: PromptFile = PromptFile {
    name: "sql.txt",
    text: include_str!("../../prompts/sql.txt"),
};

/// Quan hệ và luật nối bảng của prompt SQL khi câu hỏi đọc được bảng liên quan (service/ask/prompt.rs, #213).
pub const SQL_JOINS: PromptFile = PromptFile {
    name: "sql_joins.txt",
    text: include_str!("../../prompts/sql_joins.txt"),
};

/// Luật của trường unanswerable, chung cho prompt SQL và prompt form câu "tại sao".
pub const UNANSWERABLE_RULE: PromptFile = PromptFile {
    name: "unanswerable_rule.txt",
    text: include_str!("../../prompts/unanswerable_rule.txt"),
};

/// Luật của guard trong prompt SQL; các con số điền từ hằng số của guard (engine/guard/prompt_rules.rs).
pub const GUARD_RULES: PromptFile = PromptFile {
    name: "guard_rules.txt",
    text: include_str!("../../prompts/guard_rules.txt"),
};

/// Prompt sửa SQL (service/ask/repair.rs); problem là một trong hai file ngay dưới.
pub const SQL_REPAIR: PromptFile = PromptFile {
    name: "sql_repair.txt",
    text: include_str!("../../prompts/sql_repair.txt"),
};

pub const SQL_REPAIR_FAILED_SQL: PromptFile = PromptFile {
    name: "sql_repair_failed_sql.txt",
    text: include_str!("../../prompts/sql_repair_failed_sql.txt"),
};

pub const SQL_REPAIR_UNREADABLE_REPLY: PromptFile = PromptFile {
    name: "sql_repair_unreadable_reply.txt",
    text: include_str!("../../prompts/sql_repair_unreadable_reply.txt"),
};

/// Luật của prompt form câu "tại sao" (service/ask/why_prompt.rs).
pub const WHY_FORM: PromptFile = PromptFile {
    name: "why_form.txt",
    text: include_str!("../../prompts/why_form.txt"),
};

/// Prompt sửa form câu "tại sao" (service/explain/form.rs).
pub const WHY_FORM_REPAIR: PromptFile = PromptFile {
    name: "why_form_repair.txt",
    text: include_str!("../../prompts/why_form_repair.txt"),
};

/// Khối bài học đã duyệt (#228), chung cho prompt SQL, prompt form, prompt diễn giải câu "tại sao" và prompt lời của
/// phần giải thích câu thường (service/ask/lessons.rs).
pub const LESSONS: PromptFile = PromptFile {
    name: "lessons.txt",
    text: include_str!("../../prompts/lessons.txt"),
};

/// Prompt lời của phần giải thích câu thường (service/insight/narrate.rs, #227).
pub const INSIGHT: PromptFile = PromptFile {
    name: "insight.txt",
    text: include_str!("../../prompts/insight.txt"),
};

/// Prompt của mô hình chấm (cổng 2, #201): so lời với các khẳng định đã qua cổng 1 (service/explain/judge.rs). Luật
/// dùng chung cho hai loại câu; nghĩa các khóa của JSON điền vào chỗ {{meaning}} từ một trong hai file dưới.
pub const JUDGE_ANSWER: PromptFile = PromptFile {
    name: "judge_answer.txt",
    text: include_str!("../../prompts/judge_answer.txt"),
};

/// Nghĩa các khóa của JSON gửi mô hình chấm cho lời diễn giải câu "tại sao" (khẳng định trên bảng phân rã).
pub const JUDGE_MEANING_WHY: PromptFile = PromptFile {
    name: "judge_meaning_why.txt",
    text: include_str!("../../prompts/judge_meaning_why.txt"),
};

/// Nghĩa các khóa của JSON gửi mô hình chấm cho lời phần giải thích của câu thường (câu chính và các ý code đã viết,
/// service/insight/narrate.rs).
pub const JUDGE_MEANING_PLAIN: PromptFile = PromptFile {
    name: "judge_meaning_plain.txt",
    text: include_str!("../../prompts/judge_meaning_plain.txt"),
};

/// Prompt phân loại câu hỏi: câu thường hay câu hỏi tìm nguyên nhân (service/ask/classify.rs).
pub const CLASSIFY_QUESTION: PromptFile = PromptFile {
    name: "classify_question.txt",
    text: include_str!("../../prompts/classify_question.txt"),
};

/// Prompt hồ sơ ngữ nghĩa lúc nạp dataset (service/semantic/mod.rs).
pub const SEMANTIC_PROFILE: PromptFile = PromptFile {
    name: "semantic_profile.txt",
    text: include_str!("../../prompts/semantic_profile.txt"),
};

/// Chữ của file với mọi chỗ trống đã điền. values là các cặp (tên chỗ trống, giá trị). Panic khi file và values không
/// khớp nhau (xem đầu file): đó là lỗi lập trình, không phải lỗi của dữ liệu.
pub fn fill(file: &PromptFile, values: &[(&str, &str)]) -> String {
    match build_filled(file.text, values) {
        Ok(text) => text,
        Err(problem) => panic!("prompt file {}: {problem}", file.name),
    }
}

/// Phần làm việc của fill, trả lỗi thay vì panic để test kiểm được từng loại lỗi.
pub fn build_filled(template: &str, values: &[(&str, &str)]) -> Result<String, String> {
    // Trình soạn thảo thường lưu file kèm một dấu xuống dòng ở cuối; prompt thì không có, nên bỏ đúng một dấu đó.
    let template = template.strip_suffix('\n').unwrap_or(template);
    let mut text = String::with_capacity(template.len());
    let mut used = vec![false; values.len()];
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        check_literal(&rest[..start])?;
        text.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err("\"{{\" without a closing \"}}\"".to_string());
        };
        let name = &after[..end];
        if !is_placeholder_name(name) {
            return Err(format!("invalid placeholder {{{{{name}}}}}"));
        }
        // Tên trùng trong values: position chỉ thấy cặp đầu, cặp sau không bao giờ được dùng nên báo lỗi bên dưới.
        let Some(index) = values.iter().position(|(key, _)| *key == name) else {
            return Err(format!("no value for {{{{{name}}}}}"));
        };
        text.push_str(values[index].1);
        used[index] = true;
        rest = &after[end + 2..];
    }
    check_literal(rest)?;
    text.push_str(rest);
    if let Some(index) = used.iter().position(|was_used| !was_used) {
        return Err(format!("value {} is not used by the file", values[index].0));
    }
    Ok(text)
}

fn is_placeholder_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

/// Đoạn chữ thường của file không được có `{tên}` một ngoặc: đó gần như chắc là chỗ trống viết thiếu ngoặc, và để yên
/// thì mô hình nhận nguyên chữ "{question}" thay cho câu hỏi.
fn check_literal(literal: &str) -> Result<(), String> {
    for (start, _) in literal.match_indices('{') {
        let after = &literal[start + 1..];
        if let Some(end) = after.find('}') {
            let name = &after[..end];
            if is_placeholder_name(name) {
                return Err(format!(
                    "{{{name}}} has single braces; write {{{{{name}}}}}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled_and_the_final_newline_dropped() {
        let text = build_filled(
            "Table {{table}}, again {{table}}: {{question}}\n",
            &[("table", "sales"), ("question", "Doanh thu?")],
        );
        assert_eq!(text.unwrap(), "Table sales, again sales: Doanh thu?");
        // Chỉ bỏ một dấu xuống dòng cuối; dòng trống cố ý ở cuối vẫn giữ.
        assert_eq!(build_filled("a\n\n", &[]).unwrap(), "a\n");
    }

    /// Ngoặc nhọn đơn của mẫu JSON không phải chỗ trống.
    #[test]
    fn json_braces_stay_as_they_are() {
        let template =
            "{\"columns\": [{\"name\", \"unit\"}], \"values\": [...]}\nDataset {{dataset}}";
        let text = build_filled(template, &[("dataset", "\"DataCo\"")]).unwrap();
        assert_eq!(
            text,
            "{\"columns\": [{\"name\", \"unit\"}], \"values\": [...]}\nDataset \"DataCo\""
        );
    }

    /// Giá trị không bị quét lại: dữ liệu có chữ dạng chỗ trống vẫn đi nguyên văn, không lấy giá trị khác.
    #[test]
    fn values_are_not_scanned_again() {
        let text = build_filled(
            "Q: {{question}} T: {{table}}",
            &[("question", "{{table}} {x}"), ("table", "sales")],
        );
        assert_eq!(text.unwrap(), "Q: {{table}} {x} T: sales");
    }

    #[test]
    fn mismatches_are_errors() {
        let missing = build_filled("{{question}}", &[]).unwrap_err();
        assert!(missing.contains("no value for {{question}}"), "{missing}");
        let unused = build_filled("text", &[("question", "q")]).unwrap_err();
        assert!(unused.contains("question is not used"), "{unused}");
        let twice =
            build_filled("{{question}}", &[("question", "a"), ("question", "b")]).unwrap_err();
        assert!(twice.contains("not used"), "{twice}");
        let unclosed = build_filled("{{question", &[("question", "q")]).unwrap_err();
        assert!(unclosed.contains("closing"), "{unclosed}");
        let invalid = build_filled("{{Question Text}}", &[]).unwrap_err();
        assert!(invalid.contains("invalid placeholder"), "{invalid}");
        let single = build_filled("Question: {question}", &[("question", "q")]).unwrap_err();
        assert!(single.contains("single braces"), "{single}");
    }

    #[test]
    #[should_panic(expected = "prompt file sql.txt: no value for {{tables}}")]
    fn fill_panics_with_the_file_name() {
        fill(&SQL, &[("unanswerable_rule", "")]);
    }

    // Tên các chỗ trống `{{tên}}` trong một đoạn chữ, sắp xếp và bỏ trùng, để so bản dịch với bản gốc.
    fn placeholder_names(text: &str) -> Vec<String> {
        let mut names: Vec<String> = text
            .split("{{")
            .skip(1)
            .filter_map(|rest| rest.split_once("}}").map(|(name, _)| name.to_string()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// backend/prompts/vi/ là bản dịch tiếng Việt chỉ để đọc (không nhúng vào binary). Mỗi prompt phải có bản dịch cùng
    /// tên, cùng các chỗ trống; ngược lại không có bản dịch nào cho file đã bỏ. Nghĩa của câu dịch thì test không kiểm.
    #[test]
    fn vietnamese_mirrors_match_the_prompts() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompts");
        let txt_names = |path: &std::path::Path| -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(path)
                .expect("read prompt directory")
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| name.ends_with(".txt"))
                .collect();
            names.sort();
            names
        };
        let prompts = txt_names(&dir);
        assert!(!prompts.is_empty());
        assert_eq!(
            prompts,
            txt_names(&dir.join("vi")),
            "every prompt needs a mirror in prompts/vi/ and no extra file"
        );
        for name in &prompts {
            let english = std::fs::read_to_string(dir.join(name)).expect("read prompt");
            let vietnamese =
                std::fs::read_to_string(dir.join("vi").join(name)).expect("read mirror");
            assert_eq!(
                placeholder_names(&english),
                placeholder_names(&vietnamese),
                "placeholders differ between prompts/{name} and prompts/vi/{name}"
            );
        }
    }
}
