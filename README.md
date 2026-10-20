# xdi-platform

Phân tích dữ liệu bằng ngôn ngữ tự nhiên, có bằng chứng cho từng con số. Nạp một file CSV, Parquet hoặc Excel, hỏi
bằng tiếng Việt, nhận bảng, biểu đồ, câu SQL đã chạy, và dòng dữ liệu đứng sau mỗi con số. Với câu hỏi "tại sao",
hệ thống tự tính nhóm nào kéo con số đó lên hay xuống.

Số không do mô hình ngôn ngữ bịa. Mô hình chỉ dịch câu hỏi thành SQL và diễn giải kết quả. Số do engine tính.

Backend Rust (Axum, DuckDB nhúng, Postgres), frontend Next.js. Giấy phép Apache 2.0.

Website và tài liệu: https://pgiahuy.github.io/xdi-platform/

## Chạy bằng Docker

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up --build
```

Kiểm tra: `curl localhost:8000/health` trả `{"status":"ok"}`, `curl localhost:8000/version` trả phiên bản và nhà cung
cấp LLM đang dùng.

Dự án đang ở giai đoạn khung: backend chạy được, các tính năng nạp dữ liệu, hỏi, giải thích và giao diện đang được làm
(xem [milestone v0.1.0](https://github.com/pgiahuy/xdi-platform/milestone/1)). Khi xong, chế độ mặc định
`LLM_PROVIDER=mock` sẽ trả lời được các câu hỏi mẫu trên dataset mẫu mà không cần khóa API; `gemini` cần khóa, `ollama`
cần một máy chạy Ollama.

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
frontend/    Next.js App Router (đang thêm)
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

## Trạng thái

Đang ở giai đoạn khung. Lịch sử thay đổi: [CHANGELOG.md](CHANGELOG.md).

## Giấy phép

[Apache License 2.0](LICENSE). Bảng giấy phép của từng thành phần đang viết ([#57](https://github.com/pgiahuy/xdi-platform/issues/57)).
