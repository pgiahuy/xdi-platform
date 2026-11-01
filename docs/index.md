# xdi-platform

*Ask questions about your data in Vietnamese and get answers with evidence for every number. Connect a Postgres database
(or upload a CSV, Parquet or Excel file); the system writes SQL, joining tables along foreign keys when needed, checks
it with a SQL guard, runs it on DuckDB and returns a table, a chart and the SQL it ran. For "why" questions it computes
which groups drove the change and shows the source rows behind the numbers. Rust backend, DuckDB for analytics,
Postgres for application data, React UI. Apache 2.0. Latest release: v0.2.0. Vietnamese documentation follows.*

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên: hỏi bằng tiếng Việt, nhận câu trả lời kèm lời giải thích và minh
chứng cho từng con số. Kết nối database Postgres của tổ chức (hay nạp file), gõ câu hỏi, nhận bảng, biểu đồ và câu SQL
đã chạy, kể cả câu nối nhiều bảng. Câu hỏi nguyên nhân được tách thành phần chênh lệch của từng nhóm; bấm vào một số để
xem các dòng nguồn đứng sau nó. Những gì gửi cho mô hình ngôn ngữ: [Dữ liệu và quyền riêng tư](PRIVACY.md).

![Một câu hỏi đi qua xdi-platform: hàng trên đi từ giao diện qua dựng ngữ cảnh, mô hình, guard SQL tới DuckDB, hàng dưới quay về qua phần giải thích, kiểm số, ghi lịch sử; 16 bước](images/luong-cau-hoi.png)

## Chạy thử

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
docker compose logs backend | grep setup_code
```

Mở http://localhost:8000 và nhập mã thiết lập. Workspace đầu tiên có dataset mẫu DataCo; mô hình `mock` trả lời các câu
mẫu. Database mẫu nhiều bảng, Gemini hay Ollama: [Cài đặt](INSTALL.md). Kết nối dữ liệu và hỏi: [Dùng lần đầu](USAGE.md).
Phần đã có và chưa có: bảng Trạng thái của [Kiến trúc](ARCHITECTURE.md#trạng-thái).

## Tài liệu

| Trang | Nội dung |
|---|---|
| [Cài đặt](INSTALL.md) | Docker, build từ nguồn, cấu hình, đưa lên Internet, sao lưu, lỗi hay gặp |
| [Dùng lần đầu](USAGE.md) | Kết nối database hay nạp file, hỏi, đọc minh chứng, chỉ số, vai dữ liệu, báo cáo |
| [Kiến trúc](ARCHITECTURE.md) | Thuật ngữ, sơ đồ, luồng một câu hỏi, trạng thái từng phần |
| [API](API.md) | Đặc tả OpenAPI, đăng nhập bằng cookie, workspace và vai, mã lỗi |
| [Guard SQL](GUARD.md) | Câu SQL nào được chạy, câu nào bị từ chối |
| [Dữ liệu](DATA.md) | Định dạng file nhận, chụp database, phiên bản, tên bảng và tên cột |
| [Schema Postgres](SCHEMA.md) | Bảng của Postgres ứng dụng, ý nghĩa, ví dụ |
| [Dữ liệu và quyền riêng tư](PRIVACY.md) | Dữ liệu nằm ở đâu, gửi gì cho mô hình, cách xóa |
| [Thời gian và số lượt gọi mô hình](PERFORMANCE.md) | Một câu hỏi mất bao lâu, gọi mô hình mấy lần, tốn bao nhiêu |
| [Gói khái niệm ngành](DOMAINS.md) | Khái niệm chung của một ngành, cách thêm một ngành |
| [Phát hành](RELEASING.md) | Quy trình ra phiên bản mới |
| [Giấy phép](LICENSES.md) | Giấy phép của các thành phần bên thứ ba |

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues). Câu hỏi và ý tưởng:
  [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](https://github.com/pgiahuy/xdi-platform/blob/main/CONTRIBUTING.md). Bộ đánh giá:
  [eval/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/eval/README.md).
- Lỗ hổng bảo mật: báo riêng theo [SECURITY.md](https://github.com/pgiahuy/xdi-platform/blob/main/SECURITY.md), không
  mở issue công khai.
