# xdi-platform

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên, đang phát triển, chưa có bản phát hành. Nạp một file CSV, Parquet
hoặc Excel, hỏi bằng tiếng Việt, nhận bảng, biểu đồ và câu SQL đã chạy. Với câu hỏi "tại sao", hệ thống tính nhóm nào
làm con số tăng hay giảm và cho xem các dòng dữ liệu đứng sau từng nhóm. Con số do engine phân tích (DuckDB) tính trên
dữ liệu; mô hình ngôn ngữ chỉ viết SQL hay điền form từ câu hỏi, viết mô tả cột và lời giải thích, không nhận các dòng
dữ liệu ([docs/PRIVACY.md](docs/PRIVACY.md)).

## Trạng thái

Đã có trên `main`:

- Tài khoản và workspace: màn thiết lập lần đầu, đăng nhập, mỗi người nhiều workspace, hai vai `admin` (nạp dữ liệu,
  quản lý thành viên) và `member` (xem, hỏi), trang quản trị tài khoản cho quản trị hệ thống.
- Nạp dữ liệu: CSV (UTF-8, windows-1252), Parquet, Excel .xlsx kèm chọn sheet; mỗi lần nạp là một phiên bản. Hồ sơ
  cột (kiểu, vai trò, giá trị) lập lúc nạp; mô hình ngôn ngữ thêm mô tả, bí danh, đơn vị của cột số đo và câu hỏi gợi ý.
- Hỏi dữ liệu: câu hỏi thành SQL, qua guard (một câu `SELECT` chỉ đọc trên bảng, cột, hàm được phép, có trần số dòng)
  rồi chạy trên DuckDB, mô hình sửa một lần khi câu lỗi; câu có chữ khớp nhiều cột thì hỏi lại để chọn. Kết quả có bảng
  phân trang, tải CSV, biểu đồ cột hoặc đường, thẻ "Đã hiểu là" (đo gì, lọc gì, nhóm theo gì, khoảng thời gian) và pane
  bằng chứng với câu SQL trước và sau guard cùng các bước đã chạy. Mọi câu hỏi được ghi vào lịch sử trong database.
- Câu "tại sao": mô hình điền form (đo gì, hai kỳ hay hai nhóm nào), code kiểm form và tự dựng mọi SQL, tính phần đóng
  góp của từng nhóm theo từng chiều (tổng đóng góp bằng đúng chênh lệch) và tách lượng, giá; mô hình viết lời giải
  thích, code kiểm mọi số trong lời có trong bảng. Bấm một dòng phân rã để xem dòng nguồn.
- Mô hình ngôn ngữ: `mock` (không cần khóa, trả lời bộ câu hỏi mẫu trên dataset DataCo), Gemini, Ollama.

Chưa có trên `main`:

- Bấm một ô của bảng kết quả để xem dòng nguồn: API đã có, giao diện chưa gọi.
- Gán vai dữ liệu cho thành viên: guard đã lọc dòng theo vai dữ liệu khi hỏi và khi xem dòng nguồn, nhưng chưa có API
  hay màn nào đặt `data_role` và chính sách lọc.
- Cảnh báo kiểm hợp lý kết quả, chế độ demo, dataset mẫu đóng gói sẵn, màn xem lịch
  sử câu hỏi, màn sửa hồ sơ cột và định nghĩa chỉ số.

Tiến độ ở [milestone v0.1.0](https://github.com/pgiahuy/xdi-platform/milestone/1), lịch sử thay đổi ở
[CHANGELOG.md](CHANGELOG.md), bảng trạng thái chi tiết ở cuối [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#trạng-thái).

Backend Rust (Axum, DuckDB nhúng, Postgres), frontend React với Vite. Giấy phép Apache 2.0.

Website và tài liệu: https://pgiahuy.github.io/xdi-platform/

## Chạy bằng Docker

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up --build
```

Build lần đầu biên dịch backend Rust kèm DuckDB, cần khoảng 1 GiB RAM trống cho mỗi nhân CPU; máy ít RAM thì đặt
`CARGO_BUILD_JOBS` trong `.env` để giới hạn số tiến trình biên dịch, xem mục Lỗi hay gặp của
[docs/INSTALL.md](docs/INSTALL.md).

Mở http://localhost:8000. Lần đầu giao diện hiện màn thiết lập: nhập mã thiết lập in trong log của backend
(`docker compose logs backend | grep setup_code`) để tạo tài khoản quản trị và workspace đầu tiên. API nằm dưới `/api`,
ví dụ `curl localhost:8000/api/health` trả `{"status":"ok"}`. Đặc tả OpenAPI ở `/api/openapi.json`, xem và gọi thử trên
trình duyệt ở http://localhost:8000/api/docs.

`LLM_PROVIDER` chọn mô hình ngôn ngữ: `mock` (mặc định, không cần khóa API), `gemini` (cần khóa), `ollama` (cần một máy
chạy Ollama).

Từ màn thiết lập tới câu trả lời đầu tiên (chọn mô hình, nạp dataset, hỏi, đọc bằng chứng, mời thành viên): mục 7 của
[docs/INSTALL.md](docs/INSTALL.md).

## Build từ nguồn

Cần Rust stable (từ 1.89) kèm trình biên dịch C++, Node.js 24, Docker cho Postgres, và GNU Make.

```bash
make dev          # Postgres trong Docker, backend chạy bằng cargo run
make check-fast   # fmt, clippy, test
make help         # mọi target
```

Biến môi trường: [.env.example](.env.example). Hướng dẫn cài đặt đầy đủ (Docker, build từ nguồn, đưa lên Internet với
HTTPS, sao lưu, lỗi hay gặp): [docs/INSTALL.md](docs/INSTALL.md).

## Cấu trúc

```
backend/     crate Rust: api/ -> service/ -> engine/ và auth/, migrations/ cho Postgres
frontend/    React, Vite, Tailwind, shadcn/ui; build ra file tĩnh
docs/        tài liệu, cũng là nguồn GitHub Pages
data/        nguồn và giấy phép dataset, download.sh và samples/ (chưa có dataset), domains/ (gói khái niệm ngành)
Makefile     lối vào duy nhất cho build, test, chạy; CI gọi đúng các target này
```

Kiến trúc, luồng một câu hỏi, hướng phát triển: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
API, mã lỗi: [docs/API.md](docs/API.md). Guard SQL: [docs/GUARD.md](docs/GUARD.md). Định dạng dữ liệu nhận,
phiên bản, tên cột: [docs/DATA.md](docs/DATA.md).

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues).
- Câu hỏi: [Discussions, Q&A](https://github.com/pgiahuy/xdi-platform/discussions/categories/q-a).
- Ý tưởng: [Discussions, Ideas](https://github.com/pgiahuy/xdi-platform/discussions/categories/ideas).
- Thông báo phát hành: [Discussions, Announcements](https://github.com/pgiahuy/xdi-platform/discussions/categories/announcements).
- Đóng góp mã: [CONTRIBUTING.md](CONTRIBUTING.md). Quy tắc làm việc: [AGENTS.md](AGENTS.md).
- Lỗ hổng bảo mật: báo riêng theo [SECURITY.md](SECURITY.md), không mở issue công khai.

## Giấy phép

[Apache License 2.0](LICENSE). Giấy phép của các thành phần bên thứ ba: [docs/LICENSES.md](docs/LICENSES.md). Dữ liệu
nằm ở đâu và gửi gì cho mô hình ngôn ngữ: [docs/PRIVACY.md](docs/PRIVACY.md).
