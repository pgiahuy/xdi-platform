# xdi-platform

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên: hỏi bằng tiếng Việt, nhận câu trả lời kèm lời giải thích và minh
chứng cho từng con số.

Kết nối database Postgres của tổ chức (hay nạp file CSV, Parquet, Excel) rồi gõ câu hỏi. Hệ thống viết SQL, nối nhiều
bảng theo khóa ngoại khi cần, kiểm SQL bằng guard, chạy trên DuckDB và trả bảng, biểu đồ cùng câu SQL đã chạy. Với câu
hỏi nguyên nhân, như vì sao doanh thu tháng này giảm, hệ thống tính phần chênh lệch của từng nhóm; bấm vào một số để
xem các dòng nguồn đứng sau nó.

Con số do DuckDB tính trên máy của bạn. Mô hình ngôn ngữ nhận metadata của bảng và cột, danh sách giá trị của cột phân
loại không nhạy cảm và các con số code đã tính; nó không nhận nội dung file hay bảng. Những gì gửi cho mô hình, kể cả
các ngoại lệ: [docs/PRIVACY.md](docs/PRIVACY.md).

Bản phát hành mới nhất: [v0.2.0](https://github.com/pgiahuy/xdi-platform/releases). Giấy phép Apache 2.0. Tài liệu:
https://pgiahuy.github.io/xdi-platform/

![Một câu hỏi đi qua xdi-platform: hàng trên đi từ giao diện qua dựng ngữ cảnh, mô hình, guard SQL tới DuckDB, hàng dưới quay về qua phần giải thích, kiểm số, ghi lịch sử; 16 bước](docs/images/luong-cau-hoi.png)

## Chạy thử

Cần Docker kèm Docker Compose v2 và Git.

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
docker compose logs backend | grep setup_code
```

Mở http://localhost:8000 và nhập mã thiết lập vừa in để tạo tài khoản quản trị. Workspace đầu tiên có dataset mẫu
DataCo (1500 dòng). Mô hình mặc định `mock` không cần khóa và chỉ trả lời các câu mẫu, ví dụ "Doanh thu theo thị
trường". Hỏi câu tự do: đặt `LLM_PROVIDER=gemini` và `GEMINI_API_KEY` (hay `ollama`) trong `.env`, rồi chạy lại
`docker compose up -d`.

Thử với database mẫu nhiều bảng `sales_demo` (DataCo) và `edu_demo` (OULAD): nạp một lần theo
[data/postgres-demo](data/postgres-demo/README.md), rồi chạy `docker compose --profile demo up -d --build` trước lần
thiết lập đầu. Thiết lập lần đầu thêm hai database này thành hai dataset và chụp mọi bảng.

Cài đặt đầy đủ: [docs/INSTALL.md](docs/INSTALL.md). Dùng lần đầu: [docs/USAGE.md](docs/USAGE.md).

## Đã có

- Tài khoản, workspace, vai `admin` và `member`; vai dữ liệu lọc dòng theo bảng, cột, giá trị cho từng thành viên.
- Kết nối Postgres (TLS, tài khoản chỉ đọc), chụp mọi bảng đọc được thành phiên bản Parquet; mỗi database là một
  dataset. Nạp file CSV, Parquet, Excel.
- Hỏi theo cuộc trò chuyện trên mọi bảng của dataset. SQL qua guard rồi chạy trên DuckDB; kết quả có bảng, biểu đồ,
  CSV và thẻ "Cách tính".
- Câu hỏi nguyên nhân: mô hình điền form, code dựng mọi SQL và tính phân rã chênh lệch theo từng nhóm.
- Lời giải thích qua kiểm số (code so từng số với bảng đã tính) và kiểm nghĩa (một lần gọi mô hình riêng chấm nghĩa).
- Minh chứng: SQL trước và sau guard, phân rã, các bước, dòng nguồn của từng ô số đo, kể cả câu nối bảng.
- Chỉ số do admin định nghĩa, cách gộp của cột số đo, bài học, câu mẫu, gói khái niệm ngành.
- Báo cáo từ các câu trả lời đã chọn hay tự động cho một dataset; xuất PDF và Excel.
- Mô hình `mock`, Gemini, Ollama. Bộ đánh giá câu hỏi, phân loại và mô hình chấm: [eval/README.md](eval/README.md).

Chưa có: chế độ demo, màn xem lịch sử câu hỏi của cả workspace, màn sửa mô tả và bí danh cột, chia sẻ báo cáo; nguồn
MySQL, SQLite; truy vấn thẳng database ngoài lúc hỏi. Từng phần: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#trạng-thái).
Lịch sử thay đổi: [CHANGELOG.md](CHANGELOG.md).

## Tài liệu

| Bạn muốn | Đọc |
|---|---|
| Cài, cấu hình, đưa lên Internet, sao lưu, sửa lỗi | [INSTALL.md](docs/INSTALL.md) |
| Kết nối dữ liệu, hỏi, đọc minh chứng, làm báo cáo | [USAGE.md](docs/USAGE.md) |
| Biết dữ liệu nằm ở đâu, gửi gì cho mô hình | [PRIVACY.md](docs/PRIVACY.md) |
| Biết file nào nhận, chụp database thế nào, phiên bản | [DATA.md](docs/DATA.md) |
| Gọi API | [API.md](docs/API.md), đặc tả ở `/api/openapi.json`, thử ở `/api/docs` |
| Hiểu hệ thống chia phần thế nào, phần nào đã có | [ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Biết guard SQL chặn gì | [GUARD.md](docs/GUARD.md) |
| Đọc bảng Postgres của ứng dụng | [SCHEMA.md](docs/SCHEMA.md) |
| Xem thời gian, số lượt gọi mô hình đã đo | [PERFORMANCE.md](docs/PERFORMANCE.md) |
| Xem nguồn và giấy phép dữ liệu mẫu | [data/README.md](data/README.md) |

## Phát triển

Cần Rust stable kèm trình biên dịch C++, Node.js 24, Docker (cho Postgres) và GNU Make.

```bash
make dev            # Postgres trong Docker, backend bằng cargo run ở cổng 8000
make dev-frontend   # giao diện ở http://localhost:5173
make check-fast     # fmt, clippy, test
make help           # mọi lệnh
```

Cách chạy test và gửi PR: [CONTRIBUTING.md](CONTRIBUTING.md). Quy tắc làm việc: [AGENTS.md](AGENTS.md).

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues). Câu hỏi và ý tưởng:
  [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Lỗ hổng bảo mật: báo riêng theo [SECURITY.md](SECURITY.md), không mở issue công khai.

Giấy phép: [Apache License 2.0](LICENSE); thành phần bên thứ ba: [docs/LICENSES.md](docs/LICENSES.md).
