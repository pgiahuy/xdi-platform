# Đóng góp

Cảm ơn bạn đã quan tâm. Tài liệu này nói cách làm việc trong repo. Quy tắc code chi tiết ở [AGENTS.md](AGENTS.md).

## Trước khi bắt đầu

- Lỗi: mở [Issue](https://github.com/pgiahuy/xdi-platform/issues) theo mẫu. Ghi phiên bản, cách chạy, lệnh và thông
  báo nguyên văn.
- Tính năng hoặc ý tưởng: đăng ở [Discussions mục Ideas](https://github.com/pgiahuy/xdi-platform/discussions) trước,
  chốt rồi mới mở Issue.
- Câu hỏi: Discussions mục Q&A.

Mỗi PR ứng với một Issue. Việc lớn tách thành nhiều Issue con.

## Chạy trên máy

Xem [docs/INSTALL.md](docs/INSTALL.md) mục build từ nguồn. Tóm tắt:

```bash
make dev        # Postgres trong Docker, backend và frontend chạy local
make check      # fmt, clippy, test, lint, build, so openapi.json; CI chạy đúng lệnh này
make check-fast # bỏ build docker và frontend
make hooks      # cài hook pre-push chạy check-fast (tùy chọn)
```

## Nhánh và commit

- Nhánh từ `main`: `<type>/<mô-tả-ngắn>` hoặc `<type>/<số-issue>-<mô-tả>`. Ví dụ `feat/ingest-xlsx`,
  `fix/42-guard-cte-alias`, `docs/install`.
- Commit trong nhánh viết thế nào cũng được vì sẽ được gộp khi merge.
- `git pull --rebase` trước khi push. Không `push --force` lên `main`.

## Pull request

- Tiêu đề tiếng Anh theo Conventional Commits: `type(scope): mô tả`. Type: `feat`, `fix`, `refactor`, `docs`,
  `chore`, `test`. Scope là module hoặc khu vực: `guard`, `ingest`, `explain`, `api`, `frontend`, `ci`.
- Nội dung tiếng Việt: thay đổi gì, vì sao, đã kiểm thế nào. Nội dung này thành thông điệp commit trên `main`.
- Một PR một việc. Nên dưới khoảng 500 dòng mã thay đổi, không tính file sinh tự động, test và dữ liệu. Lớn hơn thì
  tách theo lớp: lõi, giao diện, dữ liệu.
- Nếu thay đổi nhìn thấy được với người dùng, thêm một dòng vào mục `Unreleased` của `CHANGELOG.md` trong cùng PR.
- Nếu đổi API, chạy `make openapi` và commit `openapi.json` cùng kiểu TypeScript sinh ra.
- CI phải xanh. Merge bằng Squash and merge, nhánh tự xóa.

## Code

- Rust: `cargo fmt`, `cargo clippy` không cảnh báo. Mỗi file mở đầu bằng chú thích nêu vai trò. Chú thích giải thích
  vì sao, không kể lại mã làm gì.
- Tên biến, hàm, trường JSON, mã lỗi, biến môi trường, thông điệp log: tiếng Anh. Chú thích và tài liệu: tiếng Việt.
- Không ghi cứng chuỗi, đường dẫn, cấu hình trong code.
- Không commit khóa hay bí mật. `.env` nằm trong `.gitignore`; mẫu ở `.env.example`.
- Thư viện mới phải có giấy phép tương thích Apache 2.0 và được ghi vào `docs/LICENSES.md`.

## Giấy phép

Đóng góp của bạn được phát hành theo [Apache License 2.0](LICENSE) cùng với dự án.
