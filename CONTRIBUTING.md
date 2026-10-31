# Đóng góp

Cảm ơn bạn đã quan tâm. Tài liệu này nói cách làm việc trong repo. Quy tắc code chi tiết ở [AGENTS.md](AGENTS.md).

## Trước khi bắt đầu

- Lỗi: mở [Issue](https://github.com/pgiahuy/xdi-platform/issues) theo mẫu. Ghi phiên bản, cách chạy, lệnh và thông
  báo nguyên văn.
- Tính năng hoặc ý tưởng: đăng ở [Discussions mục Ideas](https://github.com/pgiahuy/xdi-platform/discussions) trước,
  chốt rồi mới mở Issue.
- Câu hỏi: Discussions mục Q&A.
- Lỗ hổng bảo mật: không mở Issue, báo riêng theo [SECURITY.md](SECURITY.md).

Mỗi PR ứng với một Issue. Việc lớn tách thành nhiều Issue con.

## Chạy trên máy

Cần Rust stable kèm trình biên dịch C++ (DuckDB biên dịch từ mã nguồn), Node.js 24 (hoặc 22.22 trở lên), Docker và
GNU Make. Sau khi clone, chạy `make hooks` một lần để lỗi bị chặn trên máy trước khi push. Tóm tắt:

```bash
make dev            # Postgres trong Docker, backend chạy bằng cargo run
make dev-frontend   # giao diện ở http://localhost:5173, /api chuyển sang backend
make check          # check-fast, check-frontend và build image docker
make check-fast     # fmt, clippy, test backend
make check-frontend # lint, schema.ts khớp openapi.json, kiểm kiểu, build giao diện
make eval           # hỏi các câu mẫu với mock trên DataCo (và trên database mẫu nếu đang chạy), so với SQL đúng
make hooks          # cài hook pre-push: check-fast, thêm check-frontend khi nhánh đổi frontend/, openapi.json, Makefile
```

CI chạy `make check-fast` và `make eval` khi PR đổi `backend/` hay file backend đọc lúc test (`data/domains/`, `eval/`,
`openapi.json` và vài file khác, danh sách ở `.github/workflows/ci.yml`), và `make check-frontend` khi PR đổi
`frontend/`, `openapi.json` hoặc `Makefile`. CI không build image Docker; workflow "Docker image" chạy tay ở tab
Actions.

Thay đổi schema Postgres là một file migration mới trong `backend/migrations/`, đánh số tiếp theo file mới nhất, ví dụ
`0009_mo_ta.sql`. Không sửa file migration đã merge: mọi database đã áp nó sẽ từ chối khởi động vì checksum không khớp.

Test cơ sở dữ liệu cần Postgres thật. Không có `DATABASE_URL` thì chúng tự bỏ qua trên máy (CI luôn chạy). Test chạy
migration của nhánh và ghi dữ liệu vào database đó, nên dùng một database riêng, không dùng database `xdi` của
`make dev`:

```bash
docker compose up -d postgres
docker compose exec -T postgres createdb -U xdi xdi_test
DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi_test make test
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
- Nếu đổi API, chạy `make openapi` và commit `openapi.json`, kiểu TypeScript sinh ra (`frontend/src/api/schema.ts`)
  và response mẫu của hỏi dữ liệu (`frontend/src/api/examples/ask.json`). Quên thì test của `make check-fast`
  (`openapi.json`, `ask.json`) hoặc `make check-frontend` (`schema.ts`) báo lỗi. Cách làm ở
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) mục 8.
- CI phải xanh. Merge bằng Squash and merge, nhánh tự xóa.

## Code

- Rust: `cargo fmt`, `cargo clippy` không cảnh báo. Mỗi file mở đầu bằng chú thích nêu vai trò. Chú thích giải thích
  vì sao, không kể lại mã làm gì.
- Tên biến, hàm, trường JSON, mã lỗi, biến môi trường, thông điệp log: tiếng Anh. Chú thích và tài liệu: tiếng Việt.
- Không ghi cứng chuỗi, đường dẫn, cấu hình trong code.
- Không commit khóa hay bí mật. `.env` nằm trong `.gitignore`; mẫu ở `.env.example`.
- Thư viện mới phải có giấy phép tương thích Apache 2.0 (MIT, BSD, Apache, ISC, PostgreSQL). Đổi `Cargo.lock` hay
  `package-lock.json` thì chạy `make licenses` để sinh lại `docs/licenses-dependencies.md`; giấy phép ngoài nhóm cho
  phép thông dụng thì ghi thêm đánh giá vào [docs/LICENSES.md](docs/LICENSES.md).

## Giấy phép

Đóng góp của bạn được phát hành theo [Apache License 2.0](LICENSE) cùng với dự án.
