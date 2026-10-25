// SPDX-License-Identifier: Apache-2.0
/* build.rs - nhúng mọi gói khái niệm ngành (các file .json trong data/domains/, tính từ gốc repo) vào binary.

Các file JSON nằm ngoài crate nên không tự vào binary; mà image Docker không chép thư mục data/ và binary cài bằng
make install cũng không kèm data/, nên đọc file lúc chạy sẽ không có file để đọc (cùng lý do engine/llm/mock.rs nhúng
mock_answers.json). build.rs liệt kê thư mục và sinh $OUT_DIR/domain_packages.rs chứa một include_str! cho từng file,
nên thêm một ngành chỉ cần thêm một file JSON, không sửa code. Dockerfile phải chép data/domains/ vào stage builder,
vì build.rs ở đó không thấy gốc repo.

Mọi đường dẫn ghi vào file sinh và rerun-if-changed đều tương đối theo CARGO_MANIFEST_DIR: nhiều bản checkout (worktree)
có thể dùng chung một CARGO_TARGET_DIR, đường dẫn tuyệt đối sẽ làm bản này nhúng gói của bản khác. Dù vậy cargo vẫn có
thể dùng lại kết quả build script cũ khi thư mục data/domains không thay đổi, nên với target dùng chung, thêm hay sửa
file gói thì chạy lại bằng: touch backend/src/main.rs data/domains. Phải touch cả thư mục data/domains, không chỉ
main.rs: rerun-if-changed trỏ vào thư mục của bản checkout này, nên một bản khác thêm file gói thì thư mục của bản này
không đổi, build script không tự chạy lại. */

use std::{env, fs, path::Path};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let domains_dir = Path::new(&manifest_dir).join("../data/domains");
    if !domains_dir.is_dir() {
        panic!("data/domains/ not found; the Docker build must COPY data/domains/ next to the crate (backend/Dockerfile)");
    }
    // Đường tương đối: cargo tính từ thư mục crate. Khi trỏ vào một thư mục, cargo dò thay đổi của mọi file trong đó,
    // kể cả file mới thêm.
    println!("cargo:rerun-if-changed=../data/domains");

    let mut files: Vec<_> = fs::read_dir(&domains_dir)
        .expect("cannot list data/domains/")
        .map(|entry| entry.expect("cannot read an entry of data/domains/").path())
        // Chỉ nhận file thường đuôi .json: một thư mục tên x.json hay symlink hỏng (file khóa .#retail.json của Emacs)
        // sẽ làm rustc báo "Is a directory" khó hiểu nếu đưa vào include_str!.
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    // Sắp xếp để danh sách nhúng ổn định giữa các lần build, không tùy thứ tự hệ thống file trả về.
    files.sort();

    let mut generated = String::from("// Sinh bởi build.rs, không sửa tay.\n");
    generated.push_str("pub(crate) const DOMAIN_PACKAGE_FILES: &[(&str, &str)] = &[\n");
    for file in &files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .expect("domain package file name must be UTF-8");
        let id = name
            .strip_suffix(".json")
            .expect("entries are filtered by the .json extension");
        generated.push_str(&format!(
            "    ({id:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../data/domains/\", {name:?}))),\n"
        ));
    }
    generated.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo for build scripts");
    fs::write(Path::new(&out_dir).join("domain_packages.rs"), generated)
        .expect("cannot write the generated domain package list");
}
