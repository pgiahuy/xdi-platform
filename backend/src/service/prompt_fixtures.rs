// SPDX-License-Identifier: Apache-2.0
/* service/prompt_fixtures.rs - so prompt dựng ra với file mẫu trong service/testdata/, dùng chung cho test của mọi
prompt gửi mô hình (ask/prompt_tests.rs, ask/run_tests.rs, ask/summary_tests.rs, explain/check_tests.rs,
semantic/tests.rs). Chỉ biên dịch khi chạy test.

File mẫu là prompt đầy đủ của một đầu vào cố định: đổi chữ prompt thì diff của PR hiện đúng chữ gửi cho mô hình, và
đổi cách dựng prompt mà không đổi chữ (như đưa chữ ra backend/prompts/) thì file mẫu không đổi một byte. Đổi chữ có
chủ ý thì chạy `UPDATE_PROMPT_FIXTURES=1 cargo test matches_the_fixture` để ghi lại các file rồi đọc diff. */

/// So prompt với file mẫu service/testdata/<name>; biến UPDATE_PROMPT_FIXTURES có thì ghi file trước khi so. File chưa
/// có thì so với chuỗi rỗng, nên test mới không tự qua khi quên tạo file.
pub(crate) fn check_fixture(name: &str, prompt: &str) {
    let path = format!("{}/src/service/testdata/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UPDATE_PROMPT_FIXTURES").is_ok() {
        std::fs::write(&path, prompt).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        prompt, expected,
        "prompt changed; run UPDATE_PROMPT_FIXTURES=1 cargo test matches_the_fixture and review the diff"
    );
}
