# Đóng góp

Cách chạy, test và gửi PR. Quy tắc code, nhánh, commit và PR ở [AGENTS.md](AGENTS.md); trang này không chép lại.

## Trước khi bắt đầu

- Lỗi: mở [Issue](https://github.com/pgiahuy/xdi-platform/issues) theo mẫu. Ghi phiên bản, cách chạy, lệnh và thông
  báo nguyên văn.
- Tính năng hoặc ý tưởng: đăng ở [Discussions mục Ideas](https://github.com/pgiahuy/xdi-platform/discussions) trước,
  chốt rồi mới mở Issue. Câu hỏi: Discussions mục Q&A.
- Lỗ hổng bảo mật: không mở Issue, báo riêng theo [SECURITY.md](SECURITY.md).

Mỗi PR ứng với một Issue. Việc lớn tách thành nhiều Issue con.

## Chạy trên máy

Cần Rust stable kèm trình biên dịch C++ (DuckDB biên dịch từ mã nguồn), Node.js 24 (bản CI dùng), Docker và GNU Make.
Sau khi clone, chạy `make hooks` một lần để hook pre-push chặn lỗi trước khi push.

```bash
make dev            # Postgres trong Docker, backend chạy bằng cargo run
make dev-frontend   # giao diện ở http://localhost:5173, /api chuyển sang backend
make check          # check-fast, check-frontend và build image Docker
make check-fast     # fmt, clippy, test backend
make check-frontend # lint, schema.ts khớp openapi.json, kiểm kiểu, build giao diện
make eval           # câu mẫu với mock trên DataCo (và database mẫu nếu đang chạy), so với SQL đúng
make help           # mọi lệnh
```

Các bộ đánh giá khác (`eval-judge`, `eval-classify`, `eval-lessons`, bộ 57 câu OULAD): [eval/README.md](eval/README.md).

CI chạy `make check-fast` và `make eval` khi PR đổi `backend/` hay file backend đọc lúc test (`data/domains/`, `eval/`,
`openapi.json` và vài file khác, danh sách ở `.github/workflows/ci.yml`), và `make check-frontend` khi PR đổi
`frontend/`, `openapi.json` hoặc `Makefile`. CI không build image Docker; workflow "Docker image" chạy tay ở tab
Actions.

## Test cần Postgres

Test cơ sở dữ liệu cần Postgres thật; không có `DATABASE_URL` thì chúng tự bỏ qua trên máy (CI luôn chạy). Test chạy
migration của nhánh và ghi dữ liệu vào database đó, nên dùng một database riêng, không dùng database `xdi` của
`make dev`:

```bash
docker compose up -d postgres
docker compose exec -T postgres createdb -U xdi xdi_test
DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi_test make test
```

## Migration

Thay đổi schema Postgres là một file migration mới trong `backend/migrations/`, đánh số tiếp theo file mới nhất
(`ls backend/migrations`). Không sửa file migration đã merge: database đã áp nó sẽ từ chối khởi động vì checksum không
khớp. Cùng PR đó sửa [docs/SCHEMA.md](docs/SCHEMA.md) và chạy `make schema-docs` (cần tbls, Graphviz) để sinh lại sơ
đồ ER.

## Đổi API, thư viện, gói ngành

- Đổi API: `make openapi` rồi commit `openapi.json`, `frontend/src/api/schema.ts`, `frontend/src/api/examples/ask.json`.
  Quên thì test của `make check-fast` hoặc `make check-frontend` báo lỗi.
- Thư viện mới phải có giấy phép tương thích Apache 2.0 (MIT, BSD, Apache, ISC, PostgreSQL). Đổi `Cargo.lock` hay
  `package-lock.json` thì chạy `make licenses` để sinh lại `docs/licenses-dependencies.md`; giấy phép ngoài nhóm thông
  dụng thì ghi đánh giá vào [docs/LICENSES.md](docs/LICENSES.md).
- Thêm gói khái niệm ngành: [docs/DOMAINS.md](docs/DOMAINS.md).

## Giấy phép

Đóng góp của bạn được phát hành theo [Apache License 2.0](LICENSE) cùng với dự án.
