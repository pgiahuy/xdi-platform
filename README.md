# xdi-platform

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên và giải thích kết quả, đang phát triển. Mục tiêu: nạp một file CSV,
Parquet hoặc Excel, hỏi bằng tiếng Việt, nhận bảng, biểu đồ, câu SQL đã chạy và các dòng dữ liệu đứng sau mỗi con số.
Với câu hỏi "tại sao", hệ thống tính nhóm nào làm con số tăng hay giảm. Con số do engine phân tích tính trên dữ liệu;
mô hình ngôn ngữ chỉ chuyển câu hỏi thành SQL và diễn giải kết quả.

Trạng thái: mới có khung backend và khung giao diện. Tiến độ ở [milestone v0.1.0](https://github.com/pgiahuy/xdi-platform/milestone/1),
lịch sử thay đổi ở [CHANGELOG.md](CHANGELOG.md).

Backend Rust (Axum, DuckDB nhúng, Postgres), frontend React với Vite. Giấy phép Apache 2.0.

Website và tài liệu: https://pgiahuy.github.io/xdi-platform/

## Chạy bằng Docker

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up --build
```

Mở http://localhost:8000: trang chủ hiện phiên bản máy chủ và nhà cung cấp LLM đang dùng. API nằm dưới `/api`, ví dụ
`curl localhost:8000/api/health` trả `{"status":"ok"}`.

`LLM_PROVIDER` chọn mô hình ngôn ngữ: `mock` (mặc định, không cần khóa API), `gemini` (cần khóa), `ollama` (cần một máy
chạy Ollama).

## Build từ nguồn

Cần Rust (stable), Docker cho Postgres, và GNU Make.

```bash
make dev          # Postgres trong Docker, backend chạy bằng cargo run
make check-fast   # fmt, clippy, test
make help         # mọi target
```

Biến môi trường: [.env.example](.env.example). Hướng dẫn cài đặt đầy đủ đang viết ([#54](https://github.com/pgiahuy/xdi-platform/issues/54)).

## Cấu trúc

```
backend/     crate Rust: api/ -> service/ -> engine/ và auth/, migrations/ cho Postgres
frontend/    React, Vite, Tailwind, shadcn/ui; build ra file tĩnh
docs/        tài liệu, cũng là nguồn GitHub Pages
data/        nguồn và giấy phép dataset, download.sh, samples/
Makefile     lối vào duy nhất cho build, test, chạy; CI gọi đúng các target này
```

Kiến trúc, luồng một câu hỏi, hướng phát triển: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues).
- Câu hỏi: [Discussions, Q&A](https://github.com/pgiahuy/xdi-platform/discussions/categories/q-a).
- Ý tưởng: [Discussions, Ideas](https://github.com/pgiahuy/xdi-platform/discussions/categories/ideas).
- Thông báo phát hành: [Discussions, Announcements](https://github.com/pgiahuy/xdi-platform/discussions/categories/announcements).
- Đóng góp mã: [CONTRIBUTING.md](CONTRIBUTING.md). Quy tắc làm việc: [AGENTS.md](AGENTS.md).

## Giấy phép

[Apache License 2.0](LICENSE). Bảng giấy phép của từng thành phần đang viết ([#57](https://github.com/pgiahuy/xdi-platform/issues/57)).
