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

Cần Rust stable, Node.js 24 (hoặc 22.12 trở lên), Docker và GNU Make. Sau khi clone, chạy `make hooks` một lần để
lỗi bị chặn trên máy trước khi push. Tóm tắt:

```bash
make dev        # Postgres trong Docker, backend chạy bằng cargo run
make dev-frontend # giao diện ở http://localhost:5173, /api chuyển sang backend
make check      # check-fast, check-frontend và build image docker
make check-fast # fmt, clippy, test backend; CI chạy lệnh này trên mỗi PR
make check-frontend # lint, kiểm kiểu, build giao diện; CI chạy khi PR đổi frontend/
make hooks      # cài hook pre-push: check-fast, thêm check-frontend khi nhánh đổi frontend/
```

Thay đổi schema Postgres là một file migration mới trong `backend/migrations/`, đặt tên `0002_mo_ta.sql`, `0003_...`.
Không sửa file migration đã merge: mọi database đã áp nó sẽ từ chối khởi động vì checksum không khớp.

Test cơ sở dữ liệu cần Postgres thật. Không có `DATABASE_URL` thì chúng tự bỏ qua trên máy (CI luôn chạy):

```bash
docker compose up -d postgres
DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi make test
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
- Nếu đổi API, chạy `make openapi` và commit `openapi.json` cùng kiểu TypeScript sinh ra (`frontend/src/api/schema.ts`).
  Quên thì `make check-fast` và CI báo lỗi. Cách làm ở [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) mục 8.
- CI phải xanh. Merge bằng Squash and merge, nhánh tự xóa.

## Code

- Rust: `cargo fmt`, `cargo clippy` không cảnh báo. Mỗi file mở đầu bằng chú thích nêu vai trò. Chú thích giải thích
  vì sao, không kể lại mã làm gì.
- Tên biến, hàm, trường JSON, mã lỗi, biến môi trường, thông điệp log: tiếng Anh. Chú thích và tài liệu: tiếng Việt.
- Không ghi cứng chuỗi, đường dẫn, cấu hình trong code.
- Không commit khóa hay bí mật. `.env` nằm trong `.gitignore`; mẫu ở `.env.example`.
- Thư viện mới phải có giấy phép tương thích Apache 2.0 (MIT, BSD, Apache, ISC, PostgreSQL) và được ghi vào bảng giấy phép thành phần.

## Giấy phép

Đóng góp của bạn được phát hành theo [Apache License 2.0](LICENSE) cùng với dự án.
